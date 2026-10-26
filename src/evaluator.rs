//! Checked integer evaluation over flat expression nodes.

use crate::{
    ast::{BinaryOperator, Expression, ExpressionKind},
    diagnostics::{Diagnostic, DiagnosticKind, Span},
    resolver::{self, ResolvedExpression},
};

/// Evaluate nodes in postorder. Expression construction is restricted to the
/// parser, which guarantees that all referenced children precede their parent.
pub fn evaluate(expression: &Expression) -> Result<i64, Diagnostic> {
    evaluate_with(expression, |_, name, span| {
        Err(resolver::unknown_name(name, span))
    })
}

pub(crate) fn evaluate_resolved(
    expression: &ResolvedExpression,
    environment: &[i64],
) -> Result<i64, Diagnostic> {
    evaluate_with(&expression.syntax, |node, name, span| {
        expression
            .slots
            .get(node)
            .copied()
            .flatten()
            .and_then(|slot| environment.get(slot))
            .copied()
            .ok_or_else(|| resolver::unknown_name(name, span))
    })
}

fn evaluate_with(
    expression: &Expression,
    name_value: impl Fn(usize, &str, Span) -> Result<i64, Diagnostic>,
) -> Result<i64, Diagnostic> {
    let mut values: Vec<i64> = Vec::with_capacity(expression.nodes().len());
    for (index, node) in expression.nodes().iter().enumerate() {
        let value = match &node.kind {
            ExpressionKind::Integer(value) => *value,
            ExpressionKind::Name(name) => name_value(index, name, node.span)?,
            ExpressionKind::Group { expression } => values[*expression],
            ExpressionKind::Negate {
                operand,
                operator_span,
            } => values[*operand]
                .checked_neg()
                .ok_or_else(|| overflow(*operator_span))?,
            ExpressionKind::Binary {
                left,
                operator,
                right,
                operator_span,
            } => {
                let (left, right) = (values[*left], values[*right]);
                apply_binary(*operator, left, right, *operator_span)?
            }
        };
        values.push(value);
    }
    Ok(values[expression.root()])
}

pub(crate) fn apply_binary(
    operator: BinaryOperator,
    left: i64,
    right: i64,
    span: Span,
) -> Result<i64, Diagnostic> {
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
    result.ok_or_else(|| overflow(span))
}

fn overflow(span: Span) -> Diagnostic {
    Diagnostic::new(
        DiagnosticKind::ArithmeticOverflow,
        span,
        "integer arithmetic overflow",
    )
    .with_help("each result must fit in a signed 64-bit integer")
}
