//! Resumable expressions and isolated call frames; no host recursion.
use super::{OutputEvent, PrintValue};
use crate::{
    ast::{BinaryOperator, OutputTarget},
    diagnostics::{Diagnostic, DiagnosticKind, Span},
    evaluator::{self, Evaluation, Workspace},
    limits::{Budget, ExecutionOptions},
    resolver::{
        ResolvedArgument, ResolvedCall, ResolvedExpression, ResolvedOperation, ResolvedProgram,
        ResolvedStatement,
    },
    value::Value,
};

enum Stage {
    Defaults(usize),
    Needs(usize),
    Body,
    Ensures(usize),
    Done,
}
enum Completion {
    Store(usize),
    RepeatCount(usize, usize, Span),
    Update(usize, BinaryOperator, Value, Span),
    Guard(usize, Span),
    Discard,
    Return,
    Contract(bool, Span),
    Output,
}

struct PendingOutput<'a> {
    arguments: &'a [ResolvedArgument],
    target: OutputTarget,
    newline: bool,
    span: Span,
    index: usize,
    values: Vec<PrintValue<'a>>,
}

struct Frame<'a> {
    function: Option<usize>,
    call_span: Span,
    operations: &'a [ResolvedOperation],
    environment: Vec<Value>,
    supplied: Vec<bool>,
    pc: usize,
    stage: Stage,
    expression: Option<&'a ResolvedExpression>,
    completion: Completion,
    workspace: Workspace,
    output: Option<PendingOutput<'a>>,
    result: Value,
}

impl<'a> Frame<'a> {
    fn start(&mut self, expression: &'a ResolvedExpression, completion: Completion) {
        self.workspace.start(expression);
        self.expression = Some(expression);
        self.completion = completion;
    }
    fn finish(&mut self, program: &ResolvedProgram, value: Value) {
        if let Some(slot) = self
            .function
            .and_then(|id| program.functions[id].result_slot)
        {
            self.environment[slot] = value.clone();
        }
        self.result = value;
        self.stage = Stage::Ensures(0);
    }
}

