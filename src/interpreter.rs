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

fn prepare(program: &ResolvedProgram) -> Result<Vec<PrintValue<'_>>, Diagnostic> {
    let mut environment = vec![Value::Integer(0); program.binding_count()];
    let mut output = Vec::new();
    for statement in &program.statements {
        match statement {
            ResolvedStatement::Update {
                slot,
                operator,
                value,
                operator_span,
            } => {
                let previous = environment[*slot].clone();
                let right = evaluator::evaluate_resolved(value, &environment)?;
                environment[*slot] =
                    evaluator::apply_binary(*operator, &previous, &right, *operator_span)?;
            }
            ResolvedStatement::Store { slot, value } => {
                environment[*slot] = evaluator::evaluate_resolved(value, &environment)?;
            }
            ResolvedStatement::Println(argument) => {
                output.push(match argument {
                    ResolvedArgument::String(value) => PrintValue::String(value),
                    ResolvedArgument::Expression(expression) => {
                        PrintValue::Value(evaluator::evaluate_resolved(expression, &environment)?)
                    }
                });
            }
        }
    }
    Ok(output)
}

/// Resolve and evaluate through `compile` first. Arithmetic failures never write
/// program output; completed writes cannot be rolled back on an I/O failure.
pub fn execute(program: &ResolvedProgram, output: &mut impl Write) -> Result<(), RunError> {
    let values = prepare(program).map_err(RunError::Diagnostic)?;
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
    prepare(program).map(|_| ())
}
