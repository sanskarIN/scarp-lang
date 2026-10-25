//! Scarp language front end and interpreter.
//!
//! The prototype separates tokenization, parsing, and execution. A whole file is
//! validated before any statements execute.
//!
//! ```
//! let mut output = Vec::new();
//! scarp::run(r#"println("Hello from Scarp!");"#, &mut output)?;
//! assert_eq!(output, b"Hello from Scarp!\n");
//! # Ok::<(), scarp::RunError>(())
//! ```

pub mod ast;
pub mod diagnostics;
pub mod evaluator;
pub mod interpreter;
pub mod lexer;
pub mod parser;
pub mod resolver;
pub mod value;

use std::{fmt, io};

use ast::Program;
use diagnostics::Diagnostic;
pub use resolver::ResolvedProgram;

/// Parse source without performing name resolution or arithmetic evaluation.
pub fn parse(source: &str) -> Result<Program, Diagnostic> {
    parser::parse(&lexer::lex(source)?)
}

/// Parse and resolve a complete source file without evaluating arithmetic.
pub fn compile(source: &str) -> Result<ResolvedProgram, Diagnostic> {
    resolver::resolve(parse(source)?)
}

/// Validate syntax, names, and checked arithmetic without writing program output.
/// The current language has no side effects other than printing.
///
/// ```
/// scarp::check("let mut count = 1; count += 2; println(count);")?;
/// assert!(scarp::check("let value = 1 / 0;").is_err());
/// # Ok::<(), scarp::diagnostics::Diagnostic>(())
/// ```
pub fn check(source: &str) -> Result<(), Diagnostic> {
    interpreter::validate(&compile(source)?)
}

/// A source/evaluation diagnostic or a failure writing program output.
#[derive(Debug)]
pub enum RunError {
    Diagnostic(Diagnostic),
    Output(io::Error),
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Diagnostic(error) => error.fmt(f),
            Self::Output(error) => write!(f, "could not write program output: {error}"),
        }
    }
}

impl std::error::Error for RunError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Diagnostic(error) => Some(error),
            Self::Output(error) => Some(error),
        }
    }
}

/// Compile the entire file, then execute it into the provided output stream.
pub fn run(source: &str, output: &mut impl io::Write) -> Result<(), RunError> {
    let program = compile(source).map_err(RunError::Diagnostic)?;
    interpreter::execute(&program, output)
}
