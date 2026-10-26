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
    lexer, ExecutionLimits, RunError,
};

mod build_cli;

const HELP: &str = "Scarp — Build Beyond Boundaries.

Usage:
  scarp run <file.srp>       Execute a Scarp program
  scarp check <file.srp>     Validate syntax, names, and arithmetic without printing
  scarp tokens <file.srp>    Inspect lexer tokens and source locations
  scarp emit-c <file.srp>    Emit a freestanding C11 unit for the supported subset
  scarp build <file.srp> -o <output>  Build a native executable or object
  scarp compile ...         Alias for build; use build --help for options
  scarp --help               Show this help
  scarp --version            Show the prototype version

Use -- before a filename that begins with a dash.
run/check options before the path: --max-steps <count>, --max-prints <count>.
Defaults: 1000000 execution steps, 100000 selected prints.
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
    if matches!(
        args.first().and_then(|value| value.to_str()),
        Some("build" | "compile")
    ) {
        return build_cli::dispatch(&args[1..], output, errors);
    }
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
    let (command, filename, limits) = match invocation(&args) {
        Ok(invocation) => invocation,
        Err(message) => {
            let _ = writeln!(errors, "error: {message}\n\n{HELP}");
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
    if command == "emit-c" {
        match scarp::c_backend::emit_c(&source) {
            Ok(unit) => write_output(output.write_all(unit.as_bytes()), errors),
            Err(error) => report_diagnostic(&error, path, &source, errors),
        }
    } else if command == "tokens" {
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
        match scarp::check_with_limits(&source, limits) {
            Ok(()) => 0,
            Err(error) => report_diagnostic(&error, path, &source, errors),
        }
    } else {
        match scarp::run_with_limits(&source, output, limits) {
            Ok(()) => 0,
            Err(RunError::Diagnostic(error)) => report_diagnostic(&error, path, &source, errors),
            Err(RunError::Output(error)) => write_output(Err(error), errors),
        }
    }
}

fn file_command(command: &OsStr) -> bool {
    matches!(
        command.to_str(),
        Some("run" | "tokens" | "check" | "emit-c")
    )
}

fn invocation(args: &[OsString]) -> Result<(&OsStr, &OsStr, ExecutionLimits), &'static str> {
    let command = args.first().ok_or("expected a command and a file path")?;
    let mut limits = ExecutionLimits::default();
    let mut index = 1;
    let mut seen_steps = false;
    let mut seen_prints = false;
    let mut separator = false;
    while let Some(argument) = args.get(index) {
        if argument == "--" {
            separator = true;
            index += 1;
            break;
        }
        let steps = argument == "--max-steps";
        let prints = argument == "--max-prints";
        if !steps && !prints {
            break;
        }
        if command != "run" && command != "check" {
            return Err("execution limits are available only for run and check");
        }
        if (steps && seen_steps) || (prints && seen_prints) {
            return Err("execution limit options cannot be repeated");
        }
        let digits = args
            .get(index + 1)
            .and_then(|value| value.to_str())
            .ok_or("expected a decimal execution limit")?;
        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err("execution limits must contain decimal digits only");
        }
        let value = digits
            .parse::<u64>()
            .map_err(|_| "execution limit is too large")?;
        if steps {
            limits.max_steps = value;
            seen_steps = true;
        } else {
            limits.max_prints = usize::try_from(value).map_err(|_| "print limit is too large")?;
            seen_prints = true;
        }
        index += 2;
    }
    let filename = args
        .get(index)
        .ok_or("expected a command and a file path")?;
    if index + 1 != args.len() || (!separator && filename.to_string_lossy().starts_with('-')) {
        return Err("expected a command and a file path");
    }
    Ok((command.as_os_str(), filename.as_os_str(), limits))
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
