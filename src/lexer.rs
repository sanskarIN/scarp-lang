//! UTF-8 aware lexer. Token spans always refer to source bytes, not decoded text.

use std::fmt;

use crate::diagnostics::{Diagnostic, DiagnosticKind, Span};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Identifier(String),
    String(String),
    Integer(String),
    Plus,
    Minus,
    Star,
    Slash,
    Equal,
    PlusEqual,
    MinusEqual,
    StarEqual,
    SlashEqual,
    LeftParen,
    RightParen,
    Semicolon,
    Eof,
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Identifier(name) => write!(f, "identifier({name})"),
            Self::String(value) => write!(f, "string({value:?})"),
            Self::Integer(digits) => write!(f, "integer({digits})"),
            Self::Plus => f.write_str("+"),
            Self::Minus => f.write_str("-"),
            Self::Star => f.write_str("*"),
            Self::Slash => f.write_str("/"),
            Self::Equal => f.write_str("="),
            Self::PlusEqual => f.write_str("+="),
            Self::MinusEqual => f.write_str("-="),
            Self::StarEqual => f.write_str("*="),
            Self::SlashEqual => f.write_str("/="),
            Self::LeftParen => f.write_str("("),
            Self::RightParen => f.write_str(")"),
            Self::Semicolon => f.write_str(";"),
            Self::Eof => f.write_str("<eof>"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

pub fn lex(source: &str) -> Result<Vec<Token>, Diagnostic> {
    Lexer { source, cursor: 0 }.scan()
}

struct Lexer<'a> {
    source: &'a str,
    cursor: usize,
}

impl Lexer<'_> {
    fn peek(&self) -> Option<char> {
        self.source[self.cursor..].chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let character = self.peek()?;
        self.cursor += character.len_utf8();
        Some(character)
    }

    fn scan(mut self) -> Result<Vec<Token>, Diagnostic> {
        let mut tokens = Vec::new();
        // Some editors save UTF-8 files with a leading byte order mark.
        if self.peek() == Some('\u{feff}') {
            self.bump();
        }
        while let Some(character) = self.peek() {
            let start = self.cursor;
            if character.is_whitespace() {
                self.bump();
                continue;
            }
            if self.source[self.cursor..].starts_with("//") {
                while self.peek().is_some_and(|ch| ch != '\n' && ch != '\r') {
                    self.bump();
                }
                continue;
            }
            let kind = match character {
                '=' => {
                    self.bump();
                    TokenKind::Equal
                }
                '(' => {
                    self.bump();
                    TokenKind::LeftParen
                }
                ')' => {
                    self.bump();
                    TokenKind::RightParen
                }
                ';' => {
                    self.bump();
                    TokenKind::Semicolon
                }
                '"' => TokenKind::String(self.string()?),
                '+' | '-' | '*' | '/' => {
                    self.bump();
                    if self.peek() == Some('=') {
                        self.bump();
                        match character {
                            '+' => TokenKind::PlusEqual,
                            '-' => TokenKind::MinusEqual,
                            '*' => TokenKind::StarEqual,
                            _ => TokenKind::SlashEqual,
                        }
                    } else {
                        match character {
                            '+' => TokenKind::Plus,
                            '-' => TokenKind::Minus,
                            '*' => TokenKind::Star,
                            _ => TokenKind::Slash,
                        }
                    }
                }
                ch if ch.is_ascii_digit() => {
                    self.bump();
                    while self.peek().is_some_and(|ch| ch.is_ascii_digit()) {
                        self.bump();
                    }
                    TokenKind::Integer(self.source[start..self.cursor].to_string())
                }
                ch if ch.is_ascii_alphabetic() || ch == '_' => {
                    self.bump();
                    while self
                        .peek()
                        .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
                    {
                        self.bump();
                    }
                    TokenKind::Identifier(self.source[start..self.cursor].to_string())
                }
                ch => {
                    return Err(Diagnostic::new(
                        DiagnosticKind::InvalidCharacter,
                        Span::new(start, start + ch.len_utf8()),
                        format!("unexpected character {ch:?}"),
                    )
                    .with_help("use println with a string literal or integer expression"));
                }
            };
            tokens.push(Token {
                kind,
                span: Span::new(start, self.cursor),
            });
        }
        tokens.push(Token {
            kind: TokenKind::Eof,
            span: Span::new(self.cursor, self.cursor),
        });
        Ok(tokens)
    }

    fn string(&mut self) -> Result<String, Diagnostic> {
        let start = self.cursor;
        self.bump();
        let mut value = String::new();
        while let Some(character) = self.peek() {
            match character {
                '"' => {
                    self.bump();
                    return Ok(value);
                }
                '\n' | '\r' => break,
                '\\' => {
                    let escape_start = self.cursor;
                    self.bump();
                    let escaped = match self.peek() {
                        Some('\\') => '\\',
                        Some('"') => '"',
                        Some('n') => '\n',
                        Some('\n' | '\r') | None => break,
                        Some(ch) => {
                            return Err(Diagnostic::new(
                                DiagnosticKind::InvalidEscape,
                                Span::new(escape_start, self.cursor + ch.len_utf8()),
                                format!("unsupported escape {:?}", format!("\\{ch}")),
                            )
                            .with_help(r#"supported escapes are \\, \" and \n"#));
                        }
                    };
                    self.bump();
                    value.push(escaped);
                }
                ch => {
                    self.bump();
                    value.push(ch);
                }
            }
        }
        Err(Diagnostic::new(
            DiagnosticKind::UnterminatedString,
            Span::new(start, self.cursor),
            "unterminated string literal",
        )
        .with_help(r#"close the string with a double quote; use \n for a line break"#))
    }
}
