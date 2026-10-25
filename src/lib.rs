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
pub mod interpreter;
pub mod lexer;
pub mod parser;

use std::{fmt, io};

use ast::Program;
use diagnostics::Diagnostic;

/// Validate source and construct its syntax tree without executing it.
pub fn compile(source: &str) -> Result<Program, Diagnostic> {
    parser::parse(&lexer::lex(source)?)
}

/// A syntax error or a failure writing program output.
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
    interpreter::execute(&program, output).map_err(RunError::Output)
}
