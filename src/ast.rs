//! The syntax tree for the currently supported language slice.

use crate::diagnostics::Span;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Program {
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    Println { value: String, span: Span },
}
