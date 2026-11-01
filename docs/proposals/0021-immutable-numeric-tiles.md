# Proposal 0021 — Scarp immutable numerical tiles

Status: **implemented in the alpha.4 integration candidate**, including
visible windows, the cumulative allocation ledger and scope retirement.
Published alpha.3 binaries do not implement tiles. Publication is separately gated.

## Why this value type

A one-dimensional vector is useful for coefficients and sampled values, but
images, time-windowed measurements, and basic numerical tables need two axes.
Scarp's first multidimensional value is the deliberately small **tile**:
a finite, immutable, rectangular two-axis value with explicitly checked shape.
It is not a general tensor engine; there is no broadcasting, GPU dispatch,
backpropagation, rank-N type system, or hidden element mutation.

`tile` is a new static value type for function parameters, results and
inferred `bind`/`var` bindings. Users do not manage storage addresses,
references, borrows, object identities, heap allocation or reclamation.

## Construct and inspect

```scarp
bind original = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
bind picture = tile(original, 2, 3);
print(height(picture), width(picture));
print(cell(picture, 1, 2));
print(lane(picture, 0));
print(turn(picture));
```

Output:

```text
2 3
6.0
[1.0, 2.0, 3.0]
tile(3x2)[[1.0, 4.0], [2.0, 5.0], [3.0, 6.0]]
```

| Operation | Static result | Cost in native steps | Definition |
| --- | --- | --- | --- |
| `tile(vector, rows, columns)` | tile | 1 | Retains existing immutable finite storage, row-major |
| `height(tile)` | integer | 0 | First axis length |
| `width(tile)` | integer | 0 | Second axis length |
| `cell(tile, row, column)` | float | 1 | Zero-based element read |
| `lane(tile, row)` | vector | width | Copies one visible row into a new vector |
| `unroll(tile)` | vector | 1 | Shares immutable flattened backing vector |
| `turn(tile)` | tile | rows × columns | Swaps axes and copies each cell to new row-major order |

Normal expression evaluation and statement-dispatch costs are additive.
Existing user-defined functions with the same names still take precedence
over builtin calls. Native kernels are pure, positional and callable from
contracts, default arguments and other pure functions. There is no new
punctuation syntax or automatic type conversion.

## Shape and value semantics

- The two dimensions have exact `integer` type and must each lie in
  `0..=65536`. The product is checked and must exactly equal the number
  of source elements, itself bounded by 65,536. No implicit reshape,
  padding, truncation or broadcasting takes place.
- Rows are stored consecutively in row-major order:
  `cell(t, r, c)` is element `r * width(t) + c`, after bounds checks.
- Shapes with a zero axis are valid if and only if the input vector is empty.
  `tile([], 0, 3)` is **different** from `tile([], 3, 0)`; both have
  zero cells. `turn` swaps the two axes even for empty values.
- Content **and shape** define tile equality. Signed positive and negative
  zero elements compare equal while display preserves their sign. Tiles
  cannot be implicitly compared with vectors or indexed by vector `[]`.
- A tile prints as `tile(RxC)[[...], [...]]` for its visible rectangular
  rows. Zero-height prints `tile(0xC)[]`; zero-width nonzero-height
  prints a list of empty rows.
- Reassigning a `var` changes only that binding; earlier `bind` snapshots,
  result values, immutable parameters and captured output remain unchanged.

## Diagnostics, boundaries and memory

Invalid, negative, inconsistent or excessive dimensions produce E0032
(`VectorLength`). Invalid cell/row indices produce E0033 (`VectorIndex`).
Wrong types and arity still report E0015/E0024. A finite buffer allocation
failure is E0035. The exception-free interpreter validates a whole program
before writing output, so selected errors discard pending stdout/stderr.
Every cell stays finite; there is no NaN/infinity escape.

`tile` and `unroll` share an immutable `NumericVector` allocation
internally. In the present safe-Rust interpreter, this is implemented through
`Arc`. `lane` and `turn` materialize fresh bounded buffers and prepay
one step per copied cell. `turn(turn(t)) == t` is true for all valid shapes,
including empty dimensions. `NumericTile::new` and `from_vector` validate
shape for embedding hosts; no mutable cell references are exposed.

Tile equality precharges the larger cell count. Selected output snapshots
charge 80 + 8n logical bytes for n elements, including 16 bytes of two-axis
shape metadata; the separate underlying allocation may be shared.
The step, vector length, frame-slot and output quotas remain independently
bounded. This is **not** a universal process-RAM cap, general escape
analysis or an original physical memory allocator.

## Alpha.4 integration

Tiles wrap the current visible-window `NumericVector`, so row/cell/flattening
never expose hidden backing cells. `lane` and `turn` prepay new element payload
through the same cumulative E0036 ledger as every other materialization.
Retirement handles tiles as ordinary immutable values and preserves snapshots.
Selected tile output additionally prepays cell count plus height, including
empty rows. See [the current guide](../tiles.md) for all resource formulas and
the new matrix kernels, and [the baseline audit](../releases/alpha4-baseline.md)
for reviewed experimental heads. Original PR history remains preserved.
