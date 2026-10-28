//! Primitive runtime values and their inferred binding types.

use std::{fmt, sync::Arc};

use crate::diagnostics::{Diagnostic, DiagnosticKind, Span};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueType {
    /// Function result only.
    Unit,
    Integer,
    Boolean,
    String,
}

impl fmt::Display for ValueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Unit => "unit",
            Self::Integer => "integer",
            Self::Boolean => "boolean",
            Self::String => "string",
        })
    }
}

/// Strings share immutable storage. Reassignment replaces a value and cannot
/// mutate its copies or values already captured for output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// Internal marker; rejected in ordinary value contexts.
    Unit,
    Integer(i64),
    Boolean(bool),
    String(Arc<str>),
}

impl Value {
    pub fn kind(&self) -> ValueType {
        match self {
            Self::Unit => ValueType::Unit,
            Self::Integer(_) => ValueType::Integer,
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
    .with_help("keep primitive types consistent; arithmetic and ordering require integers, logic requires booleans")
}
