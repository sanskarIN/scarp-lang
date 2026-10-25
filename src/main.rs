use std::{
    env,
    ffi::{OsStr, OsString},
    fs,
    io::{self, BufWriter, Write},
    path::Path,
    process::ExitCode,
};

use scarp::{
    diagnostics::{Diagnostic, LocationTracker},
    lexer, RunError,
};

const HELP: &str = "Scarp — Build Beyond Boundaries.

Usage:
  scarp run <file.srp>       Execute a Scarp program
  scarp check <file.srp>     Validate syntax, names, and arithmetic without printing
  scarp tokens <file.srp>    Inspect lexer tokens and source locations
  scarp --help               Show this help
  scarp --version            Show the prototype version

Use -- before a filename that begins with a dash.
";

fn main() -> ExitCode {
    let stdout = io::stdout();
    let stderr = io::stderr();
    let mut output = BufWriter::new(stdout.lock());
    let mut errors = stderr.lock();
    let status = dispatch(env::args_os().skip(1).collect(), &mut output, &mut errors);
    if let Err(error) = output.flush() {
        return output_error(error, &mut errors);
    }
    ExitCode::from(status)
}

fn dispatch(args: Vec<OsString>, output: &mut impl Write, errors: &mut impl Write) -> u8 {
    match args.as_slice() {
        [flag] if flag == "--help" || flag == "-h" => {
            return write_output(output.write_all(HELP.as_bytes()), errors);
        }
        [command, flag] if file_command(command) && (flag == "--help" || flag == "-h") => {
            return write_output(output.write_all(HELP.as_bytes()), errors);
        }
        [flag] if flag == "--version" || flag == "-V" => {
            return write_output(
                writeln!(output, "scarp {}", env!("CARGO_PKG_VERSION")),
                errors,
            );
        }
        _ => {}
    }
    let (command, filename) = match args.as_slice() {
        [command, filename] if !filename.to_string_lossy().starts_with('-') => (command, filename),
        [command, separator, filename] if separator == "--" => (command, filename),
        _ => {
            let _ = writeln!(
                errors,
                "error: expected a command and a file path\n\n{HELP}"
            );
            return 2;
        }
    };
    if !file_command(command) {
        let _ = writeln!(
            errors,
            "error: unknown command {:?}\n\n{HELP}",
            command.to_string_lossy()
        );
        return 2;
    }
    let path = Path::new(filename);
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) => {
            let _ = writeln!(
                errors,
                "error: could not read '{}': {error}",
                path.display()
            );
            return 1;
        }
    };
    if command == "tokens" {
        match lexer::lex(&source) {
            Ok(tokens) => {
                let mut locations = LocationTracker::new(&source);
                for token in tokens {
                    let start = locations.at(token.span.start);
                    let end = locations.at(token.span.end);
                    if let Err(error) = writeln!(
                        output,
                        "{}:{}..{}:{}  {}  [{}..{}]",
                        start.line,
                        start.column,
                        end.line,
                        end.column,
                        token.kind,
                        token.span.start,
                        token.span.end,
                    ) {
                        return write_output(Err(error), errors);
                    }
                }
                0
            }
            Err(error) => report_diagnostic(&error, path, &source, errors),
        }
    } else if command == "check" {
        match scarp::check(&source) {
            Ok(()) => 0,
            Err(error) => report_diagnostic(&error, path, &source, errors),
        }
    } else {
        match scarp::run(&source, output) {
            Ok(()) => 0,
            Err(RunError::Diagnostic(error)) => report_diagnostic(&error, path, &source, errors),
            Err(RunError::Output(error)) => write_output(Err(error), errors),
        }
    }
}

fn file_command(command: &OsStr) -> bool {
    matches!(command.to_str(), Some("run" | "tokens" | "check"))
}

fn report_diagnostic(error: &Diagnostic, path: &Path, source: &str, errors: &mut impl Write) -> u8 {
    let _ = writeln!(errors, "{}", error.render(&path.to_string_lossy(), source));
    1
}

fn write_output(result: io::Result<()>, errors: &mut impl Write) -> u8 {
    match result {
        Ok(()) => 0,
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => 0,
        Err(error) => {
            let _ = writeln!(errors, "error: could not write output: {error}");
            1
        }
    }
}

fn output_error(error: io::Error, errors: &mut impl Write) -> ExitCode {
    ExitCode::from(write_output(Err(error), errors))
}
