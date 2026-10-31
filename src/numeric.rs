//! Checked finite-vector kernels, independent of syntax and function frames.

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
    Mse,
    Add,
    Scale,
    Clamp,
    Fill,
    Softmax,
    Norm,
    Normalize,
    LogSumExp,
    LogSoftmax,
}

impl Builtin {
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "len" => Self::Length,
            "sum" => Self::Sum,
            "mean" => Self::Mean,
            "dot" => Self::Dot,
            "mse" => Self::Mse,
            "add" => Self::Add,
            "scale" => Self::Scale,
            "clamp" => Self::Clamp,
            "fill" => Self::Fill,
            "softmax" => Self::Softmax,
            "norm" => Self::Norm,
            "normalize" => Self::Normalize,
            "logsumexp" => Self::LogSumExp,
            "logsoftmax" => Self::LogSoftmax,
            _ => return None,
        })
    }

    pub fn parameter_types(self) -> &'static [ValueType] {
        use ValueType::{Float, Integer, Vector};
        match self {
            Self::Length
            | Self::Sum
            | Self::Mean
            | Self::Softmax
            | Self::Norm
            | Self::Normalize
            | Self::LogSumExp
            | Self::LogSoftmax => &[Vector],
            Self::Dot | Self::Mse | Self::Add => &[Vector, Vector],
            Self::Scale => &[Vector, Float],
            Self::Clamp => &[Vector, Float, Float],
            Self::Fill => &[Float, Integer],
        }
    }

    pub fn result_type(self) -> ValueType {
        match self {
            Self::Length => ValueType::Integer,
            Self::Sum | Self::Mean | Self::Dot | Self::Mse | Self::Norm | Self::LogSumExp => {
                ValueType::Float
            }
            _ => ValueType::Vector,
        }
    }
}

/// Evaluate a native kernel with checked argument types and an explicit work
/// cap. This host API charges kernel work, excluding language dispatch tasks;
/// host diagnostics use an empty source span. Inputs remain immutable.
pub fn evaluate(
    function: Builtin,
    arguments: &[Value],
    max_steps: u64,
) -> Result<Value, Diagnostic> {
    evaluate_with_budget(
        function,
        arguments,
        &mut Budget::new(max_steps),
        Span::default(),
    )
}

/// Run a standalone built-in with both the step and cumulative numerical
/// payload-allocation limits. Existing `evaluate` keeps its historical
/// step-only host contract; it does not allocate any user program frames.
pub fn evaluate_with_vector_limit(
    function: Builtin,
    arguments: &[Value],
    max_steps: u64,
    max_vector_bytes: usize,
) -> Result<Value, Diagnostic> {
    evaluate_with_budget(
        function,
        arguments,
        &mut Budget::new(max_steps).with_vector_bytes(max_vector_bytes),
        Span::default(),
    )
}

pub(crate) fn length_error(span: Span) -> Diagnostic {
    Diagnostic::new(
        DiagnosticKind::VectorLength,
        span,
        "vector lengths must match and stay within 0..65536 elements",
    )
}

fn domain(span: Span, message: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticKind::NumericDomain, span, message)
}

pub(crate) fn buffer(
    count: usize,
    budget: &mut Budget,
    span: Span,
) -> Result<Vec<FiniteFloat>, Diagnostic> {
    if count > MAX_VECTOR_LENGTH {
        return Err(length_error(span));
    }
    // Charge the exact logical element payload before any allocation occurs.
    // A failed reservation ends execution; as with work steps, no refund is
    // necessary and no partially prepared output becomes visible.
    budget.charge_vector_allocation(count * std::mem::size_of::<FiniteFloat>(), span)?;
    let mut values = Vec::new();
    values.try_reserve_exact(count).map_err(|_| {
        Diagnostic::new(
            DiagnosticKind::VectorAllocation,
            span,
            "cannot reserve the bounded vector buffer",
        )
    })?;
    Ok(values)
}

pub(crate) fn vector(values: Vec<FiniteFloat>, span: Span) -> Result<Value, Diagnostic> {
    NumericVector::from_finite(values)
        .map(Value::Vector)
        .ok_or_else(|| length_error(span))
}

// Neumaier's ordered compensation keeps small cancellation terms. Every stored
// intermediate stays finite; this is not an exact or overflow-proof sum.
fn compensated(
    values: impl Iterator<Item = Result<f64, Diagnostic>>,
    span: Span,
) -> Result<f64, Diagnostic> {
    let mut sum: f64 = 0.0;
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
    Ok(finite_number(sum + correction, span)?.get())
}

