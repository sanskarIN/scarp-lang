//! Execute a resolved program with fresh variable storage for every call.

use std::io::{self, Write};

use crate::{
    ast::OutputTarget,
    diagnostics::Diagnostic,
    limits::{ExecutionLimits, ExecutionOptions},
    resolver::ResolvedProgram,
    value::Value,
    RunError,
};

enum PrintValue<'a> {
    String(&'a str),
    Value(Value),
}

struct OutputEvent<'a> {
    target: OutputTarget,
    value: PrintValue<'a>,
    newline: bool,
}

impl OutputEvent<'_> {
    fn has_text(&self) -> bool {
        match &self.value {
            PrintValue::String(value) => !value.is_empty(),
            PrintValue::Value(value) => value.as_str().map_or(true, |text| !text.is_empty()),
        }
    }

    fn has_bytes(&self) -> bool {
        self.newline || self.has_text()
    }

    fn write_to(&self, output: &mut dyn Write) -> io::Result<()> {
        if self.has_text() {
            match &self.value {
                PrintValue::String(value) => output.write_all(value.as_bytes())?,
                PrintValue::Value(value) => write!(output, "{value}")?,
            }
        }
        if self.newline {
            output.write_all(b"\n")?;
        }
        Ok(())
    }
}

mod machine;

fn prepare(
    program: &ResolvedProgram,
    capture: bool,
    options: ExecutionOptions,
) -> Result<Vec<OutputEvent<'_>>, Diagnostic> {
    machine::prepare(program, capture, options)
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
    execute_with_options(program, output, limits.into())
}

/// Execute with explicit work, call, frame-storage, and output-snapshot budgets.
pub fn execute_with_options(
    program: &ResolvedProgram,
    output: &mut impl Write,
    options: ExecutionOptions,
) -> Result<(), RunError> {
    let events = prepare(program, true, options).map_err(RunError::Diagnostic)?;
    if events
        .iter()
        .any(|event| event.target == OutputTarget::Stderr && event.has_bytes())
    {
        return Err(RunError::Output(io::Error::new(
            io::ErrorKind::Unsupported,
            "program requires a stderr destination; use execute_with_io or run_with_io",
        )));
    }
    for event in events {
        event.write_to(output).map_err(RunError::Output)?;
    }
    Ok(())
}

/// Execute with two explicit destinations. Source/evaluation errors write neither.
/// Stream switches flush the previous writer; callers own final flushing.
pub fn execute_with_io(
    program: &ResolvedProgram,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> Result<(), RunError> {
    execute_with_io_and_limits(program, stdout, stderr, ExecutionLimits::default())
}

pub fn execute_with_io_and_limits(
    program: &ResolvedProgram,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
    limits: ExecutionLimits,
) -> Result<(), RunError> {
    execute_with_io_and_options(program, stdout, stderr, limits.into())
}

/// Dual-stream execution with all interpreter resource options.
pub fn execute_with_io_and_options(
    program: &ResolvedProgram,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
    options: ExecutionOptions,
) -> Result<(), RunError> {
    let events = prepare(program, true, options).map_err(RunError::Diagnostic)?;
    let mut previous = None;
    for event in events.iter().filter(|event| event.has_bytes()) {
        if let Some(previous) = previous.filter(|previous| *previous != event.target) {
            match previous {
                OutputTarget::Stdout => stdout.flush(),
                OutputTarget::Stderr => stderr.flush(),
            }
            .map_err(RunError::Output)?;
        }
        let writer: &mut dyn Write = match event.target {
            OutputTarget::Stdout => stdout,
            OutputTarget::Stderr => stderr,
        };
        event.write_to(writer).map_err(RunError::Output)?;
        previous = Some(event.target);
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
    validate_with_options(program, limits.into())
}

/// Execute all selected paths without retaining or replaying output.
pub fn validate_with_options(
    program: &ResolvedProgram,
    options: ExecutionOptions,
) -> Result<(), Diagnostic> {
    prepare(program, false, options).map(|_| ())
}
