# Proposal 0018: scaled L2 norms (unreleased alpha.3 work)

Status: **implemented on a development branch, not included in v0.0.4-alpha.2**.
This proposal does not request a tag, prerelease, package-version change or
publication. The released alpha.2 API and assets remain immutable.

## Motivation

A dot product is not a numerically safe way to compute the Euclidean norm:
`dot([1e308], [1e308])` must reject its overflowing product, even though
the norm of that one-element vector is representable. Squaring very small
inputs can underflow before the square root is taken. A practical numerical
language needs an L2 operation that handles both cases without changing the
existing checked, finite-only vector model.

## Syntax and types

Both functions are built-in, pure, positional calls. Existing user-defined
functions with the same names retain precedence over these builtins.

| Call | Parameter | Result | Empty / zero input |
| --- | --- | --- | --- |
| `norm(v)` | `vector` | `float` | Empty and all-zero vectors return positive 0.0 |
| `normalize(v)` | `vector` | `vector` | Empty and all-zero vectors fail with E0034 |

No vector operator overloading, element mutation, imports, implicit
integer-to-float conversions or named built-in arguments are introduced.
Results remain finite; `norm` reports E0031 when the **actual** L2 magnitude
cannot fit in a finite binary64 scalar, while `normalize` can still succeed.

```scarp
bind a = [3.0, 4.0];
print(norm(a));
print(normalize(a));
```

Expected output:

```text
5.0
[0.6, 0.8]
```

## Stable two-pass algorithm

For finite components `x_i`, compute `scale = max(abs(x_i))`.

- When `scale == 0.0`, `norm` returns positive zero and `normalize`
  reports E0034; this includes both empty vectors and signed zeros.
- Otherwise calculate `sumsq = compensated_sum((x_i / scale)^2)`, using
  the same ordered compensation helper as other kernels.
- Set `magnitude = sqrt(sumsq)`. The scaled sum is between 1 and the
  vector's length, inclusive; the bounded size makes it finite.
- `norm` returns the checked finite value `scale * magnitude`.
- `normalize` allocates a new vector whose elements are
  `(x_i / scale) / magnitude`. Division is deliberately performed in
  this order: it avoids computing the potentially overflowing norm.
  Shared original inputs are not modified.

All vector entries are finite by construction. Very small results may
underflow to signed zero; component signs and signed zero are preserved
during normalization. Floating-point rounding means the returned vector
is not promised to have an exactly representable norm of 1.0. No arbitrary
precision or guaranteed cross-platform bitwise equality is implied.

## Work, memory and diagnostics

- Charge **4 * len(v)** steps up front for either operation, before
  reduction or allocating an output vector. Zero-length norm costs zero
  native kernel steps; ordinary dispatch still has its usual cost.
- `normalize` uses the existing fallible bounded buffer and returns
  an owned immutable vector. The maximum length remains 65,536.
- E0015 reports type mismatches, E0024 reports arity or named-builtin
  arguments, E0034 reports empty/zero normalization, E0031 reports a
  nonfinite L2 scalar result, E0017 reports work exhaustion, and E0035
  reports output allocation failure.
- Run and check use the same evaluation and budgets; runtime failures
  discard pending stdout/stderr rather than exposing partial output.
- No ExecutionOptions fields, default caps or existing error numbers change.
  Budget limits do not cap total process memory.

## Validation and release constraints

The independent test target `tests/vector_norm.rs` checks the public
interpreter and host API; 3-4-5 vectors, large magnitude and finite
norm overflow, subnormals, zero signs, user-name precedence, pure
contracts, static type/arity checks, atomic output and exact step caps.

Before inclusion in a future alpha.3 candidate, require debug/release
and minimum-Rust suites, source checks across Windows/Linux/macOS,
native package smoke tests, documentation/examples validation and
a fresh reviewed release note. Do not modify the active alpha.2
tag, source archives or published binaries. A merge or changed
package version alone is not permission to publish.
