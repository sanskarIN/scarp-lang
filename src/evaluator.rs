//! Primitive value evaluation with checked integer-only arithmetic.

use crate::{
    ast::{BinaryOperator, Expression, ExpressionKind, LogicalOperator},
    diagnostics::{Diagnostic, DiagnosticKind, Span},
    resolver::{self, ResolvedExpression},
    value::{type_mismatch, Value},
};

/// Evaluate a standalone integer expression, preserving the v0.0.1 return type.
/// Use `evaluate_value` when a string result is allowed.
pub fn evaluate(expression: &Expression) -> Result<i64, Diagnostic> {
    evaluate_value(expression)?.require_integer(expression.nodes()[expression.root()].span)
}

/// Evaluate a standalone primitive expression without a variable environment.
pub fn evaluate_value(expression: &Expression) -> Result<Value, Diagnostic> {
    evaluate_resolved(&resolver::resolve_standalone(expression)?, &[])
}

enum Task {
    Visit(usize),
    Negate(Span),
    Not(Span),
    Boolean(Span),
    Binary(BinaryOperator, Span),
    Logical(LogicalOperator, usize, Span),
}

pub(crate) fn evaluate_resolved(
    expression: &ResolvedExpression,
    environment: &[Value],
) -> Result<Value, Diagnostic> {
    let mut values = Vec::new();
    let mut tasks = vec![Task::Visit(expression.syntax.root())];
    while let Some(task) = tasks.pop() {
        match task {
            Task::Visit(index) => {
                let node = &expression.syntax.nodes()[index];
                match &node.kind {
                    ExpressionKind::Integer(value) => values.push(Value::Integer(*value)),
                    ExpressionKind::Boolean(value) => values.push(Value::Boolean(*value)),
                    ExpressionKind::String(value) => values.push(Value::from(value.as_str())),
                    ExpressionKind::Name(name) => {
                        let value = expression.slots[index]
                            .and_then(|slot| environment.get(slot))
                            .cloned()
                            .ok_or_else(|| resolver::unknown_name(name, node.span))?;
                        values.push(value);
                    }
                    ExpressionKind::Group { expression } => tasks.push(Task::Visit(*expression)),
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
            Task::Negate(span) => {
                let integer = values.pop().unwrap().require_integer(span)?;
                values.push(Value::Integer(
                    integer.checked_neg().ok_or_else(|| overflow(span))?,
                ));
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
    Ok(values
        .pop()
        .expect("a resolved expression produces one value"))
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
