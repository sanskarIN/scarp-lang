# Proposal 0023: Cumulative vector allocation ledger

Status: **unreleased draft, not merged**. The design adds an independent
runtime allocation quota without importing external code, changing Scarp
syntax, or exposing any manual-memory, garbage-collection or ownership API
to Scarp programs.

## Motivation and boundary

Scarp currently has three runtime storage safeguards:

- `max_live_slots`: total declared frame slots across active invocations;
- `max_capture_bytes`: cumulative logical output snapshot cost;
- `MAX_VECTOR_LENGTH`: 65,536 finite elements per numerical vector.

The step budget also indirectly bounds the cumulative bytes of element
buffers allocated by current numerical kernels. However, users who
intentionally raise `--max-steps` need a **separate cap** on the amount of
new numerical payload allocated during a run.

This proposal introduces `ExecutionOptions.max_vector_bytes` and
`scarp run/check --max-vector-bytes N`. They default to **8,388,608**
bytes (8 MiB). The limit is a cumulative allocation **ledger**, not an
estimate of peak resident memory. Raising or lowering `max_steps` never
implicitly changes the new limit.

## What is charged

When the interpreter prepares a **new finite-vector element buffer**,
it prepays exactly `element_count * size_of::<FiniteFloat>()`, currently
`8 * element_count`, to this ledger **before** attempting a fallible
reservation or writing any output. The allocation paths covered in this
milestone are:

| Allocation path | Charged payload |
| --- | --- |
| `[1.0, 2.0]` vector literal | 16 bytes |
| `fill(3.0, 2)` | 16 bytes |
| `add(a,b)` / `scale(a,f)` / `clamp(a,lo,hi)` | `8 * len(a)` bytes |
| `softmax(a)` | `8 * len(a)` bytes |
| Empty vector materialization | 0 bytes |
| Read, copy/bind, passing or returning an existing vector | 0 additional bytes |
| `len`, `sum`, `mean`, `dot`, `mse`, index | 0 additional bytes |

All selected paths share one ledger, including loops, user function calls,
returns, contract/default expressions, and recursive calls. Even when an
earlier vector becomes unreachable, its already-charged payload is **not
refunded**. This cumulative property is deliberate: it prevents a program
from repeatedly allocating and dropping values to exceed the user-selected
allocation allowance. The implementation does not need to identify Arc
reference owners or measure live heap objects.

Both `run` and `check` use the same engine and counters. With
`--max-vector-bytes 16`, `print([1.0, 2.0]);` succeeds while the
same statement with `--max-vector-bytes 15` produces **E0036**.
No pending stdout/stderr is committed on an allocation-quota error.
Failures on malformed shapes, types, arithmetic or execution steps continue
to use their existing codes.

## Rust host API

```rust
let opts = scarp::ExecutionOptions {
    max_vector_bytes: 16,
    ..scarp::ExecutionOptions::default()
};
scarp::check_with_options("print([1.0, 2.0]);", opts)?;
```

The existing native-kernel host helper `numeric::evaluate` retains its
original step-only contract. A new
`numeric::evaluate_with_vector_limit(function, args, max_steps, max_vector_bytes)`
can enforce both caps for a native kernel in isolation, including fill and
softmax, while retaining the same diagnostics and finite-value guarantees.
Host-constructed `NumericVector::new` allocations occur *outside* these
program-execution quotas; callers remain responsible for their own buffers.

## Limitations and intentional non-claims

The cap covers **new element payload requested through the interpreter's
vector buffer paths**, not the number of bytes allocated by `Vec`, `Arc`,
the Rust allocator, lexer/parser, compiled program, source strings, frame
environments, evaluator task stack, host-provided vectors, diagnostics,
formatting, thread stacks or operating-system overhead. The Vec allocator
may round capacity above requested elements. Existing values can keep
shared allocations alive without accruing new charges.

The three user-visible memory-related controls are intentionally distinct:

- `max_live_slots` restricts the number of active frame slots;
- `max_capture_bytes` restricts cumulative logical output snapshots;
- `max_vector_bytes` restricts cumulative newly materialized vector
  element payload.

None alone, or in combination, guarantees a fixed process RSS or a
security sandbox. The existing step limit, individual-vector cap,
fallible allocation and safe Rust interpreter remain in force.

This mechanism is not a novel claim about reference counting, garbage
collection or memory reclamation. Scarp uses conventional immutable
shared data structures internally; it does not expose that mechanism
through programming-language syntax.

## Separate integration review

This branch starts from current main and **does not integrate**
PR #50 (norm), #51 (scope retirement), #52 (vector windows),
#53 (numerical tiles) or #54 (terminal self-transfer).

If the storage model evolves:

- Zero-copy windows, tiles and immutable reference sharing must **not**
  charge for element buffers that were not newly allocated.
- Stitching, axis transposition, normalization and any new operation that
  creates a fresh payload must charge before its buffer reservation.
- Scope retirement and terminal frame reuse must **not refund** already
  incurred cumulative charges.
- Every new buffer creation must use the checked `numeric::buffer`
  path, or a semantically equivalent ledger gate, and receive integration
  tests on its exact byte boundary.

Draft status is mandatory until format, Clippy, Linux/Windows/macOS
tests, Rust 1.74 checks, output-atomicity cases and package smoke tests
all pass. **No tag, main-branch merge, version bump or prerelease.**
