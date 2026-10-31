# Proposal 0026 — Stable categorical logit loss and selected log probability

Status: **implemented in a feature branch; not yet published**. Scarp
introduces two pure numerical builtins that operate directly on a finite
vector of raw *logits* and a zero-based integer target class. No softmax
buffer, log-softmax buffer, GPU, automatic differentiation or network
dependency is introduced.

| Call | Inputs | Return | Empty input |
| --- | --- | --- | --- |
| `crossentropy(logits, target)` | vector, integer | nonnegative float | E0034 |
| `logprob(logits, target)` | vector, integer | nonpositive float | E0034 |

For nonempty logits `x` and integer `i` in `[0, len(x))`:

```text
m = max(x)
p = compensated_sum(exp(x_j - m) for each j)
gap = m - x_i
loss = gap + ln(p)
crossentropy(x, i) = loss
logprob(x, i) = -loss
```

This is the **single-class categorical cross-entropy / negative log
likelihood** of a finite logit vector. It is not arbitrary multi-label
cross-entropy, label smoothing, class-weighted loss or a whole-batch
mean. The target index does not accept float or probability-vector types.

The implementation calls the existing `log_partition` reduction shared
with `logsumexp` and `logsoftmax`, then computes the difference between
the finite maximum and the target *before* adding the small log-sum. It
does **not** compute `logsumexp(logits) - logits[target]`: separately
rounding a large partition to the largest logit could discard a small,
nonzero loss (for example two identical `1e308` scores should cost
approximately `ln(2)`).

If `x_j - m` underflows to negative infinity as an intermediate it
contributes zero to `exp`, as in Scarp's log-space kernels. The maximum
contributes one, so the compensated sum remains in `[1, n]`.
Intermediate differences and final results are checked for finite
binary64 values. An exceptionally large real loss that cannot be
represented produces existing E0031. This is more accurate for many
common numerical cases but **not** exact real arithmetic and not an
all-purpose overflow-proof transcendental implementation.

## Target validation and diagnostics

- Every input has a checked signature: `vector, integer -> float`.
- An empty logits vector produces **E0034**, before any target lookup.
- A negative or out-of-range target index produces **E0033**, including
  the provided index and the vector length. No reverse indexing.
- A float label or a non-vector first argument produces **E0015**.
- Bad arity or named calls produce **E0024**.
- Existing user-defined `funcra` declarations retain call-name
  precedence and do not become ambiguously overloaded.
- Failed `run` and `check` discard all pending stdout/stderr as with
  other diagnostics; no partial output from a failed loss computation.

## Resource and memory boundary

Both kernels precharge **4n numerical work steps** for a length-n vector.
They allocate no new numerical element buffer and spend **zero bytes**
of the existing `max_vector_bytes` ledger, even for large logits.
The vector argument itself, if created from a literal or kernel, is
charged according to the ordinary buffer rules. No third-party
dependencies or user-visible manual memory syntax are added.

This implementation is independent of the experimental scope-retirement,
vector-window and 2D tile draft PRs. A later integration must preserve
typed index validation and zero-allocation semantics. It must also test
extreme finite inputs, subnormals, user-function shadowing, exact-work
boundaries, call contracts, and output atomicity.

## Examples

```scarp
bind logits = [1000.0, 1000.0];
bind chosen = 0;
print(crossentropy(logits, chosen)); // approximately 0.6931471805599453
print(logprob(logits, chosen));      // approximately -0.6931471805599453
```

Published alpha.3 binaries do **not** include these operations.
Publication or version changes require a separately tested release
request and verification of all release assets.
