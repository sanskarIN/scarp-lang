# Changelog

## Unreleased — v0.0.2 development

- Immutable integer bindings and names in expressions.
- Explicit mutable bindings, assignment, and checked `+=`, `-=`, `*=`, `/=`.
- Immutable-assignment diagnostics and value snapshots for deferred output.
- Name resolution before evaluation, with fresh storage for every execution.
- Unknown, duplicate, and reserved-name diagnostics with related declarations.
- A separate parsed-syntax API and immutable resolved-program API.

These changes are under development on feature branches and are not part of the
published v0.0.1 release.

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
