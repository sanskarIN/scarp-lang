use scarp::{
    builder::{self, BuildError, BuildMode, BuildOptions, ObjectTarget},
    ExecutionLimits,
};
use std::{
    env,
    ffi::{OsStr, OsString},
    fs,
    io::Write,
    path::{Path, PathBuf},
};

pub const HELP: &str = "Usage: scarp compile <file.srp> -o <output> [options]
       scarp build <file.srp> -o <output> [options]

Build the supported integer/boolean subset; never run the result automatically.
  -o, --output <path>       Required fresh output path; parent must exist
  --emit exe|obj           Native hosted executable (default) or freestanding object
  --cc <executable>        Clang/GCC-style compiler (SCARP_CC, then platform default)
  --target <triple>        Objects only: x86_64-unknown-none-elf or aarch64-unknown-none-elf
  --max-steps <count>      Executable runtime budget (default 1000000)
  --max-prints <count>     Executable print budget (default 100000)
  --                      Treat the remaining single argument as the source path
  -h, --help              Show this help

All paths may contain spaces. Use emit-c for C source without a C compiler.
";

struct Invocation {
    source: PathBuf,
    build: BuildOptions,
}

fn parse(args: &[OsString]) -> Result<Invocation, &'static str> {
    let mut source = None;
    let (mut output, mut compiler, mut emit, mut target) = (None, None, None, None);
    let (mut steps, mut prints) = (None, None);
    let mut index = 0;
    while let Some(argument) = args.get(index) {
        if argument == "--" {
            if source.is_some() || index + 2 != args.len() {
                return Err("expected one source path after --");
            }
            source = Some(PathBuf::from(&args[index + 1]));
            break;
        }
        if argument.to_string_lossy().starts_with('-') {
            let slot = match argument.to_str() {
                Some("-o" | "--output") => &mut output,
                Some("--cc") => &mut compiler,
                Some("--emit") => &mut emit,
                Some("--target") => &mut target,
                Some("--max-steps") => &mut steps,
                Some("--max-prints") => &mut prints,
                _ => return Err("unknown build option; use scarp build --help"),
            };
            if slot.is_some() {
                return Err("build options cannot be repeated");
            }
            let value = args
                .get(index + 1)
                .ok_or("expected a value after the build option")?;
            if value.is_empty() {
                return Err("build option values cannot be empty");
            }
            *slot = Some(value.clone());
            index += 2;
        } else {
            if source.replace(PathBuf::from(argument)).is_some() {
                return Err("expected exactly one source path");
            }
            index += 1;
        }
    }
    let source = source.ok_or("expected a Scarp source path")?;
    let output = output.ok_or("build requires -o <output>")?;
    let mode = match emit.as_deref().unwrap_or(OsStr::new("exe")).to_str() {
        Some("exe") => {
            if target.is_some() {
                return Err("--target is available only with --emit obj");
            }
            let mut limits = ExecutionLimits::default();
            if let Some(value) = steps {
                limits.max_steps = count(&value)?;
            }
            if let Some(value) = prints {
                limits.max_prints =
                    usize::try_from(count(&value)?).map_err(|_| "print limit is too large")?;
            }
            BuildMode::Executable { limits }
        }
        Some("obj") => {
            if steps.is_some() || prints.is_some() {
                return Err("object budgets are supplied by the embedding host, not build options");
            }
            let target = match target.as_deref().map(|value| value.to_str()) {
                None => None,
                Some(Some("x86_64-unknown-none-elf")) => Some(ObjectTarget::X86_64),
                Some(Some("aarch64-unknown-none-elf")) => Some(ObjectTarget::Aarch64),
                _ => return Err("unsupported object target"),
            };
            BuildMode::Object { target }
        }
        _ => return Err("--emit must be exe or obj"),
    };
    let compiler = compiler
        .or_else(|| env::var_os("SCARP_CC"))
        .unwrap_or_else(|| {
            if cfg!(windows) || matches!(mode, BuildMode::Object { target: Some(_) }) {
                "clang".into()
            } else {
                "cc".into()
            }
        });
    Ok(Invocation {
        source,
        build: BuildOptions {
            output: PathBuf::from(output),
            compiler,
            mode,
        },
    })
}

fn count(value: &OsString) -> Result<u64, &'static str> {
    let digits = value
        .to_str()
        .ok_or("execution budgets require decimal digits")?;
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("execution budgets require decimal digits");
    }
    digits.parse().map_err(|_| "execution budget is too large")
}

pub fn dispatch(args: &[OsString], output: &mut impl Write, errors: &mut impl Write) -> u8 {
    if matches!(args, [flag] if flag == "--help" || flag == "-h") {
        return crate::write_output(output.write_all(HELP.as_bytes()), errors);
    }
    let invocation = match parse(args) {
        Ok(invocation) => invocation,
        Err(message) => {
            let _ = writeln!(errors, "error: {message}\n\n{HELP}");
            return 2;
        }
    };
    let source = match fs::read_to_string(&invocation.source) {
        Ok(source) => source,
        Err(error) => {
            let _ = writeln!(
                errors,
                "error: could not read '{}': {error}",
                invocation.source.display()
            );
            return 1;
        }
    };
    match builder::build(&source, &invocation.build) {
        Ok(()) => 0,
        Err(BuildError::Source(error)) => {
            crate::report_diagnostic(&error, Path::new(&invocation.source), &source, errors)
        }
        Err(error) => {
            let _ = writeln!(errors, "error: {error}");
            1
        }
    }
}
