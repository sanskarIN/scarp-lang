//! Scarp language front end and interpreter.
//!
//! The prototype separates tokenization, parsing, and execution. A whole file is
//! validated before any statements execute.
//!
//! ```
//! let mut output = Vec::new();
//! scarp::run(r#"print("Hello from Scarp!");"#, &mut output)?;
//! assert_eq!(output, b"Hello from Scarp!\n");
//! # Ok::<(), scarp::RunError>(())
//! ```

pub mod ast;
pub mod diagnostics;
pub mod evaluator;
pub mod interpreter;
pub mod lexer;
pub mod limits;
pub mod parser;
pub mod resolver;
pub mod value;

use std::{fmt, io};

use ast::Program;
use diagnostics::Diagnostic;
pub use limits::ExecutionLimits;
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
/// scarp::check("var count = 1; count += 2; print(count);")?;
/// assert!(scarp::check("bind value = 1 / 0;").is_err());
/// # Ok::<(), scarp::diagnostics::Diagnostic>(())
/// ```
pub fn check(source: &str) -> Result<(), Diagnostic> {
    interpreter::validate(&compile(source)?)
}

/// Validate with caller-selected deterministic execution and print budgets.
pub fn check_with_limits(source: &str, limits: ExecutionLimits) -> Result<(), Diagnostic> {
    interpreter::validate_with_limits(&compile(source)?, limits)
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

/// Execute with caller-selected budgets; budget failures never write output.
pub fn run_with_limits(
    source: &str,
    output: &mut impl io::Write,
    limits: ExecutionLimits,
) -> Result<(), RunError> {
    let program = compile(source).map_err(RunError::Diagnostic)?;
    interpreter::execute_with_limits(&program, output, limits)
}

/// Execute with explicit stdout and stderr destinations; never uses global I/O.
///
/// ```
/// let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
/// scarp::run_with_io("print(\"ready\", true); eprint(\"warning\");", &mut stdout, &mut stderr)?;
/// assert_eq!(stdout, b"ready true\n");
/// assert_eq!(stderr, b"warning\n");
/// # Ok::<(), scarp::RunError>(())
/// ```
pub fn run_with_io(
    source: &str,
    stdout: &mut impl io::Write,
    stderr: &mut impl io::Write,
) -> Result<(), RunError> {
    run_with_io_and_limits(source, stdout, stderr, ExecutionLimits::default())
}

/// Dual-destination execution with deterministic work and output-statement limits.
pub fn run_with_io_and_limits(
    source: &str,
    stdout: &mut impl io::Write,
    stderr: &mut impl io::Write,
    limits: ExecutionLimits,
) -> Result<(), RunError> {
    let program = compile(source).map_err(RunError::Diagnostic)?;
    interpreter::execute_with_io_and_limits(&program, stdout, stderr, limits)
}
