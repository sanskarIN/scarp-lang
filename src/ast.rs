//! The syntax tree for the currently supported language slice.

use crate::diagnostics::Span;
use crate::value::FiniteFloat;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Program {
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    Import {
        path: String,
        path_span: Span,
        alias: String,
        alias_span: Span,
        span: Span,
    },
    Export {
        declaration: Box<Statement>,
        span: Span,
    },
    Function(FunctionDeclaration),
    Return {
        value: Option<Expression>,
        span: Span,
    },
    Call {
        expression: Expression,
        span: Span,
    },
    Repeat {
        count: Expression,
        index: String,
        index_span: Span,
        statements: Vec<Statement>,
        span: Span,
    },
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
    /// Zero/multiple arguments, a raw write, or explicit stderr output.
    /// Single-argument stdout line forms retain `Println` for compatibility.
    Output {
        arguments: Vec<PrintArgument>,
        target: OutputTarget,
        newline: bool,
        span: Span,
    },
    /// An immutable bind or mutable var declaration.
    Declaration {
        name: String,
        name_span: Span,
        mutable: bool,
        annotation: Option<TypeAnnotation>,
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

/// Optional explicit binding type, retaining its source token location.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeAnnotation {
    pub kind: crate::value::ValueType,
    pub span: Span,
}

impl Statement {
    pub fn span(&self) -> Span {
        match self {
            Self::Import { span, .. } | Self::Export { span, .. } => *span,
            Self::Function(function) => function.span,
            Self::Return { span, .. } | Self::Call { span, .. } => *span,
            Self::Repeat { span, .. }
            | Self::While { span, .. }
            | Self::Break { span }
            | Self::Continue { span }
            | Self::If { span, .. }
            | Self::Block { span, .. }
            | Self::Println { span, .. }
            | Self::Output { span, .. }
            | Self::Declaration { span, .. }
            | Self::Assign { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionDeclaration {
    pub name: String,
    pub name_span: Span,
    pub parameters: Vec<Parameter>,
    pub result: crate::value::ValueType,
    pub pure: bool,
    pub contracts: Vec<Contract>,
    pub statements: Vec<Statement>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    pub name: String,
    pub span: Span,
    pub kind: crate::value::ValueType,
    pub default: Option<Expression>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contract {
    pub postcondition: bool,
    pub expression: Expression,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallArgument {
    pub label: Option<(String, Span)>,
    pub expression: usize,
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

/// The explicit destination of program output, distinct from diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputTarget {
    Stdout,
    Stderr,
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
    Vector {
        elements: Vec<usize>,
    },
    Index {
        vector: usize,
        index: usize,
        operator_span: Span,
    },
    UserCall {
        name: String,
        arguments: Vec<CallArgument>,
        function_span: Span,
    },
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
