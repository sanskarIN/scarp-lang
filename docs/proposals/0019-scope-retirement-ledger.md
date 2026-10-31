# Proposal 0019: Scarp scope-retirement ledger

Status: **development prototype, not a released language specification**.
This work does not change the Scarp source syntax, package version, CLI
contract, existing `.srp` programs, or published v0.0.4-alpha.2 assets.

## User-facing model: values, not exposed storage

Scarp users manipulate **values** and their named bindings. A name identifies
a value within its visible scope; it is not an address, pointer, borrow,
allocation handle or lifetime annotation. A `bind` cannot be reassigned,
whereas a `var` can be replaced. Copies and output snapshots retain their
original logical values even if the source binding is subsequently replaced or
its scope ends.

The proposed **scope-retirement ledger** is an execution-plan feature:
the compiler knows which slots are accessible in each lexical scope and
emits retirements at each exit. It does not require users to write `free`,
`delete`, destructors, ownership-transfer syntax, reference counting,
lifetime annotations, or garbage-collection directives. No user program can
inspect a storage identity in the current language slice.

This is a Scarp-specific *contract and implementation arrangement*, **not**
a claim that lexical lifetimes, reference counting, memory safety, immutable
values, or deterministic cleanup were invented here. The current Rust host
implementation still uses Rust's memory safety and `Arc` for shared immutable
strings and vectors. Replacing all existing infrastructure would require a
separately reviewed, much larger architecture effort. The language contract
must not claim zero physical reference counting or a proven unique algorithm.

## Compiled retirement rules

1. Each resolved binding receives a unique frame-local slot. An inner scope
   can refer to an outer binding but cannot modify the outer scope's identity.
2. The resolver emits a private `Retire` operation listing **only** slots
   declared in the scope being exited, sorted by slot index. This instruction
   is not a source statement and is omitted for scopes without local bindings.
3. `break` and `continue` carry the sorted retirement list of all scopes
   exited by that transfer. A nested loop's transfer does **not** retire slots
   in its enclosing loop. Repeat index bindings remain available to
   `RepeatNext`, and the count/index storage is numeric-only.
4. A function completing a return retains its parameters and (for non-unit
   results) the immutable result slot until `ensures` checks complete; other
   frame-local slots retire before postconditions. The result value remains
   independent of the original local slot.
5. Retiring a slot replaces its runtime value with the internal unit marker.
   This drops a slot's payload reference. An immutable snapshot previously
   captured by output or another visible binding remains valid. Repeated
   retirement of an already-empty slot is harmless.
6. Neither compiler nor runtime scans arbitrary object graphs or searches for
   unreachable values. Retirement is encoded in each resolved control-flow
   transition; it does not introduce a runtime heap tracer.
7. `run` and `check` use the same runtime machine and retirement plan;
   retirement itself is bookkeeping, excluded from existing source-step and
   statement-count accounting to preserve established budget boundaries.

## Core invariants

- **No escaping local name:** lexical resolution forbids access to a local
  after its scope finishes. Future extensions must preserve this invariant.
- **No stale access:** a retirement must only occur after no execution path
  can subsequently read the retired slot without writing a fresh value first.
- **No alias mutation:** replacing a binding does not mutate captured values,
  other bindings or older output events.
- **No cross-frame release:** an inner function call cannot retire a caller's
  slots; each frame owns a distinct indexed environment.
- **No extra user work:** existing program output, diagnostics, exit codes,
  statement_count and specified step/print budgets must remain unchanged.
- **No hidden value exposure:** the internal unit marker is not available
  as a normal user-defined value.

## Limitations and security boundaries

This prototype is **not a new general-purpose memory allocator**. Frame arrays
still reserve all statically planned slot positions at call entry. The output
event log can retain snapshots until execution completes. Physical memory is
also consumed by source text, AST, expression stacks, buffers, `Arc` headers,
and host integrations. In particular, `max_live_slots` bounds reserved slot
counts, **not bytes**, and step budgets do not account for cleanup bookkeeping
as independent source steps.

This scheme currently covers flat scalar values, immutable strings and
one-dimensional numerical vectors. It relies on the absence of user-exposed
references, closures that capture locals, cyclic user graphs, async tasks or
object identity. Introducing any of those features requires a new escape
analysis and retention model **before** they can ship. Reclamation is
deterministic at compiled scope exits for in-scope binding slots, not a promise
of immediate process-level deallocation whenever any value becomes unused.

## Validation and path to future work

- Internal resolver tests inspect retirement plans for nested scopes and
  distinguish `break` from `continue`, verifying that outer storage and
  repeat index remain live.
- Integration tests cover conditional paths, loops, function postconditions,
  value/output snapshots, arithmetic failures, and unchanged work budgets.
- Validate the whole Rust debug/release/Clippy/rustdoc test suite against
  minimum Rust 1.74 and supported Windows/Linux/macOS builds and packages.
- Before introducing new syntax or storage kinds, publish a distinct proposal
  for their actual semantics and use of lexical retirement. Prefer a genuinely
  useful behavioral distinction over renaming existing constructs to claim
  originality. Conduct a prior-art review before making novelty claims.

**No tag, public release, alpha.3 publication or main-branch merge is
authorized by this proposal.**
