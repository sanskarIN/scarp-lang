//! UTF-8 aware lexer. Token spans always refer to source bytes, not decoded text.

use std::fmt;

use crate::diagnostics::{Diagnostic, DiagnosticKind, Span};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Identifier(String),
    String(String),
    Integer(String),
    Float(String),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    BitAnd,
    BitOr,
    BitXor,
    BitNot,
    ShiftLeft,
    ShiftRight,
    UnsignedRight,
    PercentEqual,
    AndEqual,
    OrEqual,
    XorEqual,
    LeftEqual,
    RightEqual,
    UnsignedEqual,
    Equal,
    EqualEqual,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    PlusEqual,
    MinusEqual,
    StarEqual,
    SlashEqual,
    LeftParen,
    LeftBracket,
    RightBracket,
    Comma,
    Colon,
    Arrow,
    RightParen,
    LeftBrace,
    RightBrace,
    Semicolon,
    Eof,
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Identifier(name) => write!(f, "identifier({name})"),
            Self::String(value) => write!(f, "string({value:?})"),
            Self::Integer(digits) => write!(f, "integer({digits})"),
            Self::Float(digits) => write!(f, "float({digits})"),
            Self::Plus => f.write_str("+"),
            Self::Minus => f.write_str("-"),
            Self::Star => f.write_str("*"),
            Self::Slash => f.write_str("/"),
            Self::Percent => f.write_str("%"),
            Self::BitAnd => f.write_str("&"),
            Self::BitOr => f.write_str("|"),
            Self::BitXor => f.write_str("^"),
            Self::BitNot => f.write_str("~"),
            Self::ShiftLeft => f.write_str("<<"),
            Self::ShiftRight => f.write_str(">>"),
            Self::UnsignedRight => f.write_str(">>>"),
            Self::PercentEqual => f.write_str("%="),
            Self::AndEqual => f.write_str("&="),
            Self::OrEqual => f.write_str("|="),
            Self::XorEqual => f.write_str("^="),
            Self::LeftEqual => f.write_str("<<="),
            Self::RightEqual => f.write_str(">>="),
            Self::UnsignedEqual => f.write_str(">>>="),
            Self::Equal => f.write_str("="),
            Self::EqualEqual => f.write_str("=="),
            Self::NotEqual => f.write_str("!="),
            Self::Less => f.write_str("<"),
            Self::LessEqual => f.write_str("<="),
            Self::Greater => f.write_str(">"),
            Self::GreaterEqual => f.write_str(">="),
            Self::PlusEqual => f.write_str("+="),
            Self::MinusEqual => f.write_str("-="),
            Self::StarEqual => f.write_str("*="),
            Self::SlashEqual => f.write_str("/="),
            Self::LeftParen => f.write_str("("),
            Self::LeftBracket => f.write_str("["),
            Self::RightBracket => f.write_str("]"),
            Self::Comma => f.write_str(","),
            Self::Colon => f.write_str(":"),
            Self::Arrow => f.write_str("->"),
            Self::RightParen => f.write_str(")"),
            Self::LeftBrace => f.write_str("{"),
            Self::RightBrace => f.write_str("}"),
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
                ':' => {
                    self.bump();
                    TokenKind::Colon
                }
                '-' if self.source[self.cursor..].starts_with("->") => {
                    self.bump();
                    self.bump();
                    TokenKind::Arrow
                }
                '<' | '>'
                    if self.source[self.cursor..].starts_with(if character == '<' {
                        "<<"
                    } else {
                        ">>"
                    }) =>
                {
                    self.bump();
                    self.bump();
                    let unsigned = character == '>' && self.peek() == Some('>');
                    if unsigned {
                        self.bump();
                    }
                    let assign = self.peek() == Some('=');
                    if assign {
                        self.bump();
                    }
                    match (character, unsigned, assign) {
                        ('<', _, false) => TokenKind::ShiftLeft,
                        ('<', _, true) => TokenKind::LeftEqual,
                        ('>', true, false) => TokenKind::UnsignedRight,
                        ('>', true, true) => TokenKind::UnsignedEqual,
                        ('>', false, false) => TokenKind::ShiftRight,
                        _ => TokenKind::RightEqual,
                    }
                }
                '~' => {
                    self.bump();
                    TokenKind::BitNot
                }
                '%' | '&' | '|' | '^' => {
                    self.bump();
                    let assign = self.peek() == Some('=');
                    if assign {
                        self.bump();
                    }
                    match (character, assign) {
                        ('%', false) => TokenKind::Percent,
                        ('%', true) => TokenKind::PercentEqual,
                        ('&', false) => TokenKind::BitAnd,
                        ('&', true) => TokenKind::AndEqual,
                        ('|', false) => TokenKind::BitOr,
                        ('|', true) => TokenKind::OrEqual,
                        ('^', false) => TokenKind::BitXor,
                        _ => TokenKind::XorEqual,
                    }
                }
                '[' => {
                    self.bump();
                    TokenKind::LeftBracket
                }
                ']' => {
                    self.bump();
                    TokenKind::RightBracket
                }
                ',' => {
                    self.bump();
                    TokenKind::Comma
                }
                '{' => {
                    self.bump();
                    TokenKind::LeftBrace
                }
                '}' => {
                    self.bump();
                    TokenKind::RightBrace
                }
                '=' | '<' | '>' => {
                    self.bump();
                    let paired = self.peek() == Some('=');
                    if paired {
                        self.bump();
                    }
                    match (character, paired) {
                        ('=', false) => TokenKind::Equal,
                        ('=', true) => TokenKind::EqualEqual,
                        ('<', false) => TokenKind::Less,
                        ('<', true) => TokenKind::LessEqual,
                        ('>', false) => TokenKind::Greater,
                        _ => TokenKind::GreaterEqual,
                    }
                }
                '!' if self.source[self.cursor..].starts_with("!=") => {
                    self.bump();
                    self.bump();
                    TokenKind::NotEqual
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
                    if character == '0' && matches!(self.peek(), Some('x' | 'X' | 'b' | 'B')) {
                        self.bump();
                        while self
                            .peek()
                            .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
                        {
                            self.bump();
                        }
                        tokens.push(Token {
                            kind: TokenKind::Integer(self.source[start..self.cursor].to_string()),
                            span: Span::new(start, self.cursor),
                        });
                        continue;
                    }
                    while self.peek().is_some_and(|ch| ch.is_ascii_digit()) {
                        self.bump();
                    }
                    let mut float = false;
                    if self.peek() == Some('.') {
                        float = true;
                        self.bump();
                        self.decimal_digits(start)?;
                    }
                    if matches!(self.peek(), Some('e' | 'E')) {
                        float = true;
                        self.bump();
                        if matches!(self.peek(), Some('+' | '-')) {
                            self.bump();
                        }
                        self.decimal_digits(start)?;
                    }
                    let digits = self.source[start..self.cursor].to_string();
                    if float {
                        TokenKind::Float(digits)
                    } else {
                        TokenKind::Integer(digits)
                    }
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

    fn decimal_digits(&mut self, start: usize) -> Result<(), Diagnostic> {
        let digits_start = self.cursor;
        while self.peek().is_some_and(|ch| ch.is_ascii_digit()) {
            self.bump();
        }
        if self.cursor == digits_start {
            return Err(Diagnostic::new(
                DiagnosticKind::InvalidFloat,
                Span::new(start, self.cursor),
                "expected decimal digits after the point or exponent",
            ));
        }
        Ok(())
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
