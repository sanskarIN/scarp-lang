# Output statements and explicit streams

Status: implementation for review, unreleased v0.0.3-dev.
Base: `scarp/bind-var-syntax` at `3449af94910af2b25991a63c42254f99eb5c9667`.

The supplied output brief describes an older main snapshot. This implementation
preserves the current `bind`/`var` syntax, decimals, vectors, built-in numeric
expressions, comma tokens, and canonical CLI. Main and releases are unchanged.

## Language contract

| Spelling | Destination | Additional LF |
| --- | --- | --- |
| `print` | stdout | Yes; preferred spelling |
| `println` | stdout | Yes; permanent equivalent spelling |
| `write` | stdout | No |
| `eprint` | stderr | Yes |
| `ewrite` | stderr | No |

```ebnf
output_statement = output_name, "(", [ argument_list ], ")", ";" ;
output_name = "print" | "println" | "write" | "eprint" | "ewrite" ;
argument_list = expression, { ",", expression }, [ "," ] ;
```

Arguments evaluate left to right using existing value display. Insert exactly
one ASCII space between arguments and, for line forms, one final LF on every
platform. Zero arguments produce just that LF or no bytes for raw forms.
Decoded string newlines and literal braces retain their meaning. Trailing
commas are accepted; empty middle arguments and missing semicolons are errors.
Output parentheses do not add expression depth. Output names are reserved
variable names. No other output aliases or expression-level functions are added.

Each selected output statement counts once against `max_prints`, including an
empty raw write and stderr statements. Every additional argument charges one
work step for its separator, in addition to existing expression/vector work.
Single-argument statement budgets retain their previous meaning.

## Representation and replay

Retain the public single-argument `Statement::Println` shape for old clients.
Both `print` and `println` with one argument use it. Add `Statement::Output`
for zero/multiple arguments, raw writes, and stderr, with `OutputTarget`, a
newline flag, and existing `PrintArgument` values. Public enum additions require
exhaustive Rust matches to handle the new variant; existing constructors and
single-argument matches retain their meaning. All forms lower to one private
resolved output operation. Literal arguments retain the borrowed-string fast path.

The interpreter prepares one flat ordered list of owned/shared captured values
and borrowed literals, including separators and newline flags. It does not
build a concatenated output string or retain a reference to mutable storage.
Validation evaluates arguments but captures/formats/writes nothing. All static,
arithmetic, and budget errors occur before either destination receives bytes.
Once replay begins, I/O failures can leave earlier writes; rollback is impossible.

## Explicit writer API and CLI

Add `run_with_io` / `run_with_io_and_limits` and
`interpreter::execute_with_io` / `execute_with_io_and_limits`, each taking
explicit stdout and stderr writers. Existing one-writer APIs remain. After full
preparation, a one-writer call rejects any selected nonempty stderr output with
`RunError::Output` of kind `Unsupported`, before writing any stdout. Unselected
stderr and zero-byte raw writes do not require a stderr destination. Runtime
diagnostics retain precedence over the missing-destination error.

Dual-writer replay flushes the previous destination when switching between
nonempty streams. Callers retain responsibility for final flushing. This keeps
buffered writes in source order within the process, but the host can still
merge two streams differently. There is no global cross-stream atomicity.
The library never accesses process-global stdout/stderr.

CLI `exec` supplies both real streams. Program stderr is ordinary program text;
compiler/runtime diagnostics still carry their diagnostic prefix and source
location. Broken pipes keep the existing CLI convention. Static `verify` still
does not evaluate arithmetic; Rust `check` evaluates without output.

## Native backend

All stdout forms work in the existing integer/boolean/literal-string subset.
Preserve `scarp_host`, `scarp_run`, and `scarp_validate`. Add `scarp_io` and
`scarp_run_io` with independent host callbacks and an optional stream-boundary
flush callback. The one-host wrapper rejects required stderr before any writes.
The native backend keeps its bounded-workspace validation/replay passes; it
records required destinations during validation without formatting values.
Hosted executables route and flush both streams and retain binary LF on Windows.
Other value kinds remain E0027, rather than being newly claimed as compilable.

## Later formatting

Phase C is design only. A future proposal may specify explicit `f"..."`
interpolation, escaped braces, format specifications, and evaluation rules.
Plain strings remain literal. Formatting functions, logging, macros, implicit
formatting, and new string escapes are outside this change.
