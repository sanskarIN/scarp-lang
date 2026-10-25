# Scarp

**Build Beyond Boundaries.**

Scarp is an experimental programming language implemented in Rust. This repository
contains the **v0.0.1 development prototype**: a working lexer, parser, interpreter,
and command-line interface. There is no published release yet.

```scarp
// My first Scarp program
println("Hello from Scarp!");
```

```text
Hello from Scarp!
```

## Build and run

Install a stable Rust toolchain with Cargo using the instructions at
[rustup.rs](https://rustup.rs/). On Windows, use the MSVC toolchain and install the
C++ build tools if the Rust installer requests them.

```sh
git clone https://github.com/sanskarIN/scarp-lang.git
cd scarp-lang
cargo build --locked
cargo run -- run examples/hello.srp
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

The entire file is lexed and parsed before execution. Invalid source therefore
produces no program output. Output stream failures propagate as errors; an output
failure can occur after earlier statements have been written. A closed output
pipe is treated as normal CLI termination.

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
 = help: a print statement has the form println("text");
```

Exit codes are `0` for success, `1` for source, file, or output errors, and `2` for
invalid command-line usage. Diagnostics go to standard error; program output and
token listings go to standard output.

## Library

The `scarp` library exposes `lexer::lex`, `parser::parse`, `compile`,
`interpreter::execute`, and `run`. The output destination is explicit, making
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
cargo build --locked
cargo run --locked -- run examples/hello.srp
cargo run --locked -- tokens examples/hello.srp
```

Tests cover source spans, escapes, Unicode, missing punctuation, invalid files,
command-line usage, output failures, and execution of the real hello example.

## Prototype limits and planned roadmap

This is a small tree-walking interpreter. It reads a complete file into memory
and reports the first error. It has no expressions, variables, user-defined
functions, types, imports, loops, standard library, package manager, native code
generation, or self-hosting support. It does not execute arbitrary Rust code.
The syntax and library API are experimental and may change.

Future work is **planned**, in this order:

1. Expressions and literal values.
2. Variables and lexical scope.
3. User-defined functions.
4. Type checking.
5. A compiler backend.
6. Developer tooling.
7. Domain-specific libraries.

These capabilities are not implemented. The best next small milestone is integer
literal expressions with `+`, `-`, `*`, `/`, parentheses, and focused diagnostics.

## License

Original project code is licensed under the [Apache License 2.0](LICENSE).
