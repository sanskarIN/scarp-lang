# Command-line reference

Use this page to select a command, interpret its streams and status, and pass
paths reliably. Availability differs between the published release and the
development stack; see [compatibility](../compatibility.md).

## Commands and availability

| Invocation | Purpose | v0.0.1 | Development |
| --- | --- | --- | --- |
| `scarp run <path>` | Read, validate, and execute a source file | Yes | Yes |
| `scarp tokens <path>` | Print lexer tokens without parsing | Yes | Yes |
| `scarp check <path>` | Validate names, types, and arithmetic without printing | No | Yes |
| `scarp --help` or `scarp -h` | Show usage | Yes | Yes |
| `scarp --version` or `scarp -V` | Print the package version | Yes | Yes |
| `scarp run --help` | Show usage before requiring a file | No | Yes |

Development `tokens` and `check` commands also accept `--help` and `-h` after the
command name. Only one source path is accepted. There is no stdin input mode,
watch mode, import path, or `scarp build` command.

From a source checkout, place Cargo's separator before the Scarp arguments:

```sh
cargo run -- run examples/hello.srp
cargo run -- tokens examples/hello.srp
cargo run -- check examples/scopes.srp
```

`cargo build` builds the Rust interpreter. It does not compile a `.srp` file to a
native executable.

## What each file command validates

`run` validates the complete file before writing program output. Development
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

Development `run` and `check` accept `--max-steps <count>` and `--max-prints <count>`
before the path. Defaults are 1,000,000 work steps and 100,000 selected prints.
Each option can occur once; unsigned decimal zero is allowed. Budget failures use
status 1 and leave output empty; malformed options use status 2. See
[loops and budgets](../proposals/0006-bounded-loops.md) for exact accounting.

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

The last command requires development Scarp. The separator belongs after the
Scarp command; when using Cargo, that means `cargo run -- check -- -hello.srp`.

Paths use the operating system's native encoding. Source contents must be UTF-8.
A single leading UTF-8 byte order mark is accepted. LF and CRLF files work.
Missing files, directories used as files, unreadable files, and invalid UTF-8
contents produce read errors with a nonzero status. The `.srp` extension is a
convention, not a file-extension check.

## Standard streams and status

| Result | stdout | stderr | Exit code |
| --- | --- | --- | --- |
| Successful run | Program output | Empty | `0` |
| Successful check | Empty | Empty | `0` |
| Successful token listing | Tokens | Empty | `0` |
| Help or version | Requested information | Empty | `0` |
| Source, file, or output error | No program output for source errors | Diagnostic | `1` |
| Invalid command-line usage | Empty | Error and usage | `2` |

Output I/O failures can occur after some bytes have been written. The CLI treats
a broken output pipe as normal termination; library callers receive the output
error. Source and semantic errors do not produce partial program output.

In a POSIX shell, inspect the previous command's status with `echo $?`. In
PowerShell, inspect `$LASTEXITCODE`. Read stderr as well as the status when
diagnosing a failure.

## Token format

The hello example starts with this token:

```text
2:1..2:8  identifier(println)  [26..33]
```

The first range is one-based line/column information. The bracketed range is a
half-open UTF-8 byte span in the original source. String tokens show decoded
values using escaped debugging text. EOF is an empty span after all source
bytes, including trailing comments or whitespace.

Columns count Unicode scalar values rather than bytes or terminal cells. See
the [diagnostic reference](diagnostics.md) for caret interpretation and the
[troubleshooting guide](../troubleshooting.md) for version/path problems.
