//! Primitive value evaluation with checked integer-only arithmetic.

use crate::{
    ast::{BinaryOperator, Expression, ExpressionKind},
    diagnostics::{Diagnostic, DiagnosticKind, Span},
    resolver::{self, ResolvedExpression},
    value::Value,
};

/// Evaluate a standalone integer expression, preserving the v0.0.1 return type.
/// Use `evaluate_value` when a string result is allowed.
pub fn evaluate(expression: &Expression) -> Result<i64, Diagnostic> {
    evaluate_value(expression)?.require_integer(expression.nodes()[expression.root()].span)
}

/// Evaluate a standalone primitive expression without a variable environment.
pub fn evaluate_value(expression: &Expression) -> Result<Value, Diagnostic> {
    evaluate_with(expression, |_, name, span| {
        Err(resolver::unknown_name(name, span))
    })
}

pub(crate) fn evaluate_resolved(
    expression: &ResolvedExpression,
    environment: &[Value],
) -> Result<Value, Diagnostic> {
    evaluate_with(&expression.syntax, |node, name, span| {
        expression
            .slots
            .get(node)
            .copied()
            .flatten()
            .and_then(|slot| environment.get(slot))
            .cloned()
            .ok_or_else(|| resolver::unknown_name(name, span))
    })
}

fn evaluate_with(
    expression: &Expression,
    name_value: impl Fn(usize, &str, Span) -> Result<Value, Diagnostic>,
) -> Result<Value, Diagnostic> {
    let mut values: Vec<Value> = Vec::with_capacity(expression.nodes().len());
    for (index, node) in expression.nodes().iter().enumerate() {
        let value = match &node.kind {
            ExpressionKind::Integer(value) => Value::Integer(*value),
            ExpressionKind::String(value) => Value::from(value.as_str()),
            ExpressionKind::Name(name) => name_value(index, name, node.span)?,
            ExpressionKind::Group { expression } => values[*expression].clone(),
            ExpressionKind::Negate {
                operand,
                operator_span,
            } => {
                let integer = values[*operand].require_integer(*operator_span)?;
                Value::Integer(
                    integer
                        .checked_neg()
                        .ok_or_else(|| overflow(*operator_span))?,
                )
            }
            ExpressionKind::Binary {
                left,
                operator,
                right,
                operator_span,
            } => apply_binary(*operator, &values[*left], &values[*right], *operator_span)?,
        };
        values.push(value);
    }
    Ok(values[expression.root()].clone())
}

pub(crate) fn apply_binary(
    operator: BinaryOperator,
    left: &Value,
    right: &Value,
    span: Span,
) -> Result<Value, Diagnostic> {
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
