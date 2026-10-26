use std::{ffi::OsString, io::Write};

pub const GENERAL: &str = "Scarp — Build Beyond Boundaries.

Usage: scarp <command> [options]

  create <directory>       Create a new application project
  init                     Initialize an application in the current directory
  exec [file.srp]           Execute a file or current project's entrypoint
  launch                   Execute the current project; no file argument
  compile [file.srp] -o <output>  Produce an executable, object, or C source file
  verify [file.srp]         Check syntax, names, and types without execution
  inspect tokens|ast <file.srp>  Inspect actual frontend representations
  guide [command]          Show contextual help
  version                  Show the package version

Options: -h, --help; -V, --version
Examples:
  scarp exec examples/hello.srp
  scarp verify examples/scopes.srp
  scarp compile examples/bit_fields.srp -o program
  scarp guide inspect ast

Use -- before a filename beginning with a dash. Project commands use the current
directory's scarp.toml without ancestor searches. Program arguments are unsupported.
";

const EXEC: &str = "Usage: scarp exec [options] [[--] <file.srp>]

Execute the selected source path after static checks and checked evaluation.
  --max-steps <count>   Logical work budget (default 1000000)
  --max-prints <count>  Selected print budget (default 100000)
Options precede the filename; -- permits a filename beginning with a dash.
Without a file, use the current project's scarp.toml entrypoint; no ancestor search.
Program arguments are not supported.
For static verification without execution use scarp verify.
";
const VERIFY: &str = "Usage: scarp verify [[--] <file.srp>]

Check lexical validity, syntax, names, mutability, and types without execution.
No arithmetic or loops run; no artifacts are created. Success is silent.
Runtime overflow, division by zero, and budgets are checked by exec.
Without a file, use the current project's scarp.toml entrypoint; no ancestor search.
Strict/JSON modes are not implemented.
";
const TOKENS: &str = "Usage: scarp inspect tokens [--] <file.srp>

Print lexer tokens, half-open byte spans, and one-based source locations.
Only lexing is performed; syntax and names may still be invalid.
";
const AST: &str = "Usage: scarp inspect ast [--] <file.srp>

Print the actual parsed syntax tree in Rust's readable Debug format.
No resolution, arithmetic, or execution is performed. The format is experimental,
not a versioned JSON schema or a native compilation artifact.
";
const INSPECT: &str = "Usage: scarp inspect tokens|ast [--] <file.srp>

Read-only inspection of actual lexer tokens or parsed syntax.
Use scarp guide inspect tokens or scarp guide inspect ast for details.
Symbols, types, IR, dependency graphs, and project inspection are not exposed.
";
const VERSION: &str = "Usage: scarp version
       scarp --version
       scarp -V

Print the package version. Verbose/component-version modes are not implemented.
";

fn topic(command: &str, subcommand: Option<&str>) -> Option<&'static str> {
    match (command, subcommand) {
        ("exec", None) => Some(EXEC),
        ("create", None) => Some(crate::project_cli::CREATE_HELP),
        ("init", None) => Some(crate::project_cli::INIT_HELP),
        ("launch", None) => Some(crate::project_cli::LAUNCH_HELP),
        ("compile", None) => Some(crate::compile_cli::HELP),
        ("verify", None) => Some(VERIFY),
        ("inspect", Some("tokens")) => Some(TOKENS),
        ("inspect", Some("ast")) => Some(AST),
        ("inspect", None) => Some(INSPECT),
        ("version", None) => Some(VERSION),
        ("guide", None) => Some(GENERAL),
        _ => None,
    }
}

pub fn dispatch(args: &[OsString], output: &mut impl Write, errors: &mut impl Write) -> Option<u8> {
    let strings: Vec<_> = args.iter().map(|arg| arg.to_str()).collect();
    let help = match strings.as_slice() {
        [] => Some(GENERAL),
        [Some("--help" | "-h")] => Some(GENERAL),
        [Some("guide")] => Some(GENERAL),
        [Some("guide"), Some("--help" | "-h")] => Some(GENERAL),
        [Some("guide"), Some(command)] => topic(command, None),
        [Some("guide"), Some(command), Some(subcommand)] => topic(command, Some(subcommand)),
        [Some(command), Some("--help" | "-h")] => topic(command, None),
        [Some(command), Some(subcommand), Some("--help" | "-h")] => {
            topic(command, Some(subcommand))
        }
        _ => None,
    };
    if let Some(help) = help {
        return Some(crate::write_output(
            output.write_all(help.as_bytes()),
            errors,
        ));
    }
    if matches!(strings.as_slice(), [Some("version" | "--version" | "-V")]) {
        return Some(crate::write_output(
            writeln!(output, "scarp {}", env!("CARGO_PKG_VERSION")),
            errors,
        ));
    }
    if matches!(strings.first(), Some(Some("guide" | "version"))) {
        let _ = writeln!(
            errors,
            "error: unknown help topic or unsupported arguments\n\n{GENERAL}"
        );
        return Some(2);
    }
    None
}

/// Migration guidance is an error, never an accepted alias or a dispatch rewrite.
pub fn replacement(command: &str) -> Option<&'static str> {
    match command {
        "run" => Some("scarp exec"),
        "build" => Some("scarp compile"),
        "check" => Some("scarp verify for static checks, or scarp exec to execute"),
        "tokens" => Some("scarp inspect tokens"),
        "emit-c" => Some("scarp compile --emit c -o <output.c> <source.srp>"),
        "help" => Some("scarp guide"),
        "new" => Some("scarp create"),
        _ => None,
    }
}

/// Offer only a unique close spelling; never change the command being executed.
pub fn suggestion(input: &str) -> Option<&'static str> {
    if !input.is_ascii() || input.len() < 3 || input.len() > 20 {
        return None;
    }
    let mut best = None;
    let mut distance = 3;
    for candidate in [
        "create", "init", "exec", "launch", "compile", "verify", "inspect", "guide", "version",
    ] {
        let mut previous: Vec<_> = (0..=input.len()).collect();
        for (row, a) in candidate.bytes().enumerate() {
            let mut current = vec![row + 1];
            for (column, b) in input.bytes().enumerate() {
                current.push(
                    (current[column] + 1)
                        .min(previous[column + 1] + 1)
                        .min(previous[column] + usize::from(a != b)),
                );
            }
            previous = current;
        }
        let score = previous[input.len()];
        if score < distance {
            distance = score;
            best = Some(candidate);
        } else if score == distance {
            best = None;
        }
    }
    best
}
