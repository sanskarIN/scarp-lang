//! Byte spans and source diagnostics. Columns count Unicode scalar values.

use std::fmt;

/// A half-open byte range in the original UTF-8 source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Location {
    pub line: usize,
    pub column: usize,
}

fn boundary(source: &str, offset: usize) -> usize {
    let mut offset = offset.min(source.len());
    while !source.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

/// Resolve a byte offset to a one-based line and column, including at EOF.
/// Out-of-range and non-boundary offsets are clamped for diagnostic rendering.
pub fn location(source: &str, offset: usize) -> Location {
    let prefix = &source[..boundary(source, offset)];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = prefix.rsplit('\n').next().unwrap_or("").chars().count() + 1;
    Location { line, column }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticKind {
    InvalidCharacter,
    UnterminatedString,
    InvalidEscape,
    UnexpectedToken,
    UnknownStatement,
    InvalidInteger,
    ArithmeticOverflow,
    DivisionByZero,
    ExpressionNesting,
    UnknownName,
    DuplicateBinding,
    ReservedName,
    ImmutableAssignment,
}

impl DiagnosticKind {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidCharacter => "E0001",
            Self::UnterminatedString => "E0002",
            Self::InvalidEscape => "E0003",
            Self::UnexpectedToken => "E0004",
            Self::UnknownStatement => "E0005",
            Self::InvalidInteger => "E0006",
            Self::ArithmeticOverflow => "E0007",
            Self::DivisionByZero => "E0008",
            Self::ExpressionNesting => "E0009",
            Self::UnknownName => "E0010",
            Self::DuplicateBinding => "E0011",
            Self::ReservedName => "E0012",
            Self::ImmutableAssignment => "E0013",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub kind: DiagnosticKind,
    pub message: String,
    pub span: Span,
    pub help: Option<String>,
    pub related: Vec<RelatedDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelatedDiagnostic {
    pub span: Span,
    pub message: String,
}

impl Diagnostic {
    pub fn new(kind: DiagnosticKind, span: Span, message: impl Into<String>) -> Self {
        Self {
            kind,
            span,
            message: message.into(),
            help: None,
            related: Vec::new(),
        }
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    pub fn with_related(mut self, span: Span, message: impl Into<String>) -> Self {
        self.related.push(RelatedDiagnostic {
            span,
            message: message.into(),
        });
        self
    }

    /// Render the first affected source line with a caret and optional help.
    pub fn render(&self, filename: &str, source: &str) -> String {
        let mut rendered = format!("{self}\n{}", excerpt(self.span, filename, source));
        for related in &self.related {
            rendered.push_str(&format!(
                "\n = note: {}\n{}",
                related.message,
                excerpt(related.span, filename, source)
            ));
        }
        if let Some(help) = &self.help {
            rendered.push_str(&format!("\n = help: {help}"));
        }
        rendered
    }
}

fn excerpt(span: Span, filename: &str, source: &str) -> String {
    let start = boundary(source, span.start);
    let end = boundary(source, span.end).max(start);
    let position = location(source, start);
    let line_start = source[..start].rfind('\n').map_or(0, |index| index + 1);
    let line_end = source[start..]
        .find('\n')
        .map_or(source.len(), |index| start + index);
    let line = source[line_start..line_end].trim_end_matches('\r');
    let visible_start = start.min(line_start + line.len());
    let visible_end = end.min(line_start + line.len()).max(visible_start);
    let indent = display_width(&source[line_start..visible_start]);
    let width = display_width(&source[visible_start..visible_end]).max(1);
    let gutter = position.line.to_string().len();
    format!(
        " --> {filename}:{}:{}\n{:gutter$} |\n{} | {}\n{:gutter$} | {}{}",
        position.line,
        position.column,
        "",
        position.line,
        display_text(line),
        "",
        " ".repeat(indent),
        "^".repeat(width),
    )
}

fn display_text(text: &str) -> String {
    text.chars()
        .map(|character| match character {
            '\t' => "    ".to_string(),
            ch if ch.is_control() => ch.escape_default().to_string(),
            ch => ch.to_string(),
        })
        .collect()
}

fn display_width(text: &str) -> usize {
    display_text(text).chars().count()
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "error[{}]: {}", self.kind.code(), self.message)
    }
}

impl std::error::Error for Diagnostic {}
