use scarp::project::{ProjectError, ScaffoldPlan};
use std::{ffi::OsString, io::Write, path::PathBuf};

pub const CREATE_HELP: &str = "Usage: scarp create <directory> [options]

Create a new application directory with scarp.toml and src/main.srp.
  --name <name>       Override the portable manifest name
  --template app      Generate the supported application template
  --dry-run           Check paths/conflicts and report the plan without writing
  --                  Treat the remaining single argument as the directory
The parent must exist. Existing destinations and overwrite/force modes are refused.
";
pub const INIT_HELP: &str = "Usage: scarp init [options]

Initialize an application in the current directory, preserving unrelated files.
  --name <name>       Override the portable manifest name
  --template app      Generate the supported application template
  --dry-run           Check paths/conflicts and report the plan without writing
Existing manifests, entrypoints, and linked source directories are refused.
";
pub const LAUNCH_HELP: &str = "Usage: scarp launch [options]

Execute the current project's entrypoint from scarp.toml; no file argument is accepted.
  --max-steps <count>   Logical execution budget (default 1000000)
  --max-prints <count>  Selected print budget (default 100000)
No ancestor search or manifest-defined shell tasks occur. Program arguments are
not supported. Use exec <file.srp> for an explicit source file.
";

struct Options {
    target: Option<PathBuf>,
    name: Option<String>,
    dry: bool,
}
fn parse(args: &[OsString], create: bool) -> Result<Options, &'static str> {
    let (mut target, mut name, mut dry, mut template) = (None, None, false, false);
    let mut index = 0;
    while let Some(argument) = args.get(index) {
        if argument == "--" {
            if !create || target.is_some() || index + 2 != args.len() {
                return Err("expected one new directory after --");
            }
            target = Some(PathBuf::from(&args[index + 1]));
            break;
        }
        match argument.to_str() {
            Some("--dry-run") => {
                if dry {
                    return Err("--dry-run cannot be repeated");
                }
                dry = true;
                index += 1;
            }
            Some("--name" | "--template") => {
                let value = args
                    .get(index + 1)
                    .and_then(|arg| arg.to_str())
                    .ok_or("expected a text value after the project option")?;
                if value.is_empty() {
                    return Err("project option values cannot be empty");
                }
                if argument == "--name" {
                    if name.replace(value.to_owned()).is_some() {
                        return Err("--name cannot be repeated");
                    }
                } else {
                    if template {
                        return Err("--template cannot be repeated");
                    }
                    if value != "app" {
                        return Err("only --template app is supported; library templates require functions/modules");
                    }
                    template = true;
                }
                index += 2;
            }
            _ if argument.to_string_lossy().starts_with('-') => {
                return Err("unsupported project option")
            }
            _ => {
                if !create
                    || argument.is_empty()
                    || target.replace(PathBuf::from(argument)).is_some()
                {
                    return Err(
                        "create requires one directory; init accepts no directory argument",
                    );
                }
                index += 1;
            }
        }
    }
    if create && target.is_none() {
        return Err("create requires a new project directory");
    }
    Ok(Options { target, name, dry })
}

pub fn dispatch(
    command: &str,
    args: &[OsString],
    output: &mut impl Write,
    errors: &mut impl Write,
) -> u8 {
    if command == "launch" {
        let (index, _, limits) = match crate::execution_options(args, 0, true) {
            Ok(options) => options,
            Err(message) => return usage(message, LAUNCH_HELP, errors),
        };
        if index != args.len() {
            return usage(
                "launch is project-only and does not accept files or program arguments",
                LAUNCH_HELP,
                errors,
            );
        }
        let project = match crate::current_project(errors) {
            Ok(project) => project,
            Err(status) => return status,
        };
        return crate::execute_file(project.entry(), limits, output, errors);
    }
    let create = command == "create";
    let help = if create { CREATE_HELP } else { INIT_HELP };
    let options = match parse(args, create) {
        Ok(options) => options,
        Err(message) => return usage(message, help, errors),
    };
    let plan = if let Some(target) = options.target {
        ScaffoldPlan::create(&target, options.name.as_deref())
    } else {
        let directory = match std::env::current_dir() {
            Ok(directory) => directory,
            Err(error) => {
                let _ = writeln!(errors, "error: could not read current directory: {error}");
                return 1;
            }
        };
        ScaffoldPlan::init(&directory, options.name.as_deref())
    };
    let plan = match plan {
        Ok(plan) => plan,
        Err(error) => return report(error, help, errors),
    };
    if options.dry {
        return crate::write_output(
            writeln!(
                output,
                "Would {} application '{}' in '{}'\n  scarp.toml\n  src/main.srp",
                if create { "create" } else { "initialize" },
                plan.name(),
                plan.root().display()
            ),
            errors,
        );
    }
    if let Err(error) = plan.apply() {
        return report(error, help, errors);
    }
    crate::write_output(
        writeln!(
            output,
            "{} application '{}' in '{}'",
            if create { "Created" } else { "Initialized" },
            plan.name(),
            plan.root().display()
        ),
        errors,
    )
}
fn usage(message: &str, help: &str, errors: &mut impl Write) -> u8 {
    let _ = writeln!(errors, "error: {message}\n\n{help}");
    2
}
fn report(error: ProjectError, help: &str, errors: &mut impl Write) -> u8 {
    let invalid = matches!(error, ProjectError::InvalidName);
    let _ = writeln!(errors, "error: {error}");
    if invalid {
        let _ = writeln!(errors, "\n{help}");
        2
    } else {
        1
    }
}
