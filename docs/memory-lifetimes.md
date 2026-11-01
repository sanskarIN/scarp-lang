# Automatic value retirement

The alpha.4 candidate includes compiler-planned retirement of frame slots when
their lexical scopes end. Programmers continue to use ordinary `bind`, `var`,
blocks, calls, returns and loop control; no deallocation syntax is needed.

The resolver assigns a unique slot per declaration and emits internal `Retire`
operations at block exits. `break` and `continue` carry the slots of every scope
they exit, preserving outer bindings and repeat-control state. Retirement drops
a slot's immutable value by replacing it with the internal unit marker. It is
bookkeeping excluded from source statement counts and existing work charges.

Function completion retires locals even after an early return. Parameters and
the result remain available until all `ensures` checks finish. Default arguments
run in order and may read earlier parameters. Recursive invocations have isolated
frames. Eligible terminal self-calls replace their invocation state only after
outgoing arguments have been evaluated; defaults, preconditions and budgets
apply again to the next invocation. Postconditions prevent this optimization.

Clones, returned values, immutable vector windows, tiles and queued output events
retain their own references to shared backing storage. Retiring a source slot
does not change those values. Shared storage is released when its final holder
is dropped. A small window can retain a much larger original buffer.

## Verification and limits

Unit-test builds automatically audit resolver plans using a reachable-control-flow
worklist. Incoming initialized-slot sets join by intersection. Reads, updates,
repeat state, stores, retirement indices and jump targets are checked. Defaults
are checked in order, postconditions have a dedicated initialized result slot,
and parameters must survive all function-completion paths. Negative tests corrupt
valid plans to exercise stale reads, invalid slots/targets, branches, loops,
default/result dependencies and premature parameter retirement.

The checker is test-only development tooling, not a runtime tracing collector or
a proof covering arbitrary future language features. Current reasoning relies on
the absence of addressable references, closures, cycles and asynchronous escapes.
Rust's safe types and immutable `Arc` backing still provide the host storage
implementation. Fixed frame-slot arrays remain allocated for a frame's lifetime.

`max_live_slots` counts allocated frame slots rather than occupied values.
`max_vector_bytes` counts cumulative newly materialized element payload; retirement
does not refund it. `max_capture_bytes` counts logical output snapshots even if
their backing buffers are shared. Work limits count logical execution. None of
these alone or together measures total process RSS or every parser/host allocation.

See [proposal 0019](proposals/0019-scope-retirement-ledger.md),
[architecture](architecture.md), and [the numerical budget contract](budgets.md).
