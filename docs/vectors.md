# Numerical vectors

The 0.0.4-alpha.2 implementation adds immutable, one-dimensional finite
binary64 vectors. Published alpha.1 supports scalar floats but not vectors.
See [release notes](releases/v0.0.4-alpha.2.md) for the planned publication
and the [integration audit](development/numerical-vectors-audit.md).

## Construct, inspect and replace

```scarp
bind values = [1.0, 2.0, 3.0, 4.0];
print(len(values), values[0]);
var weights = [0.2, 0.3, 0.5];
bind snapshot = weights;
weights = scale(weights, 2.0);
print(snapshot, weights);
```

Literals accept empty `[]` and trailing commas. Elements must be floats:
`[1]`, `[true]` and nested vectors are type errors. Length is limited to 65,536.
`v[i]` uses a zero-based integer; negatives, an index equal to length and indexing
an empty vector fail. Calls and groups compose with indexing, including
`softmax([1.0, 2.0])[0]`. Indexing binds before unary minus.

`bind` prevents replacement; `var` permits replacing the whole vector, including
with another length. The inferred type stays vector. No element assignment,
slice, implicit conversion or broadcasting is implemented. Use named kernels
instead of `+`, `*` or compound arithmetic assignments. `==`/`!=` compare contents;
different lengths compare unequal and signed zeros compare equal.

Copies and captured outputs share immutable storage. Replacing weights cannot
change snapshot or pending output. Display uses the scalar float formatter in
brackets, for example `[1.0, -0.0]`; binary rounding remains observable.

## Ten pure kernels

All arguments are positional. Scalar arguments must be float unless marked
integer. Each returned vector owns a new buffer; inputs are borrowed unchanged.

| Call | Result | Rules |
| --- | --- | --- |
| `len(v)` | integer | Constant-time; empty returns 0 |
| `sum(v)` | float | Compensated sum; empty returns +0.0 |
| `mean(v)` | float | Nonempty; compensated with scaled overflow fallback |
| `dot(a, b)` | float | Equal lengths; empty returns +0.0 |
| `mse(a, b)` | float | Equal nonzero lengths; scaled mean squared difference |
| `add(a, b)` | vector | Equal lengths; element addition |
| `scale(v, factor)` | vector | Float multiplication |
| `clamp(v, low, high)` | vector | Float low <= high, also for empty input |
| `fill(value, count)` | vector | Float value then integer count in 0..65536 |
| `softmax(v)` | vector | Nonempty; maximum-shifted normalized exponentials |

The unreleased old #18 used the opposite fill order. This milestone follows
`fill(value, count)` from the current specification; there is no overload.

```scarp
bind a = [1.0, 2.0, 3.0];
print(dot(a, [2.0, 2.0, 2.0]));
print(mse(a, [1.0, 2.0, 5.0]));
print(clamp(scale(a, 0.5), 0.0, 1.0));
```

## Numerical guarantees and limits

Every stored element and scalar result is finite. Invalid lengths, domains,
indices and nonfinite results are diagnostics, with no pending program output.
Underflow to signed zero and subnormal values are accepted. Indexing/copying
preserves zero signs; reductions start at +0.0 and can return positive zero.

Sum and dot use ordered Neumaier compensation. For example,
`sum([1e16, 1.0, -1e16])` retains 1.0. Products, running sum, correction and final
result must remain finite, even when later terms could cancel an overflow.
These algorithms are not exact, arbitrary precision or fully overflow-proof.

Mean keeps the compensated path when possible, then tries compensated termwise
averaging and finally maximum-magnitude scaling when earlier sums overflow:
`mean([1e308, 1e308])` remains finite. Termwise averaging retains tiny residuals
when enormous terms cancel; no finite-precision method here guarantees exact
relative accuracy for all ill-conditioned inputs.
MSE scales differences before squaring and averages before rescaling. Thus a
representable average need not fail just because an unaveraged square or sum
would overflow. Difference overflow and a truly nonfinite final result fail.

