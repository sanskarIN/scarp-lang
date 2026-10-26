//! Freestanding C11 emission for the closed integer/boolean language subset.

use crate::{
    ast::{BinaryOperator, ExpressionKind, LogicalOperator, OutputTarget},
    diagnostics::{Diagnostic, DiagnosticKind, Span},
    resolver::{ResolvedArgument, ResolvedExpression, ResolvedProgram, ResolvedStatement},
    value::ValueType,
};
use std::fmt::Write;

fn unsupported(span: Span) -> Diagnostic {
    Diagnostic::new(
        DiagnosticKind::BackendUnsupported,
        span,
        "program exceeds the freestanding C subset or resource limits",
    )
    .with_help(
        "use integer/boolean expressions and literal prints; see the freestanding backend guide",
    )
}

/// Emit a complete C translation unit. This does not invoke a C compiler.
pub fn emit_c(source: &str) -> Result<String, Diagnostic> {
    emit_program(&crate::compile(source)?)
}

pub fn emit_program(program: &ResolvedProgram) -> Result<String, Diagnostic> {
    if program.binding_count() > 256 || program.statements.len() > 4096 {
        return Err(unsupported(Span::default()));
    }
    let mut max_nodes = 1;
    let mut total_nodes = 0;
    let mut text_bytes = 0;
    for operation in &program.statements {
        let mut expressions = Vec::new();
        match &operation.kind {
            ResolvedStatement::Store { value, .. } | ResolvedStatement::Update { value, .. } => {
                expressions.push(value);
            }
            ResolvedStatement::JumpIfFalse { condition, .. } => expressions.push(condition),
            ResolvedStatement::Output { arguments, .. } => {
                total_nodes += arguments.len().saturating_sub(1);
                for argument in arguments {
                    match argument {
                        ResolvedArgument::Expression(value) => expressions.push(value),
                        ResolvedArgument::String(value) => text_bytes += value.len(),
                    }
                }
            }
            _ => {}
        }
        for expression in expressions {
            if !matches!(expression.kind, ValueType::Integer | ValueType::Boolean) {
                return Err(unsupported(operation.span));
            }
            for node in expression.syntax.nodes() {
                if !matches!(
                    node.kind,
                    ExpressionKind::Integer(_)
                        | ExpressionKind::Boolean(_)
                        | ExpressionKind::Name(_)
                        | ExpressionKind::Negate { .. }
                        | ExpressionKind::BitNot { .. }
                        | ExpressionKind::Not { .. }
                        | ExpressionKind::Binary { .. }
                        | ExpressionKind::Logical { .. }
                        | ExpressionKind::Group { .. }
                ) {
                    return Err(unsupported(node.span));
                }
            }
            let count = expression.syntax.nodes().len();
            max_nodes = max_nodes.max(count);
            total_nodes += count;
        }
        if max_nodes > 1024 || total_nodes > 16384 || text_bytes > 65536 {
            return Err(unsupported(operation.span));
        }
    }
    let mut out=format!("/* Scarp freestanding C11 unit. Caller supplies workspace and output. */\n#define SCARP_SLOT_COUNT {}\n#define SCARP_TEMP_COUNT {}\n",program.binding_count().max(1),max_nodes);
    out.push_str(include_str!("c_runtime.h"));
    for (index, operation) in program.statements.iter().enumerate() {
        match &operation.kind {
            ResolvedStatement::Store { value, .. } | ResolvedStatement::Update { value, .. } => {
                emit_expression(&mut out, index, value)
            }
            ResolvedStatement::JumpIfFalse { condition, .. } => {
                emit_expression(&mut out, index, condition)
            }
            ResolvedStatement::Output { arguments, .. } => {
                for (argument_index, argument) in arguments.iter().enumerate() {
                    if let ResolvedArgument::Expression(value) = argument {
                        emit_expression(&mut out, format!("{index}_{argument_index}"), value);
                    }
                }
            }
            _ => {}
        }
    }
    out.push_str("\nstatic int scarp_pass(struct scarp_workspace *w, const struct scarp_io *io, int capture, uint64_t steps, size_t max_prints) {\nsize_t pc=0, i; int64_t value=0; (void)value; (void)io; (void)capture; (void)max_prints;\n(void)scarp_step; (void)scarp_binary; (void)scarp_print; (void)scarp_output;\nw->remaining=steps; w->prints=0; w->error_code=0; w->error_start=0; w->error_end=0; w->streams=0; w->last_stream=0;\nfor(i=0;i<SCARP_SLOT_COUNT;++i) w->slots[i]=0;\nfor(;;) switch(pc) {\n");
    for (index, operation) in program.statements.iter().enumerate() {
        writeln!(out, "case {index}: {{").unwrap();
        step(&mut out, operation.span);
        match &operation.kind {
            ResolvedStatement::Store { slot, .. } => {
                writeln!(
                    out,
                    "if(!scarp_e{index}(w,&w->slots[{slot}])) return 0;\npc={};",
                    index + 1
                )
                .unwrap();
            }
            ResolvedStatement::Update {
                slot,
                operator,
                operator_span,
                ..
            } => {
                writeln!(out, "if(!scarp_e{index}(w,&value)) return 0;").unwrap();
                writeln!(out,"if(!scarp_binary(w,{},w->slots[{slot}],value,&w->slots[{slot}],{},{})) return 0;\npc={};",opcode(*operator),operator_span.start,operator_span.end,index+1).unwrap();
            }
            ResolvedStatement::JumpIfFalse { target, .. } => {
                writeln!(
                    out,
                    "if(!scarp_e{index}(w,&value)) return 0;\npc=value?{}:{target};",
                    index + 1
                )
                .unwrap();
            }
            ResolvedStatement::Jump { target } | ResolvedStatement::LoopControl { target } => {
                writeln!(out, "pc={target};").unwrap();
            }
            ResolvedStatement::Output {
                arguments,
                target,
                newline,
            } => {
                writeln!(
                    out,
                    "if(w->prints>=max_prints) return scarp_fail(w,18u,{},{});\n++w->prints;",
                    operation.span.start, operation.span.end
                )
                .unwrap();
                let stream = usize::from(*target == OutputTarget::Stderr);
                for (argument_index, argument) in arguments.iter().enumerate() {
                    if argument_index > 0 {
                        step(&mut out, operation.span);
                        emit_bytes(&mut out, b" ", stream, operation.span);
                    }
                    match argument {
                        ResolvedArgument::Expression(expression) => {
                            writeln!(out, "if(!scarp_e{index}_{argument_index}(w,&value)) return 0;\nw->streams|={}u;", 1 << stream).unwrap();
                            writeln!(out, "if(capture && !scarp_print(w,io,{stream}u,value,{}, {},{})) return 0;", usize::from(expression.kind == ValueType::Boolean), operation.span.start, operation.span.end).unwrap();
                        }
                        ResolvedArgument::String(value) => {
                            emit_bytes(&mut out, value.as_bytes(), stream, operation.span)
                        }
                    }
                }
                if *newline {
                    emit_bytes(&mut out, b"\n", stream, operation.span);
                }
                writeln!(out, "pc={};", index + 1).unwrap();
            }
        }
        out.push_str("break; }\n");
    }
    out.push_str("default: return 1;\n}\n}\nuint32_t scarp_validate(struct scarp_workspace *w, uint64_t steps, size_t prints) {\nif(!w) return SCARP_OUTPUT_ERROR;\n(void)scarp_pass(w,0,0,steps,prints); return w->error_code;\n}\nuint32_t scarp_run_io(struct scarp_workspace *w, const struct scarp_io *io, uint64_t steps, size_t prints) {\nif(!w) return SCARP_OUTPUT_ERROR;\nif(!scarp_pass(w,0,0,steps,prints)) return w->error_code;\nif(((w->streams&1u) && (!io || !io->stdout_host || !io->stdout_host->write)) || ((w->streams&2u) && (!io || !io->stderr_host || !io->stderr_host->write))) { scarp_fail(w,SCARP_OUTPUT_ERROR,0,0); return w->error_code; }\n(void)scarp_pass(w,io,1,steps,prints); return w->error_code;\n}\nuint32_t scarp_run(struct scarp_workspace *w, const struct scarp_host *host, uint64_t steps, size_t prints) {\nconst struct scarp_io io={host,0,0,0};\nreturn scarp_run_io(w,&io,steps,prints);\n}\n");
    Ok(out)
}

