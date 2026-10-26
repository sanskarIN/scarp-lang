# Version and API compatibility

Use this guide before copying an example into a different Scarp executable or
updating a Rust embedding host. It compares the published v0.0.1 and v0.0.2
snapshots with the `0.0.3-dev` feature stack documented here.

## Identify the code you are running

`scarp --version` identifies the executable on `PATH`.
`cargo run -- --version` identifies a build from the current checkout. These can
be different commands with different behavior.

For a source snapshot, `git status --short`, `git branch --show-current`, and
`git rev-parse HEAD` identify local changes, the branch, and the commit. A branch
name alone is not an immutable version. The published release is associated with
the `v0.0.1` and `v0.0.2` tags. This development branch uses `0.0.3-dev`
package metadata and has no v0.0.3 tag.

## Language and command matrix

| Capability | v0.0.1 | v0.0.2 | Development |
| --- | --- | --- | --- |
| UTF-8 source, comments, three string escapes | Yes | Yes | Yes |
| String literal print statements | Yes | Yes | Yes |
| Checked signed 64-bit arithmetic | Yes | Yes | Yes |
| Integer declarations and variable references | No | Yes | Yes |
| Explicit mutability and compound assignment | No | Yes | Yes |
| Block scopes and shadowing | No | Yes | Yes |
| String variables and grouped string values | No | Yes | Yes |
| Primitive type consistency checks | No variable types | Yes | Yes |
| `run`, `tokens`, global help/version | Yes | Yes | Yes |
| `check` and per-command help | No | Yes | Yes |
| Boolean values, comparisons, short-circuit logic | No | Yes | Yes |
| `if`/`elif`/`else` statements with scoped bodies | No | Yes | Yes |
| While loops and deterministic execution budgets | No | No | Yes |
| Finite binary64 decimals and scientific literals | No | No | Yes |
| Immutable float vectors and native numerical kernels | No | No | Yes |
| Hex/binary literals, remainder, integer bit operations | No | No | Yes |
| Freestanding C emission for the integer/boolean subset | No | No | Experimental |
| Native executable/object build commands for that subset | No | No | Experimental |
| `exec`, static `verify`, contextual `guide`, and token/AST inspection | No | No | Yes |
| User functions, imports | No | No | No |
| Whole-language native compilation and kernel boot | No | No | No |

Integer range, precedence, division truncation, mandatory semicolons, and
no-output-on-invalid-source behavior remain consistent. Development adds
semantic checks before evaluation, so a later unknown name or type mismatch can
precede an earlier arithmetic failure in the reported diagnostic.

The complete development stack is available on `scarp/cli-foundation`.
Its implementation prerequisites are split into PRs for immutable bindings,
mutability, lexical scopes, validation tooling, and an optional string extension.
Earlier feature branches intentionally contain smaller subsets.

The boolean extension adds `Boolean` variants, comparison operators, and logical
syntax nodes to the public enums. Rust hosts using exhaustive matches must
handle these additions. `Value::as_boolean` inspects boolean values. See
[the boolean design](proposals/0004-boolean-expressions.md) for precedence and
short-circuit behavior.
The conditional extension adds `Statement::If` and `ConditionalBranch` to syntax.
`statement_count` includes branch guards, while omitting bookkeeping jumps and
block containers. All branches contribute to binding metadata, even if unselected.

## Library migration

| API or type | v0.0.1 | Development |
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

The documented snapshots declare Rust 1.74 as the minimum for source builds.
Published v0.0.1 and v0.0.2 archives cover Windows x64 MSVC, Linux x64 GNU, and
Apple Silicon macOS. Development-package checks cover those same native targets;
0.0.3-dev remains unpublished.

A successful source test on one host is not evidence of a universal binary.
Use the matching archive or build with a compatible local Rust toolchain. The
[release notes](releases/v0.0.2.md) describe the current published archive scope.

## Integration and future changes

Syntax and public Rust APIs remain experimental. Review the
[v0.0.2 notes](releases/v0.0.2.md) when updating a host application. Planned
features in a roadmap or proposal are not available merely because they are named.

The feature stack and this documentation are reviewed through separate PRs.
Documentation review does not integrate implementation into main. Follow the
maintainer's explicit instructions for integration, tagging, and publication;
the documentation work itself performs none of those release actions.
