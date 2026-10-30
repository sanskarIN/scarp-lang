//! Primitive evaluation with checked integers and finite binary64 arithmetic.

use crate::{
    ast::{BinaryOperator, Expression, ExpressionKind, LogicalOperator},
    diagnostics::{Diagnostic, DiagnosticKind, Span},
    limits::Budget,
    numeric::{self, Builtin},
    resolver::{self, ResolvedCall, ResolvedExpression},
    value::{finite_number, type_mismatch, Value, ValueType},
};

/// Evaluate a standalone integer expression, preserving the v0.0.1 return type.
/// Use `evaluate_value` when any primitive result is allowed.
pub fn evaluate(expression: &Expression) -> Result<i64, Diagnostic> {
    evaluate_value(expression)?.require_integer(expression.nodes()[expression.root()].span)
}

/// Evaluate a standalone primitive expression without a variable environment.
pub fn evaluate_value(expression: &Expression) -> Result<Value, Diagnostic> {
    Workspace::default().evaluate(
        &resolver::resolve_standalone(expression)?,
        &[],
        &mut Budget::new(u64::MAX),
    )
}

/// Scratch storage belongs to one execution, never to the immutable program.
#[derive(Default)]
pub(crate) struct Workspace {
    values: Vec<Value>,
    tasks: Vec<Task>,
}

enum Task {
    Vector(usize, Span),
    Index(Span),
    Builtin(Builtin, usize, Span),
    UserCall(usize, usize, Span),
    Visit(usize),
    Negate(Span),
    Not(Span),
    Boolean(Span),
    Binary(BinaryOperator, Span),
    Logical(LogicalOperator, usize, Span),
}

pub(crate) enum Evaluation {
    Complete(Value),
    Call {
        node: usize,
        arguments: Vec<Value>,
        span: Span,
    },
}

impl Workspace {
    pub(crate) fn evaluate(
        &mut self,
        expression: &ResolvedExpression,
        environment: &[Value],
        budget: &mut Budget,
    ) -> Result<Value, Diagnostic> {
        self.start(expression);
        match self.resume(expression, environment, budget)? {
            Evaluation::Complete(value) => Ok(value),
            Evaluation::Call { span, .. } => Err(Diagnostic::new(
                DiagnosticKind::FunctionUnsupported,
                span,
                "user calls require a program execution frame",
            )),
        }
    }

    pub(crate) fn start(&mut self, expression: &ResolvedExpression) {
        self.values.clear();
        self.tasks.clear();
        self.tasks.push(Task::Visit(expression.syntax.root()));
    }

    pub(crate) fn accept(&mut self, value: Value) {
        self.values.push(value);
    }

