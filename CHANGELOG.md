# Changelog

## 0.0.3-beta.3.dev — 2026-10-27 — prerelease

- Contextual repairs for incomplete bind/var declarations and assignments,
  preserving precise expression errors and output atomicity.

- Canonical `print`, raw `write`, and explicit stderr `eprint`/`ewrite`, with
  zero/multiple arguments and optional trailing commas. `println` remains an
  exact alias. Output events preserve snapshots, budgets, and atomic diagnostics.
- Additive dual-writer execution APIs; the CLI routes program stderr explicitly.
  Single-writer hosts receive an actionable error before stdout when a program
  requires stderr. Current examples and guides use `print` by default.

- Immutable `bind` and mutable `var` declarations replace `let`/`let mut`.
  Legacy declarations report E0005 with migration help; Rust syntax uses
  `Statement::Declaration` instead of `Statement::Let`.
- Current examples and guides use the new declarations; the v0.0.2 source
  examples remain preserved as exact historical compatibility fixtures.

## 0.0.3-beta.1 — 2026-10-27 — prerelease

- Actionable loop/count/index diagnostics, configured limits in budget errors,
  contextual run/check/tokens help, and iteration/budget guides.
- Tab `\t` and carriage-return `\r` string escapes, with exact-byte fixture checks.
- Counted `repeat count as index` with a one-time count snapshot, immutable
  zero-based index, fixed storage, and nearest-loop control shared with while.
- Scoped while loops, nearest-loop break/continue, per-call work/print budgets,
  and CLI/embedding controls with atomic budget diagnostics.


## 0.0.2 — 2026-10-26 — prerelease

- Execution-local expression work buffers, immediate disposal of validation print
  values, and reproducible stage benchmarks with verified workloads.

- Conditional `if`/`elif`/`else` statements with boolean guards, scoped bodies,
  static checking of all paths, and execution of the selected path only.

- Boolean values, same-type equality, integer ordering, and short-circuit `and`,
  `or`, and `not` expressions.

- Immutable integer bindings and names in expressions.
- Explicit mutable bindings, assignment, and checked `+=`, `-=`, `*=`, `/=`.
- Immutable-assignment diagnostics and value snapshots for deferred output.
- Lexical block scopes, shadowing, and bounded block parsing/resolution.
- Name resolution before evaluation, with fresh storage for every execution.
- Unknown, duplicate, and reserved-name diagnostics with related declarations.
- A separate parsed-syntax API and immutable resolved-program API.
- Silent `check` validation of syntax, names, mutability, scopes, and arithmetic.
- Sequential token locations without repeated source-prefix scans.
- Generated-program conformance cases and malformed source/token mutation corpora.
- Package version `0.0.2`, native archives, and verified source/checksum records.
- String variables, primitive type inference,
  same-type mutable assignment, and immutable string value snapshots.

The variable milestone is integrated into main. Loops, floating-point/vector
arithmetic, bit operations, and freestanding C emission remain in separate PRs
and are excluded from this prerelease.

## 0.0.1 — 2026-10-25 — prerelease

The first experimental Scarp interpreter. Syntax and library APIs may change.

### Language

- UTF-8 `.srp` files with whitespace, line comments, and escaped string literals.
- `println` statements accepting strings or signed 64-bit integer expressions.
- Arithmetic precedence, grouping, unary minus, and left-associative operators.
- Checked arithmetic, literal range checks, and division-by-zero diagnostics.
- Full validation and evaluation before program output is written.
- Source byte spans, line/column locations, carets, error codes, and help.
- A nesting limit with diagnostics and flat expression storage for long chains.

### Tools and distribution

- `scarp run`, `scarp tokens`, help, and version commands.
- A reusable Rust library with explicit output destinations and error handling.
- Debug/release tests, cross-platform CI, and minimum Rust 1.74.0 checks.
- Runnable examples, a language reference, and contributor documentation.
- Native binary archives with SHA-256 checksums and recorded build information.
- Packaging smoke tests performed on the extracted deliverables.

### Known limits

No bindings, user-defined functions, floating-point values, type checker, modules,
loops, package manager, or native code generation. Source and intermediate values
are held in memory. Only the first diagnostic is reported. Expression grouping
is limited to 64 levels. The prerelease includes Windows x64, Linux x64 with
GNU libc, and Apple Silicon macOS builds; other platforms can build from source
where a compatible Rust toolchain is available.
