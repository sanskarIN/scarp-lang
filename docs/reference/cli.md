# Command-line reference

Use this page to select a command, interpret its streams and status, and pass
paths reliably. Availability differs between the published release and the
development stack; see [compatibility](../compatibility.md).

## Commands and availability

| Invocation | Purpose | v0.0.1 | v0.0.2 through beta.2 |
| --- | --- | --- | --- |
| `scarp run <path>` | Read, validate, and execute a source file | Yes | Yes |
| `scarp tokens <path>` | Print lexer tokens without parsing | Yes | Yes |
| `scarp check <path>` | Validate names, types, and arithmetic without printing | No | Yes |
| `scarp --help` or `scarp -h` | Show usage | Yes | Yes |
| `scarp --version` or `scarp -V` | Print the package version | Yes | Yes |
| `scarp run --help` | Show usage before requiring a file | No | Yes |

The `tokens` and `check` commands also accept `--help` and `-h` after the
command name. Only one source path is accepted. There is no stdin input mode,
watch mode, import path, or `scarp build` command.
`exec`, `verify`, `inspect`, and `compile` are not accepted commands in beta.2;
proposed CLI designs do not change the shipped command contract.

v0.0.3 keeps these spellings and adds contextual guidance for run/check/tokens.
Run/check help shows budget placement, defaults, and iteration examples. Check
still evaluates selected loops and arithmetic; tokens does not execute code.

From a source checkout, place Cargo's separator before the Scarp arguments:

```sh
cargo run -- run examples/hello.srp
cargo run -- tokens examples/hello.srp
cargo run -- check examples/scopes.srp
```

`cargo build` builds the Rust interpreter. It does not compile a `.srp` file to a
native executable.

## What each file command validates

`run` validates the complete file before writing program output. Beta.2
validation includes names, scopes, mutability, and primitive types. Arithmetic
is checked before any pending prints are written.

`check` uses the same preparation pass as execution, discarding each print value
immediately instead of retaining pending output. It evaluates declarations and assignments in fresh local storage, so
reachable unused initializers can still fail. Unselected conditional bodies do
not evaluate arithmetic, but they still undergo static checks. A successful check is silent. It does not
format or write the program's print values.

`tokens` only checks lexical validity. It can succeed when a file has missing
punctuation, unknown variables, or arithmetic failures. It never executes the
program. Use `check` rather than a token listing to validate development source.

## Paths and encoding

Since beta.1, `run` and `check` accept `--max-steps <count>` and `--max-prints <count>`
before the path. Defaults are 1,000,000 work steps and 100,000 selected prints.
Each option can occur once; unsigned decimal zero is allowed. Budget failures use
status 1 and leave output empty; malformed options use status 2. See
[loops and budgets](../proposals/0006-bounded-loops.md) for exact accounting.

The [budget guide](../budgets.md) provides copy-paste invocations and explains
zero limits, E0017/E0018, and embedding APIs.

Relative paths are resolved from the process working directory. Quote paths
containing spaces:

```sh
scarp run "my examples/hello.srp"
```

Use an explicit separator for a filename beginning with a dash:

```sh
scarp run -- -hello.srp
scarp tokens -- -hello.srp
scarp check -- -hello.srp
```

The last command requires v0.0.2 or later. The separator belongs after the
Scarp command; when using Cargo, that means `cargo run -- check -- -hello.srp`.

Paths use the operating system's native encoding. Source contents must be UTF-8.
A single leading UTF-8 byte order mark is accepted. LF and CRLF files work.
Missing files, directories used as files, unreadable files, and invalid UTF-8
contents produce read errors with a nonzero status. The `.srp` extension is a
convention, not a file-extension check.

## Standard streams and status

| Result | stdout | stderr | Exit code |
| --- | --- | --- | --- |
| Successful run | Selected stdout output | Selected `eprint`/`ewrite` output | `0` |
| Successful check | Empty | Empty | `0` |
| Successful token listing | Tokens | Empty | `0` |
| Help or version | Requested information | Empty | `0` |
| Source, evaluation, or budget error | Empty | Diagnostic, without user-program stderr | `1` |
| File-read error | Empty | Read error | `1` |
| Output I/O error | Earlier bytes may remain | Earlier user bytes and/or an error if writable | `1` except broken pipe |
| Invalid command-line usage | Empty | Error and usage | `2` |

Output I/O failures can occur after some bytes have been written. The CLI treats
a broken output pipe as normal termination; library callers receive the output
error. Source and semantic errors do not produce partial program output.
An `eprint` message is user-program output, not automatically a failure:
inspect the exit code to distinguish it from a diagnostic. Successful `check`
suppresses both stdout and user-program stderr. Budget flags are rejected by
`tokens`; it performs no execution to budget.

In a POSIX shell, inspect the previous command's status with `echo $?`. In
PowerShell, inspect `$LASTEXITCODE`. Read stderr as well as the status when
diagnosing a failure.

## Token format

The hello example starts with this token:

```text
2:1..2:6  identifier(print)  [27..32]
```

The first range is one-based line/column information. The bracketed range is a
half-open UTF-8 byte span in the original source. String tokens show decoded
values using escaped debugging text. EOF is an empty span after all source
bytes, including trailing comments or whitespace.

Columns count Unicode scalar values rather than bytes or terminal cells. See
the [diagnostic reference](diagnostics.md) for caret interpretation and the
[troubleshooting guide](../troubleshooting.md) for version/path problems.

Development output statements can write to stdout or stderr; see the
[output guide](../output.md). Successful `check` stays silent on both streams.
