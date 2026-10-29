# Typed funcra functions

Status: integrated into main through PR #41 and included in v0.0.3-beta.3.
Published beta.2 does not include this syntax. This adaptation uses the function
core reviewed in PRs #27/#37 and excludes their older prerequisite stack.

## Decisions

- File-level named definitions, with mandatory integer/boolean/string parameter
  types and integer/boolean/string/unit results. No implicit capture or callable
  values. Collect signatures before resolving bodies to support mutual recursion.
- Explicit arguments evaluate in source order before parameter mapping. Omitted
  trailing defaults evaluate in declaration order in the new invocation and see
  only earlier parameters. Default expressions and contracts require pure calls.
- Parameters are immutable. Function-body locals occupy an inner lexical scope.
  Return checks the declared result, exits only its invocation, then evaluates
  postconditions. Loop bodies alone never establish definite return.
- Declared purity is checked statically, including unreachable branches. Local
  mutation is allowed. Contracts are boolean runtime checks, not proofs.
- Expression evaluation suspends at a call. The interpreter pushes a heap frame
  with its own program counter, slots, expression workspace, and pending output.
  Returning resumes the caller's task stack without Rust call-stack recursion.
- Repeat count evaluation can suspend. The result initializes the existing saved
  count/index slots exactly once. Loop jumps remain local to their frame.
- Preserve the two-field ExecutionLimits struct; add ExecutionOptions for call
  depth/count, live slots, and cumulative snapshot storage. Check caps before
  allocating new frames. Validate and run charge the same selected work/output.
- Prepare both streams before replay. Nested call output precedes the caller's
  pending output statement. Diagnostic failures discard all events; sink failures
  can leave earlier bytes. Bound call traces to 16 source locations.
- Preserve E0001–E0019. Append function codes E0020–E0029. Keep the public single
  argument Println AST shape, normalizing it only in the private resolved plan.

## Deliberately separate work

No decimal/vector/builtin/bit-operation/native backend or CLI redesign code is
imported. Main's `run`, `check`, and `tokens` commands remain. Native compilation
is unavailable: `compile` fails usage parsing before any artifact is emitted.
Closures, nested/anonymous functions, generics, overloads, callable types, and
resource annotations need separate designs and complete validation.

Read [the function guide](../functions.md) for executable examples, grammar,
runtime ordering, limits, diagnostics, and embedding APIs. Integration requires
fresh Windows/Linux/macOS/Rust 1.74 checks and native package smoke tests. A
development version and green branch checks do not publish a release.
