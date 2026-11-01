# Repair numerical errors

In alpha.4, E0032 also identifies incompatible tile shapes and invalid dimensions.
For example, `matmul` reports both operand shapes and requires matching inner
axes. E0033 reports the row or column and highlights that index argument.
`matvec` expects a tile and a vector of its width; `tileadd` expects equal shapes;
`tilescale` takes a float factor. See [the tile contract](../tiles.md).
Matrix work exhaustion is E0017; fresh vector/tile payload exhaustion is E0036.

Use the error code, highlighted source and call trace together. Run/check
discard pending stdout and stderr on source, type, evaluation, contract or
quota failure. A diagnostic can quote a print statement from the source; that
quoted text is not replayed program output.

## Keep scalar and vector types explicit

```scarp-invalid
print([1, 2]);
```

This is E0015: numerical vector elements must be float. Write `[1.0, 2.0]`.
Likewise, fill takes value then count: `fill(0.5, 3)` is valid, while
`fill(3, 0.5)` has the wrong argument types. Native kernels accept positional
arguments; labels such as `sum(values: [1.0])` produce E0024. Put a kernel inside
a typed user function if a named-argument interface is useful.

If a kernel name behaves like a user function, inspect declarations with that
name. Existing user functions take precedence over newly introduced builtins.
A binding with that name remains separate from call position.

## Match binary shapes

```scarp-invalid
print(dot([1.0], []));
```

This is E0032. Dot, add and mse require equal lengths; mse also requires nonempty
inputs. Do not truncate or pad silently. Establish matching data or state the
requirement in a function contract:

```scarp
pure funcra product(a: vector, b: vector) -> float
    needs len(a) == len(b);
{
    return dot(a, b);
}
print(product([1.0, 2.0], [3.0, 4.0]));
```

This prints `11.0` followed by LF. A mismatched call fails the explicit needs
contract before entering the body. Negative fill counts or counts above 65,536
also produce E0032; lengths never wrap.

## Guard an index before reading

```scarp-invalid
print([1.0][1]);
```

This is E0033: valid indices start at zero and are strictly below length.
Negative indices are not reverse indexing. A selected empty input has no valid
index. For optional data, guard access explicitly:

```scarp
bind values = [];
bind index = 0;
if index >= 0 and index < len(values) {
    print(values[index]);
}
```

This program succeeds without output. Runtime bounds are checked only when the
access is selected; all branches still undergo static type checking.

## Development-only tile shape and cell errors

The **unreleased** two-axis numerical-tile prototype reports E0032 for
`tile([1.0], 2, 2)` (inconsistent dimensions) and negative or excessive
dimensions. `cell(tile([1.0], 1, 1), 1, 0)` reports E0033 because valid
rows are 0 through height minus one. `lane` performs the same row check.
An empty tile may have shape `0x3` or `3x0`, but no cell in either is
addressable. See [proposal 0021](../proposals/0021-immutable-numeric-tiles.md).
These features do not exist in published alpha.2.

## Decide what an empty result means

```scarp-invalid
print(mean([]));
```

This is E0034. Mean, mse and softmax require nonempty input. Sum and empty dot
return +0.0; empty add/scale/fill are allowed. Choose the domain policy in your
program rather than assuming every reduction has the same empty behavior.
Clamp also reports E0034 when its lower bound exceeds the upper bound.

## Development-only cumulative numerical payload budget

When a new vector element buffer would exceed the proposed
`--max-vector-bytes` limit, Scarp reports **E0036** before reserving the
buffer. Each element requires 8 logical payload bytes. For example,
`print([1.0, 2.0]);` needs 16 bytes; a limit of 15 fails, but 16
succeeds. Reusing an immutable vector in bindings and function calls
does not allocate another payload. The ledger is cumulative across
function invocations and loops and never refunds earlier materialization.
This is an **alpha.3 candidate** option, not present in alpha.2.
See [proposal 0023](../proposals/0023-vector-allocation-ledger.md).

## Distinguish overflow from an exhausted quota

| Code | Meaning | Repair |
| --- | --- | --- |
| E0030 | Malformed/nonfinite scalar literal | Use complete finite decimal/scientific spelling |
| E0031 | Nonfinite arithmetic or kernel result | Reduce magnitude or choose the operation you actually need |
| E0017 | Execution work exhausted | Bound the computation or deliberately adjust max-steps |
| E0029 | Logical output snapshot quota exhausted | Reduce selected output or adjust max-capture-bytes |
| E0035 | Bounded vector payload reservation failed | Reduce data size; this is not a type conversion error |
| E0036 | Cumulative numerical element payload budget exhausted (alpha.3) | Reduce allocations or adjust `--max-vector-bytes` |

For example, sum of `[1e308, 1e308]` overflows, while mean of those values is
representable. Request mean directly when that is the intended calculation.
Compensation improves cancellation but does not make sum/dot exact or immune
to overflowing intermediate values.

Work is charged before native allocation/loops. Each captured vector costs
64 + 8n logical bytes even when sharing storage; check enforces that cost too.
These controls do not measure every parser/host allocation or encoded output
byte. See [the vector contract](../vectors.md) and [all diagnostics](diagnostics.md).