    pub(crate) fn resume(
        &mut self,
        expression: &ResolvedExpression,
        environment: &[Value],
        budget: &mut Budget,
    ) -> Result<Evaluation, Diagnostic> {
        let Self { values, tasks } = self;
        while let Some(task) = tasks.pop() {
            let span = match &task {
                Task::Visit(index) => expression.syntax.nodes()[*index].span,
                Task::UserCall(_, _, span)
                | Task::Vector(_, span)
                | Task::Index(span)
                | Task::Builtin(_, _, span)
                | Task::Negate(span)
                | Task::Not(span)
                | Task::Boolean(span)
                | Task::Binary(_, span)
                | Task::Logical(_, _, span) => *span,
            };
            budget.charge(1, span)?;
            match task {
                Task::Visit(index) => {
                    let node = &expression.syntax.nodes()[index];
                    match &node.kind {
                        ExpressionKind::UserCall { arguments, .. } => {
                            tasks.push(match &expression.calls[index] {
                                Some(ResolvedCall::Builtin(builtin)) => {
                                    Task::Builtin(*builtin, arguments.len(), node.span)
                                }
                                _ => Task::UserCall(index, arguments.len(), node.span),
                            });
                            for argument in arguments.iter().rev() {
                                tasks.push(Task::Visit(argument.expression));
                            }
                        }
                        ExpressionKind::Vector { elements } => {
                            tasks.push(Task::Vector(elements.len(), node.span));
                            for &element in elements.iter().rev() {
                                tasks.push(Task::Visit(element));
                            }
                        }
                        ExpressionKind::Index {
                            vector,
                            index,
                            operator_span,
                        } => {
                            tasks.push(Task::Index(*operator_span));
                            tasks.push(Task::Visit(*index));
                            tasks.push(Task::Visit(*vector));
                        }
                        ExpressionKind::Integer(value) => values.push(Value::Integer(*value)),
                        ExpressionKind::Float(value) => values.push(Value::Float(*value)),
                        ExpressionKind::Boolean(value) => values.push(Value::Boolean(*value)),
                        ExpressionKind::String(value) => values.push(Value::from(value.as_str())),
                        ExpressionKind::Name(name) => {
                            let value = expression.slots[index]
                                .and_then(|slot| environment.get(slot))
                                .cloned()
                                .ok_or_else(|| resolver::unknown_name(name, node.span))?;
                            values.push(value);
                        }
                        ExpressionKind::Group { expression } => {
                            tasks.push(Task::Visit(*expression))
                        }
                        ExpressionKind::Negate {
                            operand,
                            operator_span,
                        } => {
                            tasks.push(Task::Negate(*operator_span));
                            tasks.push(Task::Visit(*operand));
                        }
                        ExpressionKind::Not {
                            operand,
                            operator_span,
                        } => {
                            tasks.push(Task::Not(*operator_span));
                            tasks.push(Task::Visit(*operand));
                        }
                        ExpressionKind::Binary {
                            left,
                            operator,
                            right,
                            operator_span,
                        } => {
                            tasks.push(Task::Binary(*operator, *operator_span));
                            tasks.push(Task::Visit(*right));
                            tasks.push(Task::Visit(*left));
                        }
                        ExpressionKind::Logical {
                            left,
                            operator,
                            right,
                            operator_span,
                        } => {
                            tasks.push(Task::Logical(*operator, *right, *operator_span));
                            tasks.push(Task::Visit(*left));
                        }
                    }
                }
                Task::Vector(count, span) => {
                    budget.charge(count as u64, span)?;
                    let start = values.len() - count;
                    let mut elements = numeric::buffer(count, span)?;
                    for value in &values[start..] {
                        elements.push(finite_number(value.require_float(span)?, span)?);
                    }
                    values.truncate(start);
                    values.push(numeric::vector(elements, span)?);
                }
                Task::Index(span) => {
                    let index = values.pop().unwrap().require_integer(span)?;
                    let value = values.pop().unwrap();
                    let vector = value.require_vector(span)?;
                    let element = usize::try_from(index)
                        .ok()
                        .and_then(|i| vector.as_slice().get(i))
                        .ok_or_else(|| {
                            Diagnostic::new(
                                DiagnosticKind::VectorIndex,
                                span,
                                format!("index {index} is outside vector length {}", vector.len()),
                            )
                        })?;
                    values.push(Value::Float(*element));
                }
                Task::Builtin(builtin, count, span) => {
                    let start = values.len() - count;
                    let value =
                        numeric::evaluate_with_budget(builtin, &values[start..], budget, span)?;
                    values.truncate(start);
                    values.push(value);
                }
                Task::UserCall(node, count, span) => {
                    let arguments = values.split_off(values.len() - count);
                    return Ok(Evaluation::Call {
                        node,
                        arguments,
                        span,
                    });
                }
                Task::Negate(span) => {
                    let value = values.pop().unwrap();
                    values.push(match value {
                        Value::Float(value) => finite(-value.get(), span)?,
                        _ => Value::Integer(
                            value
                                .require_integer(span)?
                                .checked_neg()
                                .ok_or_else(|| overflow(span))?,
                        ),
                    });
                }
                Task::Not(span) => {
                    let value = values.pop().unwrap().require_boolean(span)?;
                    values.push(Value::Boolean(!value));
                }
                Task::Boolean(span) => {
                    let value = values.pop().unwrap().require_boolean(span)?;
                    values.push(Value::Boolean(value));
                }
                Task::Binary(operator, span) => {
                    let right = values.pop().unwrap();
                    let left = values.pop().unwrap();
                    if let (Some(a), Some(b)) = (left.as_vector(), right.as_vector()) {
                        budget.charge(a.len().max(b.len()) as u64, span)?;
                    }
                    values.push(apply_binary(operator, &left, &right, span)?);
                }
                Task::Logical(operator, right, span) => {
                    let left = values.pop().unwrap().require_boolean(span)?;
                    if (operator == LogicalOperator::And && !left)
                        || (operator == LogicalOperator::Or && left)
                    {
                        values.push(Value::Boolean(left));
                    } else {
                        tasks.push(Task::Boolean(span));
                        tasks.push(Task::Visit(right));
                    }
                }
            }
        }
        Ok(Evaluation::Complete(
            values
                .pop()
                .expect("a resolved expression produces one value"),
        ))
    }
}

