# Command-line reference

Use this page to select a command, interpret its streams and status, and pass
paths reliably. Availability differs between the published release and the
development stack; see [compatibility](../compatibility.md).

## Commands and availability

| Invocation | Purpose | v0.0.1 | v0.0.2 | Development |
| --- | --- | --- | --- | --- |
| `scarp run <path>` | Read, validate, and execute a source file | Yes | Yes | Yes |
| `scarp exec <path>` | Exact alias for file execution with `run` | No | No | Yes |
| `scarp tokens <path>` | Print lexer tokens without parsing | Yes | Yes | Yes |
| `scarp inspect tokens <path>` | Exact alias for the token listing | No | No | Yes |
| `scarp inspect ast <path>` | Print actual parsed syntax without resolution/execution | No | No | Yes |
| `scarp verify <path>` | Check syntax, names, mutability, and types statically | No | No | Yes |
| `scarp check <path>` | Validate names, types, and selected arithmetic without printing | No | Yes | Yes |
| `scarp emit-c <path>` | Emit supported source as a freestanding C11 unit | No | No | Yes |
| `scarp compile <path> -o <output>` | Compile an executable or object with a C compiler | No | No | Yes |
| `scarp build ...` | Exact alias for `compile` | No | No | Yes |
| `scarp guide [command]` / `scarp help ...` | Show global or contextual help | No | No | Yes |
| `scarp --help` or `scarp -h` | Show usage | Yes | Yes | Yes |
| `scarp --version` or `scarp -V` | Print the package version | Yes | Yes | Yes |
| `scarp version` | Exact short-version alias | No | No | Yes |
| `scarp run --help` | Show usage before requiring a file | No | Yes | Yes |

Development commands accept contextual `--help`/`-h`; nested inspection supports
`scarp inspect ast --help` and `scarp guide inspect ast`. Only one source path is
accepted. There is no stdin input mode,
watch mode, or import path.
The separate `emit-c` command generates source without running it or invoking a
C compiler. It accepts one path and an optional filename separator, but no
execution-limit flags. See [the backend guide](../proposals/0010-freestanding-c.md).
`build` and `compile` have their own `--help`, compiler/output options, and
executable runtime budgets. See [native builds](../proposals/0011-native-build-cli.md)
for supported values, targets, exit codes, and output preservation.

From a source checkout, place Cargo's separator before the Scarp arguments:

```sh
cargo run -- run examples/hello.srp
cargo run -- tokens examples/hello.srp
cargo run -- check examples/scopes.srp
```

`cargo build` builds the Rust interpreter. It does not compile a `.srp` file to a
native executable.

## What each file command validates

`exec`/`run` validate the complete file before writing program output. Development
validation includes names, scopes, mutability, and primitive types. Arithmetic
is checked before any pending prints are written.

`check` uses the same preparation pass as execution, discarding each print value
immediately instead of retaining pending output. It evaluates declarations and assignments in fresh local storage, so
reachable unused initializers can still fail. Unselected conditional bodies do
not evaluate arithmetic, but they still undergo static checks. A successful check is silent. It does not
format or write the program's print values.

`verify` stops after lexing, parsing, and resolution/type checking. It does not
evaluate initializers, print arguments, loops, or branches, and creates no build
artifacts. A well-typed division by zero or infinite loop can pass `verify` and
fail `check`/`exec`. It is intentionally not an alias for the existing `check`.

`tokens` only checks lexical validity. It can succeed when a file has missing
punctuation, unknown variables, or arithmetic failures. It never executes the
program. Use `verify` for static checks or `check` for selected runtime validation.
`inspect ast` parses without resolving names or evaluating arithmetic. It emits
the real syntax tree in readable Debug format, which is experimental and is not
a stable JSON schema. Missing names can appear in a valid syntax dump.

## Paths and encoding

Development `exec`, `run`, and `check` accept `--max-steps <count>` and `--max-prints <count>`
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
