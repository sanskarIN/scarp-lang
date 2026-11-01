# Proposal 0031 - Bounded scalar constant expressions

Status: unreleased development. The package remains 0.0.5-alpha.1. No release,
publication workflow, tag or registry publication is authorized by this work.

## Language contract

Imported files' immutable file-level bindings and entry files' `pub bind`
declarations are module constants. Extend their initializers from scalar literals
to integer/finite-float arithmetic, comparisons, equality, boolean operators,
grouping and references to constants. Integer and float operands do not implicitly
mix. Strings support literals, references and equality, without concatenation.

Within one file, references must name earlier module constants. Imports may appear
anywhere at file scope; qualified references may use public constants from imported
files. Ordinary entry-script bindings retain runtime semantics and cannot supply
module constant initializers. Functions can still use constants declared later.

All initializer nodes undergo static checking, including short-circuited operands.
Evaluation follows existing left-to-right and boolean short-circuit semantics.
Calls (including pure functions and built-ins), vectors and indexing are forbidden
even in skipped branches. No I/O, user execution frames or module initialization.
Unused invalid constants fail compilation too. Imports remain acyclic; local
constant cycles receive source-aware cycle diagnostics before forward-reference
diagnostics. Otherwise, forward references identify the later declaration.

## Architecture and limits

Reuse the existing resolver expression typing and checked scalar evaluator rather
than inventing a second arithmetic/type system. Retain the module loader's existing
dependency-first completion order for constant collection while preserving public
discovery order and inspection output order. Resolve member visibility through the
same lookup used by ordinary module expressions.

New additive `ConstantLimits` controls cumulative initializer nodes and evaluation
tasks across the entire graph (defaults: 1,000,000 nodes and 2,000,000 tasks).
Both phases are iterative. The node limit applies before initializer cloning or
type resolution, including branches skipped at evaluation. Existing expansion-byte
limits charge substituted constant strings and new reference-derived folded
storage before cloning; literal storage keeps its existing source-bound policy.
These are deterministic compiler work/storage allowances, not process heap caps.
Existing ModuleLimits literals and compile entry points remain compatible; additive
entry points permit hosts to choose constant limits. Runtime budgets stay separate.

Expose immutable direct constant-reference edges (owning source/declaration,
target source/declaration and use span). Edges include statically checked skipped
branches, retain declaration/reference order, and use compilation-local coordinates.
Do not add values to public inspection output or change existing symbol layouts.

Use existing diagnostic assignments: E0007 overflow, E0008 division by zero,
E0010 unknown name, E0015 operand types, E0031 nonfinite float, E0037 unsupported
initializer form, E0039 dependency cycle, E0040 compiler limit, E0041 private member,
E0042 invalid constant reference. Attach the owning initializer declaration to
compile-time failures. Add operand/source notes where they help explain nested
type errors without changing existing primary diagnostic spans or codes.

## Validation and deferred scope

Cover literal compatibility, arithmetic/type parity with runtime, ordering and
cycles, diamond imports, public/private access, argument/default/return typing,
Unicode/CRLF locations, exact node/task/storage boundaries, long iterative chains,
and CLI run/check/modules behavior. Add runnable examples and negative fixtures.
Run format, strict Clippy, debug/release suites, build, Rustdoc and native CI.

No constant function execution, implicit casts, vectors/tiles, forward-reference
evaluation, general partial evaluation, new syntax, packages or website changes.