pub(crate) fn apply_binary(
    operator: BinaryOperator,
    left: &Value,
    right: &Value,
    span: Span,
) -> Result<Value, Diagnostic> {
    if matches!(operator, BinaryOperator::Equal | BinaryOperator::NotEqual) {
        if left.kind() != right.kind() {
            return Err(type_mismatch(span, left.kind(), right.kind()));
        }
        return Ok(Value::Boolean(if operator == BinaryOperator::Equal {
            left == right
        } else {
            left != right
        }));
    }
    if left.kind() == ValueType::Float || right.kind() == ValueType::Float {
        let left = left.require_float(span)?;
        let right = right.require_float(span)?;
        return match operator {
            BinaryOperator::Add => finite(left + right, span),
            BinaryOperator::Subtract => finite(left - right, span),
            BinaryOperator::Multiply => finite(left * right, span),
            BinaryOperator::Divide => {
                if right == 0.0 {
                    return Err(Diagnostic::new(
                        DiagnosticKind::DivisionByZero,
                        span,
                        "division by zero",
                    )
                    .with_help("the float divisor must be nonzero, including signed zero"));
                }
                finite(left / right, span)
            }
            BinaryOperator::Less => Ok(Value::Boolean(left < right)),
            BinaryOperator::LessEqual => Ok(Value::Boolean(left <= right)),
            BinaryOperator::Greater => Ok(Value::Boolean(left > right)),
            BinaryOperator::GreaterEqual => Ok(Value::Boolean(left >= right)),
            BinaryOperator::Equal | BinaryOperator::NotEqual => {
                unreachable!("equality handled above")
            }
        };
    }
    let left = left.require_integer(span)?;
    let right = right.require_integer(span)?;
    let result = match operator {
        BinaryOperator::Add => left.checked_add(right),
        BinaryOperator::Subtract => left.checked_sub(right),
        BinaryOperator::Multiply => left.checked_mul(right),
        BinaryOperator::Divide => {
            if right == 0 {
                return Err(Diagnostic::new(
                    DiagnosticKind::DivisionByZero,
                    span,
                    "division by zero",
                )
                .with_help("the divisor must evaluate to a nonzero integer"));
            }
            left.checked_div(right)
        }
        BinaryOperator::Less => return Ok(Value::Boolean(left < right)),
        BinaryOperator::LessEqual => return Ok(Value::Boolean(left <= right)),
        BinaryOperator::Greater => return Ok(Value::Boolean(left > right)),
        BinaryOperator::GreaterEqual => return Ok(Value::Boolean(left >= right)),
        BinaryOperator::Equal | BinaryOperator::NotEqual => unreachable!("equality handled above"),
    };
    result.map(Value::Integer).ok_or_else(|| overflow(span))
}

fn overflow(span: Span) -> Diagnostic {
    Diagnostic::new(
        DiagnosticKind::ArithmeticOverflow,
        span,
        "integer arithmetic overflow",
    )
    .with_help("each result must fit in a signed 64-bit integer")
}

pub(crate) fn finite(value: f64, span: Span) -> Result<Value, Diagnostic> {
    finite_number(value, span).map(Value::Float)
}
