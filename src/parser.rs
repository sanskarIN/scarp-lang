//! Parse print statements without executing them.

use crate::{
    ast::{Program, Statement},
    diagnostics::{Diagnostic, DiagnosticKind, Span},
    lexer::{Token, TokenKind},
};

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

    fn eof_span(&self) -> Span {
        let end = self.tokens.last().map_or(0, |token| token.span.end);
        Span::new(end, end)
    }

    fn expected(&self, expected: &str) -> Diagnostic {
        let (found, span) = self.current().map_or_else(
            || ("<eof>".to_string(), self.eof_span()),
            |token| (token.kind.to_string(), token.span),
        );
        Diagnostic::new(
            DiagnosticKind::UnexpectedToken,
            span,
            format!("expected {expected}, found {found}"),
        )
        .with_help("a print statement has the form println(\"text\");")
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
        let mut statements = Vec::new();
        while let Some(token) = self.current() {
            match &token.kind {
                TokenKind::Eof => {
                    // Reject malformed externally supplied streams with tokens after EOF.
                    if self.cursor + 1 != self.tokens.len() {
                        return Err(self.expected("the end of the token stream"));
                    }
                    break;
                }
                TokenKind::Identifier(name) if name == "println" => {
                    let start = token.span.start;
                    self.cursor += 1;
                    self.punctuation(TokenKind::LeftParen, "'('")?;
                    let value = match self.current().map(|token| &token.kind) {
                        Some(TokenKind::String(value)) => value.clone(),
                        _ => return Err(self.expected("a string literal")),
                    };
                    self.cursor += 1;
                    self.punctuation(TokenKind::RightParen, "')'")?;
                    let end = self.punctuation(TokenKind::Semicolon, "';'")?.end;
                    statements.push(Statement::Println {
                        value,
                        span: Span::new(start, end),
                    });
                }
                TokenKind::Identifier(name) => {
                    return Err(Diagnostic::new(
                        DiagnosticKind::UnknownStatement,
                        token.span,
                        format!("unknown statement {name:?}"),
                    )
                    .with_help("only println(\"text\"); is supported in this prototype"));
                }
                _ => return Err(self.expected("a println statement")),
            }
        }
        Ok(Program { statements })
    }
}
