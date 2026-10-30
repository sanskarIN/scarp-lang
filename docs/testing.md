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
| Assignment and state isolation | `tests/mutability.rs` |
| Shadowing, scope exit, and nesting | `tests/scopes.rs` |
| String snapshots and inferred types | `tests/string_bindings.rs` |
| Boolean types, comparisons, and skipped arithmetic | `tests/booleans.rs` |
| Branch selection, scoped bodies, and unreachable static errors | `tests/conditionals.rs` |
| Validation and incremental locations | `tests/validation.rs` |
| Arguments, paths, streams, and exit codes | `tests/cli.rs` |
| Generated programs and malformed input | `tests/conformance.rs` |
| Shipped source/output pairs | `tests/examples.rs` |
| Output destinations, spacing, snapshot/replay failures | `tests/output.rs` |
| While/repeat control, count snapshots, and budgets | `tests/loops.rs`, `tests/repeat.rs` |
| Incomplete declarations/assignments and nesting boundaries | `tests/variable_diagnostics.rs` |

For example, `cargo test --test bindings` runs one integration test target.
`cargo test --test bindings duplicate_declarations_include_the_original_location`
filters that target to the named case. Use a focused test while fixing a failure,
then run the complete checks for a finished change.

## Add a runnable example

Create a `.srp` file and a sibling `.stdout` file under `examples/`. Derive the
expected output from the intended semantics rather than copying an unchecked
implementation result. For example:

```scarp
print("tested");
print(2 + 2);
```

Output:

```text
tested
4
```

The expected file contains the two output lines and their final LF. Save it as
UTF-8 with LF line endings. Empty strings, embedded `\n`, and final newlines are
significant bytes. Cargo progress messages are stderr, not expected program output.

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

Iteration tests cover fixed-slot while/repeat execution, count snapshots, mixed
control targets, and parallel reuse. The iteration diagnostic fixtures compare
exact library and run/check errors, including Unicode/CRLF source, guard/count
types, index writes, missing punctuation, and both budgets. The control_escapes example's
stdout is binary so Git preserves its tab/CR bytes. Package checks capture raw
streams and compare every fixture without normalizing line endings.

`cargo doc` checks Rustdoc; it does not automatically compile code fences in
Markdown guides. Run new Scarp examples with the intended version, check their
expected output, and compile complete Rust embedding examples against the
documented checkout. Negative examples should produce their stated code.
To validate Markdown changes, build the intended checkout first with
`cargo build --locked --offline`; a stale `target/debug/libscarp.rlib` may belong
to another branch. Check each complete `scarp` fence with that executable, run
it to compare any stated stdout/stderr bytes, and confirm every `scarp-invalid`
fence fails with the documented code. Compile and execute complete Rust host
examples against the freshly built library. Resolve local links from the page's
directory, including links in older release notes.

Historical release-note examples use that release's syntax; do not migrate them
to current syntax just to make a current-executable check pass. Validate them
against the tagged source/binary or preserved compatibility fixtures instead.

Package CI builds native archives, extracts them, and smoke-tests the delivered
executables. That complements the language suite; it does not replace it.
An uploaded development artifact is not a published prerelease. See
[the release process](releasing.md) for the separate publication gate.

For execution changes, use `cargo bench --locked --offline --bench runtime` and
follow the [performance guide](performance.md). The quick workload smoke run is
checked on CI, including minimum Rust. Timing thresholds are not correctness tests.

The v0.0.2 example manifest locks all 22 original source/output files to their
published bytes. The Python release-verifier suite checks those digests; Cargo
and package discovery execute every example, including comments/unicode samples.

## Numerical vector validation

The vector suites test literals/indexing, finite host invariants, immutable
sharing, every kernel, malformed/deep/Unicode source, recursive/named/default
calls, purity/contracts, concurrent reuse and all six exact resource boundaries.
Fraction-model reductions use independently accumulated integer numerators.
`tests/fixtures/softmax-reference.csv` contains 37 probability cases generated
with Python Decimal.exp at precision 60; Rust tests use numerical tolerances.
Mean/MSE regressions cover representable results whose naive intermediates
overflow, plus cancellation, signed zero, subnormals and underflow.
Maximum block/group/call combinations additionally parse on an explicit 512 KiB
thread and resolve/execute on the normal worker, preserving existing depth tests.

```sh
cargo test --locked --offline --test vectors --test vector_functions
cargo bench --locked --offline --bench numerics -- --quick
```

The new benchmark smoke workloads run on stable native hosts and Rust 1.74,
with outputs verified before timing. Full performance records and raw repeated
runs are in [performance](performance.md#numerical-vector-measurements).
Native archives run all 23 exact-output examples and vector stream/failure probes.
