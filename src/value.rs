//! Runtime values, finite numerical storage and inferred binding types.

use std::{fmt, sync::Arc};

use crate::diagnostics::{Diagnostic, DiagnosticKind, Span};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueType {
    /// Function result only.
    Unit,
    Integer,
    Float,
    Vector,
    Tile,
    Boolean,
    String,
}

impl fmt::Display for ValueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Unit => "unit",
            Self::Integer => "integer",
            Self::Float => "float",
            Self::Vector => "vector",
            Self::Tile => "tile",
            Self::Boolean => "boolean",
            Self::String => "string",
        })
    }
}

impl ValueType {
    pub(crate) fn is_numeric(self) -> bool {
        matches!(self, Self::Integer | Self::Float)
    }
}

/// A finite IEEE-754 binary64 scalar. Private storage excludes NaN and infinity.
/// Equality is numeric: signed zeros compare equal; `get().to_bits()` and
/// display retain their sign. This type deliberately does not implement Hash.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FiniteFloat(f64);

// Finite numeric equality is reflexive, symmetric and transitive. NaN, the
// floating-point counterexample to Eq, cannot be constructed through this API.
impl Eq for FiniteFloat {}

impl FiniteFloat {
    /// Reject NaN and either infinity, including values supplied by a Rust host.
    pub fn new(value: f64) -> Option<Self> {
        if value.is_finite() {
            Some(Self(value))
        } else {
            None
        }
    }

    /// Inspect the finite payload without permitting mutation of its invariant.
    pub fn get(self) -> f64 {
        self.0
    }
}

impl fmt::Display for FiniteFloat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Shortest round-trip digits, a float marker, and preserved signed zero.
        write!(f, "{:?}", self.0)
    }
}

/// Maximum elements in either a source-created or host-created numeric vector.
pub const MAX_VECTOR_LENGTH: usize = 65_536;

/// Shared contiguous immutable finite elements. Moving an owned buffer into
/// this wrapper does not copy its payload; cloning only shares the Arc.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumericVector(Arc<Vec<FiniteFloat>>);

impl NumericVector {
    /// Reject oversized/nonfinite input or failed buffer reservation.
    pub fn new(values: &[f64]) -> Option<Self> {
        if values.len() > MAX_VECTOR_LENGTH {
            return None;
        }
        let mut finite = Vec::new();
        finite.try_reserve_exact(values.len()).ok()?;
        for &value in values {
            finite.push(FiniteFloat::new(value)?);
        }
        Self::from_finite(finite)
    }

    /// Consume an already checked finite buffer without copying its elements.
    pub fn from_finite(values: Vec<FiniteFloat>) -> Option<Self> {
        (values.len() <= MAX_VECTOR_LENGTH).then(|| Self(Arc::new(values)))
    }

    pub fn as_slice(&self) -> &[FiniteFloat] {
        self.0.as_slice()
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for NumericVector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[")?;
        for (index, value) in self.as_slice().iter().enumerate() {
            if index > 0 {
                f.write_str(", ")?;
            }
            value.fmt(f)?;
        }
        f.write_str("]")
    }
}

/// Immutable row-major rectangular numerical value with an explicit shape.
/// The cells reuse a finite vector's shared backing storage without copying.
/// Zero-sized shapes are allowed and remain distinguishable by their axes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumericTile {
    cells: NumericVector,
    height: usize,
    width: usize,
}

impl NumericTile {
    /// Preserve the original immutable vector without copying its elements.
    /// Both dimensions and their product are bounded by MAX_VECTOR_LENGTH.
    pub fn from_vector(cells: NumericVector, height: usize, width: usize) -> Option<Self> {
        if height > MAX_VECTOR_LENGTH || width > MAX_VECTOR_LENGTH {
            return None;
        }
        if height.checked_mul(width)? != cells.len() {
            return None;
        }
        Some(Self {
            cells,
            height,
            width,
        })
    }

