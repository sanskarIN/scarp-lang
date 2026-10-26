# Values and operators

Use this reference when an expression's result or type differs from what you
expected. Checked integer arithmetic works in v0.0.1 and development Scarp;
bindings and string-valued expressions require the development stack.

## Primitive values

| Kind | Examples | Rules |
| --- | --- | --- |
| Integer | `0`, `42`, `-7` | Signed 64-bit values; decimal ASCII digits |
| Float | `1.25`, `1e2` | Finite binary64; no implicit integer conversions |
| Vector | `[1.0, 2.0]`, `[]` | Immutable finite float elements, at most 65,536 |
| Boolean | `true`, `false` | Development only; explicit booleans without truthiness |
| String | `"Scarp"`, `""`, `"line\nbreak"` | UTF-8 text with three supported escapes |

Development bindings infer their kind from the initializer. Mutable assignment
must preserve that kind. Strings do not automatically convert to integers, and
integers do not automatically become strings for arithmetic.

General collections and user-defined values are not implemented. The
[numerical vector design](../proposals/0008-numeric-vectors.md) specifies the
specialized float collection, indexing, native signatures, and work bounds.
Local type annotations and explicit conversion functions are not implemented.
[Functions](../functions.md) require explicit parameter/result types, and add
`unit` as a result-only type. Unit cannot be printed, bound, or used as an operand.

## Precedence and associativity

| Order, strongest first | Syntax | Behavior |
| --- | --- | --- |
| Grouping | `(expression)` | Evaluate the grouped expression |
| Vector indexing | `vector[index]` | Zero-based integer index returning a float |
| Unary minus | `-primary` | Checked integer or finite float negation |
| Multiplication/division | `*`, `/` | Left-associative checked integer arithmetic |
| Addition/subtraction | `+`, `-` | Left-associative checked integer arithmetic |
| Comparison | `==`, `!=`, `<`, `<=`, `>`, `>=` | One comparison; ordering needs same-kind numbers |
| Boolean negation | `not` | Negate a boolean, including a complete comparison |
| Boolean conjunction | `and` | Skip the right boolean if the left is false |
| Boolean disjunction | `or` | Skip the right boolean if the left is true |

```scarp
println(1 + 2 * 3);
println((1 + 2) * 3);
println(8 - 3 - 1);
println(24 / 3 / 2);
println(-7 / 2);
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
println(-9223372036854775808);
println(0007);
println(-9223372036854775808 + 1);
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

Finite decimal spelling, rounding, checked results, and distinct numeric types
are specified in [the decimal design](../proposals/0007-finite-decimals.md).

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
| `+=` | Add an integer expression to the previous integer value |
| `-=` | Subtract an integer expression from the previous integer value |
| `*=` | Multiply the previous integer value by an integer expression |
| `/=` | Divide the previous integer value by an integer expression |

The right-hand expression sees the previous environment. For example,
`var x = 2; x += x + 1;` leaves `x` equal to `5`.

Compound arithmetic failures identify the complete compound operator. Failures
inside the right-hand expression identify that expression's operator instead.

## Strings, copies, and output

Strings support `\\`, `\"`, and `\n`. Other escapes are errors. A physical CR or LF
inside a string is invalid. Parenthesized strings are supported in development.

Arithmetic and compound assignment require matching numeric kinds. There is no string
concatenation, indexing, interpolation, or automatic formatting of placeholders.
Development can join labels and values with `print("count:", count);`.
The legacy separate-statement form remains supported:

```scarp
bind count = 3;
println("count:");
println(count);
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
All output forms use these same value representations. Empty strings preserve
inter-argument spaces; `write` omits the final LF. Braces and `%s` are literal.
See [output](../output.md) for stream and formatting rules.
