# Version and API compatibility

Development declarations use `bind` and `var`; see the [migration guide](syntax-migration.md). Published tags retain their original syntax.

Use this guide before copying an example into a different Scarp executable or
updating a Rust embedding host. It compares the published v0.0.1, v0.0.2,
v0.0.3-beta.1, beta.2, and beta.3 snapshots. Beta.3 uses the syntax described below;
separate feature branches can differ from both main and downloaded binaries.

## Identify the code you are running

`scarp --version` identifies the executable on `PATH`.
`cargo run -- --version` identifies a build from the current checkout. These can
be different commands with different behavior.

For a source snapshot, `git status --short`, `git branch --show-current`, and
`git rev-parse HEAD` identify local changes, the branch, and the commit. A branch
name alone is not an immutable version. The published release is associated with
the corresponding tag: `v0.0.1`, `v0.0.2`, `v0.0.3-beta.1`, or
`v0.0.3-beta.2`, or `v0.0.3-beta.3`. Check that a release archive's BUILD-INFO version and source
commit match the intended tag. A development package's version alone does not
prove its contents are the published release.

## Language and command matrix

| Capability | v0.0.1 | v0.0.2 | v0.0.3-beta.1 | v0.0.3-beta.2 | v0.0.3-beta.3 |
| --- | --- | --- | --- | --- | --- |
| UTF-8 source, comments, backslash/quote/LF escapes | Yes | Yes | Yes | Yes | Yes |
| Checked signed 64-bit arithmetic | Yes | Yes | Yes | Yes | Yes |
| Integer/string/boolean bindings, scopes and decisions | No | Yes | Yes | Yes | Yes |
| Declaration spelling | None | `let` / `let mut` | `let` / `let mut` | `bind` / `var` | `bind` / `var` |
| Line output spelling | `println` | `println` | `println` | `print`, with `println` alias | `print`, with `println` alias |
| Raw output and explicit stderr | No | No | No | `write`, `eprint`, `ewrite` | `write`, `eprint`, `ewrite` |
| Zero/multiple output arguments and trailing commas | No | No | No | Yes | Yes |
| `run`, `tokens`, global help/version | Yes | Yes | Yes | Yes | Yes |
| `check` and per-command help | No | Yes | Yes | Yes | Yes |
| While/repeat, nearest-loop break/continue | No | No | Yes | Yes | Yes |
| Work/output limits and CLI budget options | No | No | Yes | Yes | Yes |
| Tab/carriage-return string escapes | No | No | Yes | Yes | Yes |
| Typed funcra, recursion, named/default arguments and contracts | No | No | No | No | Yes |
| Imports or native compilation | No | No | No | No | No |

To migrate beta.1 source, replace declaration tokens `let` with `bind` and
`let mut` with `var`. `println` continues to work in beta.2; `print` is the
canonical line-output spelling. Strings/comments retain their original text.
See [declaration migration](syntax-migration.md) and [output](output.md).

Integer range, precedence, division truncation, mandatory semicolons, and
no-output-on-invalid-source behavior remain consistent. Development adds
semantic checks before evaluation, so a later unknown name or type mismatch can
precede an earlier arithmetic failure in the reported diagnostic.

The variable milestone is available on main. Its implementation was reviewed in
PRs #7–#15, covering bindings, mutation, scopes, validation, strings, documentation,
booleans, conditionals, and execution workspace improvements. Later feature
branches intentionally remain outside this release.

The boolean extension adds `Boolean` variants, comparison operators, and logical
syntax nodes to the public enums. Rust hosts using exhaustive matches must
handle these additions. `Value::as_boolean` inspects boolean values. See
[the boolean design](proposals/0004-boolean-expressions.md) for precedence and
short-circuit behavior.
The conditional extension adds `Statement::If` and `ConditionalBranch` to syntax.
`statement_count` includes branch guards, while omitting bookkeeping jumps and
block containers. All branches contribute to binding metadata, even if unselected.

## Library migration

| API or type | v0.0.1 | v0.0.3-beta.2 |
| --- | --- | --- |
| `compile(source)` | Returns syntax `Program` | Returns immutable `ResolvedProgram` |
| `parse(source)` convenience function | Absent | Returns unresolved syntax |
| `parser::parse(tokens)` | Parses tokens | Continues to parse tokens |
| `interpreter::execute` input | Syntax program | Resolved program |
| Validation without a writer | No convenience entry point | `check` and `interpreter::validate` |
| `evaluator::evaluate` result | Integer | Integer, retained for compatibility |
| Primitive value evaluation | Integer helper only | `evaluate_value`, `Value`, `ValueType` |
| General print expression variant | `PrintArgument::Integer` | `PrintArgument::Expression` |