fn mean(values: &NumericVector, span: Span) -> Result<f64, Diagnostic> {
    let floats = || values.as_slice().iter().map(|value| value.get());
    if let Ok(sum) = compensated(floats().map(Ok), span) {
        return Ok(sum / values.len() as f64);
    }
    // Average terms before summing to retain tiny cancellation residuals that
    // maximum-magnitude normalization could underflow away.
    if let Ok(average) = compensated(floats().map(|value| Ok(value / values.len() as f64)), span) {
        return Ok(average);
    }
    // Preserve the compensated fast path for cancellation/subnormals, but avoid
    // overflowing a sum when only its finite mean was requested.
    let scale = floats().map(f64::abs).fold(0.0, f64::max);
    let normalized = compensated(floats().map(|value| Ok(value / scale)), span)?;
    let fraction = (normalized / values.len() as f64).clamp(-1.0, 1.0);
    Ok(finite_number(fraction * scale, span)?.get())
}

fn mse(left: &NumericVector, right: &NumericVector, span: Span) -> Result<f64, Diagnostic> {
    let differences = || {
        left.as_slice()
            .iter()
            .zip(right.as_slice())
            .map(|(a, b)| finite_number(a.get() - b.get(), span).map(FiniteFloat::get))
    };
    let mut scale: f64 = 0.0;
    for difference in differences() {
        scale = scale.max(difference?.abs());
    }
    if scale == 0.0 {
        return Ok(0.0);
    }
    let squares = differences().map(|difference| {
        let normalized = difference? / scale;
        Ok(normalized * normalized)
    });
    let average = compensated(squares, span)? / left.len() as f64;
    // Average before rescaling: scale^2 itself need not be representable.
    Ok(finite_number((scale * average) * scale, span)?.get())
}

// Divide by the largest magnitude before squaring so finite input vectors
// can be normalized even if their unscaled squared sum or L2 norm overflows.
// The second component is sqrt(sum((x / scale)^2)); it lies in [1, sqrt(n)]
// for nonzero inputs and remains finite under the vector length bound.
fn scaled_l2(values: &NumericVector, span: Span) -> Result<(f64, f64), Diagnostic> {
    let scale = values
        .as_slice()
        .iter()
        .map(|value| value.get().abs())
        .fold(0.0_f64, f64::max);
    if scale == 0.0 {
        return Ok((0.0, 0.0));
    }
    let squares = values.as_slice().iter().map(|value| {
        let normalized = value.get() / scale;
        Ok(normalized * normalized)
    });
    Ok((scale, compensated(squares, span)?.sqrt()))
}

// Maximum-shifted log normalization keeps the exponential sum finite and
// nonzero without materializing a second vector. For a finite input vector,
// shifted terms are <= 0; a negatively overflowing difference represents a
// negligible contribution (exp(-infinity) = 0), not a nonfinite stored value.
// Log outputs are individually checked at the call site.
fn log_partition(values: &NumericVector, span: Span) -> Result<(f64, f64), Diagnostic> {
    let maximum = values
        .as_slice()
        .iter()
        .map(|value| value.get())
        .fold(f64::NEG_INFINITY, f64::max);
    let terms = values.as_slice().iter().map(|value| {
        let shifted = value.get() - maximum;
        Ok(if shifted.is_finite() {
            shifted.exp()
        } else {
            0.0
        })
    });
    let sum = compensated(terms, span)?;
    // The maximum always contributes 1.0, so sum is in [1, vector length].
    Ok((maximum, sum.ln()))
}