Softmax subtracts the maximum before exponentiation. An overflowing negative
shift contributes zero, and the maximum contributes 1.0. Compensated
normalization avoids a zero denominator. Transcendental rounding can vary by
platform: compare nontrivial probabilities with tolerances, not exact text.
The [contract](proposals/0017-numerical-vectors.md) cites algorithm references.

## Functions, contracts and names

```scarp
pure funcra twice(values: vector = [0.5, 1.0]) -> vector
    ensures len(result) == len(values);
{
    return scale(values, 2.0);
}
print(twice(), twice(values: [2.0]));
```

Vectors work with positional/named/default user arguments, recursive frames,
immutable parameters and needs/ensures contracts. All ten kernels are pure.
Native kernels add no user-call graph edge and consume no user-call depth/count;
their arguments still execute in source order and can call user functions.

Existing user functions named sum, len or another kernel keep precedence over
the builtin. A binding named sum remains independent of call position. A user
function that shadows a kernel makes that builtin unavailable by that name in
the program. Builtins do not support labels, defaults or first-class values.

## Deterministic work and storage

Keep all six existing execution options and CLI controls. Beyond dispatch tasks,
each literal/fill/add/scale/clamp/sum charges n steps, dot 2n, and mean/mse/softmax
4n. Length and indexing have constant dispatch cost. Equality charges the larger
input length; selected output preparation charges n. Work is prepaid before
kernel loops and payload allocation, including failing operations.

Each runtime-allocated element costs at least one step. Cumulative vector
payload allocation is therefore bounded by 8 * max_steps bytes, excluding
container/task overhead. The per-vector cap and fallible buffer reservation
also apply. Raising work limits raises this bound. No total process-memory cap
is promised: parsing, host allocations and small control allocations are outside
it. Legacy standalone expression evaluation has an unbounded work allowance;
use capped program APIs or the capped native API for controlled execution.

Snapshots charge 64 + 8n logical bytes per vector, even when sharing a buffer.
An empty vector costs 64 bytes. This is not a formatted-byte count. Run and check
apply identical work/capture limits; check discards snapshots without formatting
them. File/function slots retain their previous meaning. See [budgets](budgets.md).

| Code | Meaning |
| --- | --- |
| E0015 | Wrong element, index or argument type |
| E0017 / E0029 | Work / output snapshot quota exhausted |
| E0024 | Wrong arity or named arguments to a builtin |
| E0031 | Nonfinite numerical result |
| E0032 | Excessive/negative length or mismatched kernel shapes |
| E0033 | Index outside bounds |
| E0034 | Empty required input or reversed clamp bounds |
| E0035 | Bounded vector buffer reservation failed |

## Examples and host use

[Basic vectors](../examples/vectors.srp) cover indexing, snapshots and kernels.
[Probabilities](../examples/vector_probabilities.srp) normalize nonnegative
weights by their sum and demonstrate softmax. This is sum normalization, not
L2 normalization. [Coefficient fitting](../examples/coefficient_fit.srp) fits
y = coefficient * x with an explicitly written gradient and an MSE function.
No automatic differentiation, tensors or numerical library is implied.

Rust hosts use checked `NumericVector::new`, consuming `from_finite`, immutable
`as_slice`, `Value::as_vector` and capped `numeric::evaluate`. The owned Vec is
moved into Arc without a second element copy. No mutable slice escapes and no
host variable injection is added. See [embedding](embedding.md) and
[measured performance](performance.md#numerical-vector-measurements).

## Unreleased next milestone: stable L2 operations

The active `scarp/alpha3-vector-norms` development branch adds `norm(v)`
(returning a finite float) and `normalize(v)` (returning a new vector).
**These operations are not present in the published v0.0.4-alpha.2 binaries.**
They use a maximum-magnitude scaled reduction to avoid unnecessary square
overflow/underflow, with prepaid 4n work per call, the existing 65,536-element
bound and unchanged diagnostic codes. Empty/zero vectors cannot be normalized;
`norm([])` returns 0.0. A nonrepresentable final scalar norm still fails.
See [proposal 0018](proposals/0018-scaled-l2-norms.md) for precise semantics,
tests and release criteria. No alpha.3 release is currently authorized.
