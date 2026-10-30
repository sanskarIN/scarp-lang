//! Primitive runtime values and their inferred binding types.

use std::{fmt, sync::Arc};

use crate::diagnostics::{Diagnostic, DiagnosticKind, Span};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueType {
    /// Function result only.
    Unit,
    Integer,
    Float,
    Boolean,
    String,
}

impl fmt::Display for ValueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Unit => "unit",
            Self::Integer => "integer",
            Self::Float => "float",
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

/// Strings share immutable storage. Reassignment replaces a value and cannot
/// mutate its copies or values already captured for output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// Internal marker; rejected in ordinary value contexts.
    Unit,
    Integer(i64),
    Float(FiniteFloat),
    Boolean(bool),
    String(Arc<str>),
}

impl Value {
    pub fn kind(&self) -> ValueType {
        match self {
            Self::Unit => ValueType::Unit,
            Self::Integer(_) => ValueType::Integer,
            Self::Float(_) => ValueType::Float,
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
