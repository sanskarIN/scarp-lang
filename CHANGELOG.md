# Changelog

## Unreleased — v0.0.3 development

- Preferred `print`, permanent `println`, raw `write`, zero/multiple arguments,
  exact single-space separators, and optional trailing commas.
- Explicit `eprint`/`ewrite`, dual-writer Rust and native-host APIs, ordered
  stream replay, prevalidation, and unsupported-stderr errors in single-writer APIs.
- Exact-byte package output checks, including raw output without a final LF.
- Breaking declaration syntax: immutable `bind` and mutable `var` replace
  `let` and `let mut`, with migration diagnostics. The Rust AST variant is now
  `Statement::Declaration`; scope, typing, and execution rules are unchanged.
- Canonical CLI names only: exec, compile, verify, inspect, guide, and version.
  Retired aliases fail with migration guidance; C output uses compile --emit c.
  Bare invocation now shows guidance successfully.
- File execution aliases, static verification, contextual help, short version,
  read-only token/AST inspection, and an audited CLI capability inventory.
- Native build/compile commands for hosted executables and freestanding objects,
  compiler selection, runtime budgets, and preserved output files.
- Freestanding C11 emission with caller-owned workspace and output adapters.
- Hex/binary literals, fixed-width bit operations, remainder, and checked shifts.
- Immutable finite numeric vectors, indexing, reductions, and numerical kernels.
- Finite binary64 decimals and scientific literals with checked results.
- Scoped loops, break/continue, deterministic work/print budgets, and CLI limits.

These features remain on development branches; main and v0.0.2 are unchanged.

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
