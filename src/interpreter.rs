//! Execute a resolved program with fresh variable storage for every call.

use std::io::{self, Write};

use crate::{
    ast::OutputTarget,
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

fn prepare(
    program: &ResolvedProgram,
    capture: bool,
    limits: ExecutionLimits,
) -> Result<Vec<OutputEvent<'_>>, Diagnostic> {
    let mut environment = vec![Value::Integer(0); program.binding_count()];
    let mut workspace = evaluator::Workspace::default();
    let mut output = Vec::new();
    let mut pc = 0;
    let mut budget = Budget::new(limits.max_steps);
    let mut prints = 0;
    while let Some(operation) = program.statements.get(pc) {
        budget.charge(1, operation.span)?;
        match &operation.kind {
            ResolvedStatement::RepeatInit {
                count,
                count_slot,
                index_slot,
            } => {
                let value = workspace
                    .evaluate(count, &environment, &mut budget)?
                    .require_integer(operation.span)?;
                if value < 0 {
                    return Err(Diagnostic::new(
                        DiagnosticKind::RepeatCount,
                        operation.span,
                        "repeat count must be nonnegative",
                    )
                    .with_help("use zero to skip the loop or a positive integer count"));
                }
                environment[*count_slot] = Value::Integer(value);
                environment[*index_slot] = Value::Integer(0);
            }
            ResolvedStatement::RepeatGuard {
                count_slot,
                index_slot,
                target,
            } => {
                if environment[*index_slot].require_integer(operation.span)?
                    >= environment[*count_slot].require_integer(operation.span)?
                {
                    pc = *target;
                    continue;
                }
            }
            ResolvedStatement::RepeatNext { index_slot } => {
                let next = environment[*index_slot]
                    .require_integer(operation.span)?
                    .checked_add(1)
                    .ok_or_else(|| {
                        Diagnostic::new(
                            DiagnosticKind::ArithmeticOverflow,
                            operation.span,
                            "repeat index overflow",
                        )
                    })?;
                environment[*index_slot] = Value::Integer(next);
            }
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
            ResolvedStatement::Output {
                arguments,
                target,
                newline,
            } => {
                if prints >= limits.max_prints {
                    return Err(Diagnostic::new(
                        DiagnosticKind::OutputLimit,
                        operation.span,
                        format!("print count limit of {} exceeded", limits.max_prints),
                    )
                    .with_help(
                        "reduce output calls or raise --max-prints / ExecutionLimits.max_prints",
                    ));
                }
                prints += 1;
                for (index, argument) in arguments.iter().enumerate() {
                    if index > 0 {
                        budget.charge(1, operation.span)?;
                        if capture {
                            output.push(OutputEvent {
                                target: *target,
                                value: PrintValue::String(" "),
                                newline: false,
                            });
                        }
                    }
                    let value = match argument {
                        ResolvedArgument::String(value) => PrintValue::String(value),
                        ResolvedArgument::Expression(expression) => {
                            let value =
                                workspace.evaluate(expression, &environment, &mut budget)?;
                            PrintValue::Value(value)
                        }
                    };
                    if capture {
                        output.push(OutputEvent {
                            target: *target,
                            value,
                            newline: *newline && index + 1 == arguments.len(),
                        });
                    }
                }
                if capture && *newline && arguments.is_empty() {
                    output.push(OutputEvent {
                        target: *target,
                        value: PrintValue::String(""),
                        newline: true,
                    });
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
    let events = prepare(program, true, limits).map_err(RunError::Diagnostic)?;
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
    let events = prepare(program, true, limits).map_err(RunError::Diagnostic)?;
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
    prepare(program, false, limits).map(|_| ())
}
