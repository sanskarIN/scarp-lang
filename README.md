# Scarp

**Build Beyond Boundaries.**

This development branch adds variables for the planned v0.0.2 milestone. The
latest published version is v0.0.1; its packages do not contain these additions.

Scarp is an experimental programming language implemented in Rust. This repository
contains the **v0.0.1 experimental prerelease**: a working lexer, parser,
interpreter, and command-line interface. Syntax and library APIs may change.

```scarp
// My first Scarp program
println("Hello from Scarp!");
```

```text
Hello from Scarp!
```

## Build and run

Binary archives and checksums are available through
[GitHub Releases](https://github.com/sanskarIN/scarp-lang/releases). See the
[v0.0.1 notes](docs/releases/v0.0.1.md) for platform support, verification, and
running an extracted package. The [changelog](CHANGELOG.md) records this version.

To build from source:

Install a stable Rust toolchain with Cargo using the instructions at
[rustup.rs](https://rustup.rs/). On Windows, use the MSVC toolchain and install the
C++ build tools if the Rust installer requests them.

```sh
git clone https://github.com/sanskarIN/scarp-lang.git
cd scarp-lang
cargo build --locked
cargo run -- run examples/hello.srp
cargo run -- run examples/arithmetic.srp
cargo run -- tokens examples/hello.srp
cargo run -- --help
cargo run -- --version
```

The project has no third-party Rust dependencies. After installing Rust, the
prototype can be built and tested offline with `cargo build --offline` and
`cargo test --offline`.

To install the command from this checkout:

```sh
cargo install --path . --locked
scarp run examples/hello.srp
scarp tokens examples/hello.srp
```

Filenames with spaces should be quoted. Use `scarp run -- -example.srp` for a
filename beginning with a dash. Source paths use the platform's native path
encoding; source contents must be UTF-8. The `.srp` suffix is conventional, and
the CLI does not reject other filename extensions.

## What works now

- `println("text");` statements, executed in source order.
- Multiple statements, empty strings, and empty or comment-only files.
- Signed 64-bit integers with `+`, `-`, `*`, `/`, unary minus, and parentheses.
- Operator precedence, left-associative binary operators, and checked arithmetic.
- Immutable integer bindings with `let name = expression;` and resolved names.
- Explicit `let mut` bindings, assignment, and checked compound assignment.
- Unicode text in strings and comments, Unicode whitespace, and LF or CRLF files.
- Optional UTF-8 byte order mark at the very start of a file.
- `//` comments to the end of the line, and flexible whitespace between tokens.
- Escapes `\\` (backslash), `\"` (double quote), and `\n` (line break).
- `run`, `tokens`, `--help`/`-h`, and `--version`/`-V` commands.
- Syntax errors with a code, filename, line, column, source excerpt, and help.

`println` appends a newline. Physical line breaks inside a string are rejected;
use `\n` instead. Identifiers use ASCII letters, digits, and underscores, cannot
start with a digit, and are case-sensitive. Only `println` is recognized as a
statement name. Escapes other than the three listed above are rejected.

The entire file is lexed, parsed, and evaluated before any program output is
written. Syntax and arithmetic errors therefore produce no program output.
Output stream failures propagate as errors; an output
failure can occur after earlier statements have been written. A closed output
pipe is treated as normal CLI termination.

```scarp
println(1 + 2 * 3);           // 7
println((1 + 2) * 3);         // 9
println(-7 / 2);              // -3, division truncates toward zero
println(-9223372036854775808); // the minimum signed 64-bit integer
```

Integer overflow, division by zero, and out-of-range literals are errors in both
debug and release builds. Grouping is limited to 64 nested parentheses; long
binary chains use a flat syntax tree. Strings remain direct print arguments,
with no concatenation or mixed string/integer arithmetic.

`tokens` checks lexical validity without parsing or executing the file. It prints
one token per line, including decoded string text, one-based line/column ranges,
and half-open UTF-8 byte ranges. For the hello example, the first token is:

```text
2:1..2:8  identifier(println)  [26..33]
```

An end-of-file token is always included. Columns count Unicode scalar values;
terminal display widths for combining marks and wide characters can differ.
Tabs are expanded to four spaces in error excerpts.

```text
error[E0004]: expected ';', found <eof>
 --> example.srp:1:17
  |
1 | println("hello")
  |                 ^
 = help: use println("text"); or println(1 + 2);
```

Exit codes are `0` for success, `1` for source, file, or output errors, and `2` for
invalid command-line usage. Diagnostics go to standard error; program output and
token listings go to standard output.

## Library

The `scarp` library exposes `lexer::lex`, `parser::parse`, `compile`,
`evaluator::evaluate`, `interpreter::execute`, and `run`. `parse` exposes syntax;
`compile` validates syntax and resolves names into an immutable `ResolvedProgram`.
`evaluator::evaluate` checks standalone literal arithmetic. `execute` and `run` validate all
arguments before writing and return `RunError`. The output destination is explicit, making
execution usable without redirecting process-wide standard output.

```rust
let mut output = Vec::new();
scarp::run(r#"println("Hello from Scarp!");"#, &mut output).unwrap();
assert_eq!(output, b"Hello from Scarp!\n");
```

## Development checks

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo test --locked --release
cargo build --locked
cargo run --locked -- run examples/hello.srp
cargo run --locked -- tokens examples/hello.srp
```

Tests cover source spans, escapes, Unicode, missing punctuation, invalid files,
command-line usage, output failures, arithmetic boundaries, precedence, nesting,
long expression chains, and all shipped examples.
The complete check sequence, including rustdoc, is available as
`sh scripts/check.sh --offline` or `./scripts/check.ps1 -Offline`. See
[CONTRIBUTING.md](CONTRIBUTING.md) for the development and PR workflow.

## Prototype limits and planned roadmap

This is a small tree-walking interpreter. It reads a complete file into memory
and reports the first error. It has no user-defined functions,
booleans, floating-point values, types, imports, loops, standard library,
package manager, native code
generation, or self-hosting support. It does not execute arbitrary Rust code.
The syntax and library API are experimental and may change.

Read the [implemented language reference](docs/language.md) and
[architecture notes](docs/architecture.md) for precise rules. Runnable programs
in [`examples/`](examples/) are paired with `.stdout` files and checked by the
test suite.

Future work is **planned**, in this order:

1. Lexical scope and shadowing.
2. User-defined functions.
3. Type checking.
4. A compiler backend.
5. Developer tooling.
6. Domain-specific libraries.

These capabilities are not implemented. Integer expressions are implemented as
specified in the [expression design](docs/proposals/0001-integer-expressions.md).
Immutable integer bindings are implemented on this branch. Read the
[variable reference](docs/variables.md) and [staged variable design](docs/proposals/0002-variables.md).

## License

Original project code is licensed under the [Apache License 2.0](LICENSE).
