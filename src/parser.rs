//! Parse declarations, assignments, blocks, and print statements without execution.

use crate::{
    ast::{
        AssignmentOperator, BinaryOperator, ConditionalBranch, Expression, ExpressionKind,
        ExpressionNode, LogicalOperator, PrintArgument, Program, Statement,
    },
    diagnostics::{Diagnostic, DiagnosticKind, Span},
    lexer::{Token, TokenKind},
};

/// Maximum nested expression parentheses, excluding the println call itself.
pub const MAX_EXPRESSION_DEPTH: usize = 64;

/// Maximum nested statement blocks, independent of expression parentheses.
pub const MAX_BLOCK_DEPTH: usize = 64;

/// Parse a token stream. An omitted EOF token is treated as the end of input.
pub fn parse(tokens: &[Token]) -> Result<Program, Diagnostic> {
    Parser { tokens, cursor: 0 }.program()
}

struct Parser<'a> {
    tokens: &'a [Token],
    cursor: usize,
}

impl Parser<'_> {
    fn current(&self) -> Option<&Token> {
        self.tokens.get(self.cursor)
    }

    fn expected(&self, expected: &str) -> Diagnostic {
        let (found, span) = self.current().map_or_else(
            || {
                let end = self.tokens.last().map_or(0, |token| token.span.end);
                ("<eof>".to_string(), Span::new(end, end))
            },
            |token| (token.kind.to_string(), token.span),
        );
        Diagnostic::new(
            DiagnosticKind::UnexpectedToken,
            span,
            format!("expected {expected}, found {found}"),
        )
        .with_help("use println(\"text\"); or println(1 + 2);")
    }

    fn punctuation(&mut self, kind: TokenKind, label: &str) -> Result<Span, Diagnostic> {
        if let Some(token) = self.current() {
            if token.kind == kind {
                let span = token.span;
                self.cursor += 1;
                return Ok(span);
            }
        }
        Err(self.expected(label))
    }

    fn program(mut self) -> Result<Program, Diagnostic> {
        Ok(Program {
            statements: self.statement_list(0, false)?,
        })
    }

    fn statement_list(
        &mut self,
        depth: usize,
        in_block: bool,
    ) -> Result<Vec<Statement>, Diagnostic> {
        let mut statements = Vec::new();
        while let Some(token) = self.current() {
            match &token.kind {
                TokenKind::Eof => {
                    if in_block {
                        return Err(self
                            .expected("'}'")
                            .with_help("close the statement block with '}'"));
                    }
                    if self.cursor + 1 != self.tokens.len() {
                        return Err(self.expected("the end of the token stream"));
                    }
                    break;
                }
                TokenKind::RightBrace if in_block => break,
                TokenKind::LeftBrace => {
                    let (inner, span) = self.block(depth)?;
                    statements.push(Statement::Block {
                        statements: inner,
                        span,
                    });
                }
                TokenKind::Identifier(name) if name == "if" => {
                    statements.push(self.conditional(depth)?);
                }
                TokenKind::Identifier(name)
                    if name == "repeat"
                        && !self
                            .tokens
                            .get(self.cursor + 1)
                            .is_some_and(|token| assignment_operator(&token.kind).is_some()) =>
                {
                    let start = token.span.start;
                    self.cursor += 1;
                    let mut nodes = Vec::new();
                    let root = self.logical_or(&mut nodes, 0)?;
                    if !self.keyword("as") {
                        return Err(self
                            .expected("as followed by an index name")
                            .with_help("use repeat count as index { statements }"));
                    }
                    self.cursor += 1;
                    let (index, index_span) = match self.current() {
                        Some(Token {
                            kind: TokenKind::Identifier(name),
                            span,
                        }) => (name.clone(), *span),
                        _ => {
                            return Err(self
                                .expected("an immutable repeat index name")
                                .with_help("use repeat count as index { statements }"))
                        }
                    };
                    self.cursor += 1;
                    if !matches!(self.current().map(|t| &t.kind), Some(TokenKind::LeftBrace)) {
                        return Err(self.expected("'{'").with_help("enclose the repeat body in braces: repeat count as index { statements }"));
                    }
                    let (body, body_span) = self.block(depth)?;
                    statements.push(Statement::Repeat {
                        count: Expression { nodes, root },
                        index,
                        index_span,
                        statements: body,
                        span: Span::new(start, body_span.end),
                    });
                }
                TokenKind::Identifier(name) if name == "while" => {
                    let start = token.span.start;
                    self.cursor += 1;
                    let mut nodes = Vec::new();
                    let root = self.logical_or(&mut nodes, 0).map_err(|error| {
                        if error.kind == DiagnosticKind::UnexpectedToken {
                            error.with_help("use while boolean_condition { statements }")
                        } else {
                            error
                        }
                    })?;
                    if !matches!(self.current().map(|t| &t.kind), Some(TokenKind::LeftBrace)) {
                        return Err(self.expected("'{'").with_help(
                            "enclose the while body in braces: while condition { statements }",
                        ));
                    }
                    let (body, span) = self.block(depth)?;
                    statements.push(Statement::While {
                        condition: Expression { nodes, root },
                        statements: body,
                        span: Span::new(start, span.end),
                    });
                }
                TokenKind::Identifier(name) if name == "break" || name == "continue" => {
                    let start = token.span.start;
                    let is_break = name == "break";
                    self.cursor += 1;
                    let end = self.punctuation(TokenKind::Semicolon, "';'")?.end;
                    let span = Span::new(start, end);
                    statements.push(if is_break {
                        Statement::Break { span }
                    } else {
                        Statement::Continue { span }
                    });
                }
                TokenKind::Identifier(name) if name == "println" => {
                    let start = token.span.start;
                    self.cursor += 1;
                    self.punctuation(TokenKind::LeftParen, "'('")?;
                    let direct_string = self
                        .tokens
                        .get(self.cursor + 1)
                        .is_some_and(|token| token.kind == TokenKind::RightParen);
                    let argument = if let Some(Token {
                        kind: TokenKind::String(value),
                        ..
                    }) = self.current().filter(|_| direct_string)
                    {
                        let argument = PrintArgument::String(value.clone());
                        self.cursor += 1;
                        argument
                    } else {
                        if !matches!(
                            self.current().map(|token| &token.kind),
                            Some(
                                TokenKind::Integer(_)
                                    | TokenKind::String(_)
                                    | TokenKind::Identifier(_)
                                    | TokenKind::Minus
                                    | TokenKind::LeftParen
                            )
                        ) {
                            return Err(self.expected("a string literal or value expression"));
                        }
                        let mut nodes = Vec::new();
                        let root = self.logical_or(&mut nodes, 0)?;
                        PrintArgument::Expression(Expression { nodes, root })
                    };
                    self.punctuation(TokenKind::RightParen, "')'")?;
                    let end = self.punctuation(TokenKind::Semicolon, "';'")?.end;
                    statements.push(Statement::Println {
                        argument,
                        span: Span::new(start, end),
                    });
                }
                TokenKind::Identifier(name) if name == "let" => {
                    let start = token.span.start;
                    self.cursor += 1;
                    let mutable = matches!(self.current().map(|token| &token.kind), Some(TokenKind::Identifier(name)) if name == "mut");
                    if mutable {
                        self.cursor += 1;
                    }
                    let (name, name_span) = match self.current() {
                        Some(Token {
                            kind: TokenKind::Identifier(name),
                            span,
                        }) => (name.clone(), *span),
                        _ => return Err(self.expected("a binding name")),
                    };
                    self.cursor += 1;
                    self.punctuation(TokenKind::Equal, "'='")?;
                    let mut nodes = Vec::new();
                    let root = self.logical_or(&mut nodes, 0)?;
                    let end = self.punctuation(TokenKind::Semicolon, "';'")?.end;
                    statements.push(Statement::Let {
                        name,
                        name_span,
                        mutable,
                        initializer: Expression { nodes, root },
                        span: Span::new(start, end),
                    });
                }
                TokenKind::Identifier(name) => {
                    if let Some(operator) = self
                        .tokens
                        .get(self.cursor + 1)
                        .and_then(|token| assignment_operator(&token.kind))
                    {
                        let name = name.clone();
                        let name_span = token.span;
                        let operator_span = self.tokens[self.cursor + 1].span;
                        self.cursor += 2;
                        let mut nodes = Vec::new();
                        let root = self.logical_or(&mut nodes, 0)?;
                        let end = self.punctuation(TokenKind::Semicolon, "';'")?.end;
                        statements.push(Statement::Assign {
                            name,
                            name_span,
                            operator,
                            operator_span,
                            value: Expression { nodes, root },
                            span: Span::new(name_span.start, end),
                        });
                        continue;
                    }
                    return Err(Diagnostic::new(
                        DiagnosticKind::UnknownStatement,
                        token.span,
                        format!("unknown statement {name:?}"),
                    )
                    .with_help(
                        "use let, println, or an assignment to a declared mutable variable",
                    ));
                }
                _ => return Err(self.expected("a println statement or binding declaration")),
            }
        }
        if in_block && self.current().is_none() {
            return Err(self
                .expected("'}'")
                .with_help("close the statement block with '}'"));
        }
        Ok(statements)
    }

    fn keyword(&self, expected: &str) -> bool {
        matches!(self.current().map(|token| &token.kind), Some(TokenKind::Identifier(name)) if name == expected)
    }

    fn block(&mut self, depth: usize) -> Result<(Vec<Statement>, Span), Diagnostic> {
        let opening = self.punctuation(TokenKind::LeftBrace, "'{'")?;
        if depth >= MAX_BLOCK_DEPTH {
            return Err(Diagnostic::new(
                DiagnosticKind::BlockNesting,
                opening,
                format!("block nesting exceeds {MAX_BLOCK_DEPTH} levels"),
            )
            .with_help("reduce the number of nested statement blocks"));
        }
        let inner = self.statement_list(depth + 1, true)?;
        let end = self.punctuation(TokenKind::RightBrace, "'}'")?.end;
        Ok((inner, Span::new(opening.start, end)))
    }

    fn conditional(&mut self, depth: usize) -> Result<Statement, Diagnostic> {
        let start = self.current().unwrap().span.start;
        let mut branches = Vec::new();
        let mut end;
        loop {
            self.cursor += 1; // if or elif
            let mut nodes = Vec::new();
            let root = self.logical_or(&mut nodes, 0)?;
            let (statements, span) = self.block(depth)?;
            end = span.end;
            branches.push(ConditionalBranch {
                condition: Expression { nodes, root },
                statements,
                span,
            });
            if !self.keyword("elif") {
                break;
            }
        }
        let otherwise = if self.keyword("else") {
            self.cursor += 1;
            let (statements, span) = self.block(depth)?;
            end = span.end;
            statements
        } else {
            Vec::new()
        };
        Ok(Statement::If {
            branches,
            otherwise,
            span: Span::new(start, end),
        })
    }

    fn logical_or(
        &mut self,
        nodes: &mut Vec<ExpressionNode>,
        depth: usize,
    ) -> Result<usize, Diagnostic> {
        let mut left = self.logical_and(nodes, depth)?;
        while self.keyword("or") {
            let operator_span = self.current().unwrap().span;
            self.cursor += 1;
            let right = self.logical_and(nodes, depth)?;
            left = logical(nodes, left, LogicalOperator::Or, right, operator_span);
        }
        Ok(left)
    }

    fn logical_and(
        &mut self,
        nodes: &mut Vec<ExpressionNode>,
        depth: usize,
    ) -> Result<usize, Diagnostic> {
        let mut left = self.logical_not(nodes, depth)?;
        while self.keyword("and") {
            let operator_span = self.current().unwrap().span;
            self.cursor += 1;
            let right = self.logical_not(nodes, depth)?;
            left = logical(nodes, left, LogicalOperator::And, right, operator_span);
        }
        Ok(left)
    }

    fn logical_not(
        &mut self,
        nodes: &mut Vec<ExpressionNode>,
        depth: usize,
    ) -> Result<usize, Diagnostic> {
        let mut operators = Vec::new();
        while self.keyword("not") {
            operators.push(self.current().unwrap().span);
            self.cursor += 1;
        }
        let mut operand = self.comparison(nodes, depth)?;
        for operator_span in operators.into_iter().rev() {
            let span = Span::new(operator_span.start, nodes[operand].span.end);
            operand = push(
                nodes,
                ExpressionKind::Not {
                    operand,
                    operator_span,
                },
                span,
            );
        }
        Ok(operand)
    }

    fn comparison(
        &mut self,
        nodes: &mut Vec<ExpressionNode>,
        depth: usize,
    ) -> Result<usize, Diagnostic> {
        let left = self.sum(nodes, depth)?;
        if let Some(operator) = self
            .current()
            .and_then(|token| comparison_operator(&token.kind))
        {
            let operator_span = self.current().unwrap().span;
            self.cursor += 1;
            let right = self.sum(nodes, depth)?;
            if self
                .current()
                .and_then(|token| comparison_operator(&token.kind))
                .is_some()
            {
                return Err(self
                    .expected("'and' between separate comparisons")
                    .with_help("write a < b and b < c; chained comparisons are not supported"));
            }
            return Ok(binary(nodes, left, operator, right, operator_span));
        }
        Ok(left)
    }

    fn sum(&mut self, nodes: &mut Vec<ExpressionNode>, depth: usize) -> Result<usize, Diagnostic> {
        let mut left = self.product(nodes, depth)?;
        while let Some(token) = self.current() {
            let operator = match token.kind {
                TokenKind::Plus => BinaryOperator::Add,
                TokenKind::Minus => BinaryOperator::Subtract,
                _ => break,
            };
            let operator_span = token.span;
            self.cursor += 1;
            let right = self.product(nodes, depth)?;
            left = binary(nodes, left, operator, right, operator_span);
        }
        Ok(left)
    }

    fn product(
        &mut self,
        nodes: &mut Vec<ExpressionNode>,
        depth: usize,
    ) -> Result<usize, Diagnostic> {
        let mut left = self.unary(nodes, depth)?;
        while let Some(token) = self.current() {
            let operator = match token.kind {
                TokenKind::Star => BinaryOperator::Multiply,
                TokenKind::Slash => BinaryOperator::Divide,
                _ => break,
            };
            let operator_span = token.span;
            self.cursor += 1;
            let right = self.unary(nodes, depth)?;
            left = binary(nodes, left, operator, right, operator_span);
        }
        Ok(left)
    }

    fn unary(
        &mut self,
        nodes: &mut Vec<ExpressionNode>,
        depth: usize,
    ) -> Result<usize, Diagnostic> {
        if let Some(Token {
            kind: TokenKind::Minus,
            span,
        }) = self.current()
        {
            let operator_span = *span;
            self.cursor += 1;
            // The minimum integer has no representable positive i64 magnitude.
            // Only a direct literal after '-' gets this special treatment.
            if let Some(Token {
                kind: TokenKind::Integer(digits),
                span,
            }) = self.current()
            {
                if digits.parse::<u64>() == Ok(1_u64 << 63) {
                    let span = Span::new(operator_span.start, span.end);
                    self.cursor += 1;
                    return Ok(push(nodes, ExpressionKind::Integer(i64::MIN), span));
                }
            }
            let operand = self.primary(nodes, depth)?;
            let span = Span::new(operator_span.start, nodes[operand].span.end);
            Ok(push(
                nodes,
                ExpressionKind::Negate {
                    operand,
                    operator_span,
                },
                span,
            ))
        } else {
            self.primary(nodes, depth)
        }
    }

    fn primary(
        &mut self,
        nodes: &mut Vec<ExpressionNode>,
        depth: usize,
    ) -> Result<usize, Diagnostic> {
        match self.current() {
            Some(Token {
                kind: TokenKind::String(value),
                span,
            }) => {
                let kind = ExpressionKind::String(value.clone());
                let span = *span;
                self.cursor += 1;
                Ok(push(nodes, kind, span))
            }
            Some(Token {
                kind: TokenKind::Identifier(name),
                span,
            }) => {
                let kind = match name.as_str() {
                    "true" => ExpressionKind::Boolean(true),
                    "false" => ExpressionKind::Boolean(false),
                    _ => ExpressionKind::Name(name.clone()),
                };
                let span = *span;
                self.cursor += 1;
                Ok(push(nodes, kind, span))
            }
            Some(Token {
                kind: TokenKind::Integer(digits),
                span,
            }) => {
                let span = *span;
                let value = digits.parse::<i64>().map_err(|_| {
                    Diagnostic::new(
                        DiagnosticKind::InvalidInteger,
                        span,
                        "integer literal is outside the signed 64-bit range",
                    )
                    .with_help("use a value from -9223372036854775808 to 9223372036854775807")
                })?;
                self.cursor += 1;
                Ok(push(nodes, ExpressionKind::Integer(value), span))
            }
            Some(Token {
                kind: TokenKind::LeftParen,
                span,
            }) => {
                let start = span.start;
                if depth >= MAX_EXPRESSION_DEPTH {
                    return Err(Diagnostic::new(
                        DiagnosticKind::ExpressionNesting,
                        *span,
                        format!("expression nesting exceeds {MAX_EXPRESSION_DEPTH} levels"),
                    )
                    .with_help("reduce the number of nested parentheses"));
                }
                self.cursor += 1;
                let expression = self.logical_or(nodes, depth + 1)?;
                let end = self.punctuation(TokenKind::RightParen, "')'")?.end;
                Ok(push(
                    nodes,
                    ExpressionKind::Group { expression },
                    Span::new(start, end),
                ))
            }
            _ => Err(self.expected("a literal, variable name, or '('")),
        }
    }
}

