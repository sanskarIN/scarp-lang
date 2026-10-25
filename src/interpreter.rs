//! Execute a resolved program with fresh variable storage for every call.

use std::io::Write;

use crate::{
    diagnostics::Diagnostic,
    evaluator,
    resolver::{ResolvedArgument, ResolvedProgram, ResolvedStatement},
    RunError,
};

enum PrintValue<'a> {
    String(&'a str),
    Integer(i64),
}

fn prepare(program: &ResolvedProgram) -> Result<Vec<PrintValue<'_>>, Diagnostic> {
    let mut environment = vec![0; program.slots];
    let mut output = Vec::new();
    for statement in &program.statements {
        match statement {
            ResolvedStatement::Store { slot, value } => {
                environment[*slot] = evaluator::evaluate_resolved(value, &environment)?;
            }
            ResolvedStatement::Println(argument) => {
                output.push(match argument {
                    ResolvedArgument::String(value) => PrintValue::String(value),
                    ResolvedArgument::Integer(expression) => {
                        PrintValue::Integer(evaluator::evaluate_resolved(expression, &environment)?)
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
            PrintValue::Integer(value) => writeln!(output, "{value}"),
        }
        .map_err(RunError::Output)?;
    }
    Ok(())
}
