//! Parse declarations, assignments, blocks, and print statements without execution.

use crate::{
    ast::{
        AssignmentOperator, BinaryOperator, CallArgument, ConditionalBranch, Contract, Expression,
        ExpressionKind, ExpressionNode, FunctionDeclaration, LogicalOperator, OutputTarget,
        Parameter, PrintArgument, Program, Statement,
    },
    diagnostics::{Diagnostic, DiagnosticKind, Span},
    lexer::{Token, TokenKind},
    numeric::Builtin,
    value::MAX_VECTOR_LENGTH,
    value::{FiniteFloat, ValueType},
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
        .with_help("use print(\"text\"); or print(1 + 2);")
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
                TokenKind::Identifier(name) if name == "funcra" || name == "pure" => {
                    if in_block {
                        return Err(Diagnostic::new(
                            DiagnosticKind::FunctionUnsupported,
                            token.span,
                            "nested function declarations are not supported",
                        ));
                    }
                    statements.push(Statement::Function(self.function()?));
                }
                TokenKind::Identifier(name) if name == "return" => {
                    let start = token.span.start;
                    self.cursor += 1;
                    let value = if self.at(&TokenKind::Semicolon) {
                        None
                    } else {
                        Some(self.expression()?)
                    };
                    let end = self.punctuation(TokenKind::Semicolon, "';'")?.end;
                    statements.push(Statement::Return {
                        value,
                        span: Span::new(start, end),
                    });
                }
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
                TokenKind::Identifier(name) if name == "while" => {
                    let start = token.span.start;
                    self.cursor += 1;
                    let mut nodes = Vec::new();
                    let root = self.logical_or(&mut nodes, 0)?;
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
                TokenKind::Identifier(name)
                    if matches!(
                        name.as_str(),
                        "print" | "println" | "write" | "eprint" | "ewrite"
                    ) =>
                {
                    statements.push(self.output_statement()?);
                }
                TokenKind::Identifier(name) if name == "let" => {
                    let modifier = self.tokens.get(self.cursor + 1).filter(
                        |next| matches!(&next.kind, TokenKind::Identifier(name) if name == "mut"),
                    );
                    let (span, old, replacement) = if let Some(modifier) = modifier {
                        (
                            Span::new(token.span.start, modifier.span.end),
                            "let mut",
                            "var",
                        )
                    } else {
                        (token.span, "let", "bind")
                    };
                    return Err(Diagnostic::new(
                        DiagnosticKind::UnknownStatement,
                        span,
                        format!("legacy declaration {old:?} is not supported"),
                    )
                    .with_help(format!(
                        "replace {old:?} with {replacement:?}: {replacement} name = expression;"
                    )));
                }
                TokenKind::Identifier(name) if name == "bind" || name == "var" => {
                    let start = token.span.start;
                    let mutable = name == "var";
                    self.cursor += 1;
                    if self.keyword("mut")
                        && matches!(
                            self.tokens.get(self.cursor + 1).map(|token| &token.kind),
                            Some(TokenKind::Identifier(_))
                        )
                    {
                        return Err(self.expected("a binding name followed by '='").with_help(
                            "use var name = expression; for a mutable variable; remove mut",
                        ));
                    }
                    let (name, name_span) = match self.current() {
                        Some(Token {
                            kind: TokenKind::Identifier(name),
                            span,
                        }) => (name.clone(), *span),
                        _ => return Err(self.expected("a binding name")),
                    };
                    self.cursor += 1;
                    self.punctuation(TokenKind::Equal, "'='").map_err(|error| {
                        error.with_help("declarations require an initializer: bind name = expression; or var name = expression;")
                    })?;
                    let mut nodes = Vec::new();
                    let root = self.logical_or(&mut nodes, 0)?;
                    let end = self.punctuation(TokenKind::Semicolon, "';'")?.end;
                    statements.push(Statement::Declaration {
                        name,
                        name_span,
                        mutable,
                        initializer: Expression { nodes, root },
                        span: Span::new(start, end),
                    });
                }
                TokenKind::Identifier(name) => {
                    if self
                        .tokens
                        .get(self.cursor + 1)
                        .is_some_and(|t| t.kind == TokenKind::LeftParen)
                    {
                        let start = token.span.start;
                        let expression = self.expression()?;
                        if !matches!(
                            expression.nodes[expression.root].kind,
                            ExpressionKind::Call { .. } | ExpressionKind::UserCall { .. }
                        ) {
                            return Err(self.expected("a standalone function call"));
                        }
                        let end = self.punctuation(TokenKind::Semicolon, "';'")?.end;
                        statements.push(Statement::Call {
                            expression,
                            span: Span::new(start, end),
                        });
                        continue;
                    }
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
                        "use bind, var, print, write, or an assignment to a declared mutable variable",
                    ));
                }
                _ => return Err(self.expected("an output statement or binding declaration")),
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

    fn at(&self, kind: &TokenKind) -> bool {
        self.current().is_some_and(|token| &token.kind == kind)
    }

    fn expression(&mut self) -> Result<Expression, Diagnostic> {
        let mut nodes = Vec::new();
        let root = self.logical_or(&mut nodes, 0)?;
        Ok(Expression { nodes, root })
    }

    fn identifier(&mut self) -> Result<(String, Span), Diagnostic> {
        match self.current() {
            Some(Token {
                kind: TokenKind::Identifier(name),
                span,
            }) => {
                let result = (name.clone(), *span);
                self.cursor += 1;
                Ok(result)
            }
            _ => Err(self.expected("an identifier")),
        }
    }

    fn value_type(&mut self, result: bool) -> Result<ValueType, Diagnostic> {
        let (name, span) = self.identifier()?;
        match name.as_str() {
            "integer" => Ok(ValueType::Integer),
            "float" => Ok(ValueType::Float),
            "boolean" => Ok(ValueType::Boolean),
            "string" => Ok(ValueType::String),
            "vector" => Ok(ValueType::Vector),
            "unit" if result => Ok(ValueType::Unit),
            "funcra" | "pure" => Err(Diagnostic::new(
                DiagnosticKind::FunctionUnsupported,
                span,
                "function types and first-class functions are not supported yet",
            )),
            _ => Err(Diagnostic::new(
                DiagnosticKind::FunctionDefinition,
                span,
                format!(
                    "invalid {} type {name:?}",
                    if result { "result" } else { "parameter" }
                ),
            )
            .with_help("use integer, float, boolean, string, vector, or unit for a result")),
        }
    }

    fn function(&mut self) -> Result<FunctionDeclaration, Diagnostic> {
        let start = self.current().unwrap().span.start;
        let pure = self.keyword("pure");
        if pure {
            self.cursor += 1;
        }
        if !self.keyword("funcra") {
            return Err(self.expected("funcra"));
        }
        self.cursor += 1;
        let (name, name_span) = self.identifier()?;
        self.punctuation(TokenKind::LeftParen, "'('")?;
        let mut parameters = Vec::new();
        while !self.at(&TokenKind::RightParen) {
            let (name, span) = self.identifier()?;
            self.punctuation(TokenKind::Colon, "':' before the parameter type")?;
            let kind = self.value_type(false)?;
            let default = if self.at(&TokenKind::Equal) {
                self.cursor += 1;
                Some(self.expression()?)
            } else {
                None
            };
            parameters.push(Parameter {
                name,
                span,
                kind,
                default,
            });
            if !self.at(&TokenKind::Comma) {
                break;
            }
            self.cursor += 1;
        }
        self.punctuation(TokenKind::RightParen, "')'")?;
        self.punctuation(TokenKind::Arrow, "'->' before the result type")?;
        let result = self.value_type(true)?;
        let mut contracts = Vec::new();
        while self.keyword("needs") || self.keyword("ensures") {
            let start = self.current().unwrap().span.start;
            let postcondition = self.keyword("ensures");
            self.cursor += 1;
            let expression = self.expression()?;
            let end = self
                .punctuation(TokenKind::Semicolon, "';' after the contract")?
                .end;
            contracts.push(Contract {
                postcondition,
                expression,
                span: Span::new(start, end),
            });
        }
        let (statements, body) = self.block(0)?;
        Ok(FunctionDeclaration {
            name,
            name_span,
            parameters,
            result,
            pure,
            contracts,
            statements,
            span: Span::new(start, body.end),
        })
    }

    fn output_statement(&mut self) -> Result<Statement, Diagnostic> {
        let token = self.current().expect("output keyword is present");
        let TokenKind::Identifier(name) = &token.kind else {
            unreachable!()
        };
        let start = token.span.start;
        let target = if name.starts_with('e') {
            OutputTarget::Stderr
        } else {
            OutputTarget::Stdout
        };
        let newline = name != "write" && name != "ewrite";
        self.cursor += 1;
        self.punctuation(TokenKind::LeftParen, "'('")?;
        let mut arguments = Vec::new();
        if !matches!(
            self.current().map(|token| &token.kind),
            Some(TokenKind::RightParen)
        ) {
            loop {
                let direct_string = self.tokens.get(self.cursor + 1).is_some_and(|next| {
                    matches!(next.kind, TokenKind::Comma | TokenKind::RightParen)
                });
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
                                | TokenKind::Float(_)
                                | TokenKind::LeftBracket
                                | TokenKind::String(_)
                                | TokenKind::Identifier(_)
                                | TokenKind::Minus
                                | TokenKind::BitNot
                                | TokenKind::LeftParen
                        )
                    ) {
                        return Err(self.expected("a string literal or value expression"));
                    }
                    let mut nodes = Vec::new();
                    let root = self.logical_or(&mut nodes, 0)?;
                    PrintArgument::Expression(Expression { nodes, root })
                };
                arguments.push(argument);
                if !matches!(
                    self.current().map(|token| &token.kind),
                    Some(TokenKind::Comma)
                ) {
                    break;
                }
                self.cursor += 1;
                if matches!(
                    self.current().map(|token| &token.kind),
                    Some(TokenKind::RightParen)
                ) {
                    break;
                }
            }
        }
        self.punctuation(TokenKind::RightParen, "')'")?;
        let end = self.punctuation(TokenKind::Semicolon, "';'")?.end;
        let span = Span::new(start, end);
        if target == OutputTarget::Stdout && newline && arguments.len() == 1 {
            Ok(Statement::Println {
                argument: arguments.pop().unwrap(),
                span,
            })
        } else {
            Ok(Statement::Output {
                arguments,
                target,
                newline,
                span,
            })
        }
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
        let left = self.arithmetic(nodes, depth)?;
        if let Some(operator) = self
            .current()
            .and_then(|token| comparison_operator(&token.kind))
        {
            let operator_span = self.current().unwrap().span;
            self.cursor += 1;
            let right = self.arithmetic(nodes, depth)?;
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

    fn arithmetic(
        &mut self,
        nodes: &mut Vec<ExpressionNode>,
        depth: usize,
    ) -> Result<usize, Diagnostic> {
        let first = self.unary(nodes, depth)?;
        if self
            .current()
            .and_then(|token| infix_operator(&token.kind))
            .is_none()
        {
            return Ok(first);
        }
        let mut operands = vec![first];
        let mut operators: Vec<(BinaryOperator, Span, u8)> = Vec::new();
        while let Some((operator, precedence)) =
            self.current().and_then(|token| infix_operator(&token.kind))
        {
            let span = self.current().unwrap().span;
            while operators
                .last()
                .is_some_and(|(_, _, previous)| *previous >= precedence)
            {
                let (operator, span, _) = operators.pop().unwrap();
                let right = operands.pop().unwrap();
                let left = operands.pop().unwrap();
                operands.push(binary(nodes, left, operator, right, span));
            }
            operators.push((operator, span, precedence));
            self.cursor += 1;
            operands.push(self.unary(nodes, depth)?);
        }
        while let Some((operator, span, _)) = operators.pop() {
            let right = operands.pop().unwrap();
            let left = operands.pop().unwrap();
            operands.push(binary(nodes, left, operator, right, span));
        }
        Ok(operands.pop().unwrap())
    }

    fn unary(
        &mut self,
        nodes: &mut Vec<ExpressionNode>,
        depth: usize,
    ) -> Result<usize, Diagnostic> {
        if matches!(
            self.current().map(|token| &token.kind),
            Some(TokenKind::BitNot)
        ) {
            let operator_span = self.current().unwrap().span;
            self.cursor += 1;
            let operand = self.postfix(nodes, depth)?;
            let span = Span::new(operator_span.start, nodes[operand].span.end);
            return Ok(push(
                nodes,
                ExpressionKind::BitNot {
                    operand,
                    operator_span,
                },
                span,
            ));
        }
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
                if integer_magnitude(digits) == Some(1_u64 << 63) {
                    let span = Span::new(operator_span.start, span.end);
                    self.cursor += 1;
                    return Ok(push(nodes, ExpressionKind::Integer(i64::MIN), span));
                }
            }
            let operand = self.postfix(nodes, depth)?;
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
            self.postfix(nodes, depth)
        }
    }

    fn postfix(
        &mut self,
        nodes: &mut Vec<ExpressionNode>,
        depth: usize,
    ) -> Result<usize, Diagnostic> {
        let mut vector = self.primary(nodes, depth)?;
        while matches!(
            self.current().map(|token| &token.kind),
            Some(TokenKind::LeftBracket)
        ) {
            let opening = self.current().unwrap().span;
            self.expression_depth(depth, opening)?;
            self.cursor += 1;
            let index = self.logical_or(nodes, depth + 1)?;
            let end = self.punctuation(TokenKind::RightBracket, "']'")?.end;
            let span = Span::new(nodes[vector].span.start, end);
            vector = push(
                nodes,
                ExpressionKind::Index {
                    vector,
                    index,
                    operator_span: Span::new(opening.start, end),
                },
                span,
            );
        }
        Ok(vector)
    }

    fn expression_depth(&self, depth: usize, span: Span) -> Result<(), Diagnostic> {
        if depth >= MAX_EXPRESSION_DEPTH {
            return Err(Diagnostic::new(
                DiagnosticKind::ExpressionNesting,
                span,
                "expression nesting exceeds 64 levels",
            ));
        }
        Ok(())
    }

    fn elements(
        &mut self,
        nodes: &mut Vec<ExpressionNode>,
        depth: usize,
        opening: Span,
        closing: TokenKind,
        vector: bool,
    ) -> Result<Vec<usize>, Diagnostic> {
        self.expression_depth(depth, opening)?;
        let mut elements = Vec::new();
        if self.current().is_some_and(|token| token.kind == closing) {
            return Ok(elements);
        }
        loop {
            let element = self.logical_or(nodes, depth + 1)?;
            elements.push(element);
            if vector && elements.len() > MAX_VECTOR_LENGTH {
                return Err(Diagnostic::new(
                    DiagnosticKind::VectorLength,
                    nodes[element].span,
                    "vector literal exceeds 65536 elements",
                ));
            }
            if !matches!(
                self.current().map(|token| &token.kind),
                Some(TokenKind::Comma)
            ) {
                break;
            }
            self.cursor += 1;
            if self.current().is_some_and(|token| token.kind == closing) {
                break;
            }
        }
        Ok(elements)
    }

    fn primary(
        &mut self,
        nodes: &mut Vec<ExpressionNode>,
        depth: usize,
    ) -> Result<usize, Diagnostic> {
        match self.current() {
            Some(Token {
                kind: TokenKind::LeftBracket,
                span,
            }) => {
                let opening = *span;
                self.cursor += 1;
                let elements =
                    self.elements(nodes, depth, opening, TokenKind::RightBracket, true)?;
                let end = self.punctuation(TokenKind::RightBracket, "']'")?.end;
                Ok(push(
                    nodes,
                    ExpressionKind::Vector { elements },
                    Span::new(opening.start, end),
                ))
            }
            Some(Token {
                kind: TokenKind::Float(digits),
                span,
            }) => {
                let value = digits
                    .parse::<f64>()
                    .ok()
                    .and_then(FiniteFloat::new)
                    .ok_or_else(|| {
                        Diagnostic::new(
                            DiagnosticKind::InvalidFloat,
                            *span,
                            "decimal literal is not a finite binary64 value",
                        )
                    })?;
                let span = *span;
                self.cursor += 1;
                Ok(push(nodes, ExpressionKind::Float(value), span))
            }
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
                if name == "funcra" || name == "pure" {
                    return Err(Diagnostic::new(
                        DiagnosticKind::FunctionUnsupported,
                        *span,
                        "anonymous funcra expressions are not supported yet",
                    ));
                }
                if self
                    .tokens
                    .get(self.cursor + 1)
                    .is_some_and(|token| token.kind == TokenKind::LeftParen)
                {
                    let function_span = *span;
                    let name = name.clone();
                    self.cursor += 1;
                    let opening = self.punctuation(TokenKind::LeftParen, "'('")?;
                    self.expression_depth(depth, opening)?;
                    let mut arguments = Vec::new();
                    while !self.at(&TokenKind::RightParen) {
                        let label = if self
                            .tokens
                            .get(self.cursor + 1)
                            .is_some_and(|t| t.kind == TokenKind::Colon)
                        {
                            let label = self.identifier()?;
                            self.cursor += 1;
                            Some(label)
                        } else {
                            None
                        };
                        let expression = self.logical_or(nodes, depth + 1)?;
                        arguments.push(CallArgument { label, expression });
                        if !self.at(&TokenKind::Comma) {
                            break;
                        }
                        self.cursor += 1;
                    }
                    let end = self.punctuation(TokenKind::RightParen, "')'")?.end;
                    let kind = if let Some(function) = Builtin::from_name(&name)
                        .filter(|_| arguments.iter().all(|a| a.label.is_none()))
                    {
                        ExpressionKind::Call {
                            function,
                            arguments: arguments.into_iter().map(|a| a.expression).collect(),
                            function_span,
                        }
                    } else {
                        ExpressionKind::UserCall {
                            name,
                            arguments,
                            function_span,
                        }
                    };
                    return Ok(push(nodes, kind, Span::new(function_span.start, end)));
                }
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
                let value = integer_magnitude(digits)
                    .and_then(|value| i64::try_from(value).ok())
                    .ok_or_else(|| {
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
        TokenKind::PercentEqual => AssignmentOperator::Compound(BinaryOperator::Remainder),
        TokenKind::AndEqual => AssignmentOperator::Compound(BinaryOperator::BitAnd),
        TokenKind::OrEqual => AssignmentOperator::Compound(BinaryOperator::BitOr),
        TokenKind::XorEqual => AssignmentOperator::Compound(BinaryOperator::BitXor),
        TokenKind::LeftEqual => AssignmentOperator::Compound(BinaryOperator::ShiftLeft),
        TokenKind::RightEqual => AssignmentOperator::Compound(BinaryOperator::ShiftRight),
        TokenKind::UnsignedEqual => AssignmentOperator::Compound(BinaryOperator::UnsignedRight),
        _ => return None,
    })
}

fn integer_magnitude(digits: &str) -> Option<u64> {
    if let Some(digits) = digits
        .strip_prefix("0x")
        .or_else(|| digits.strip_prefix("0X"))
    {
        u64::from_str_radix(digits, 16).ok()
    } else if let Some(digits) = digits
        .strip_prefix("0b")
        .or_else(|| digits.strip_prefix("0B"))
    {
        u64::from_str_radix(digits, 2).ok()
    } else {
        digits.parse().ok()
    }
}

fn infix_operator(kind: &TokenKind) -> Option<(BinaryOperator, u8)> {
    Some(match kind {
        TokenKind::BitOr => (BinaryOperator::BitOr, 1),
        TokenKind::BitXor => (BinaryOperator::BitXor, 2),
        TokenKind::BitAnd => (BinaryOperator::BitAnd, 3),
        TokenKind::ShiftLeft => (BinaryOperator::ShiftLeft, 4),
        TokenKind::ShiftRight => (BinaryOperator::ShiftRight, 4),
        TokenKind::UnsignedRight => (BinaryOperator::UnsignedRight, 4),
        TokenKind::Plus => (BinaryOperator::Add, 5),
        TokenKind::Minus => (BinaryOperator::Subtract, 5),
        TokenKind::Star => (BinaryOperator::Multiply, 6),
        TokenKind::Slash => (BinaryOperator::Divide, 6),
        TokenKind::Percent => (BinaryOperator::Remainder, 6),
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
