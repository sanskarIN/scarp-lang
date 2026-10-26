use std::{ffi::OsString, io::Write};

pub const GENERAL: &str = "Scarp — Build Beyond Boundaries.

Usage: scarp <command> [options]

  exec <file.srp>           Execute source (run is an alias)
  compile <file.srp> -o <output>  Build a native executable or object (build is an alias)
  verify <file.srp>         Check syntax, names, and types without execution
  check <file.srp>          Validate selected arithmetic without printing
  inspect tokens|ast <file.srp>  Inspect actual frontend representations
  tokens <file.srp>         Print tokens and source locations
  emit-c <file.srp>         Emit supported source as freestanding C11
  guide [command]          Show contextual help (help is an alias)
  version                  Show the package version

Options: -h, --help; -V, --version
Examples:
  scarp exec examples/hello.srp
  scarp verify examples/scopes.srp
  scarp compile examples/bit_fields.srp -o program
  scarp guide inspect ast

Use -- before a filename beginning with a dash. Program arguments and projects
are not supported. For legacy commands: scarp run, scarp check, scarp tokens.
";

const EXEC: &str = "Usage: scarp exec [options] [--] <file.srp>
       scarp run [options] [--] <file.srp>

Execute the selected source path after static checks and checked evaluation.
  --max-steps <count>   Logical work budget (default 1000000)
  --max-prints <count>  Selected print budget (default 100000)
Options precede the filename; -- permits a filename beginning with a dash.
Program arguments and project discovery are not supported.
For validation without output use scarp check or scarp verify.
";
const CHECK: &str = "Usage: scarp check [options] [--] <file.srp>

Evaluate selected arithmetic and validate without writing program output.
  --max-steps <count>   Logical work budget (default 1000000)
  --max-prints <count>  Selected print budget (default 100000)
Unlike verify, check executes selected arithmetic and loops within these limits.
";
const VERIFY: &str = "Usage: scarp verify [--] <file.srp>

Check lexical validity, syntax, names, mutability, and types without execution.
No arithmetic or loops run; no artifacts are created. Success is silent.
Runtime overflow, division by zero, and budgets require check or exec.
Strict/JSON modes and project discovery are not implemented.
";
const TOKENS: &str = "Usage: scarp inspect tokens [--] <file.srp>
       scarp tokens [--] <file.srp>

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
const EMIT_C: &str = "Usage: scarp emit-c [--] <file.srp>

Write freestanding C11 for the supported integer/boolean computation subset.
No source execution or C compiler invocation occurs. Unsupported values use E0027.
Use scarp compile --help to build an executable or object from supported source.
";
const VERSION: &str = "Usage: scarp version
       scarp --version
       scarp -V

Print the package version. Verbose/component-version modes are not implemented.
";

fn topic(command: &str, subcommand: Option<&str>) -> Option<&'static str> {
    match (command, subcommand) {
        ("exec" | "run", None) => Some(EXEC),
        ("compile" | "build", None) => Some(crate::build_cli::HELP),
        ("verify", None) => Some(VERIFY),
        ("check", None) => Some(CHECK),
        ("tokens", None) | ("inspect", Some("tokens")) => Some(TOKENS),
        ("inspect", Some("ast")) => Some(AST),
        ("inspect", None) => Some(INSPECT),
        ("emit-c", None) => Some(EMIT_C),
        ("version", None) => Some(VERSION),
        ("guide" | "help", None) => Some(GENERAL),
        _ => None,
    }
}

pub fn dispatch(args: &[OsString], output: &mut impl Write, errors: &mut impl Write) -> Option<u8> {
    let strings: Vec<_> = args.iter().map(|arg| arg.to_str()).collect();
    let help = match strings.as_slice() {
        [Some("--help" | "-h")] => Some(GENERAL),
        [Some("guide" | "help")] => Some(GENERAL),
        [Some("guide" | "help"), Some("--help" | "-h")] => Some(GENERAL),
        [Some("guide" | "help"), Some(command)] => topic(command, None),
        [Some("guide" | "help"), Some(command), Some(subcommand)] => {
            topic(command, Some(subcommand))
        }
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
    if matches!(strings.first(), Some(Some("guide" | "help" | "version"))) {
        let _ = writeln!(
            errors,
            "error: unknown help topic or unsupported arguments\n\n{GENERAL}"
        );
        return Some(2);
    }
    None
}

/// Offer only a unique close spelling; never change the command being executed.
pub fn suggestion(input: &str) -> Option<&'static str> {
    if !input.is_ascii() || input.len() < 3 || input.len() > 20 {
        return None;
    }
    let mut best = None;
    let mut distance = 3;
    for candidate in [
        "exec", "run", "compile", "build", "verify", "check", "tokens", "inspect", "emit-c",
        "guide", "help", "version",
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
