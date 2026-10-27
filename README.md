# Scarp

**Build Beyond Boundaries.**

This development branch adds native build commands and experimental features
beyond the published v0.0.2 variable milestone. Main and v0.0.2 stay unchanged.
This syntax branch uses `bind` for immutable bindings and `var` for mutable
variables. Published v0.0.2 uses `let` / `let mut`; see the
[declaration migration table](docs/compatibility.md#declaration-syntax-migration).

Scarp is an experimental programming language implemented in Rust. This repository
contains **v0.0.3-dev**: a working lexer, parser, name resolver, interpreter,
and command-line interface. Syntax and library APIs may change. This development
version has no v0.0.3 tag or published prerelease.

```scarp
// My first Scarp program
print("Hello, Scarp!");
```

```text
Hello, Scarp!
```

The [documentation hub](docs/README.md) provides setup, tutorials, command and
diagnostic references, Rust embedding, testing, troubleshooting, and compatibility
guides. Start there for a reading path matched to your task.
Use `print` for line output; `println` is a permanent equivalent spelling.
`write("Loading"); print("... done");` inserts no separator between statements.
The [output guide](docs/output.md) covers arguments, bytes, and explicit stderr.
The [function guide](docs/functions.md) covers typed `funcra` declarations,
recursion, named/default arguments, verified `pure` functions, and executable
`needs`/`ensures` contracts. These run in the interpreter; native function
compilation and closures remain unsupported.

## Build and run

Binary archives and checksums are available through
[GitHub Releases](https://github.com/sanskarIN/scarp-lang/releases). See the
[v0.0.2 notes](docs/releases/v0.0.2.md) for platform support, verification, and
running an extracted package. The [changelog](CHANGELOG.md) records this version.

To build from source:

Install a stable Rust toolchain with Cargo using the instructions at
[rustup.rs](https://rustup.rs/). On Windows, use the MSVC toolchain and install the
C++ build tools if the Rust installer requests them.

```sh
git clone https://github.com/sanskarIN/scarp-lang.git
cd scarp-lang
git switch scarp/funcra-function-system
cargo build --locked
cargo run -- exec examples/hello.srp
cargo run -- exec examples/arithmetic.srp
cargo run -- inspect tokens examples/hello.srp
cargo run -- verify examples/scopes.srp
cargo run -- --help
cargo run -- --version
```

The project has no third-party Rust dependencies. After installing Rust, the
prototype can be built and tested offline with `cargo build --offline` and
`cargo test --offline`.

To install the command from this checkout:

```sh
cargo install --path . --locked
scarp exec examples/hello.srp
scarp inspect tokens examples/hello.srp
```

Filenames with spaces should be quoted. Use `scarp exec -- -example.srp` for a
filename beginning with a dash. Source paths use the platform's native path
encoding; source contents must be UTF-8. The `.srp` suffix is conventional, and
the CLI does not reject other filename extensions.

## What works now

- Typed `funcra` functions, explicit returns/unit, forward calls, and bounded recursion.
- Named/default arguments, checked purity/contracts, isolated frames, and static function metadata.
- `print`, permanent `println`, and raw `write`, with zero/multiple arguments and trailing commas.
- Explicit `eprint` / `ewrite` stderr and dual-writer embedding APIs.
- Multiple statements, empty strings, and empty or comment-only files.
- Signed 64-bit integers with `+`, `-`, `*`, `/`, unary minus, and parentheses.
- Operator precedence, left-associative binary operators, and checked arithmetic.
- Immutable integer/float/string/boolean bindings with `bind name = expression;` and resolved names.
- Explicit `var` bindings, assignment, and checked compound assignment.
- Lexical block scopes, shadowing, and nearest-binding assignment.
- Inferred primitive types, checked same-type assignment, and string value snapshots.
- Unicode text in strings and comments, Unicode whitespace, and LF or CRLF files.
- Optional UTF-8 byte order mark at the very start of a file.
- `//` comments to the end of the line, and flexible whitespace between tokens.
- Escapes `\\` (backslash), `\"` (double quote), and `\n` (line break).
- `exec`, static `verify`, and read-only `inspect tokens` / `inspect ast`.
- Contextual `guide`, `version`, and the documented help/version flags.
- Experimental `compile --emit exe|obj|c` for real native artifacts or C source.
- Syntax errors with a code, filename, line, column, source excerpt, and help.

`print` and `println` append one LF; `write` appends none. Arguments use one
ASCII space between values. Physical line breaks inside a string are rejected;
use `\n` instead. Identifiers use ASCII letters, digits, and underscores, cannot
start with a digit, and are case-sensitive. Statements include declarations,
assignments, blocks, `if`/`elif`/`else` decisions, and output statements. Escapes other than the three listed
above are rejected.

The entire file is lexed, parsed, resolved, checked for primitive type consistency,
and its selected execution path is evaluated before any program output is written. Source and semantic errors
therefore produce no program output.
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
binary chains use a flat syntax tree. Strings can be bound, copied, reassigned
with the same type, and printed. Concatenation and mixed string/integer arithmetic
are unsupported.

Booleans use `true` and `false`. Comparisons support same-type `==` and `!=`,
and integer `<`, `<=`, `>`, `>=`. Readable `not`, `and`, and `or` operators require
booleans; `and` and `or` short-circuit at runtime while all operands are checked
for names and types during compilation. See the
[boolean design](docs/proposals/0004-boolean-expressions.md).
Conditional statements select the first true `if`/`elif` body, or the optional
`else` body. Every condition and body undergoes static checks; arithmetic executes
only on the selected path. Start with the [decisions tutorial](docs/tutorials/decisions.md).

`verify` validates syntax, names, mutability, scopes, and primitive types without
execution or output. It returns zero and remains silent on success. Arithmetic,
loops, and execution budgets are evaluated by `exec`; artifact compilation does
not execute the source.

`inspect tokens` checks lexical validity without parsing or executing the file. It prints
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
`check` and `interpreter::validate` also evaluate without an output destination.
`evaluator::evaluate` preserves an integer result, while `evaluate_value` returns
an integer/float/string/boolean `Value`. `execute` and `run` validate all
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
cargo run --locked -- exec examples/hello.srp
cargo run --locked -- inspect tokens examples/hello.srp
```

Tests cover source spans, escapes, Unicode, missing punctuation, invalid files,
command-line usage, output failures, arithmetic boundaries, precedence, nesting,
long expression chains, and all shipped examples.
The complete check sequence, including rustdoc, is available as
`sh scripts/check.sh --offline` or `./scripts/check.ps1 -Offline`. See
[CONTRIBUTING.md](CONTRIBUTING.md) for the development and PR workflow.

## Prototype limits and planned roadmap

The [CLI inventory](docs/cli-status.md) records all 46 proposed command groups
and what is actually implemented. `scarp guide compile` explains native builds;
`scarp guide inspect ast` explains read-only syntax inspection. `verify` stops
after static checks. Only the specification's canonical command names are accepted;
retired aliases return usage errors with migration guidance.

`scarp compile file.srp -o app` compiles a native executable for the supported
integer/boolean subset. A Clang/GCC-style C compiler is required for executables
and objects. Object output supports low-level
embedding. See the [native build guide](docs/proposals/0011-native-build-cli.md).

`scarp compile --emit c file.srp -o program.c` writes a freestanding C11 computation unit
for the integer/boolean subset. A caller supplies fixed workspace and output.
The [backend guide](docs/proposals/0010-freestanding-c.md) covers supported syntax,
object builds, checks, and the systems features still needed for an operating system.

Signed 64-bit masks, hexadecimal/binary literals, remainder, and explicit
arithmetic/logical shifts support bit-field work. See
[integer bit operations](docs/proposals/0009-integer-bit-operations.md).

Immutable float vectors provide native `sum`, `mean`, `dot`, `mse`, `softmax`,
`add`, `scale`, `clamp`, `fill`, and `len` operations. Work is charged to the same
budget as loops. See [numeric vectors](docs/proposals/0008-numeric-vectors.md)
and run `examples/numerics.srp` for checked numerical building blocks.
`examples/linear_fit.srp` combines the kernels and loops to fit a known linear
coefficient with an explicit squared-error gradient and tolerance checks.

While loops and `break`/`continue` support iterative computations with explicit
per-call budgets. See [loops and limits](docs/proposals/0006-bounded-loops.md).

The [performance guide](docs/performance.md) provides stage benchmarks and
measured execution improvements. General native compilation and C/C++/Rust
performance comparisons remain future work.

This is a small interpreter. It reads a complete file into memory
and reports the first error. It has no first-class functions or closures,
user-defined types, imports, general standard library,
package manager, a general native backend, or self-hosting support. It does not execute arbitrary Rust code.
The syntax and library API are experimental and may change.

Read the [implemented language reference](docs/language.md) and
[architecture notes](docs/architecture.md) for precise rules. Runnable programs
in [`examples/`](examples/) are paired with `.stdout` files and checked by the
test suite.

Future work is **planned**, in this order:

1. Review syntax and the safe project-manifest model.
2. Native function lowering, then first-class functions and immutable closures.
3. Modules, user-defined types, and richer numerical data structures.
4. Extend the experimental native backend beyond its current subset.
5. Formatter, editor tooling, and project test runner.
6. Standard-library primitives and domain-specific libraries.

These capabilities are not implemented. Integer expressions are implemented as
specified in the [expression design](docs/proposals/0001-integer-expressions.md).
Immutable integer bindings are implemented on this branch. Read the
[variable reference](docs/variables.md) and [staged variable design](docs/proposals/0002-variables.md).
The [v0.0.2 notes](docs/releases/v0.0.2.md) describe the published variable
milestone. Later numerical, low-level, and native-build features remain on
separate branches until the maintainer requests integration.

## License

Original project code is licensed under the [Apache License 2.0](LICENSE).