fn assignment_operator(kind: &TokenKind) -> Option<AssignmentOperator> {
    Some(match kind {
        TokenKind::Equal => AssignmentOperator::Set,
        TokenKind::PlusEqual => AssignmentOperator::Compound(BinaryOperator::Add),
        TokenKind::MinusEqual => AssignmentOperator::Compound(BinaryOperator::Subtract),
        TokenKind::StarEqual => AssignmentOperator::Compound(BinaryOperator::Multiply),
        TokenKind::SlashEqual => AssignmentOperator::Compound(BinaryOperator::Divide),
        _ => return None,
    })
}

fn comparison_operator(kind: &TokenKind) -> Option<BinaryOperator> {
    Some(match kind {
        TokenKind::EqualEqual => BinaryOperator::Equal,
        TokenKind::NotEqual => BinaryOperator::NotEqual,
        TokenKind::Less => BinaryOperator::Less,
        TokenKind::LessEqual => BinaryOperator::LessEqual,
        TokenKind::Greater => BinaryOperator::Greater,
        TokenKind::GreaterEqual => BinaryOperator::GreaterEqual,
        _ => return None,
    })
}

fn logical(
    nodes: &mut Vec<ExpressionNode>,
    left: usize,
    operator: LogicalOperator,
    right: usize,
    operator_span: Span,
) -> usize {
    let span = Span::new(nodes[left].span.start, nodes[right].span.end);
    push(
        nodes,
        ExpressionKind::Logical {
            left,
            operator,
            right,
            operator_span,
        },
        span,
    )
}

fn push(nodes: &mut Vec<ExpressionNode>, kind: ExpressionKind, span: Span) -> usize {
    let id = nodes.len();
    nodes.push(ExpressionNode { kind, span });
    id
}

fn binary(
    nodes: &mut Vec<ExpressionNode>,
    left: usize,
    operator: BinaryOperator,
    right: usize,
    operator_span: Span,
) -> usize {
    let span = Span::new(nodes[left].span.start, nodes[right].span.end);
    push(
        nodes,
        ExpressionKind::Binary {
            left,
            operator,
            right,
            operator_span,
        },
        span,
    )
}