fn emit_bytes(out: &mut String, bytes: &[u8], stream: usize, span: Span) {
    if bytes.is_empty() {
        return;
    }
    out.push_str("{ static const uint8_t text[]={");
    for byte in bytes {
        write!(out, "{byte},").unwrap();
    }
    writeln!(out, "0}}; w->streams|={}u;\nif(capture && !scarp_output(w,io,{stream}u,text,{}, {},{})) return 0; }}", 1 << stream, bytes.len(), span.start, span.end).unwrap();
}

fn step(out: &mut String, span: Span) {
    writeln!(
        out,
        "if(!scarp_step(w,{},{})) return 0;",
        span.start, span.end
    )
    .unwrap();
}
fn integer(value: i64) -> String {
    if value == i64::MIN {
        "INT64_MIN".into()
    } else if value < 0 {
        format!("(-INT64_C({}))", -value)
    } else {
        format!("INT64_C({value})")
    }
}
fn opcode(operator: BinaryOperator) -> usize {
    use BinaryOperator::*;
    match operator {
        Add => 0,
        Subtract => 1,
        Multiply => 2,
        Divide => 3,
        Remainder => 4,
        BitAnd => 5,
        BitOr => 6,
        BitXor => 7,
        ShiftLeft => 8,
        ShiftRight => 9,
        UnsignedRight => 10,
        Equal => 11,
        NotEqual => 12,
        Less => 13,
        LessEqual => 14,
        Greater => 15,
        GreaterEqual => 16,
    }
}

