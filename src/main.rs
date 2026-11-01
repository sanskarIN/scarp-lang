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
    lexer,
    modules::{self, ModuleLimits, ModuleRunError},
    ExecutionOptions,
};

const HELP: &str = "Scarp — Build Beyond Boundaries.

Usage:
  scarp run <file.srp>       Execute a Scarp program
  scarp check <file.srp>     Validate syntax, names, and arithmetic without printing
  scarp tokens <file.srp>    Inspect lexer tokens and source locations
  scarp --help               Show this help
  scarp --version            Show the prototype version

Use -- before a filename that begins with a dash.
run/check options before the path: --max-steps, --max-prints, --max-call-depth,
--max-calls, --max-live-slots, --max-capture-bytes, --max-vector-bytes (counts).
Defaults: 1000000 execution steps, 100000 selected output statements.
Iteration: while condition { ... }, repeat count as index { ... }.
Use scarp run --help or scarp check --help for budget examples.
";

const RUN_HELP: &str = "Usage: scarp run [budget options] [--] <file.srp>

Execute checked source, preparing every selected output statement before writing either stream.
while guards are boolean; repeat count as index saves one nonnegative integer count.
break; exits the nearest loop; continue; rechecks while or advances repeat.
Defaults: --max-steps 1000000 and --max-prints 100000. Options precede the path.
Function defaults: --max-call-depth 256, --max-calls 100000,
--max-live-slots 1000000, --max-capture-bytes 16777216,
--max-vector-bytes 8388608 (cumulative numerical element payload).
Vector and tile buffers share this cumulative quota; retirement does not refund it.
Snapshot bytes are a logical storage cap, not an exact process-memory limit.
Counts are unsigned decimal digits, zero is valid, and each option may occur once.
Example: scarp run --max-steps 2000000 --max-prints 10 examples/repeat.srp
Use scarp check with the same options to evaluate without printing.
Imports use an explicit alias and resolve relative to each file within the entry directory.
";

const CHECK_HELP: &str = "Usage: scarp check [budget options] [--] <file.srp>

Check syntax, names, and types, then evaluate the selected path without printing.
Loops and arithmetic execute with fresh storage and the same limits as run.
This evaluates code; it is not a static-only check. All bodies are type-checked.
Defaults: --max-steps 1000000 and --max-prints 100000. Options precede the path.
Function defaults: --max-call-depth 256, --max-calls 100000,
--max-live-slots 1000000, --max-capture-bytes 16777216,
--max-vector-bytes 8388608 (cumulative numerical element payload).
Vector and tile buffers share this cumulative quota; retirement does not refund it.
All seven options take unsigned decimal counts, allow zero, and may occur once.
Snapshot bytes are a logical storage cap, including when check discards output.
Example: scarp check --max-steps 2000000 examples/repeat.srp
Use tokens for lexing without executing a program. No execution-limit flags on tokens.
All imported modules are loaded and checked before evaluation; see docs/modules.md.
";

const TOKENS_HELP: &str = "Usage: scarp tokens [--] <file.srp>

List tokens and UTF-8 byte/source locations without parsing or executing loops.
No execution-limit options are accepted. Use scarp check to evaluate source.
Import declarations are tokenized without loading their target files.
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
    if let Err(error) = errors.flush() {
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
            let help = match command.to_str() {
                Some("run") => RUN_HELP,
                Some("check") => CHECK_HELP,
                _ => TOKENS_HELP,
            };
            return write_output(output.write_all(help.as_bytes()), errors);
        }
        [flag] if flag == "--version" || flag == "-V" => {
            return write_output(
                writeln!(output, "scarp {}", env!("CARGO_PKG_VERSION")),
                errors,
            );
        }
        _ => {}
    }
    let (command, filename, options) = match invocation(&args) {
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
    if command != "tokens" {
        let program = match modules::compile_file(path, ModuleLimits::default()) {
            Ok(program) => program,
            Err(error) => {
                let _ = writeln!(errors, "{}", error.render());
                return 1;
            }
        };
        return if command == "check" {
            match program.check(options) {
                Ok(()) => 0,
                Err(error) => {
                    let _ = writeln!(errors, "{}", error.render());
                    1
                }
            }
        } else {
            match program.execute(output, errors, options) {
                Ok(()) => 0,
                Err(ModuleRunError::Diagnostic(error)) => {
                    let _ = writeln!(errors, "{}", error.render());
                    1
                }
                Err(ModuleRunError::Output(error)) => write_output(Err(error), errors),
            }
        };
    }
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
    } else {
        unreachable!("run/check handled by the module compiler")
    }
}

fn file_command(command: &OsStr) -> bool {
    matches!(command.to_str(), Some("run" | "tokens" | "check"))
}

fn invocation(args: &[OsString]) -> Result<(&OsStr, &OsStr, ExecutionOptions), &'static str> {
    let command = args.first().ok_or("expected a command and a file path")?;
    let mut options = ExecutionOptions::default();
    let mut index = 1;
    let mut seen = Vec::new();
    let mut separator = false;
    while let Some(argument) = args.get(index) {
        if argument == "--" {
            separator = true;
            index += 1;
            break;
        }
        let flag = match argument.to_str() {
            Some(
                flag @ ("--max-steps"
                | "--max-prints"
                | "--max-call-depth"
                | "--max-calls"
                | "--max-live-slots"
                | "--max-capture-bytes"
                | "--max-vector-bytes"),
            ) => flag,
            _ => break,
        };
        if command != "run" && command != "check" {
            return Err("execution limits are available only for run and check");
        }
        if seen.contains(&flag) {
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
        match flag {
            "--max-steps" => options.limits.max_steps = value,
            "--max-calls" => options.max_calls = value,
            _ => {
                let value = usize::try_from(value).map_err(|_| "resource limit is too large")?;
                match flag {
                    "--max-prints" => options.limits.max_prints = value,
                    "--max-call-depth" => options.max_call_depth = value,
                    "--max-live-slots" => options.max_live_slots = value,
                    "--max-capture-bytes" => options.max_capture_bytes = value,
                    "--max-vector-bytes" => options.max_vector_bytes = value,
                    _ => unreachable!(),
                }
            }
        }
        seen.push(flag);
        index += 2;
    }
    let filename = args
        .get(index)
        .ok_or("expected a command and a file path")?;
    if index + 1 != args.len() || (!separator && filename.to_string_lossy().starts_with('-')) {
        return Err("expected a command and a file path");
    }
    Ok((command.as_os_str(), filename.as_os_str(), options))
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
