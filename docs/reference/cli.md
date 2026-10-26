# Command-line reference

The development CLI uses the specification's canonical names. The supported
command groups are `exec`, `compile`, `verify`, `inspect`, `guide`, and `version`.
Other proposed groups remain planned and are absent from production help.
Main and the published v0.0.1/v0.0.2 releases retain their historical interfaces.

## Supported commands

| Invocation | Behavior |
| --- | --- |
| `scarp exec <path>` | Execute the selected source path after static checks and checked evaluation |
| `scarp compile <path> -o <output>` | Compile the supported subset into a native executable |
| `scarp compile --emit obj <path> -o <output>` | Produce a freestanding object |
| `scarp compile --emit c <path> -o <output.c>` | Write a freestanding C11 unit without a C compiler |
| `scarp verify <path>` | Check syntax, names, mutability, and types without execution |
| `scarp inspect tokens <path>` | Show lexer tokens, byte spans, and source locations |
| `scarp inspect ast <path>` | Show actual parsed syntax without name resolution or execution |
| `scarp guide [command]` | Show global or contextual guidance |
| `scarp guide inspect ast` | Show nested inspection guidance |
| `scarp version` | Show the package version |

Bare `scarp`, `--help`, and `-h` show guidance and return 0. `--version` and `-V`
print the same version as `version`. Every implemented group accepts contextual
help; `scarp inspect ast --help` and `scarp guide inspect ast` agree.
Help/version operations require no source file, C compiler, or network access.

From a checkout:

```sh
cargo run -- exec examples/hello.srp
cargo run -- verify examples/scopes.srp
cargo run -- inspect tokens examples/arithmetic.srp
cargo run -- guide compile
```

`cargo build` builds the Rust implementation. Scarp's own `compile` command
produces `.srp` artifacts. It requires a fresh explicit output path and an existing
parent directory. See [native compilation](../proposals/0011-native-build-cli.md)
for compiler selection, supported kinds, targets, runtime budgets, and filesystem
publication behavior. Executables/objects require a Clang/GCC-style compiler;
`--emit c` does not. C output rejects explicit compiler, target, and runtime-budget
options instead of silently ignoring them. Successful compilation is silent and
never executes the artifact automatically.

## Execution, verification, and inspection

`exec` checks the complete source before evaluation, then prepares selected
prints before writing output. Runtime overflow, invalid shifts, zero divisors,
shape/index failures, and exhausted budgets therefore leave stdout empty.
It accepts `--max-steps <count>` and `--max-prints <count>` before the filename.
Defaults are 1,000,000 steps and 100,000 selected prints. Each option can occur
once; unsigned decimal zero is valid. Malformed options return usage status 2.

`verify` stops after lexing, parsing, and resolution/type checking. It evaluates
no arithmetic, initializers, print arguments, loops, or branches and creates no
artifacts. Well-typed division by zero or an infinite loop can pass `verify`
and fail `exec`. Warnings, strict mode, JSON output, and project discovery remain
unimplemented and their flags are rejected.

`inspect tokens` performs only lexing. Its output has one token per line with
one-based line/column ranges, decoded/escaped token values, and half-open byte
spans. The hello example begins with:

```text
2:1..2:8  identifier(println)  [26..33]
```

EOF is an empty span after all source bytes. Columns count Unicode scalar values,
not terminal cells; see [diagnostics](diagnostics.md). A token listing may succeed
for incomplete syntax. `inspect ast` parses without resolving names or evaluating
arithmetic and prints the actual tree in readable Debug format. This format is
experimental and is not a stable JSON schema.

## Paths and separators

Quote paths containing spaces or non-ASCII names. A standalone `--` introduces
one filename beginning with a dash:

```sh
scarp exec -- -hello.srp
scarp verify -- -hello.srp
scarp inspect tokens -- -hello.srp
scarp compile --emit c -o program.c -- -hello.srp
```

Source paths retain native operating-system encoding where the filesystem
supports it. Source contents must be UTF-8; a leading UTF-8 BOM, LF, and CRLF work.
Missing/unreadable files, directories passed as files, and invalid UTF-8 source
produce status 1. The `.srp` suffix is a convention, not an extension check.
There is no stdin source mode, program-argument API, implicit project entrypoint,
watch mode, or import search path. Extra filenames and program arguments fail.

## Migration and exit statuses

Optional compatibility aliases are not enabled, following the maintainer's
canonical-name requirement. Retired names return status 2 and migration guidance;
they never dispatch to another operation or create artifacts.

| Retired spelling | Current command |
| --- | --- |
| `scarp run` | `scarp exec` |
| `scarp build` | `scarp compile` |
| `scarp check` | `scarp verify` for static checks; `scarp exec` for execution |
| `scarp tokens` | `scarp inspect tokens` |
| `scarp emit-c` | `scarp compile --emit c -o <output.c>` |
| `scarp help` | `scarp guide` |

The old CLI `check` evaluated arithmetic; `verify` is static. Rust library APIs
`run`, `check`, and `interpreter::validate` remain available with their established
semantics. Historical release notes describe their own shipped command names.

| Result | stdout | stderr | Exit |
| --- | --- | --- | --- |
| Successful execution | Program output | Empty | 0 |
| Successful verification/compilation | Empty | Empty | 0 |
| Guidance, version, or inspection | Requested report | Empty | 0 |
| Source, file, compiler, or output failure | No program output on source failure | Diagnostic | 1 |
| Invalid/unsupported command or option | Empty | Error and usage | 2 |

Output failures can leave earlier bytes. A broken CLI output pipe is treated as
normal termination; library callers receive the output error. Commands do not
install packages, publish releases, or contact registries. The
[46-group inventory](../cli-status.md) records the remaining compiler and project
prerequisites.