enum Task {
    Visit(usize),
    Finish(usize),
    Logical(usize),
    Close(usize),
}
fn emit_expression(out: &mut String, id: impl std::fmt::Display, expression: &ResolvedExpression) {
    writeln!(
        out,
        "\nstatic int scarp_e{id}(struct scarp_workspace *w,int64_t *out) {{"
    )
    .unwrap();
    let mut tasks = vec![Task::Visit(expression.syntax.root())];
    while let Some(task) = tasks.pop() {
        match task {
            Task::Visit(index) => {
                let node = &expression.syntax.nodes()[index];
                step(out, node.span);
                match &node.kind {
                    ExpressionKind::Integer(value) => {
                        writeln!(out, "w->values[{index}]={};", integer(*value)).unwrap();
                    }
                    ExpressionKind::Boolean(value) => {
                        writeln!(out, "w->values[{index}]={};", usize::from(*value)).unwrap();
                    }
                    ExpressionKind::Name(_) => {
                        writeln!(
                            out,
                            "w->values[{index}]=w->slots[{}];",
                            expression.slots[index].unwrap()
                        )
                        .unwrap();
                    }
                    ExpressionKind::Group { expression: child } => {
                        tasks.push(Task::Finish(index));
                        tasks.push(Task::Visit(*child));
                    }
                    ExpressionKind::Negate { operand, .. }
                    | ExpressionKind::BitNot { operand, .. }
                    | ExpressionKind::Not { operand, .. } => {
                        tasks.push(Task::Finish(index));
                        tasks.push(Task::Visit(*operand));
                    }
                    ExpressionKind::Binary { left, right, .. } => {
                        tasks.push(Task::Finish(index));
                        tasks.push(Task::Visit(*right));
                        tasks.push(Task::Visit(*left));
                    }
                    ExpressionKind::Logical { left, .. } => {
                        tasks.push(Task::Logical(index));
                        tasks.push(Task::Visit(*left));
                    }
                    _ => unreachable!("unsupported syntax checked before emission"),
                }
            }
            Task::Finish(index) => match expression.syntax.nodes()[index].kind {
                ExpressionKind::Group { expression: child } => {
                    writeln!(out, "w->values[{index}]=w->values[{child}];").unwrap();
                }
                ExpressionKind::Negate {
                    operand,
                    operator_span,
                } => {
                    step(out, operator_span);
                    writeln!(out,"if(w->values[{operand}]==INT64_MIN) return scarp_fail(w,7u,{},{});\nw->values[{index}]=-w->values[{operand}];",operator_span.start,operator_span.end).unwrap();
                }
                ExpressionKind::BitNot {
                    operand,
                    operator_span,
                } => {
                    step(out, operator_span);
                    writeln!(
                        out,
                        "w->values[{index}]=scarp_from_bits(~(uint64_t)w->values[{operand}]);"
                    )
                    .unwrap();
                }
                ExpressionKind::Not {
                    operand,
                    operator_span,
                } => {
                    step(out, operator_span);
                    writeln!(out, "w->values[{index}]=!w->values[{operand}];").unwrap();
                }
                ExpressionKind::Binary {
                    left,
                    operator,
                    right,
                    operator_span,
                } => {
                    step(out, operator_span);
                    writeln!(out,"if(!scarp_binary(w,{},w->values[{left}],w->values[{right}],&w->values[{index}],{},{})) return 0;",opcode(operator),operator_span.start,operator_span.end).unwrap();
                }
                _ => unreachable!("finish task refers to a nonlogical parent"),
            },
            Task::Logical(index) => {
                let ExpressionKind::Logical {
                    left,
                    right,
                    operator,
                    operator_span,
                } = expression.syntax.nodes()[index].kind
                else {
                    unreachable!()
                };
                step(out, operator_span);
                writeln!(
                    out,
                    "if({}w->values[{left}]) {{ w->values[{index}]=w->values[{left}]; }} else {{",
                    if operator == LogicalOperator::And {
                        "!"
                    } else {
                        ""
                    }
                )
                .unwrap();
                tasks.push(Task::Close(index));
                tasks.push(Task::Visit(right));
            }
            Task::Close(index) => {
                let ExpressionKind::Logical {
                    right,
                    operator_span,
                    ..
                } = expression.syntax.nodes()[index].kind
                else {
                    unreachable!()
                };
                step(out, operator_span);
                writeln!(out, "w->values[{index}]=w->values[{right}]; }}").unwrap();
            }
        }
    }
    writeln!(
        out,
        "*out=w->values[{}]; return 1;\n}}",
        expression.syntax.root()
    )
    .unwrap();
}
