# Proposal 0025 — Stable vector extrema and decoding selectors

Status: **development branch only**. This proposal adds deterministic
selection of a finite vector's best score and its original index.

## Functions and types

| Call | Result | Meaning |
| --- | --- | --- |
| `argmax(v: vector)` | integer | First index of the greatest value |
| `argmin(v: vector)` | integer | First index of the least value |
| `maxval(v: vector)` | float | Greatest value, original floating bits |
| `minval(v: vector)` | float | Least value, original floating bits |

Each requires at least one finite entry. An empty vector fails with E0034,
rather than returning a fake or sentinel index. The result index is zero-based
and fits in `integer` because a vector has at most 65,536 elements.

```scarp
bind scores = [1.0, 3.0, 3.0];
print(argmax(scores));  // 1; ties always choose the first position
print(maxval(scores));  // 3.0
```

## Deterministic ties and numerical values

Scanning proceeds left-to-right. An entry only replaces the selected
candidate if it is **strictly** greater (`argmax`/`maxval`) or strictly
less (`argmin`/`minval`) than the current selection. Equal values do not
change the original winner. This also matters for signed zero: `+0.0` and
`-0.0` are numerically equal in Scarp, but their distinct IEEE-754 signs
are preserved in the `maxval`/`minval` result by returning the selected
original finite value rather than re-encoding it. Nonfinite values cannot
enter Scarp numeric vectors; subnormals are compared normally.

A function declaration with the same name takes precedence over a builtin,
per existing Scarp function resolution. No implicit float/integer conversion,
sorting, changing the input or package dependency is introduced.

## Resource accounting

Each function precharges exactly `n` numerical work steps (beyond existing
language dispatch). This makes even a 65,536-element selection bounded
before entering its scan. They are allocation-free: `0` additional bytes
to `max_vector_bytes`, including their float-valued variants, which reuse
one existing finite payload value. They do not allocate a new vector, add
user call frames or modify the existing output snapshot semantics.

## Validation

Regression tests cover ordinary ties, signed-zero bit preservation, IEEE
subnormals, type errors, arity errors, empty E0034, checked `funcra`
contracts, user-defined function precedence, output atomicity,
exact work/zero allocation and maximum-length tie inputs.

The implementation is independent of the unmerged retirement, vector
windows and multidimensional tile drafts, and does not change the public
release version or publish a tag.
