# Finite floating-point values

This source checkout adds the scalar float foundation for 0.0.4-alpha.1.
Published beta.3 does not support floats. The implementation uses finite IEEE-754
binary64, not exact decimal arithmetic. This guide covers scalars; the alpha.2
candidate adds [vectors](vectors.md). Implicit conversions remain unavailable.
See [the design](proposals/0016-finite-floats.md) and
[integration audit](development/finite-floats-audit.md).

## Literals and types

| Source | Meaning |
| --- | --- |
| `0.5`, `12.25`, `0001.5` | Fractional float literal |
| `1e3`, `2.5E-4`, `1e+2` | Float with decimal exponent |
| `1` | Signed i64 integer, not implicitly a float |
| `-0.0` | Unary minus applied to a float zero; sign retained |
| `1.`, `.5`, `1e`, `1e+`, `1.2.3`, `1.0_suffix` | Invalid float spelling, E0030 |
| `1e309` | Well-formed token but nonfinite literal magnitude, E0030 during parsing |

Both sides of a decimal point require digits. An exponent requires digits after
its optional sign. There are no numeric separators, radix float literals, or
special NaN/infinity literals. `tokens` preserves the original float spelling
and byte span; it can list `1e309` because token inspection does not range-check.
Malformed spellings fail lexing even in an unselected branch.

```scarp
bind fraction = 0.5;
var total = 1.0;
total += fraction;
print(total, 1 / 2, 1.0 / 2.0);
```

Exact stdout: `1.5 0 0.5\n`. Bindings infer their primitive type and retain it
on assignment. Local type annotations are not introduced. Float supports unary
minus, `+`, `-`, `*`, `/`, and all six comparisons. Operands must share a numeric
kind; `1 + 0.5`, `1 == 1.0`, and assigning an integer to a float binding are E0015.
Integer arithmetic and its overflow checks remain unchanged. Repeat counts
remain integer; guards and contracts require Boolean results.

## Functions and contracts

```scarp
pure funcra advance(value: float, step: float = 0.25) -> float
    needs step >= 0.0;
    ensures result >= value;
{
    return value + step;
}
print(advance(1.0), advance(step: 0.5, value: 2.0));
```

Exact stdout: `1.25 2.5\n`. Positional/named/default arguments, recursion, purity,
returns, and needs/ensures follow [the existing function rules](functions.md).
Arguments and defaults are checked without int/float conversion. Invalid return
types retain E0021, while operand/assignment/default/argument mismatches use
E0015. Unused bodies and skipped paths still receive static checks.

## Rounding and failures

Every runtime float is finite. A checked constructor excludes NaN and either
infinity; arithmetic checks each result before storing or returning it.
Nonfinite results report E0031 at the operator, including a compound-assignment
operator. Division by either signed zero reports E0008 before the operation.
Such failures discard both pending output streams, including earlier prints.

Binary rounding is visible: `0.1 + 0.2` displays `0.30000000000000004`. Equal
decimal-looking expressions need not have equal binary results. Choose a
domain-appropriate tolerance when comparing approximations; this feature does
not provide exact decimal, arbitrary precision, or rounding-mode controls.

Underflow is accepted. Subnormals are retained (`5e-324` is the smallest positive
binary64 value); smaller rounded magnitudes may become zero. Negative underflow
preserves negative zero. `0.0 == -0.0` is true and neither orders before the other,
while output distinguishes `0.0` and `-0.0`. Negation toggles the sign of zero.

Output uses shortest round-trip finite-f64 debug formatting: whole float values
keep a float marker (`1.0`), small/large magnitudes may use scientific notation,
and the sign of zero is preserved. Formatting is locale-free. Supported-toolchain
fixtures check representative strings; finite bit patterns are also tested for
round-trip parsing. The fixed/scientific cutoff is a presentation choice.

## Embedding, budgets, and scope

`value::FiniteFloat::new(f64)` returns None for NaN/infinity. Its private payload
cannot be mutated through the public API. `get()` and `Value::as_float()` inspect
the finite value. Numeric Eq remains valid because NaN is excluded; signed zeros
compare equal. Use `get().to_bits()` when the sign of zero matters. No Hash or
Ord implementation is added. Syntax hosts must handle the new float variants
in TokenKind, ExpressionKind, Value and ValueType.

`evaluate_value` returns float values; the older integer-only `evaluate` helper
keeps its i64 result and rejects a float result. Both output streams use the same
formatting. Captured scalar values and function results retain value snapshots
after later mutations. Existing compiled programs remain immutable and reusable.

All six work/output/depth/call/live-slot/snapshot limits apply during run and
silent check. Float expression tasks have the same accounting as other scalar
tasks, and a captured float has the existing eight-byte scalar payload charge.
This is logical storage accounting, not an encoded-output or process-memory cap.
Writer failures during replay may still leave earlier bytes.

Try [root refinement](../examples/float_refinement.srp) and
[smoothing](../examples/float_smoothing.srp). Vectors, tensors, native compilation,
bitwise operations, and CLI redesign remain separate milestones.
