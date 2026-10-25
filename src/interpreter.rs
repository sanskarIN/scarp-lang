//! Execute a resolved program with fresh variable storage for every call.

use std::io::Write;

use crate::{
    diagnostics::{Diagnostic, DiagnosticKind},
    evaluator,
    limits::{Budget, ExecutionLimits},
    resolver::{ResolvedArgument, ResolvedProgram, ResolvedStatement},
    value::Value,
    RunError,
};

enum PrintValue<'a> {
    String(&'a str),
    Value(Value),
}

fn prepare(
    program: &ResolvedProgram,
    capture: bool,
    limits: ExecutionLimits,
) -> Result<Vec<PrintValue<'_>>, Diagnostic> {
    let mut environment = vec![Value::Integer(0); program.binding_count()];
    let mut workspace = evaluator::Workspace::default();
    let mut output = Vec::new();
    let mut pc = 0;
    let mut budget = Budget::new(limits.max_steps);
    let mut prints = 0;
    while let Some(operation) = program.statements.get(pc) {
        budget.charge(1, operation.span)?;
        match &operation.kind {
            ResolvedStatement::JumpIfFalse { condition, target } => {
                let value = workspace.evaluate(condition, &environment, &mut budget)?;
                let span = condition.syntax.nodes()[condition.syntax.root()].span;
                if !value.require_boolean(span)? {
                    pc = *target;
                    continue;
                }
            }
            ResolvedStatement::Jump { target } | ResolvedStatement::LoopControl { target } => {
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
                let right = workspace.evaluate(value, &environment, &mut budget)?;
                environment[*slot] =
                    evaluator::apply_binary(*operator, &previous, &right, *operator_span)?;
            }
            ResolvedStatement::Store { slot, value } => {
                environment[*slot] = workspace.evaluate(value, &environment, &mut budget)?;
            }
            ResolvedStatement::Println(argument) => {
                if prints >= limits.max_prints {
                    return Err(Diagnostic::new(
                        DiagnosticKind::OutputLimit,
                        operation.span,
                        "print count limit exceeded",
                    )
                    .with_help("reduce printed results or select a larger print budget"));
                }
                prints += 1;
                match argument {
                    ResolvedArgument::String(value) => {
                        if capture {
                            output.push(PrintValue::String(value));
                        }
                    }
                    ResolvedArgument::Expression(expression) => {
                        let value = workspace.evaluate(expression, &environment, &mut budget)?;
                        if let Some(vector) = value.as_vector() {
                            budget.charge(vector.len() as u64, operation.span)?;
                        }
                        if capture {
                            output.push(PrintValue::Value(value));
                        }
                    }
                }
            }
        }
        pc += 1;
    }
    Ok(output)
}

/// Resolve and evaluate through `compile` first. Arithmetic failures never write
/// program output; completed writes cannot be rolled back on an I/O failure.
pub fn execute(program: &ResolvedProgram, output: &mut impl Write) -> Result<(), RunError> {
    execute_with_limits(program, output, ExecutionLimits::default())
}

pub fn execute_with_limits(
    program: &ResolvedProgram,
    output: &mut impl Write,
    limits: ExecutionLimits,
) -> Result<(), RunError> {
    let values = prepare(program, true, limits).map_err(RunError::Diagnostic)?;
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
    validate_with_limits(program, ExecutionLimits::default())
}

pub fn validate_with_limits(
    program: &ResolvedProgram,
    limits: ExecutionLimits,
) -> Result<(), Diagnostic> {
    prepare(program, false, limits).map(|_| ())
}
