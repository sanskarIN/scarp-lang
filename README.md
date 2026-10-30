# Scarp

**Build Beyond Boundaries.**

Scarp is an experimental programming language implemented in Rust. This checkout
is the 0.0.4-alpha.1 release candidate, with finite binary64 floats, a lexer, parser,
name resolver, interpreter, and command-line interface. Syntax and library APIs
may change.

Use immutable `bind` and mutable `var` declarations, checked integers, finite floats, strings,
booleans, conditionals, while/repeat loops, and explicit output streams.
[Typed `funcra` functions](docs/functions.md) support named/default arguments,
recursion, unit returns, purity, contracts, and bounded heap call frames.
See the [migration guide](docs/syntax-migration.md) for older declaration syntax.

```scarp
// My first Scarp program
print("Hello from Scarp!");
```

```text
Hello from Scarp!
```

The [documentation hub](docs/README.md) provides setup, tutorials, command and
diagnostic references, Rust embedding, testing, troubleshooting, and compatibility
guides. Start there for a reading path matched to your task.

The [finite-float guide](docs/floats.md) describes the alpha candidate in this
checkout: decimal/scientific literals, checked finite arithmetic, and float
function parameters/results. Published beta.3 binaries remain integer/string/
Boolean only. The [alpha candidate notes](docs/releases/v0.0.4-alpha.1.md)
describe the new scope; no public alpha tag or release is created by preparation.

## Build and run

