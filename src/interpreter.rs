//! Execute a resolved program with fresh variable storage for every call.

use std::io::Write;

use crate::{
    diagnostics::Diagnostic,
    evaluator,
    resolver::{ResolvedArgument, ResolvedProgram, ResolvedStatement},
    value::Value,
    RunError,
};

enum PrintValue<'a> {
    String(&'a str),
    Value(Value),
}

fn prepare(program: &ResolvedProgram, capture: bool) -> Result<Vec<PrintValue<'_>>, Diagnostic> {
    let mut environment = vec![Value::Integer(0); program.binding_count()];
    let mut workspace = evaluator::Workspace::default();
    let mut output = Vec::new();
    let mut pc = 0;
    while let Some(statement) = program.statements.get(pc) {
        match statement {
            ResolvedStatement::JumpIfFalse { condition, target } => {
                let value = workspace.evaluate(condition, &environment)?;
                let span = condition.syntax.nodes()[condition.syntax.root()].span;
                if !value.require_boolean(span)? {
                    pc = *target;
                    continue;
                }
            }
            ResolvedStatement::Jump { target } => {
                pc = *target;
                continue;
            }
            ResolvedStatement::Update {
                slot,
                operator,
                value,
                operator_span,
            } => {
                let previous = environment[*slot].clone();
                let right = workspace.evaluate(value, &environment)?;
                environment[*slot] =
                    evaluator::apply_binary(*operator, &previous, &right, *operator_span)?;
            }
            ResolvedStatement::Store { slot, value } => {
                environment[*slot] = workspace.evaluate(value, &environment)?;
            }
            ResolvedStatement::Println(argument) => match argument {
                ResolvedArgument::String(value) => {
                    if capture {
                        output.push(PrintValue::String(value));
                    }
                }
                ResolvedArgument::Expression(expression) => {
                    let value = workspace.evaluate(expression, &environment)?;
                    if capture {
                        output.push(PrintValue::Value(value));
                    }
                }
            },
        }
        pc += 1;
    }
    Ok(output)
}

/// Resolve and evaluate through `compile` first. Arithmetic failures never write
/// program output; completed writes cannot be rolled back on an I/O failure.
pub fn execute(program: &ResolvedProgram, output: &mut impl Write) -> Result<(), RunError> {
    let values = prepare(program, true).map_err(RunError::Diagnostic)?;
    for value in values {
        match value {
            PrintValue::String(value) => writeln!(output, "{value}"),
            PrintValue::Value(value) => writeln!(output, "{value}"),
        }
        .map_err(RunError::Output)?;
    }
    Ok(())
}

/// Evaluate a resolved program without formatting or writing program output.
/// All variables are local to this validation, just as in `execute`.
pub fn validate(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    prepare(program, false).map(|_| ())
}
