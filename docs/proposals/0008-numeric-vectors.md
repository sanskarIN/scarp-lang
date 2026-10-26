# Immutable numerical vectors and native kernels

Status: implemented on `scarp/numeric-vectors`, dependent on finite decimals and
bounded loops. Unreleased. Kernels use standard-library Rust code without a new
registry dependency, external runtime, network service, or accelerator.

## Values and expression syntax

`[1.0, 2.0]` constructs an immutable vector of finite float values. Empty `[]`
and trailing commas are allowed. Integer, string, boolean, and nested-vector
elements are type errors; there are no hidden numeric conversions. A vector has
at most 65,536 elements. Literal length is checked during parsing; dynamic
constructor lengths and binary shapes are checked at runtime.

`vector[index]` returns a float with a zero-based integer index. Negative and
out-of-range indices are errors, not wrapping or reverse indexing. Calls and
index expressions compose: `softmax(values)[0]` is valid. Calls, vectors, indices,
and parentheses share the 64-level structured expression nesting limit. Flat
lists, operator chains, and repeated postfix parsing remain iterative.

Vector copies share contiguous immutable finite storage. `let mut values` allows
replacement of the whole vector, including a different length, but not a change
of inferred kind. Existing copies and captured prints retain their contents.
There is no element assignment. Use native operations to produce a new vector.
`==`/`!=` compare vector contents; other operators do not broadcast or perform
vector arithmetic. Printing uses `[1.0, 2.0]` formatting.

## Fixed built-in signatures

| Expression | Result | Behavior |
| --- | --- | --- |
| `len(v)` | integer | Number of elements, constant-time |
| `sum(v)` | float | Compensated reduction; empty result is 0.0 |
| `mean(v)` | float | Compensated sum divided by nonzero length |
| `dot(a, b)` | float | Same-length products with compensated accumulation |
| `mse(a, b)` | float | Mean squared element difference; equal nonzero lengths |
| `softmax(v)` | vector | Maximum-shifted exponentials normalized to probabilities |
| `add(a, b)` | vector | Same-length element addition |
| `scale(v, factor)` | vector | Multiply by a float factor |
| `clamp(v, low, high)` | vector | Float bounds with low <= high |
| `fill(count, value)` | vector | Integer count and float value |

Call names are recognized in call position, independently of binding names.
`let sum = sum(values);` is legal; a local binding does not replace the built-in
call. This preserves existing identifiers. Built-ins are not first-class values,
user-defined functions, callbacks, or an extensible package system. Unknown calls
and wrong arity are static errors. Argument and result types are fixed.

## Numerical behavior

Sum, dot, mean, and squared-error reduction use Neumaier compensation, preserving
small cancellation terms in cases such as `[1e16, 1.0, -1e16]`. Products,
differences, accumulation, corrections, and final division must remain finite.
This does not promise arbitrary precision or exact answers for all inputs.

Softmax subtracts the maximum before exponentiation. The maximum contribution
is 1.0, preventing an all-zero normalizer. An extreme negative difference that
overflows during shifting contributes zero rather than propagating infinity.
Finite exponentials can underflow to zero. Other kernels retain checked finite
arithmetic; no fused reassociation or unchecked fast-math mode is enabled.
Exponential rounding can vary across platform math implementations, so use
tolerances for nontrivial probability results rather than exact text equality.

## Work bounds and diagnostics

Native work shares the program's execution budget. Literal materialization,
fill, sum/mean, add, scale, clamp, vector equality, and vector print preparation
charge one step per element in addition to expression/instruction steps. Dot
and mse charge two per element; softmax charges four. Length and indexing are
constant-time. Kernel work is charged before its allocation or element loop.
Checks discard print values while enforcing the same work and print counts.
This bounds repeated native work as well as interpreted loops.

| Code | Failure |
| --- | --- |
| E0015 | Element, index, or function argument type mismatch |
| E0017 | Shared work budget exhausted |
| E0020 | Nonfinite numerical result |
| E0021 | Excessive/negative length or mismatched binary shapes |
| E0022 | Index outside vector bounds |
| E0023 | Empty required input or reversed clamp bounds |
| E0024 | Unknown built-in call |
| E0025 | Wrong argument count |

Static checks inspect unselected expressions; runtime shape, bounds, and kernel
errors occur only when reached. All failures leave program output empty.

## Library and validation

`examples/linear_fit.srp` fits the known relation `y = 2*x`. It computes the
explicit squared-error gradient `2*dot(inputs, residuals)/3`, updates one scalar
coefficient for 40 iterations, and checks both the coefficient tolerance and
final loss. Run it with `cargo run -- exec examples/linear_fit.srp`; both checks
print `true`. The derivative is written in the program; automatic gradient
tracking is not part of this feature. The example exercises vectors, typed
scalars, native kernels, per-iteration locals, and deterministic budgets together.

`NumericVector::new(&[f64])` rejects nonfinite or oversized host input. Its
immutable slice contains `FiniteFloat` values. `Value::as_vector` inspects it.
New public syntax/value variants and `numeric::Builtin` require matching hosts
to account for vectors, calls, and indexing. Source and runtime values own their
data; there is no external variable injection or shared mutable cache.

Tests cover all ten signatures, cancellation, large-input softmax, squared error,
shapes, indices, empty inputs, fixed kinds, snapshots, host invariants, maximum
length, nested syntax, shared budgets, CLI errors, and concurrent execution.
The native-dot and indexed-loop benchmarks verify the same exact output before
timing. This feature supplies one-dimensional numeric building blocks; matrices,
automatic differentiation, device execution, and model runtimes remain future work.
