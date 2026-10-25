//! The syntax tree for the currently supported language slice.

use crate::diagnostics::Span;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Program {
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    Println {
        argument: PrintArgument,
        span: Span,
    },
    Let {
        name: String,
        name_span: Span,
        mutable: bool,
        initializer: Expression,
        span: Span,
    },
    Assign {
        name: String,
        name_span: Span,
        operator: AssignmentOperator,
        operator_span: Span,
        value: Expression,
        span: Span,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignmentOperator {
    Set,
    Compound(BinaryOperator),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrintArgument {
    String(String),
    Integer(Expression),
}

/// A flat, postorder syntax tree. Children always precede their parent.
/// Keeping nodes in a vector avoids recursive evaluation and destruction of
/// long left-associative expressions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expression {
    pub(crate) nodes: Vec<ExpressionNode>,
    pub(crate) root: usize,
}

impl Expression {
    pub fn nodes(&self) -> &[ExpressionNode] {
        &self.nodes
    }

    pub fn root(&self) -> usize {
        self.root
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpressionNode {
    pub kind: ExpressionKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpressionKind {
    Integer(i64),
    Name(String),
    Negate {
        operand: usize,
        operator_span: Span,
    },
    Binary {
        left: usize,
        operator: BinaryOperator,
        right: usize,
        operator_span: Span,
    },
    Group {
        expression: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
}