pub(super) fn prepare(
    program: &ResolvedProgram,
    capture: bool,
    options: ExecutionOptions,
) -> Result<Vec<OutputEvent<'_>>, Diagnostic> {
    if program.binding_count() > options.max_live_slots {
        return Err(limit(
            Span::default(),
            "live binding slot",
            options.max_live_slots,
            "max_live_slots",
        ));
    }
    let mut frames = vec![Frame {
        function: None,
        call_span: Span::default(),
        operations: &program.statements,
        environment: vec![Value::Unit; program.binding_count()],
        supplied: Vec::new(),
        pc: 0,
        stage: Stage::Body,
        expression: None,
        completion: Completion::Discard,
        workspace: Workspace::default(),
        output: None,
        result: Value::Unit,
    }];
    let mut budget =
        Budget::new(options.limits.max_steps).with_vector_bytes(options.max_vector_bytes);
    let mut events = Vec::new();
    let mut calls = 0_u64;
    let mut prints = 0;
    let mut live_slots = program.binding_count();
    let mut captured = 0_usize;
    let outcome = (|| -> Result<(), Diagnostic> {
        while !frames.is_empty() {
            let depth = frames.len();
            let frame = frames.last_mut().unwrap();
            if let Some(expression) = frame.expression {
                match frame
                    .workspace
                    .resume(expression, &frame.environment, &mut budget)?
                {
                    Evaluation::Call {
                        node,
                        arguments,
                        span,
                    } => {
                        let call = expression.calls[node]
                            .as_ref()
                            .expect("resolved call identity");
                        let ResolvedCall::User {
                            function: id,
                            parameters,
                        } = call
                        else {
                            unreachable!("native kernels complete in the expression workspace")
                        };
                        let function = &program.functions[*id];
                        if depth > options.max_call_depth {
                            return Err(limit(
                                span,
                                "function call depth",
                                options.max_call_depth,
                                "max_call_depth",
                            ));
                        }
                        if calls >= options.max_calls {
                            return Err(limit(
                                span,
                                "total function call",
                                options.max_calls,
                                "max_calls",
                            ));
                        }
                        if function.slots > options.max_live_slots.saturating_sub(live_slots) {
                            return Err(limit(
                                span,
                                "live binding slot",
                                options.max_live_slots,
                                "max_live_slots",
                            ));
                        }
                        // Charge frame initialization before allocating its storage.
                        budget.charge(1 + function.slots as u64, span)?;
                        calls += 1;
                        live_slots += function.slots;
                        let mut environment = vec![Value::Unit; function.slots];
                        let mut supplied = vec![false; function.defaults.len()];
                        for (parameter, value) in parameters.iter().zip(arguments) {
                            environment[*parameter] = value;
                            supplied[*parameter] = true;
                        }
                        frames.push(Frame {
                            function: Some(*id),
                            call_span: span,
                            operations: &function.statements,
                            environment,
                            supplied,
                            pc: 0,
                            stage: Stage::Defaults(0),
                            expression: None,
                            completion: Completion::Discard,
                            workspace: Workspace::default(),
                            output: None,
                            result: Value::Unit,
                        });
                    }
                    Evaluation::Complete(value) => {
                        frame.expression = None;
                        match std::mem::replace(&mut frame.completion, Completion::Discard) {
                            Completion::Store(slot) => frame.environment[slot] = value,
                            Completion::RepeatCount(count_slot, index_slot, span) => {
                                let count = value.require_integer(span)?;
                                if count < 0 {
                                    return Err(Diagnostic::new(
                                        DiagnosticKind::RepeatCount,
                                        span,
                                        "repeat count must be nonnegative",
                                    )
                                    .with_help(
                                        "use zero to skip the loop or a positive integer count",
                                    ));
                                }
                                frame.environment[count_slot] = Value::Integer(count);
                                frame.environment[index_slot] = Value::Integer(0);
                            }
                            Completion::Update(slot, operator, previous, span) => {
                                frame.environment[slot] =
                                    evaluator::apply_binary(operator, &previous, &value, span)?;
                            }
                            Completion::Guard(target, span) => {
                                if !value.require_boolean(span)? {
                                    frame.pc = target;
                                }
                            }
                            Completion::Discard => {}
                            Completion::Return => frame.finish(program, value),
                            Completion::Contract(post, span) => {
                                if !value.require_boolean(span)? {
                                    let name = &program.functions()[frame.function.unwrap()].name;
                                    return Err(Diagnostic::new(
                                        DiagnosticKind::ContractViolation,
                                        span,
                                        format!(
                                            "{} contract failed in funcra {}",
                                            if post { "ensures" } else { "needs" },
                                            bounded_name(name)
                                        ),
                                    ));
                                }
                            }
                            Completion::Output => {
                                if let Some(vector) = value.as_vector() {
                                    budget.charge(
                                        vector.len() as u64,
                                        frame.output.as_ref().unwrap().span,
                                    )?;
                                }
                                let output = frame.output.as_mut().unwrap();
                                let value = PrintValue::Value(value);
                                charge_capture(&value, &mut captured, options, output.span)?;
                                if capture {
                                    output.values.push(value);
                                }
                            }
                        }
                    }
                }
                continue;
            }
            if let Some(output) = &mut frame.output {
                if let Some(argument) = output.arguments.get(output.index) {
                    if output.index > 0 {
                        budget.charge(1, output.span)?;
                        charge_capture(
                            &PrintValue::String(" "),
                            &mut captured,
                            options,
                            output.span,
                        )?;
                    }
                    output.index += 1;
                    match argument {
                        ResolvedArgument::String(text) => {
                            let value = PrintValue::String(text);
                            charge_capture(&value, &mut captured, options, output.span)?;
                            if capture {
                                output.values.push(value);
                            }
                        }
                        ResolvedArgument::Expression(expression) => {
                            frame.start(expression, Completion::Output)
                        }
                    }
                } else {
                    let output = frame.output.take().unwrap();
                    if output.newline && output.arguments.is_empty() {
                        charge_capture(
                            &PrintValue::String(""),
                            &mut captured,
                            options,
                            output.span,
                        )?;
                        if capture {
                            events.push(OutputEvent {
                                target: output.target,
                                value: PrintValue::String(""),
                                newline: true,
                            });
                        }
                    }
                    let length = output.values.len();
                    for (index, value) in output.values.into_iter().enumerate() {
                        if index > 0 {
                            events.push(OutputEvent {
                                target: output.target,
                                value: PrintValue::String(" "),
                                newline: false,
                            });
                        }
                        events.push(OutputEvent {
                            target: output.target,
                            value,
                            newline: output.newline && index + 1 == length,
                        });
                    }
                }
                continue;
            }
            match frame.stage {
                Stage::Defaults(index) => {
                    let function = &program.functions[frame.function.unwrap()];
                    if index == function.defaults.len() {
                        frame.stage = Stage::Needs(0);
                        continue;
                    }
                    frame.stage = Stage::Defaults(index + 1);
                    if !frame.supplied[index] {
                        let default = function.defaults[index]
                            .as_ref()
                            .expect("checked missing arguments");
                        budget.charge(1, default.syntax.nodes()[default.syntax.root()].span)?;
                        frame.start(default, Completion::Store(index));
                    }
                }
                Stage::Needs(index) | Stage::Ensures(index) => {
                    let post = matches!(frame.stage, Stage::Ensures(_));
                    if let Some(id) = frame.function {
                        let contracts = &program.functions[id].contracts;
                        if let Some(contract) = contracts.get(index) {
                            frame.stage = if post {
                                Stage::Ensures(index + 1)
                            } else {
                                Stage::Needs(index + 1)
                            };
                            if contract.postcondition == post {
                                budget.charge(1, contract.span)?;
                                frame.start(
                                    &contract.expression,
                                    Completion::Contract(post, contract.span),
                                );
                            }
                            continue;
                        }
                    }
                    frame.stage = if post { Stage::Done } else { Stage::Body };
                }
                Stage::Body => {
                    let Some(operation) = frame.operations.get(frame.pc) else {
                        frame.finish(program, Value::Unit);
                        continue;
                    };
                    budget.charge(1, operation.span)?;
                    frame.pc += 1;
                    match &operation.kind {
                        ResolvedStatement::RepeatInit {
                            count,
                            count_slot,
                            index_slot,
                        } => {
                            frame.start(
                                count,
                                Completion::RepeatCount(*count_slot, *index_slot, operation.span),
                            );
                        }
                        ResolvedStatement::RepeatGuard {
                            count_slot,
                            index_slot,
                            target,
                        } => {
                            if frame.environment[*index_slot].require_integer(operation.span)?
                                >= frame.environment[*count_slot].require_integer(operation.span)?
                            {
                                frame.pc = *target;
                            }
                        }
                        ResolvedStatement::RepeatNext { index_slot } => {
                            let next = frame.environment[*index_slot]
                                .require_integer(operation.span)?
                                .checked_add(1)
                                .ok_or_else(|| {
                                    Diagnostic::new(
                                        DiagnosticKind::ArithmeticOverflow,
                                        operation.span,
                                        "repeat index overflow",
                                    )
                                })?;
                            frame.environment[*index_slot] = Value::Integer(next);
                        }
                        ResolvedStatement::Jump { target }
                        | ResolvedStatement::LoopControl { target } => frame.pc = *target,
                        ResolvedStatement::JumpIfFalse { condition, target } => {
                            frame.start(
                                condition,
                                Completion::Guard(
                                    *target,
                                    condition.syntax.nodes()[condition.syntax.root()].span,
                                ),
                            );
                        }
                        ResolvedStatement::Store { slot, value } => {
                            frame.start(value, Completion::Store(*slot))
                        }
                        ResolvedStatement::Update {
                            slot,
                            operator,
                            value,
                            operator_span,
                        } => {
                            frame.start(
                                value,
                                Completion::Update(
                                    *slot,
                                    *operator,
                                    frame.environment[*slot].clone(),
                                    *operator_span,
                                ),
                            );
                        }
                        ResolvedStatement::Call { expression } => {
                            frame.start(expression, Completion::Discard)
                        }
                        ResolvedStatement::Return { value: Some(value) } => {
                            frame.start(value, Completion::Return)
                        }
                        ResolvedStatement::Return { value: None } => {
                            frame.finish(program, Value::Unit)
                        }
                        ResolvedStatement::Output {
                            arguments,
                            target,
                            newline,
                        } => {
                            if prints >= options.limits.max_prints {
                                return Err(Diagnostic::new(
                                    DiagnosticKind::OutputLimit,
                                    operation.span,
                                    format!("print count limit of {} exceeded", options.limits.max_prints),
                                )
                                .with_help(
                                    "reduce output calls or raise --max-prints / ExecutionLimits.max_prints",
                                ));
                            }
                            prints += 1;
                            frame.output = Some(PendingOutput {
                                arguments,
                                target: *target,
                                newline: *newline,
                                span: operation.span,
                                index: 0,
                                values: Vec::new(),
                            });
                        }
                    }
                }
                Stage::Done => {
                    let finished = frames.pop().unwrap();
                    live_slots -= finished.environment.len();
                    if let Some(caller) = frames.last_mut() {
                        caller.workspace.accept(finished.result);
                    }
                }
            }
        }
        Ok(())
    })();
    outcome.map_err(|mut error| {
        for frame in frames
            .iter()
            .rev()
            .filter(|f| f.function.is_some())
            .take(16)
        {
            let name = &program.functions()[frame.function.unwrap()].name;
            error = error.with_related(
                frame.call_span,
                format!("called funcra {} here", bounded_name(name)),
            );
        }
        error
    })?;
    Ok(events)
}

