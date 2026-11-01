//! Shape-checked two-axis kernels. Syntax and frame management stay outside this module.

use super::{buffer, compensated, length_error, vector, Builtin};
use crate::{
    diagnostics::{Diagnostic, DiagnosticKind, Span},
    limits::Budget,
    value::{finite_number, FiniteFloat, NumericTile, NumericVector, Value, MAX_VECTOR_LENGTH},
};

fn shape_error(span: Span, message: String) -> Diagnostic {
    Diagnostic::new(DiagnosticKind::VectorLength, span, message)
        .with_help("use compatible tile shapes with each axis and the cell count in 0..65536")
}

fn dimension(value: &Value, axis: &str, span: Span) -> Result<usize, Diagnostic> {
    let value = value.require_integer(span)?;
    usize::try_from(value)
        .ok()
        .filter(|&n| n <= MAX_VECTOR_LENGTH)
        .ok_or_else(|| {
            shape_error(
                span,
                format!("tile {axis} must be in 0..65536, found {value}"),
            )
        })
}

fn index(value: &Value, count: usize, axis: &str, span: Span) -> Result<usize, Diagnostic> {
    let value = value.require_integer(span)?;
    usize::try_from(value)
        .ok()
        .filter(|&n| n < count)
        .ok_or_else(|| {
            Diagnostic::new(
                DiagnosticKind::VectorIndex,
                span,
                format!("tile {axis} index {value} is outside axis length {count}"),
            )
            .with_help("use a zero-based index smaller than the corresponding tile axis")
        })
}

fn tile(
    values: Vec<FiniteFloat>,
    height: usize,
    width: usize,
    span: Span,
) -> Result<Value, Diagnostic> {
    NumericVector::from_finite(values)
        .and_then(|values| NumericTile::from_vector(values, height, width))
        .map(Value::Tile)
        .ok_or_else(|| length_error(span))
}

// Two units per multiply/accumulate term, plus one per output. Even a
// zero-inner product must pay for producing its explicit zero elements.
fn product_work(count: usize, inner: usize, span: Span) -> Result<u64, Diagnostic> {
    (inner as u64)
        .checked_mul(2)
        .and_then(|work| work.checked_add(1))
        .and_then(|work| work.checked_mul(count as u64))
        .ok_or_else(|| {
            Diagnostic::new(
                DiagnosticKind::ExecutionLimit,
                span,
                "matrix work estimate exceeds the execution budget range",
            )
        })
}

pub(super) fn evaluate(
    function: Builtin,
    args: &[Value],
    budget: &mut Budget,
    span: Span,
    argument_span: impl Fn(usize) -> Span,
) -> Result<Value, Diagnostic> {
    if function == Builtin::Tile {
        let height = dimension(&args[1], "height", argument_span(1))?;
        let width = dimension(&args[2], "width", argument_span(2))?;
        let values = args[0].require_vector(argument_span(0))?;
        if height.checked_mul(width) != Some(values.len()) {
            return Err(shape_error(
                span,
                format!(
                    "tile shape {height}x{width} does not match vector length {}",
                    values.len()
                ),
            ));
        }
        budget.charge(1, span)?;
        return NumericTile::from_vector(values.clone(), height, width)
            .map(Value::Tile)
            .ok_or_else(|| length_error(span));
    }
    let left = args[0].require_tile(argument_span(0))?;
    let (height, width) = (left.height(), left.width());
    match function {
        Builtin::Height => Ok(Value::Integer(height as i64)),
        Builtin::Width => Ok(Value::Integer(width as i64)),
        Builtin::Cell => {
            let row = index(&args[1], height, "row", argument_span(1))?;
            let column = index(&args[2], width, "column", argument_span(2))?;
            budget.charge(1, span)?;
            Ok(Value::Float(
                left.cell(row, column).expect("checked coordinates"),
            ))
        }
        Builtin::Lane => {
            let row = index(&args[1], height, "row", argument_span(1))?;
            budget.charge(width as u64, span)?;
            let mut values = buffer(width, budget, span)?;
            values.extend_from_slice(left.row(row).expect("checked row"));
            vector(values, span)
        }
        Builtin::Unroll => {
            budget.charge(1, span)?;
            Ok(Value::Vector(left.as_vector().clone()))
        }
        Builtin::Turn => {
            budget.charge(left.len() as u64, span)?;
            let mut values = buffer(left.len(), budget, span)?;
            // Empty shapes must not scan an uncharged, nonempty outer axis.
            if !left.is_empty() {
                for column in 0..width {
                    for row in 0..height {
                        values.push(left.cell(row, column).expect("validated tile coordinates"));
                    }
                }
            }
            tile(values, width, height, span)
        }
        Builtin::TileAdd | Builtin::TileScale => {
            let right = if function == Builtin::TileAdd {
                let right = args[1].require_tile(argument_span(1))?;
                if (height, width) != (right.height(), right.width()) {
                    return Err(shape_error(
                        span,
                        format!(
                            "tileadd requires equal shapes, found {height}x{width} and {}x{}",
                            right.height(),
                            right.width()
                        ),
                    ));
                }
                Some(right)
            } else {
                None
            };
            let factor = if function == Builtin::TileScale {
                args[1].require_float(argument_span(1))?
            } else {
                0.0
            };
            budget.charge(left.len() as u64, span)?;
            let mut values = buffer(left.len(), budget, span)?;
            for (i, value) in left.cells().iter().enumerate() {
                let result = match right {
                    Some(right) => value.get() + right.cells()[i].get(),
                    None => value.get() * factor,
                };
                values.push(finite_number(result, span)?);
            }
            tile(values, height, width, span)
        }
        Builtin::MatVec => {
            let right = args[1].require_vector(argument_span(1))?;
            if width != right.len() {
                return Err(shape_error(
                    span,
                    format!(
                        "matvec requires vector length {width} for tile {height}x{width}, found {}",
                        right.len()
                    ),
                ));
            }
            budget.charge(product_work(height, width, span)?, span)?;
            let mut values = buffer(height, budget, span)?;
            for row in 0..height {
                let products = left
                    .row(row)
                    .expect("validated row")
                    .iter()
                    .zip(right.as_slice())
                    .map(|(a, b)| finite_number(a.get() * b.get(), span).map(FiniteFloat::get));
                values.push(finite_number(compensated(products, span)?, span)?);
            }
            vector(values, span)
        }
        Builtin::MatMul => {
            let right = args[1].require_tile(argument_span(1))?;
            if width != right.height() {
                return Err(shape_error(span, format!("matmul requires matching inner dimensions, found {height}x{width} and {}x{}", right.height(), right.width())));
            }
            let columns = right.width();
            let count = height
                .checked_mul(columns)
                .filter(|&n| n <= MAX_VECTOR_LENGTH)
                .ok_or_else(|| {
                    shape_error(
                        span,
                        format!("matmul result shape {height}x{columns} exceeds 65536 cells"),
                    )
                })?;
            budget.charge(product_work(count, width, span)?, span)?;
            let mut values = buffer(count, budget, span)?;
            if count != 0 {
                for row in 0..height {
                    let row = left.row(row).expect("validated row");
                    for column in 0..columns {
                        let products = row.iter().enumerate().map(|(k, a)| {
                            finite_number(a.get() * right.cells()[k * columns + column].get(), span)
                                .map(FiniteFloat::get)
                        });
                        values.push(finite_number(compensated(products, span)?, span)?);
                    }
                }
            }
            tile(values, height, columns, span)
        }
        _ => unreachable!("tile operation dispatch"),
    }
}
