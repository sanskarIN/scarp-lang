# Finite decimal values

Status: implemented on `scarp/finite-decimals`, dependent on bounded loops.
Unreleased. The integer behavior of the published v0.0.1 is unchanged.

## Numeric contract

Decimal values use finite binary64 arithmetic. A literal needs a fractional
part or exponent: `1.25`, `1e2`, `2.5E-1`. Digits are required on both sides of
a decimal point and after an optional exponent sign. `.5`, `1.`, and `1e+` are
invalid. A minus remains a unary operator; signed integer-minimum rules remain.

Integers and floats are distinct inferred types. There are no implicit numeric
conversions: write `1.0 + 2.0`, not `1 + 2.0`. This avoids hidden loss of integer
precision. Existing integer `/` still truncates; float `/` produces a float.
All arithmetic, ordering, and compound assignment accept same-kind numeric
operands. Equality also requires matching types. Assignment retains its type.

Binary floating-point is approximate: `0.1 + 0.2` prints
`0.30000000000000004`. This is not arbitrary-precision decimal arithmetic.
Output uses Rust's shortest debug representation, preserving `.0` on integral
float values and `-0.0`. Scientific notation can appear for large/small values.
Positive and negative zero compare equal, but their signs are preserved.

Literals that cannot produce a finite value use `E0019`. Arithmetic producing
infinity or NaN uses `E0020` at the operator. Dividing by either zero sign uses
the existing `E0008`. Every intermediate result must be finite, even if later
operations could reduce its magnitude. Underflow to a finite subnormal or zero
is accepted, including literal underflow. No `nan`/`inf` literals are introduced.

Unselected arithmetic remains skipped; all literals, names, and types are
validated before execution. Checked failures retain empty output. Budgets and
fresh per-call state apply to numeric loops too.

## Rust API and representation

`ValueType::Float`, `Value::Float`, `ExpressionKind::Float`, and `TokenKind::Float`
extend the public enums. Hosts using exhaustive matches must handle them.
`FiniteFloat::new(f64)` returns `None` for nonfinite inputs. Its private storage
preserves reflexive equality without permitting a NaN value. `get()` retrieves
the scalar; `Value::as_float()` inspects a float without converting integers.
The integer-only standalone evaluator continues to reject noninteger results;
`evaluate_value` accepts every primitive kind.

Literal parsing and every arithmetic operation use the standard library. No
external numerical runtime, registry dependency, platform-specific extension,
fast-math assumption, or fused reassociation is introduced.

## Iterative example and checks

`examples/decimals.srp` refines an estimate of the square root of two and checks
a tolerance interval. Tests cover scientific spelling, binary rounding, signed
zero, underflow, literal/running overflow, both zero divisors, all comparisons,
compound mutation, error spans, selected paths, type boundaries, host invariants,
and repeated numerical execution. The previous decimal-negative case is replaced
with a malformed double-point literal. Token mutation includes invalid finite
representations, and packages smoke-test the numerical example.
