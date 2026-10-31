# Proposal 0027 — Explicit categorical-loss gradient and target encoding

Status: **development only**. The implementation is not yet merged or
published. These two built-in kernels extend the released Scarp language's
finite binary64 vector model and the newer, unreleased classification-loss
feature. They are **not** automatic differentiation, a general tensor
library, a training optimizer, or a new programming-language syntax.

## Functions

| Call | Input types | Output | Purpose |
| --- | --- | --- | --- |
| `lossgrad(logits, target)` | vector, integer | vector | Derivative of categorical cross-entropy with respect to every logit |
| `onehot(count, target)` | integer, integer | vector | Construct a finite, zero-filled count-element vector with a 1.0 at the selected target |

A target is always a **zero-based integer**, not a float or probability.
For `lossgrad`, empty input fails E0034. A target outside the logits'
length fails E0033, including negative indices. A one-hot count is in
`1..=65536`: negative or oversized counts fail E0032, zero fails
E0034, and target outside that count fails E0033.

## Numerical behavior

Let the input logits be a nonempty vector of finite binary64 scores `x`,
target index `k`, and maximum value `m`. First calculate a
max-shifted compensated log partition:

```text
s = sum_exp_compensated(exp(x_j - m) for j in 0..n)
log_s = ln(s)
p_i = exp((x_i - m) - log_s)
lossgrad(x, k)[i] = p_i - (1 if i == k else 0)
```

If the intermediate `x_i - m` overflows negatively, `p_i = 0`:
the corresponding probability is mathematically negligible and Scarp
never stores the nonfinite intermediate. This handles opposite-signed
extreme logits even when their difference does not fit binary64.

For two identical logits, the gradient is `[-0.5, 0.5]` when class
zero is selected. For a single logit, the gradient is `[0.0]`. The
elements approximately sum to zero (within floating-point rounding);
they are not adjusted afterward to force an exact algebraic sum.

The result has the same vector length as the input. The original input
is never modified. This explicit gradient is specific to the built-in
single-class categorical loss and is not a framework-wide autograd graph,
numerical gradient approximation, Jacobian, or optimizer.

`onehot(count, target)` allocates a new immutable vector of zeros and
sets exactly the target element to positive 1.0. It does not share any
previous vector and does not silently clamp or wrap invalid indices.

## Budget and failure contract

- `lossgrad` precharges **6n work steps**, then **8n new vector payload
  bytes** before buffer reservation. It uses the preexisting log
  partition and never creates a second vector.
- `onehot` precharges **n work steps**, then **8n new vector payload
  bytes**, all before filling the result.
- A program's vector-literal allocation is separately cumulative under
  `max_vector_bytes`; argument sharing does not double-count payload.
- Work exhaustion is E0017; allocation quota exhaustion is E0036.
  Failed `run` and `check` never publish pending stdout or stderr.
- No general process-memory cap is promised; Arc metadata, parser,
  source strings, stacks and host allocations remain outside this quota.
- All stored scalars are finite; the implementation uses the same
  safe Rust implementation and checked vector length cap.

## Example

```scarp
bind logits = [0.0, 0.0];
bind target = 1;
print(onehot(2, target));
print(lossgrad(logits, target));
```

Outputs:

```text
[0.0, 1.0]
[0.5, -0.5]
```

## Validation plan

Require numeric equality to known reference results, extreme positive/
negative logits, subnormal inputs, empty/invalid values, type and arity
errors, signed zeros, source/user function shadowing, structured
`funcra` contracts, zero/one-byte-under quota boundaries and maximal
65,536-element construction. Cross-platform Rust 1.74, Windows/macOS/
Linux, exact-output examples and all three native package jobs must
pass on the exact PR head before merging. No tag, version bump, or
prerelease should be made for this feature.
