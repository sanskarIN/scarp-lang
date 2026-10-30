# Finite numerical vector contract

Status: specified before implementation for the 0.0.4-alpha.2 milestone.

`vector` denotes a one-dimensional immutable sequence of finite binary64 floats.
Literals `[1.0, 2.0]`, empty `[]` and trailing commas are supported; every element
must have type float. No nested vectors or implicit conversions. The maximum
length is 65,536, checked statically for literals and dynamically for fill.
Postfix `expression[index]` binds before unary minus and returns a float. Indices
are zero-based integers; negative, equal-to-length and empty indexing fail.
Calls, list elements, indices and groups share the 64-level structured nesting
bound. Flat nodes and postfix chains remain iterative.

Bindings infer a fixed type. A var may replace its whole vector with any valid
length; element assignment and arithmetic operators on vectors are unsupported.
Equality compares numeric element contents, including equal signed zeros.
Float element formatting is reused inside `[1.0, -0.0]` output. Stored copies and
output snapshots share immutable contiguous storage through Arc<Vec<FiniteFloat>>;
wrapping an owned buffer does not copy its elements. No mutable slice escapes.
Checked Rust constructors reject nonfinite or oversized input.

Typed funcra parameters/results/defaults accept `vector`. Existing user functions
take precedence over new builtin names, preserving programs with a funcra named
sum or len. Local bindings keep their separate namespace. Kernels are pure,
fixed positional calls; named/default arguments remain supported for user calls.
Kernel calls do not consume user-call depth/count or add user call-graph edges.

| Call | Result | Empty/domain rules | Additional work |
| --- | --- | --- | --- |
| len(v) | integer | Empty returns 0 | 0 |
| sum(v) | float | Empty returns +0.0 | n |
| mean(v) | float | Nonempty | 4n |
| dot(a, b) | float | Equal lengths; empty returns +0.0 | 2n |
| mse(a, b) | float | Equal nonzero lengths | 4n |
| add(a, b) | vector | Equal lengths; empty allowed | n |
| scale(v, factor) | vector | Float factor; empty allowed | n |
| clamp(v, low, high) | vector | Float bounds low <= high, including empty | n |
| fill(value, count) | vector | Float value, integer count 0..65536 | count |
| softmax(v) | vector | Nonempty | 4n |

Sum and dot use ordered Neumaier compensation, checking products, running sum,
correction and result; a nonfinite intermediate is an error even if later terms
could cancel. Mean first attempts compensated sum/division, then compensated
termwise averaging and finally maximum-magnitude scaling on overflow. MSE scales differences by their
maximum magnitude before squaring and averages before rescaling, avoiding a
needless overflowing square when the mean is representable. Rounded binary64
results are not exact or arbitrary precision. Zero underflow/subnormals remain
valid. Reductions start at +0.0; copying/indexing preserves element zero signs.

Softmax subtracts the maximum, uses compensated normalization and permits an
overflowing negative shift to contribute zero (its exponential underflows).
The maximum contributes 1.0. Exponential rounding can differ across platforms;
nontrivial probabilities require tolerance comparisons. See the primary
[softmax error analysis](https://academic.oup.com/imajna/article/41/4/2311/5893596)
and the [scaled sum-of-squares principle](https://netlib.org/lapack/explore-html/d8/d76/group__lassq.html).
These are algorithm references, not imported dependencies or copied routines.

Work is prepaid before each kernel loop or payload allocation; each allocated
element consumes at least one execution step. Thus cumulative runtime vector
payload allocation is bounded by 8 * max_steps bytes, excluding container and
temporary scalar/task overhead. An owned Vec is moved into Arc without an extra
payload copy. Per-vector caps, fallible buffer reservation and the work quota
prevent unchecked dynamic construction. Raising/disabling work limits raises
this allocation bound. This is not a total process-memory limit; parsing and
host constructors have separate documented constraints.

Literal construction additionally charges n, equality charges max input length,
and each selected vector output charges n before replay. Snapshots charge
64 + 8n logical bytes per captured value even if shared. Run and check apply
identical work/capture limits; check does not retain or format snapshots. Frame
slots, user-call limits and all six public option fields keep their meanings.
Source/evaluation/contract/budget failure replays neither output stream.

Preserve E0001–E0031. Add E0032 for invalid/mismatched lengths, E0033 for bounds,
E0034 for numeric domains, E0035 for a failed vector buffer reservation. Existing
E0015 handles type errors, E0024 call arguments, E0017 prepaid work exhaustion,
E0029 snapshot exhaustion and E0031 nonfinite results. Diagnostics retain UTF-8
byte spans and bounded user call traces.

No tensors, differentiation, bitwise/native stack, CLI rename, dependency or
release publication is included. Alpha.2 tagging/publication requires a later
explicit maintainer instruction.