pub(crate) fn evaluate_with_budget(
    function: Builtin,
    args: &[Value],
    budget: &mut Budget,
    span: Span,
) -> Result<Value, Diagnostic> {
    let types = function.parameter_types();
    if args.len() != types.len() {
        return Err(Diagnostic::new(
            DiagnosticKind::CallArguments,
            span,
            format!("expected {} arguments, found {}", types.len(), args.len()),
        ));
    }
    for (value, &expected) in args.iter().zip(types) {
        if value.kind() != expected {
            return Err(crate::value::type_mismatch(span, expected, value.kind()));
        }
    }
    if function == Builtin::Fill {
        let count = usize::try_from(args[1].require_integer(span)?)
            .ok()
            .filter(|&count| count <= MAX_VECTOR_LENGTH)
            .ok_or_else(|| length_error(span))?;
        budget.charge(count as u64, span)?;
        let value = finite_number(args[0].require_float(span)?, span)?;
        let mut values = buffer(count, budget, span)?;
        values.resize(count, value);
        return vector(values, span);
    }
    let left = args[0].require_vector(span)?;
    if function == Builtin::Length {
        return Ok(Value::Integer(left.len() as i64));
    }
    let right = if matches!(function, Builtin::Dot | Builtin::Mse | Builtin::Add) {
        let right = args[1].require_vector(span)?;
        if left.len() != right.len() {
            return Err(length_error(span));
        }
        Some(right)
    } else {
        None
    };
    if matches!(
        function,
        Builtin::Mean
            | Builtin::Mse
            | Builtin::Softmax
            | Builtin::Normalize
            | Builtin::LogSumExp
            | Builtin::LogSoftmax
    ) && left.is_empty()
    {
        return Err(domain(
            span,
            "this numerical kernel requires a nonempty vector",
        ));
    }
    let work = match function {
        Builtin::Mean
        | Builtin::Mse
        | Builtin::Softmax
        | Builtin::Norm
        | Builtin::Normalize
        | Builtin::LogSumExp
        | Builtin::LogSoftmax => 4,
        Builtin::Dot => 2,
        _ => 1,
    };
    budget.charge(left.len() as u64 * work, span)?;
    let floats = || left.as_slice().iter().map(|value| value.get());
    match function {
        Builtin::LogSumExp | Builtin::LogSoftmax => {
            let (maximum, log_sum) = log_partition(left, span)?;
            if function == Builtin::LogSumExp {
                return finite_number(maximum + log_sum, span).map(Value::Float);
            }
            let mut values = buffer(left.len(), budget, span)?;
            for value in floats() {
                // The exact log probability can be too negative to fit in
                // binary64; reject it instead of silently storing -infinity.
                values.push(finite_number((value - maximum) - log_sum, span)?);
            }
            vector(values, span)
        }
        Builtin::Norm | Builtin::Normalize => {
            let (scale, magnitude) = scaled_l2(left, span)?;
            if function == Builtin::Norm {
                return finite_number(scale * magnitude, span).map(Value::Float);
            }
            if scale == 0.0 {
                return Err(domain(span, "cannot normalize a zero vector"));
            }
            let mut values = buffer(left.len(), budget, span)?;
            for value in floats() {
                values.push(finite_number((value / scale) / magnitude, span)?);
            }
            vector(values, span)
        }
        Builtin::Sum => compensated(floats().map(Ok), span)
            .and_then(|v| finite_number(v, span))
            .map(Value::Float),
        Builtin::Mean => mean(left, span)
            .and_then(|v| finite_number(v, span))
            .map(Value::Float),
        Builtin::Mse => mse(left, right.unwrap(), span)
            .and_then(|v| finite_number(v, span))
            .map(Value::Float),
        Builtin::Dot => {
            let products = floats()
                .zip(right.unwrap().as_slice())
                .map(|(a, b)| finite_number(a * b.get(), span).map(FiniteFloat::get));
            compensated(products, span)
                .and_then(|v| finite_number(v, span))
                .map(Value::Float)
        }
        Builtin::Add | Builtin::Scale | Builtin::Clamp => {
            let factor = if function == Builtin::Scale {
                args[1].require_float(span)?
            } else {
                0.0
            };
            let (low, high) = if function == Builtin::Clamp {
                (args[1].require_float(span)?, args[2].require_float(span)?)
            } else {
                (0.0, 0.0)
            };
            if low > high {
                return Err(domain(span, "clamp lower bound exceeds its upper bound"));
            }
            let mut values = buffer(left.len(), budget, span)?;
            for (index, value) in floats().enumerate() {
                let result = match function {
                    Builtin::Add => value + right.unwrap().as_slice()[index].get(),
                    Builtin::Scale => value * factor,
                    _ => {
                        if value < low {
                            low
                        } else if value > high {
                            high
                        } else {
                            value
                        }
                    }
                };
                values.push(finite_number(result, span)?);
            }
            vector(values, span)
        }
        Builtin::Softmax => {
            let maximum = floats().fold(f64::NEG_INFINITY, f64::max);
            let mut values = buffer(left.len(), budget, span)?;
            for value in floats() {
                let shifted = value - maximum;
                // A negative overflowing shift has a negligible exponential.
                values.push(finite_number(
                    if shifted.is_finite() {
                        shifted.exp()
                    } else {
                        0.0
                    },
                    span,
                )?);
            }
            let normalizer = compensated(values.iter().map(|v| Ok(v.get())), span)?;
            for value in &mut values {
                *value = finite_number(value.get() / normalizer, span)?;
            }
            vector(values, span)
        }
        Builtin::Length | Builtin::Fill => unreachable!("handled before vector kernels"),
    }
}
