//! Native finite-vector kernels with explicit types, shapes, and work accounting.

use crate::{
    diagnostics::{Diagnostic, DiagnosticKind, Span},
    limits::Budget,
    value::{finite_number, FiniteFloat, NumericVector, Value, ValueType, MAX_VECTOR_LENGTH},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Builtin {
    Length,
    Sum,
    Mean,
    Dot,
    Add,
    Scale,
    Clamp,
    Fill,
    Softmax,
    Mse,
}

impl Builtin {
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "len" => Self::Length,
            "sum" => Self::Sum,
            "mean" => Self::Mean,
            "dot" => Self::Dot,
            "add" => Self::Add,
            "scale" => Self::Scale,
            "clamp" => Self::Clamp,
            "fill" => Self::Fill,
            "softmax" => Self::Softmax,
            "mse" => Self::Mse,
            _ => return None,
        })
    }
    pub fn parameter_types(self) -> &'static [ValueType] {
        use ValueType::{Float, Integer, Vector};
        match self {
            Self::Length | Self::Sum | Self::Mean | Self::Softmax => &[Vector],
            Self::Dot | Self::Add | Self::Mse => &[Vector, Vector],
            Self::Scale => &[Vector, Float],
            Self::Clamp => &[Vector, Float, Float],
            Self::Fill => &[Integer, Float],
        }
    }
    pub fn result_type(self) -> ValueType {
        match self {
            Self::Length => ValueType::Integer,
            Self::Sum | Self::Mean | Self::Dot | Self::Mse => ValueType::Float,
            _ => ValueType::Vector,
        }
    }
}

fn length_error(span: Span) -> Diagnostic {
    Diagnostic::new(
        DiagnosticKind::VectorLength,
        span,
        "vector lengths must match and stay within 65536 elements",
    )
}
fn domain_error(span: Span, message: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticKind::InvalidArgument, span, message)
}

fn compensated(
    values: impl Iterator<Item = Result<f64, Diagnostic>>,
    span: Span,
) -> Result<FiniteFloat, Diagnostic> {
    let mut sum = 0.0;
    let mut correction = 0.0;
    for value in values {
        let value = value?;
        let next = finite_number(sum + value, span)?.get();
        let delta = if sum.abs() >= value.abs() {
            (sum - next) + value
        } else {
            (value - next) + sum
        };
        correction = finite_number(correction + delta, span)?.get();
        sum = next;
    }
    finite_number(sum + correction, span)
}

pub(crate) fn evaluate(
    function: Builtin,
    args: &[Value],
    budget: &mut Budget,
    span: Span,
) -> Result<Value, Diagnostic> {
    if function == Builtin::Fill {
        let count = args[0].require_integer(span)?;
        let count = usize::try_from(count)
            .ok()
            .filter(|count| *count <= MAX_VECTOR_LENGTH)
            .ok_or_else(|| length_error(span))?;
        let value = finite_number(args[1].require_float(span)?, span)?;
        budget.charge(count as u64, span)?;
        return Ok(Value::Vector(NumericVector::from_finite(vec![
            value;
            count
        ])));
    }
    let left = args[0].require_vector(span)?;
    if function == Builtin::Length {
        return Ok(Value::Integer(left.len() as i64));
    }
    let right = if matches!(function, Builtin::Dot | Builtin::Add | Builtin::Mse) {
        let right = args[1].require_vector(span)?;
        if left.len() != right.len() {
            return Err(length_error(span));
        }
        Some(right)
    } else {
        None
    };
    if matches!(function, Builtin::Mean | Builtin::Mse | Builtin::Softmax) && left.is_empty() {
        return Err(domain_error(span, "this kernel requires a nonempty vector"));
    }
    let work = match function {
        Builtin::Softmax => 4,
        Builtin::Dot | Builtin::Mse => 2,
        _ => 1,
    };
    budget.charge(left.len() as u64 * work, span)?;
    let floats = || left.as_slice().iter().map(|value| value.get());
    match function {
        Builtin::Sum | Builtin::Mean => {
            let sum = compensated(floats().map(Ok), span)?;
            if function == Builtin::Mean {
                Ok(Value::Float(finite_number(
                    sum.get() / left.len() as f64,
                    span,
                )?))
            } else {
                Ok(Value::Float(sum))
            }
        }
        Builtin::Dot | Builtin::Mse => {
            let values = floats()
                .zip(right.unwrap().as_slice())
                .map(|(left, right)| {
                    let value = if function == Builtin::Dot {
                        finite_number(left * right.get(), span)?
                    } else {
                        let delta = finite_number(left - right.get(), span)?.get();
                        finite_number(delta * delta, span)?
                    };
                    Ok(value.get())
                });
            let sum = compensated(values, span)?;
            if function == Builtin::Mse {
                Ok(Value::Float(finite_number(
                    sum.get() / left.len() as f64,
                    span,
                )?))
            } else {
                Ok(Value::Float(sum))
            }
        }
        Builtin::Add => {
            let values = floats()
                .zip(right.unwrap().as_slice())
                .map(|(a, b)| finite_number(a + b.get(), span))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Value::Vector(NumericVector::from_finite(values)))
        }
        Builtin::Scale => {
            let factor = args[1].require_float(span)?;
            let values = floats()
                .map(|value| finite_number(value * factor, span))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Value::Vector(NumericVector::from_finite(values)))
        }
        Builtin::Clamp => {
            let low = args[1].require_float(span)?;
            let high = args[2].require_float(span)?;
            if low > high {
                return Err(domain_error(
                    span,
                    "clamp lower bound exceeds its upper bound",
                ));
            }
            let values = floats()
                .map(|value| {
                    finite_number(
                        if value < low {
                            low
                        } else if value > high {
                            high
                        } else {
                            value
                        },
                        span,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Value::Vector(NumericVector::from_finite(values)))
        }
        Builtin::Softmax => {
            let max = floats().fold(f64::NEG_INFINITY, f64::max);
            let mut weights = floats()
                .map(|value| {
                    let shifted = value - max;
                    // An overflowing negative shift is an underflowing contribution.
                    finite_number(
                        if shifted.is_finite() {
                            shifted.exp()
                        } else {
                            0.0
                        },
                        span,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            let normalizer = compensated(weights.iter().map(|value| Ok(value.get())), span)?.get();
            for value in &mut weights {
                *value = finite_number(value.get() / normalizer, span)?;
            }
            Ok(Value::Vector(NumericVector::from_finite(weights)))
        }
        Builtin::Length | Builtin::Fill => unreachable!("handled before vector kernels"),
    }
}
