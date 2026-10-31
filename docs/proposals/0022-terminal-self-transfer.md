# Proposal 0022 — Terminal self-transfer for Scarp functions

Status: **unreleased, draft-only**. This is a targeted interpreter
optimization, not a new syntax or a change to published alpha.2.

## Purpose

A `funcra` that ends a selected path by directly returning the result of
calling itself should not need an ever-growing chain of suspended frames.
The interpreter already uses an explicit frame machine; this proposal lets
one frame represent a sequence of **terminal self-transfers**.

```scarp
pure funcra tally(n: integer, result_so_far: integer = 0) -> integer
    needs n >= 0;
{
    if n == 0 { return result_so_far; }
    return tally(n - 1, result_so_far + 1);
}
print(tally(5000));
```

The expected output is `5000`. Every logical call still runs its defaults,
`needs` clauses and normal statements, and every call still counts against
`ExecutionOptions.max_calls` and the step budget. Only *the physical frame*
is reused. No programmer manages a pointer, borrow, manual free, garbage
collector or ownership transfer.

## Eligibility

The execution engine applies a terminal self-transfer **only** if all of these
conditions hold at a selected runtime call:

1. The caller is a `funcra` frame (not top-level output).
2. The callee is the very same resolved function identity.
3. The selected return expression's root is that direct user call:
   `return function(arguments);`.
4. The current function has **no `ensures` contracts**, because such
   contracts must check each logical caller result after its nested call.
5. Standard call-count, source-expression evaluation and storage validation
   have succeeded before replacing the frame.

An indirect expression, even `return (function(n));`, and any expression
with subsequent computation, such as `return 1 + function(n);`, uses
existing ordinary call frames. Non-self calls likewise use ordinary frames.
The use of this narrow direct pattern is deliberate and observable through
resource limits, not a promise of general tail-call elimination.

`needs` contracts are supported: each replacement frame checks them again.
Omitted default arguments are evaluated in declaration order in the
replacement frame; explicitly supplied arguments still evaluate exactly once
before the transfer. All bindings and locals from the previous invocation
are dropped when the frame is replaced. Typed return values and result
identity remain unchanged. New function-local storage uses the existing
checked slot count.

## Resource and diagnostic semantics

- Every terminal transfer counts as one selected call toward
  `max_calls`, and the old entry cost **1 + function slots** is charged.
  Expression, guard, statement, default and contract charges remain intact.
- A sequence of eligible self-transfers uses a **single active function
  frame**, so it is not restricted by `max_call_depth` for each logical
  invocation. It continues to use the same bounded live-slot allocation.
  Initial calls from top-level and nonterminal nested calls obey the
  existing depth and slot limits.
- The runtime retains up to **15 predecessor call-source spans** in the
  current frame, plus its active span. If an error occurs, at most 16
  related call sites are emitted overall. No argument data or secrets
  are included in diagnostics. Full history is intentionally not retained.
- Precondition failure, checked arithmetic errors, explicit call limits,
  step budget exhaustion and the existing two-stream output atomicity
  remain observable in `run` and `check` alike.

## Distinctiveness and integration

This work makes Scarp's chosen function semantics predictable under bounded
execution. It does not claim to have invented tail-call optimization,
frame reuse, recursion or finite state machines; those are longstanding
programming concepts. No third-party source or dependency was introduced.

PR #50 (vector norms), #51 (scope retirement), #52 (vector windows), and
#53 (two-axis tiles) remain independent draft branches. This PR is based on
the original protected main and does not integrate or publish them.
A later integration review must verify that slot retirement remains correct
when terminal transfer replaces a frame and that richer values retire without
leaking references. A successful PR CI alone is not authorization to release.

## Validation expectations

Regression tests require deep (5000-step) terminal recursion with
`max_call_depth=1`, exact total call boundaries, nested callers, default
and `needs` evaluation, ordinary behavior with `ensures` and nontail
expressions, bounded call trace correctness, failure atomicity and
execution-step enforcement. Run minimum Rust 1.74, Linux/macOS/Windows
tests, formatting, Clippy, example checks and package smoke tests.
