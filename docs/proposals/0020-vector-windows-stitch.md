# Proposal 0020: Shared windows and bounded stitching

Status: **unreleased development candidate**. This work extends the
immutable finite numerical vector model without new source punctuation,
implicit conversions, user-visible references or memory-management syntax.
It is not in the published `v0.0.4-alpha.3` binaries. This integration
updates the original experimental proposal to enforce the newer cumulative
numerical allocation ledger and preserve subsequent pure numeric kernels.

## Motivation

AI data preparation and numeric programs frequently need a short contiguous
part of a larger sequence, then occasionally a new sequence assembled from
separate parts. Constructing the selected range by copying every element
can consume significant work and storage before any calculation. Conversely,
combining two values must produce a new immutable sequence without mutating
the source buffers.

## Operations

| Call | Result | Semantics |
| --- | --- | --- |
| `window(v, start, count)` | vector | Select `count` visible elements beginning at zero-based `start`; share storage |
| `stitch(left, right)` | vector | Produce a fresh value by concatenating both visible sequences in order |

All arguments are positional. `start` and `count` have exact `integer`
type; `v`, `left` and `right` are `vector`. Both kernels are pure and
may appear in default arguments or pure function contracts. Existing
user-defined `funcra` declarations with these names take precedence.

```scarp
bind source = [10.0, 20.0, 30.0, 40.0];
bind middle = window(source, 1, 2);
print(middle);
print(stitch(middle, [50.0]));
```

Expected output:

```text
[20.0, 30.0]
[20.0, 30.0, 50.0]
```

## Bounds and errors

- `window(v, 0, 0)` is valid for empty input; `window(v, len(v), 0)`
  is valid for any vector. Every index is relative to the **current visible
  vector**, including when `v` itself is a window.
- A negative start, a start above `len(v)`, or a selected range exceeding
  the visible boundary fails with E0033 (vector index).
- A negative count or count greater than 65,536 fails with E0032 (vector
  length); no arithmetic wraps when a host passes large indices.
- `stitch([], [])` produces `[]`. The combined length may not exceed
  65,536 elements; E0032 is reported before allocation.
- Wrong argument types report E0015, wrong arity or labeled builtin
  arguments E0024. Fallible stitch buffer allocation reports E0035.
- Any error suppresses all pending program stdout/stderr; `run` and
  `check` use the same diagnostics.

## Allocation, budgets and value identity

Internally, a vector is `(shared immutable buffer, offset, length)`.
Its **value** is only its visible finite numbers. Equality, output formatting,
indexing, kernels, and function contracts use the visible range rather than
the entire backing buffer. Signed zeros retain their sign when displayed but
compare numerically equal. Nested windows compose offsets without copying
element payloads.

`window` charges one native work step, independent of selection length, and
creates no new element buffer. `stitch` charges exactly `len(left) +
len(right)` native work steps before making a fallible fresh buffer and
copying visible finite elements. Ordinary expression dispatch charges remain
unchanged. The per-value length bound and existing output-capture limits
remain in force; captured window values count their **visible** length.

A small window **retains the entire backing buffer** while it remains
reachable, including a zero-length window. The **cumulative vector payload allocation ledger** (E0036) now charges
newly reserved element buffers: `window` charges **zero new element
bytes**, while `stitch` charges exactly `8 * (len(left) + len(right))`
new element bytes. Input literals and earlier materializations remain charged;
nested windows and immutable sharing do not charge again. This is an
allocation-throughput quota, **not** a live heap, backing allocation, or
process RSS cap. A tiny window may keep the entire large buffer alive.

The existing Rust interpreter
uses `Arc` internally to share immutable memory. This is not a claim of
inventing contiguous slices, reference counting, or memory safety, and it
does not eliminate Rust or change the interpreter's actual memory allocator.
It introduces no memory operations, borrowing obligations, pointer arithmetic
or ownership transfer rules for Scarp users.

## Test and release gates

- Visible equality versus backing-buffer identity; nested windows; empty
  endpoints; negative/overflow bounds and exact maximum-length behaviors.
- Original value and output snapshots remain unchanged after reassignment.
- Works with existing numerical kernels and named/default function calls.
- Host API shares selected element addresses for a window and allocates
  distinct elements for a stitch.
- Native work budgets and runtime failure atomicity are verified exactly.
- Verify exact 0 / one-byte-below `max_vector_bytes` boundaries and that
  `window` of an existing vector does not consume the element quota.
- Existing `crossentropy`, `lossgrad`, `logsoftmax` and other numerical kernels
  must operate on **visible windows**, never on backing-buffer contents.
- Required CI: debug/release tests, formatting, Clippy, docs, minimum Rust
  1.74, native package smoke tests on supported operating systems.

This integration does not authorize merging the separate scope-retirement or
2D tile draft PRs, changing package versions, tagging, or publishing a prerelease.
