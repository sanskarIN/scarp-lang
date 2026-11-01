# Tiles and linear algebra (alpha.4)

Scarp alpha.4 implements bounded immutable two-axis numerical values. Published
alpha.3 binaries do not include tiles or matrix kernels. Use the
[verified alpha.4 packages](releases/v0.0.4-alpha.4-validation.md) or build from
source to run the examples.

```scarp
bind weights = tile([1.0, 2.0, -1.0, 3.0], 2, 2);
print(matvec(weights, [2.0, 1.0]));
print(turn(weights));
```

```text
[4.0, 1.0]
tile(2x2)[[1.0, -1.0], [2.0, 3.0]]
```

Each axis is an integer in `0..=65536`. Their checked product must equal the
visible vector length, at most 65,536 cells. Elements are finite IEEE-754
binary64 values in row-major order. Integers never convert implicitly to floats.
No cell is mutable; replacing a `var` leaves earlier values and output intact.

| Call | Result | Meaning |
| --- | --- | --- |
| `tile(values, rows, columns)` | tile | Bind a shape to a visible vector without copying cells |
| `height(t)`, `width(t)` | integer | Inspect the two axes |
| `cell(t, row, column)` | float | Read a zero-based cell |
| `lane(t, row)` | vector | Materialize one row |
| `unroll(t)` | vector | Share the visible row-major sequence |
| `turn(t)` | tile | Materialize the transpose with axes swapped |
| `matvec(t, v)` | vector | Matrix-vector product; width must equal vector length |
| `matmul(a, b)` | tile | `(m,k) * (k,n)` produces `(m,n)` |
| `tileadd(a, b)` | tile | Add corresponding cells of identical shapes |
| `tilescale(t, factor)` | tile | Multiply each cell by a float |

All calls take positional arguments and are pure. File-level `funcra`
declarations take precedence over same-named built-ins. `tile` can annotate
function parameters and results, including defaults and executable contracts.
There is no broadcasting, implicit vector/tile conversion, or `t[row][column]`
syntax. Tile equality compares both axes and visible cells. Signed zeros compare
equal, while display and scalar multiplication preserve their stored signs.

## Empty dimensions and numerical behavior

`tile([], 0, 3)` and `tile([], 3, 0)` are distinct. Transpose preserves both
dimensions by swapping them. A valid row of a zero-width tile is an empty vector;
there is no valid cell in a tile with an empty axis.

Multiplying `(m,0)` by `(0,n)` produces `m*n` positive zeros, subject to the
ordinary output-cell, work and allocation limits. A matrix-vector product with
zero columns similarly produces one zero per row. Zero output dimensions
produce an empty result with the specified shape. Inner dimensions still must
match, even when no output cells exist. Adding two empty tiles still requires
identical shapes.

Products are computed in increasing inner-index order, using the same Neumaier
compensation as `dot`. Each product, accumulated sum, compensation and final
cell must stay finite. Overflow is E0031 even if later cancellation could make
the mathematical answer finite. Results are rounded binary64, not exact real
arithmetic; representable subnormals are retained and underflow can produce zero.
An exact-zero compensated product is positive zero. This is a small portable
kernel implementation, with no production BLAS or tensor-framework claim.

## Resource contract

These are native work units, additive to expression/statement dispatch:

| Operation | Native work | Newly allocated element bytes |
| --- | --- | --- |
| tile construction, unroll | 1 | 0 |
| height, width | 0 | 0 |
| cell | 1 | 0 |
| lane | width | `8*width` |
| turn, tileadd, tilescale | cell count | `8*cell_count` |
| matvec with shape `(m,k)` | `m*(2*k+1)` | `8*m` |
| matmul with shapes `(m,k)`, `(k,n)` | `m*n*(2*k+1)` | `8*m*n` |

Checked shape and work arithmetic precedes loops; the entire work fee precedes
payload reservation. Every new buffer prepays the execution-wide
`--max-vector-bytes` quota. The quota remains cumulative after retirement.
Tile equality prepays the larger cell count. Selected tile output prepays
`cell_count + height` work units so even zero-width row formatting is bounded.
Each tile snapshot costs `80 + 8*cell_count` logical bytes; `check` enforces the
same charges while discarding output. All semantic failures discard pending
program stdout and stderr.

Tile construction from a nested `window` preserves that window's offset and
visible length. Neither `cell`, `lane`, `unroll`, transpose nor matrix arithmetic
can expose hidden backing cells. A shared small or empty view may still retain
the entire original allocation. Cumulative new payload, logical visible values,
retained backing storage, output snapshot bytes and process memory are different
quantities. See [budgets](budgets.md) and [memory lifetimes](memory-lifetimes.md).

E0032 identifies an incompatible/excessive shape or invalid dimension; E0033
identifies an invalid row/column and highlights its argument. E0015/E0024 retain
their type/argument meanings. E0017 is work exhaustion, E0036 is cumulative
payload exhaustion, E0035 is a failed bounded reservation, and E0029 is snapshot
exhaustion. No existing diagnostic code was renumbered.

## Executable demonstrations

Every source has a `.stdout` fixture checked by `tests/examples.rs` and by the
extracted native-package smoke tests:

- [Matrix-vector inference](../examples/matrix_inference.srp)
- [Rectangular linear composition](../examples/linear_transform.srp)
- [Classification score selection](../examples/matrix_classification.srp)
- [Softmax and cross-entropy](../examples/matrix_probabilities.srp)
- [One-hot classification targets](../examples/matrix_targets.srp)
- [A typed forward computation](../examples/matrix_forward.srp)
- [Shared windows and materialized transforms](../examples/tile_windows.srp)

```sh
cargo run --locked --offline -- run examples/matrix_forward.srp
cargo run --locked --offline -- check --max-vector-bytes 1024 examples/tile_windows.srp
```
