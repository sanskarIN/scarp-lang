//! Primitive runtime values and their inferred binding types.

use std::{fmt, sync::Arc};

use crate::diagnostics::{Diagnostic, DiagnosticKind, Span};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueType {
    /// Function result only; not a source-level primitive value.
    Unit,
    Integer,
    Float,
    Vector,
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

/// A finite binary64 value. Private storage keeps NaN out of equality contracts.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FiniteFloat(f64);

impl Eq for FiniteFloat {}

impl FiniteFloat {
    pub fn new(value: f64) -> Option<Self> {
        value.is_finite().then_some(Self(value))
    }
    pub fn get(self) -> f64 {
        self.0
    }
}

impl fmt::Display for FiniteFloat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

pub const MAX_VECTOR_LENGTH: usize = 65_536;

/// Contiguous immutable finite values, shared by copies and captured output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumericVector(Arc<[FiniteFloat]>);

impl NumericVector {
    pub fn new(values: &[f64]) -> Option<Self> {
        if values.len() > MAX_VECTOR_LENGTH {
            return None;
        }
        let values: Option<Vec<_>> = values.iter().copied().map(FiniteFloat::new).collect();
        values.map(Self::from_finite)
    }
    pub fn as_slice(&self) -> &[FiniteFloat] {
        &self.0
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub(crate) fn from_finite(values: Vec<FiniteFloat>) -> Self {
        debug_assert!(values.len() <= MAX_VECTOR_LENGTH);
        Self(Arc::from(values))
    }
}

impl fmt::Display for NumericVector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[")?;
        for (index, value) in self.0.iter().enumerate() {
            if index != 0 {
                f.write_str(", ")?;
            }
            value.fmt(f)?;
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
    /// Internal no-result marker. The resolver prevents ordinary value use.
    Unit,
    Integer(i64),
    Float(FiniteFloat),
    Vector(NumericVector),
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
