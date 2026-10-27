# Output statements

Development main uses `print` for ordinary output and `write` when the next
statement should continue on the same line. Published v0.0.3-beta.1 and earlier
executables use the original single-argument `println` syntax. Check the version
of the executable you are using; this change does not publish a release.

| Statement | Destination | Adds LF |
| --- | --- | --- |
| `print(...)` | stdout | Yes |
| `println(...)` | stdout | Yes, permanent alias of `print` |
| `write(...)` | stdout | No |
| `eprint(...)` | stderr | Yes |
| `ewrite(...)` | stderr | No |

```scarp
bind language = "Scarp";
var version = 3;
write("Hello, ");
print(language, version, true,);
print();
write("done");
```

The exact stdout is `Hello, Scarp 3 true\n\ndone`, with no final newline.
Arguments evaluate left to right and use their normal string, integer, or
boolean representation. One ASCII space separates adjacent arguments, even
when an argument is an empty string. No automatic space precedes the first
argument or follows the last. A trailing comma is optional; semicolons are
required. Commas separate output arguments and are not general expressions.

`print();` and `print("");` each emit one LF byte. `write();` and `write("");`
emit no bytes. Embedded decoded newlines remain intact; `print("x\n");` emits
two LF bytes. Generated line endings are LF on every supported platform. Unicode
and embedded string bytes are preserved without normalization. Output names are
case-sensitive, reserved binding names, and statement forms rather than callable
expression functions. Interpolation and custom formatting remain proposals.

## Validation, loops, and limits

The entire program is checked for syntax, names, and types. Only the selected
control-flow path is evaluated. That path prepares output snapshots before
writing either stream. A later reachable arithmetic error, invalid repeat count,
or exhausted budget prevents all program output, including stderr. Later
mutations cannot change an earlier captured value. `scarp check` evaluates the
same selected path and limits silently, without retaining output events.

`--max-prints` counts selected output statements across both streams, including
empty raw writes. Each additional argument separator consumes one work step;
expression work uses the existing step budget. These limits do not cap the byte
length of a single string. See [budgets](budgets.md).

The CLI directs `eprint`/`ewrite` to stderr without diagnostic prefixes. Source
diagnostics also use stderr, but source/evaluation errors occur before program
output is replayed. I/O failures can happen after earlier bytes were accepted;
those bytes cannot be rolled back. Broken pipes retain the CLI's existing normal
termination behavior.

## Rust embedding

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    scarp::run_with_io(
        "write(\"ready\"); eprint(\"warning\", 2);",
        &mut stdout,
        &mut stderr,
    )?;
    assert_eq!(stdout, b"ready");
    assert_eq!(stderr, b"warning 2\n");
    Ok(())
}
```

`run_with_io_and_limits` accepts `ExecutionLimits`. Compiled programs offer
`interpreter::execute_with_io` and `execute_with_io_and_limits`. The library uses
only the supplied writers. It flushes the previous writer when replay switches
between nonempty streams; callers own final flushing. Write and boundary-flush
failures propagate as `RunError::Output`.

The existing single-writer APIs keep their signatures and stdout behavior. A
selected nonempty stderr event returns an `Unsupported` output error before any
stdout is written, with help pointing to the dual-writer APIs. Unselected stderr
and zero-byte raw stderr writes need no destination. Validation never requires
writers. Evaluation diagnostics take precedence over a missing stderr writer.

Syntax inspection retains `Statement::Println` for single-argument stdout line
forms, including `print`. Other forms use the additive `Statement::Output` with
arguments, `OutputTarget`, newline flag, and whole-statement span. Rust exhaustive
matches must handle the new variant. The lexer adds `TokenKind::Comma` with
source byte spans; output parentheses do not count toward expression depth.
