//! Checked integer evaluation over flat expression nodes.

use crate::{
    ast::{BinaryOperator, Expression, ExpressionKind},
    diagnostics::{Diagnostic, DiagnosticKind, Span},
};

/// Evaluate nodes in postorder. Expression construction is restricted to the
/// parser, which guarantees that all referenced children precede their parent.
pub fn evaluate(expression: &Expression) -> Result<i64, Diagnostic> {
    let mut values: Vec<i64> = Vec::with_capacity(expression.nodes().len());
    for node in expression.nodes() {
        let value = match node.kind {
            ExpressionKind::Integer(value) => value,
            ExpressionKind::Group { expression } => values[expression],
            ExpressionKind::Negate {
                operand,
                operator_span,
            } => values[operand]
                .checked_neg()
                .ok_or_else(|| overflow(operator_span))?,
            ExpressionKind::Binary {
                left,
                operator,
                right,
                operator_span,
            } => {
                let (left, right) = (values[left], values[right]);
                let result = match operator {
                    BinaryOperator::Add => left.checked_add(right),
                    BinaryOperator::Subtract => left.checked_sub(right),
                    BinaryOperator::Multiply => left.checked_mul(right),
                    BinaryOperator::Divide => {
                        if right == 0 {
                            return Err(Diagnostic::new(
                                DiagnosticKind::DivisionByZero,
                                operator_span,
                                "division by zero",
                            )
                            .with_help("the divisor must evaluate to a nonzero integer"));
                        }
                        left.checked_div(right)
                    }
                };
                result.ok_or_else(|| overflow(operator_span))?
            }
        };
        values.push(value);
    }
    Ok(values[expression.root()])
}

fn overflow(span: Span) -> Diagnostic {
    Diagnostic::new(
        DiagnosticKind::ArithmeticOverflow,
        span,
        "integer arithmetic overflow",
    )
    .with_help("each result must fit in a signed 64-bit integer")
}
