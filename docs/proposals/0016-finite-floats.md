# Finite binary64 scalar contract

Status: contract specified before implementation; integrated into main through
[PR #45](https://github.com/sanskarIN/scarp-lang/pull/45) for 0.0.4-alpha.1.
The canonical type name is `float`, consistent with the earlier scalar proposal.
It means finite IEEE-754 binary64, not exact decimal arithmetic.

## Grammar and diagnostics

```ebnf
digits = DIGIT, { DIGIT } ;
exponent = ("e" | "E"), [ "+" | "-" ], digits ;
float = digits, ".", digits, [ exponent ] | digits, exponent ;
```

Leading zeros are decimal. The sign is the existing unary operator. Plain digit
literals remain i64, including their existing minimum-value special case.
`1.`, `.5`, `1e`, `1e+`, `1.2.3`, and float suffixes are rejected: write `1.0`,
`0.5`, or a complete exponent. Token spelling and byte spans retain the source.
Lexing rejects malformed forms; parsing rejects nonfinite literal magnitudes.
`tokens` does not evaluate/range-check otherwise well-formed float spellings.
Append InvalidFloat E0030 and NonFiniteNumber E0031; retain E0001–E0029.

## Values, types, and operations

FiniteFloat has private f64 storage and a checked constructor returning None for
NaN or either infinity. No unchecked public constructor, raw-f64 enum variant,
unsafe code, or dependency is added. Finite IEEE numeric equality is reflexive,
symmetric and transitive, so a documented Eq implementation is valid; signed
zeros compare equal while get().to_bits() and formatting preserve their signs.
No Hash/Ord contract is introduced.

Bindings infer float from the initializer and retain that type. Parameters,
defaults and results may explicitly use float. Unary minus and same-type
arithmetic/comparisons work. Integer/float mixtures are E0015 for operands, equality, arguments, defaults
and assignment. Invalid function return types retain E0021. No implicit or explicit conversions
are added. Repeat counts remain integer and conditions/contracts remain Boolean.

Every literal and arithmetic result must be finite. Overflow-to-infinity is a
source-linked error. Division by either signed zero is E0008 before performing
the operation. Existing integer overflow/division behavior remains unchanged.
Underflow is allowed: subnormals are retained and smaller rounded values may
become signed zero. Binary rounding is observable, e.g. 0.1 + 0.2 need not be 0.3.

Output uses Rust's shortest round-trip f64 debug formatting, retaining a float
marker for integral values (1.0), signed zero (-0.0), and scientific notation for
small/large values. Regression fixtures cover supported toolchains; round-trip
bit preservation is tested across finite patterns. Formatting is locale-free;
the fixed/scientific cutoff is a presentation choice, not a new numeric type.
See the [Rust f64 reference](https://doc.rust-lang.org/std/primitive.f64.html).

## Execution and compatibility

Reuse the flat expression tasks and heap function frames. Float values copy
their scalar payload, including results and pending output; no new host recursion
or mutable shared state is introduced. Existing work/output/depth/call/live-slot
and snapshot budgets apply, including during silent check. A float snapshot is
an eight-byte scalar payload in the existing logical storage accounting, not an
encoded-output-byte cap. Source/runtime/contract/budget failures discard both
streams; sink failures retain their documented partial-write behavior.

Public enums gain float variants, requiring exhaustive host matches to update.
The integer evaluator helper retains its i64 result; evaluate_value handles
float results. The type word remains contextual. Existing examples, function
semantics, CLI spellings, and zero third-party dependency policy are preserved.
