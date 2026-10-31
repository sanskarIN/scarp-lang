# Proposal 0024 — Bounded log-space vector kernels

Status: **implemented on a feature branch; not yet published**. These are
ordinary numerical algorithms, not proprietary or newly invented concepts.
Scarp specifies exact language types, finite-only values, resource charges,
and observable errors for its own execution model.

## Purpose

Machine-learning models frequently produce *logits*: unnormalized finite
numerical scores. Direct exponentiation of large positive logits can
overflow even when relative probabilities are well-defined. Scarp adds two
pure positional built-ins without introducing a dependency or new syntax.

| Function | Input | Result | Empty input |
| --- | --- | --- | --- |
| `logsumexp(v)` | vector | float | E0034 |
| `logsoftmax(v)` | vector | vector | E0034 |

```scarp
bind logits = [0.0, 0.0];
print(logsumexp(logits));   // approximately 0.6931471805599453
print(logsoftmax(logits));  // approximately [-0.693..., -0.693...]
```

The exact printing of transcendental results can vary by host platform.

## Numerical definition

Given nonempty finite entries `x_i`, set `m = max(x_i)`.
The finite range guarantees that a maximum exists. Compute

```text
s = compensated_sum(exp(x_i - m))
L = ln(s)
logsumexp(x)  = m + L
logsoftmax(x) = [ (x_i - m) - L for each i ]
```

- All shifts are nonpositive. If `x_i - m` overflows **negative** to
  `-infinity`, that term contributes zero to the exponential sum. Since a
  maximum contributes exactly one, the sum is always at least one.
- The `sum` reducer uses Scarp's ordered Neumaier compensation; finite
  `s <= 65536` and `L >= 0`.
- Each returned scalar/vector component goes through the existing finite
  gate. If a log probability is below the most-negative binary64 value
  (for example with two oppositely extreme finite logits), `logsoftmax`
  reports **E0031**, rather than storing `-infinity`.
- Transcendental rounding and relative-error limits remain those of
  binary64 libm. `logsumexp` near the largest finite scalar can round to
  that scalar even if the exact real result is slightly larger. This is an
  explicit floating-point limitation; no arbitrary precision is implied.
- All user-provided values and results remain finite, with no implicit
  conversions or dimension expansion. User `funcra` declarations retain
  precedence when they use these names.

## Deterministic work and memory

Both kernels precharge `4 * len(v)` native work steps, then perform the
ordered reduction. `logsumexp` returns a scalar with **zero new vector
payload bytes**. `logsoftmax` must materialize one new immutable vector,
charged at `8 * len(v)` bytes against the cumulative
`max_vector_bytes` ledger **before** reservation.

No mutable source buffers, new user frame, garbage-collector rules,
ownership syntax, third-party code or outside API are added.
The source/runtime tests require exact work and allocation boundaries,
atomic stdout/stderr under domain and finite errors, function shadowing,
typing and nontrivial numerical tolerances.

## Release discipline

This proposal does not change `Cargo.toml`, publish a tag or request a
prerelease. It is deliberately independent of unmerged compiler retirement,
vector-window and two-axis tile drafts. If the value model changes, confirm
the log-space allocation gate still applies to newly materialized vectors.