fn limit(
    span: Span,
    resource: &str,
    configured: impl std::fmt::Display,
    field: &str,
) -> Diagnostic {
    Diagnostic::new(
        DiagnosticKind::CallLimit,
        span,
        format!("{resource} limit of {configured} exceeded"),
    )
    .with_help(format!(
        "check recursion and frame size, or raise --{} / ExecutionOptions.{field}",
        field.replace('_', "-")
    ))
}

fn bounded_name(name: &str) -> String {
    name.chars().take(80).collect()
}

fn charge_capture(
    value: &PrintValue<'_>,
    used: &mut usize,
    options: ExecutionOptions,
    span: Span,
) -> Result<(), Diagnostic> {
    // Charge each snapshot's logical storage even when Arc shares the payload.
    // Validation applies the same limit without formatting or retaining values.
    let payload = match value {
        PrintValue::String(text) => text.len(),
        PrintValue::Value(Value::String(text)) => text.len(),
        PrintValue::Value(Value::Vector(vector)) => vector.len().saturating_mul(8),
        _ => 8,
    };
    let amount = payload.saturating_add(64);
    if amount > options.max_capture_bytes.saturating_sub(*used) {
        return Err(Diagnostic::new(
            DiagnosticKind::CaptureLimit,
            span,
            format!("output snapshot storage limit of {} exceeded", options.max_capture_bytes),
        )
        .with_help("reduce selected output or raise --max-capture-bytes / ExecutionOptions.max_capture_bytes"));
    }
    *used += amount;
    Ok(())
}
