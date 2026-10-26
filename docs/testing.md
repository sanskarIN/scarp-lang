# Test and validate changes

Use this guide in a source checkout to select the smallest meaningful test for a
change and then run the project's required checks. Binary archives contain
examples and documentation, but omit the Rust sources and test suite.

## Complete checks

On a machine with Rust, rustfmt, and Clippy installed:

```sh
sh scripts/check.sh --offline
```

Or in PowerShell:

```powershell
./scripts/check.ps1 -Offline
```

The scripts run formatting, Clippy with warnings denied, debug and release
tests, builds, rustdoc, and example commands. They stop at the first failure.
The offline option is useful because the current project has no third-party
Rust dependencies. It does not install a toolchain or missing components.

The individual commands remain available:

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo test --locked --release
cargo build --locked
cargo doc --locked --no-deps
```

## Choose the right layer

| Change | Relevant coverage |
| --- | --- |
| Token spelling, escapes, or byte spans | `tests/front_end.rs`, `tests/expressions.rs` |
| Declaration ordering and name resolution | `tests/bindings.rs` |
| `bind`/`var`, legacy migration errors, and keyword byte spans | `tests/binding_syntax.rs` |
| Assignment and state isolation | `tests/mutability.rs` |
| Shadowing, scope exit, and nesting | `tests/scopes.rs` |
| String snapshots and inferred types | `tests/string_bindings.rs` |
| Boolean types, comparisons, and skipped arithmetic | `tests/booleans.rs` |
| Branch selection, scoped bodies, and unreachable static errors | `tests/conditionals.rs` |
| Validation and incremental locations | `tests/validation.rs` |
| Arguments, paths, streams, and exit codes | `tests/cli.rs` |
| Output bytes, alias parity, dual writers, and I/O/flush failures | `tests/output.rs`, `tests/cli_foundation.rs` |
| Native dual-host API, stream prevalidation, and output parity | `tests/c_backend.rs`, `tests/compile_cli.rs` |
| Generated programs and malformed input | `tests/conformance.rs` |
| Shipped source/output pairs | `tests/examples.rs` |

For example, `cargo test --test bindings` runs one integration test target.
`cargo test --test bindings duplicate_declarations_include_the_original_location`
filters that target to the named case. Use a focused test while fixing a failure,
then run the complete checks for a finished change.

## Add a runnable example

Create a `.srp` file and a sibling `.stdout` file under `examples/`. Derive the
expected output from the intended semantics rather than copying an unchecked
implementation result. For example:

```scarp
println("tested");
println(2 + 2);
```

Output:

```text
tested
4
```

The expected file contains the two output lines and their final LF. Save it as
UTF-8 with LF line endings. Empty strings, embedded `\n`, and final newlines are
significant bytes. Cargo progress messages are stderr, not expected program output.
Raw `write` output may intentionally lack a final LF: `output_basics.stdout`
ends with `Tail` and no newline. Package checks capture stdout/stderr as raw
bytes and compare exact fixtures, preserving that distinction across platforms.

The example test discovers every `.srp` file, executes it, compares exact bytes,
and rejects orphaned `.stdout` files. Invalid programs belong in focused negative
tests rather than the runnable example directory.

## Test failures as observable behavior

A useful language regression test identifies the error kind and meaningful span,
and checks that invalid programs leave output empty. A CLI test should additionally
check the exit code and stderr. Avoid comparing a complete operating-system file
error sentence when its exact wording is not part of Scarp's contract.

Test integer boundaries in both build modes. Test scoped name lookup with
shadowing and scope exit, not only a declaration followed by one print. For
mutable values, capture a print before assignment and verify that it retains the
earlier value. Keep temporary files unique so parallel tests cannot collide.

Generated-program tests use fixed seeds and an independent value model.
Mutation tests insert, delete, or truncate source and alter public token streams;
they check panic handling and diagnostic boundaries. Preserve deterministic
reproductions when a generated case fails.

## Check documentation and packages

`cargo doc` checks Rustdoc; it does not automatically compile code fences in
Markdown guides. Run new Scarp examples with the intended version, check their
expected output, and compile complete Rust embedding examples against the
documented checkout. Negative examples should produce their stated code.

Package CI builds native archives, extracts them, and smoke-tests the delivered
executables. That complements the language suite; it does not replace it.
An uploaded development artifact is not a published prerelease. See
[the release process](releasing.md) for the separate publication gate.

For execution changes, use `cargo bench --locked --offline --bench runtime` and
follow the [performance guide](performance.md). The quick workload smoke run is
checked on CI, including minimum Rust. Timing thresholds are not correctness tests.