    pub fn new(values: &[f64], height: usize, width: usize) -> Option<Self> {
        Self::from_vector(NumericVector::new(values)?, height, width)
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    pub fn cells(&self) -> &[FiniteFloat] {
        self.cells.as_slice()
    }

    pub fn as_vector(&self) -> &NumericVector {
        &self.cells
    }

    pub fn cell(&self, row: usize, column: usize) -> Option<FiniteFloat> {
        if row >= self.height || column >= self.width {
            return None;
        }
        self.cells
            .as_slice()
            .get(row.checked_mul(self.width)?.checked_add(column)?)
            .copied()
    }

    pub fn row(&self, row: usize) -> Option<&[FiniteFloat]> {
        if row >= self.height {
            return None;
        }
        let start = row.checked_mul(self.width)?;
        self.cells
            .as_slice()
            .get(start..start.checked_add(self.width)?)
    }
}

impl fmt::Display for NumericTile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "tile({}x{})[", self.height, self.width)?;
        for row in 0..self.height {
            if row != 0 {
                f.write_str(", ")?;
            }
            f.write_str("[")?;
            for (index, value) in self
                .row(row)
                .expect("validated tile row")
                .iter()
                .enumerate()
            {
                if index != 0 {
                    f.write_str(", ")?;
                }
                value.fmt(f)?;
            }
            f.write_str("]")?;
        }
        f.write_str("]")
    }
}

pub(crate) fn finite_number(value: f64, span: Span) -> Result<FiniteFloat, Diagnostic> {
    FiniteFloat::new(value).ok_or_else(|| {
        Diagnostic::new(
            DiagnosticKind::NonFiniteNumber,
            span,
            "numeric operation produced a non-finite result",
        )
        .with_help("keep every intermediate result finite")
    })
}

/// Strings share immutable storage. Reassignment replaces a value and cannot
/// mutate its copies or values already captured for output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// Internal marker; rejected in ordinary value contexts.
    Unit,
    Integer(i64),
    Float(FiniteFloat),
    Vector(NumericVector),
    Tile(NumericTile),
    Boolean(bool),
    String(Arc<str>),
}

impl Value {
    pub fn kind(&self) -> ValueType {
        match self {
            Self::Unit => ValueType::Unit,
            Self::Integer(_) => ValueType::Integer,
            Self::Float(_) => ValueType::Float,
            Self::Vector(_) => ValueType::Vector,
            Self::Tile(_) => ValueType::Tile,
            Self::Boolean(_) => ValueType::Boolean,
            Self::String(_) => ValueType::String,
        }
    }

    pub fn as_integer(&self) -> Option<i64> {
        match self {
            Self::Integer(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_float(&self) -> Option<f64> {
        match self {
            Self::Float(value) => Some(value.get()),
            _ => None,
        }
    }

    pub fn as_vector(&self) -> Option<&NumericVector> {
        match self {
            Self::Vector(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_tile(&self) -> Option<&NumericTile> {
        match self {
            Self::Tile(value) => Some(value),
            _ => None,
        }
    }

    pub(crate) fn require_tile(&self, span: Span) -> Result<&NumericTile, Diagnostic> {
        self.as_tile()
            .ok_or_else(|| type_mismatch(span, ValueType::Tile, self.kind()))
    }

    pub(crate) fn require_vector(&self, span: Span) -> Result<&NumericVector, Diagnostic> {
        self.as_vector()
            .ok_or_else(|| type_mismatch(span, ValueType::Vector, self.kind()))
    }

    pub(crate) fn require_float(&self, span: Span) -> Result<f64, Diagnostic> {
        self.as_float()
            .ok_or_else(|| type_mismatch(span, ValueType::Float, self.kind()))
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_boolean(&self) -> Option<bool> {
        match self {
            Self::Boolean(value) => Some(*value),
            _ => None,
        }
    }

    pub(crate) fn require_boolean(&self, span: Span) -> Result<bool, Diagnostic> {
        self.as_boolean()
            .ok_or_else(|| type_mismatch(span, ValueType::Boolean, self.kind()))
    }

    pub(crate) fn require_integer(&self, span: Span) -> Result<i64, Diagnostic> {
        self.as_integer()
            .ok_or_else(|| type_mismatch(span, ValueType::Integer, self.kind()))
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Self::String(Arc::from(value))
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Self::String(Arc::from(value))
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unit => f.write_str("unit"),
            Self::Integer(value) => value.fmt(f),
            Self::Float(value) => value.fmt(f),
            Self::Vector(value) => value.fmt(f),
            Self::Tile(value) => value.fmt(f),
            Self::Boolean(value) => value.fmt(f),
            Self::String(value) => f.write_str(value),
        }
    }
}

pub(crate) fn type_mismatch(span: Span, expected: ValueType, found: ValueType) -> Diagnostic {
    Diagnostic::new(
        DiagnosticKind::TypeMismatch,
        span,
        format!("expected {expected}, found {found}"),
    )
    .with_help("keep primitive types consistent; numeric operands must share a numeric kind, logic requires booleans")
}