Binary archives and checksums are available through
[GitHub Releases](https://github.com/sanskarIN/scarp-lang/releases). See the
[v0.0.3-beta.3 notes](docs/releases/v0.0.3-beta.3.md) for platform support, verification, and
running an extracted package. The [changelog](CHANGELOG.md) records this version.

To build from source:

Install a stable Rust toolchain with Cargo using the instructions at
[rustup.rs](https://rustup.rs/). On Windows, use the MSVC toolchain and install the
C++ build tools if the Rust installer requests them.

```sh
git clone https://github.com/sanskarIN/scarp-lang.git
cd scarp-lang
git switch main
cargo build --locked
cargo run -- run examples/hello.srp
cargo run -- run examples/arithmetic.srp
cargo run -- tokens examples/hello.srp
cargo run -- check examples/scopes.srp
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

- `print("text");` statements, executed in source order.
- Multiple statements, empty strings, and empty or comment-only files.
- Signed 64-bit integers with `+`, `-`, `*`, `/`, unary minus, and parentheses.
- Operator precedence, left-associative binary operators, and checked arithmetic.
- Immutable integer/string/boolean bindings with `bind name = expression;` and resolved names.
- Explicit `var` bindings, assignment, and checked compound assignment.
- Lexical block scopes, shadowing, and nearest-binding assignment.
- Inferred primitive types, checked same-type assignment, and string value snapshots.
- Boolean comparisons and short-circuit `and`, `or`, and `not` expressions.
- Scoped `if`/`elif`/`else` statements with explicit boolean conditions.
- `while` and counted `repeat` loops, nearest-loop `break`/`continue`.
- Typed file-level `funcra`, positional/named/default arguments, forward and
  mutual recursion, explicit returns, verified purity, and executable contracts.
- Heap call frames and six deterministic CLI/embedding resource limits shared
  by `run` and `check`; see [budgets](docs/budgets.md).
- Unicode text in strings and comments, Unicode whitespace, and LF or CRLF files.
- Optional UTF-8 byte order mark at the very start of a file.
- `//` comments to the end of the line, and flexible whitespace between tokens.
- Escapes `\\` (backslash), `\"` (double quote), `\n` (LF), `\t` (tab), and `\r` (CR).
- `run`, `check`, `tokens`, `--help`/`-h`, and `--version`/`-V` commands.
- Syntax errors with a code, filename, line, column, source excerpt, and help.

`print` appends a newline; `write` omits it. `println` remains a supported alias.
Both accept zero or more comma-separated values. `eprint` and `ewrite` use stderr.
See the [output guide](docs/output.md) for exact bytes and embedding APIs. Physical line breaks inside a string are rejected;
use `\n` instead. Identifiers use ASCII letters, digits, and underscores, cannot
start with a digit, and are case-sensitive. Statements include declarations,
assignments, blocks, `if`/`elif`/`else` decisions, and `print` calls. Escapes other than the five listed
above are rejected.

The entire file is lexed, parsed, resolved, checked for primitive type consistency,
and its selected execution path is evaluated before any program output is written. Source and semantic errors
therefore produce no program output.
Output stream failures propagate as errors; an output
failure can occur after earlier statements have been written. A closed output
pipe is treated as normal CLI termination.

```scarp
print(1 + 2 * 3);           // 7
print((1 + 2) * 3);         // 9
print(-7 / 2);              // -3, division truncates toward zero
print(-9223372036854775808); // the minimum signed 64-bit integer
```

Integer overflow, division by zero, and out-of-range literals are errors in both
debug and release builds. Grouping is limited to 64 nested parentheses; long
binary chains use a flat syntax tree. Strings can be bound, copied, reassigned
with the same type, and printed. Concatenation and mixed string/integer arithmetic
are unsupported.

Booleans use `true` and `false`. Comparisons support same-type `==` and `!=`,
and same-type numeric `<`, `<=`, `>`, `>=`. Readable `not`, `and`, and `or` operators require
booleans; `and` and `or` short-circuit at runtime while all operands are checked
for names and types during compilation. See the
[boolean design](docs/proposals/0004-boolean-expressions.md).
Conditional statements select the first true `if`/`elif` body, or the optional
`else` body. Every condition and body undergoes static checks; arithmetic executes
only on the selected path. Start with the [decisions tutorial](docs/tutorials/decisions.md).

`check` validates syntax, names, mutability, scopes, primitive types, and arithmetic
without writing program output. It returns zero and remains silent on success.
It evaluates selected loops, declarations, and assignments in fresh local
storage, so reachable unused arithmetic failures are still reported. The Rust
library's `scarp::compile` only parses and resolves; it does not evaluate arithmetic.
There is no `scarp compile` CLI command in beta.3.

`tokens` checks lexical validity without parsing or executing the file. It prints
one token per line, including decoded string text, one-based line/column ranges,
and half-open UTF-8 byte ranges. For the hello example, the first token is:

```text
2:1..2:6  identifier(print)  [27..32]
```

An end-of-file token is always included. Columns count Unicode scalar values;
terminal display widths for combining marks and wide characters can differ.
Tabs are expanded to four spaces in error excerpts.

```text
error[E0004]: expected ';', found <eof>
 --> example.srp:1:15
  |
1 | print("hello")
  |               ^
 = help: use print("text"); or print(1 + 2);
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
scarp::run(r#"print("Hello from Scarp!");"#, &mut output).unwrap();
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

While loops and `break`/`continue` support iterative computations with explicit
per-call budgets. See [loops and limits](docs/proposals/0006-bounded-loops.md).

The [performance guide](docs/performance.md) provides stage benchmarks and
measured execution improvements. C/C++/Rust performance comparisons and native
code generation remain future work.

This is a small interpreter. It reads a complete file into memory
and reports the first error. Functions are interpreted using heap call frames.
It has no collections, user-defined types, imports, standard library,
package manager, native code
generation, or self-hosting support. It does not execute arbitrary Rust code.
The syntax and library API are experimental and may change.

Read the [implemented language reference](docs/language.md) and
[architecture notes](docs/architecture.md) for precise rules. Runnable programs
in [`examples/`](examples/) are paired with `.stdout` files and checked by the
test suite.

Future work is **planned**, in this order:

1. Finite float stabilization, then numerical collections.
2. Function tooling and further diagnostic improvements.
3. User-defined types and callable values.
4. A compiler backend.
5. Developer tooling.
6. Domain-specific libraries.

The remaining capabilities require separate milestones. Integer expressions are implemented as
specified in the [expression design](docs/proposals/0001-integer-expressions.md).
Integer, string, and boolean bindings are implemented on main. Read the
[variable reference](docs/variables.md) and [staged variable design](docs/proposals/0002-variables.md).
The [v0.0.3-beta.3 notes](docs/releases/v0.0.3-beta.3.md) describe the integrated variable
milestones, downloads, and limitations. Experimental numerical/native features
remain separate PRs and
require an explicit integration decision.

The [function integration in PR #41](https://github.com/sanskarIN/scarp-lang/pull/41)
integrated typed calls, defaults, contracts, and bounded heap frames into main.
Beta.3 publishes that work with CLI resource controls. Beta.2 binaries do not
contain functions. Release notes
describe immutable snapshots; branch names can move.

## License

Original project code is licensed under the [Apache License 2.0](LICENSE).