Use `parse` when you need syntax inspection or transformation. Use `compile`
before executing complete programs with declarations. Resolved storage plans
remain private so callers cannot change slot assignments independently of the
expressions that reference them.

`statement_count`, `binding_count`, and `binding_types` expose read-only execution
metadata. The counts include declarations inside blocks; statement counts exclude
block containers. Repeated execution always starts with fresh storage.

The host `Value` API does not provide external variable injection. Complete
programs establish their bindings through source declarations.

## Platform and toolchain scope

All five documented releases declare Rust 1.74 as the minimum for source builds.
Their native archives cover Windows x64 MSVC, Linux x64 GNU, and Apple Silicon
macOS. Beta.2 Linux binaries are built on Ubuntu 22.04. Other architectures need
their own source build and validation; a supported OS name is not enough.

A successful source test on one host is not evidence of a universal binary.
Use the matching archive or build with a compatible local Rust toolchain. The
[release notes](releases/v0.0.3-beta.3.md) describe the current archive scope.

## Integration and future changes

Syntax and public Rust APIs remain experimental. Review the
[beta.3 notes](releases/v0.0.3-beta.3.md) when updating a host application. Planned
features in a roadmap or proposal are not available merely because they are named.

The feature stack and this documentation are reviewed through separate PRs.
Documentation review does not integrate implementation into main. Follow the
maintainer's explicit instructions for integration, tagging, and publication;
the documentation work itself performs none of those release actions.

The v0.0.3-beta.1 iteration milestone adds while/break/continue and execution budgets after
the v0.0.2 baseline. See [the loop design](proposals/0006-bounded-loops.md).

Counted `repeat expression as index` adds `Statement::Repeat` to the public AST
and `DiagnosticKind::RepeatCount` (E0019). Update exhaustive Rust enum matches.
No function signatures or ExecutionLimits fields change. Compiled binding
metadata includes a saved count and immutable index slot per repeat, even when
the body does not run. Existing identifier spellings repeat/as remain legal.

Beta.2 retains existing run/check/execute entry-point signatures and the
two-field `ExecutionLimits` struct. Use the additive dual-writer APIs for
`eprint`/`ewrite`. Syntax hosts must replace `Statement::Let` with
`Statement::Declaration` and handle `Statement::Output`, `OutputTarget`, and
`TokenKind::Comma`. Single-argument stdout line forms retain `Statement::Println`.
See [output and dual-writer migration](output.md) for details.

## Function migration from beta.2 to beta.3

Beta.3 adds [typed functions](functions.md) after beta.2, integrated on main.
Existing run/check/execute entry points and the two-field `ExecutionLimits`
struct keep their signatures. They use the default call-depth/count/live-slot
and cumulative output-snapshot limits in `ExecutionOptions`, including for
programs without functions. Hosts needing different caps can select options.
These limits do not bound all parser allocations and are not a sandbox.

Syntax hosts must handle `Statement::Function`, `Return`, `Call`,
`ExpressionKind::UserCall`, `TokenKind::Colon`/`Arrow`, and `ValueType::Unit` /
`Value::Unit` in exhaustive matches. Unit is a function result marker, not a
source value literal. E0001–E0019 retain their assignments; E0020–E0029 are new.
Unknown call names now report E0023 instead of E0005. The newly reserved words
`funcra`, `pure`, `return`, `needs`, `ensures` may require binding renames.
`repeat`/`as` bindings remain compatible; `repeat` cannot be a function name.

## Finite-float alpha candidate

This source checkout extends beta.3 with [finite binary64 scalars](floats.md).
Published beta.3 and earlier tags keep their original integer/string/Boolean
behavior. No existing diagnostic code is renumbered: E0030/E0031 are new.
Previously rejected valid float literals now parse; malformed forms remain errors.
No implicit numeric conversion, local type annotation, CLI rename, or dependency
is introduced. Syntax/embedding hosts must handle float variants in TokenKind,
ExpressionKind, Value and ValueType; FiniteFloat::new checks host values.
The legacy integer evaluate helper keeps its result type. Default execution
options and all six budget fields remain unchanged. Public alpha tagging and
publication require explicit authorization after the final source/package gates.
