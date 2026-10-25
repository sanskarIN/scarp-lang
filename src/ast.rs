//! The syntax tree for the currently supported language slice.

use crate::diagnostics::Span;
use crate::value::FiniteFloat;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Program {
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    While {
        condition: Expression,
        statements: Vec<Statement>,
        span: Span,
    },
    Break {
        span: Span,
    },
    Continue {
        span: Span,
    },
    If {
        branches: Vec<ConditionalBranch>,
        otherwise: Vec<Statement>,
        span: Span,
    },
    Block {
        statements: Vec<Statement>,
        span: Span,
    },
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

impl Statement {
    pub fn span(&self) -> Span {
        match self {
            Self::While { span, .. }
            | Self::Break { span }
            | Self::Continue { span }
            | Self::If { span, .. }
            | Self::Block { span, .. }
            | Self::Println { span, .. }
            | Self::Let { span, .. }
            | Self::Assign { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConditionalBranch {
    pub condition: Expression,
    pub statements: Vec<Statement>,
    /// The branch's brace-delimited body, including both braces.
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignmentOperator {
    Set,
    Compound(BinaryOperator),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrintArgument {
    String(String),
    Expression(Expression),
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
    Float(FiniteFloat),
    Boolean(bool),
    String(String),
    Name(String),
    Negate {
        operand: usize,
        operator_span: Span,
    },
    Not {
        operand: usize,
        operator_span: Span,
    },
    Logical {
        left: usize,
        operator: LogicalOperator,
        right: usize,
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
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicalOperator {
    And,
    Or,
}
