# Values and operators

Use this reference when an expression's result or type differs from what you
expected. Checked integer arithmetic works in v0.0.1 and development Scarp;
bindings and string-valued expressions require the development stack.

## Primitive values

| Kind | Examples | Rules |
| --- | --- | --- |
| Integer | `0`, `42`, `-7` | Signed 64-bit values; decimal ASCII digits |
| Float (alpha) | `0.5`, `1e3`, `-0.0` | Finite binary64; no integer coercion |
| Vector (alpha.2 candidate) | `[1.0, -0.0]`, `[]` | Immutable finite floats, at most 65,536 elements |
| Boolean | `true`, `false` | v0.0.2 and later; explicit booleans without truthiness |
| String | `"Scarp"`, `""`, `"line\nbreak"` | UTF-8 text; v0.0.3 adds tab/CR to the three older escapes |

Development bindings infer their kind from the initializer. Mutable assignment
must preserve that kind. Strings do not automatically convert to integers, and
integers do not automatically become strings for arithmetic.

The [alpha](../floats.md) adds finite binary64 float values.
The alpha.2 candidate adds [numerical vectors](../vectors.md); general collections
and user-defined values remain unavailable.
Local type annotations and explicit conversions are not implemented. On the
[function system in beta.3](../functions.md), parameter and result types are
mandatory; they do not change primitive value or operator rules.

## Precedence and associativity

| Order, strongest first | Syntax | Behavior |
| --- | --- | --- |
| Grouping | `(expression)` | Evaluate the grouped expression |
| Indexing | `expression[index]` | Zero-based checked vector access before unary minus |
| Unary minus | `-primary` | Integer checked negation or float sign change |
| Multiplication/division | `*`, `/` | Left-associative same-type numeric arithmetic |
| Addition/subtraction | `+`, `-` | Left-associative same-type numeric arithmetic |
| Comparison | `==`, `!=`, `<`, `<=`, `>`, `>=` | One comparison; ordering requires matching numeric kinds |
| Boolean negation | `not` | Negate a boolean, including a complete comparison |
| Boolean conjunction | `and` | Skip the right boolean if the left is false |
| Boolean disjunction | `or` | Skip the right boolean if the left is true |

```scarp
print(1 + 2 * 3);
print((1 + 2) * 3);
print(8 - 3 - 1);
print(24 / 3 / 2);
print(-7 / 2);
```

Output:

```text
7
9
4
4
-3
```

Division truncates toward zero. The signs of `7 / -2` and `-7 / 2` therefore both
produce `-3`. One unary minus is allowed before a primary: `--1` is invalid,
while `-(-1)` is valid. `1--2` is parsed as subtraction followed by unary minus
and evaluates to `3`.

## Integer boundaries

The range is `-9223372036854775808` through `9223372036854775807`. Leading zeros
are accepted as decimal; there are no base prefixes, digit separators, exponents,
or decimal fractions.

The minimum value needs a special literal rule. Its magnitude
`9223372036854775808` is accepted only directly after unary minus, ignoring trivia.
`-(9223372036854775808)` is invalid because the positive literal inside the
parentheses is already out of range.

```scarp
print(-9223372036854775808);
print(0007);
print(-9223372036854775808 + 1);
```

Output:

```text
-9223372036854775808
7
-9223372036854775807
```

Out-of-range literals produce `E0006`. Overflowing intermediate results produce
`E0007`, even if a later operation would bring the final value back into range.
Division by zero produces `E0008`. These rules are the same in debug and release
builds. Negating the minimum integer or dividing it by `-1` overflows.

## Assignment operators

The comparison and logical operators require the development boolean extension.
Equality requires matching primitive types; strings compare decoded contents.
Chained comparisons are rejected: write `low < x and x < high`. There is no
conversion between booleans and integers. Compilation checks all operands even
when arithmetic in a right-hand operand will be skipped during execution.

Development assignments are statements, not expression values. The target must
already exist and be declared with `var`.

| Operator | Effect |
| --- | --- |
| `=` | Replace the value with another value of the same inferred type |
| `+=` | Add a same-kind numeric value to the previous value |
| `-=` | Subtract a same-kind numeric value from the previous value |
| `*=` | Multiply by a same-kind numeric value |
| `/=` | Divide by a same-kind nonzero numeric value |

The right-hand expression sees the previous environment. For example,
`var x = 2; x += x + 1;` leaves `x` equal to `5`.

Compound arithmetic failures identify the complete compound operator. Failures
inside the right-hand expression identify that expression's operator instead.

## Strings, copies, and output

Strings support `\\`, `\"`, `\n`, `\t`, and `\r` in v0.0.3. Other escapes are errors. A physical CR or LF
inside a string is invalid. Parenthesized strings are supported in development.

Arithmetic and compound assignment accept same-type integer or float operands. There is no string
concatenation, indexing, interpolation, or automatic formatting of placeholders.
Print labels and values with separate statements:

```scarp
bind count = 3;
print("count:");
print(count);
```

Output:

```text
count:
3
```

String assignment replaces a value without changing earlier copies. Pending
print values also keep the contents captured when their statement was evaluated.
See [variables](../variables.md) for mutation and scope examples, and
[diagnostics](diagnostics.md) for type mismatch repairs.
