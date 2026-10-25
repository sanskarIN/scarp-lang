//! Execute a validated program with an explicit output destination.

use std::io::Write;

use crate::{
    ast::{PrintArgument, Program, Statement},
    evaluator, RunError,
};

/// Evaluate every argument before writing, so arithmetic failures produce no
/// program output. Already completed writes cannot be rolled back on I/O errors.
pub fn execute(program: &Program, output: &mut impl Write) -> Result<(), RunError> {
    let mut values = Vec::with_capacity(program.statements.len());
    for statement in &program.statements {
        match statement {
            Statement::Println { argument, .. } => {
                let value = match argument {
                    PrintArgument::String(value) => value.clone(),
                    PrintArgument::Integer(expression) => evaluator::evaluate(expression)
                        .map_err(RunError::Diagnostic)?
                        .to_string(),
                };
                values.push(value);
            }
        }
    }
    for value in values {
        writeln!(output, "{value}").map_err(RunError::Output)?;
    }
    Ok(())
}
