# Proposal 0001: integer expressions

Historical design record: current declaration and output spellings are documented
in the [syntax migration](../syntax-migration.md) and [output guide](../output.md).

**Status: implemented for the v0.0.1 prototype.** This document records the
accepted expression design following the initial print-only slice. Its examples
and acceptance cases are covered by the test suite.

## Goal

Allow `println` to print a signed 64-bit integer expression while preserving all
existing string-literal programs. This introduces precedence parsing and values
without simultaneously adding bindings, user-defined functions, or a type system.

Examples:

```scarp
println(1 + 2 * 3);       // 7
println((1 + 2) * 3);     // 9
println(-7 / 2);          // -3
println("still works");  // still works
```

## Syntax

```ebnf
print_statement = "println", "(", argument, ")", ";" ;
argument        = STRING | expression ;
expression      = sum ;
sum             = product, { ("+" | "-"), product } ;
product         = unary, { ("*" | "/"), unary } ;
unary           = [ "-" ], primary ;
primary         = INTEGER | "(", expression, ")" ;
INTEGER         = DIGIT, { DIGIT } ;
DIGIT           = "0" | "1" | "2" | "3" | "4"
                | "5" | "6" | "7" | "8" | "9" ;
```

Multiplication and division bind more tightly than addition and subtraction.
Binary operators associate left to right. One unary minus is allowed per primary;
`--1` is rejected, while `-(-1)` is valid. Unary plus is not included. Decimal
digits are ASCII, leading zeros are accepted as decimal, and numeric separators,
floats, exponents, and base prefixes are deferred.

Strings remain a direct argument alternative, with no string concatenation or
mixed string/integer operators. Keeping that boundary explicit makes this slice
small and avoids implying a general type system.

## Numeric semantics

Values are signed 64-bit integers. Evaluate operations with checked arithmetic
in every build profile. Division truncates toward zero. Division by zero,
overflow, and literals outside the representable range produce diagnostics;
they must never panic or silently wrap.

Recognize the magnitude `9223372036854775808` only as the direct operand of a
unary minus, producing the minimum integer `-9223372036854775808`. The same
positive literal, including inside parentheses before negation, is rejected.
This special case needs a documented parse rule and boundary tests. Negating the
minimum value and dividing it by `-1` are overflow errors.

Preserve the current no-output-on-invalid-source behavior by evaluating and
validating all print arguments before writing any of them. Arithmetic failures
therefore produce no program output even if an earlier statement is valid.
Output stream failures can still occur after some output has been written.

## Implementation

1. Add integer and operator tokens with original byte spans. Preserve `//`
   comments while adding `/` as an operator.
2. Introduce expression nodes for literals, unary negation, binary operators,
   and grouping. Retain operator spans for precise errors.
3. Implement precedence parsing with loops for binary chains and a limit of 64
   nested parenthesis groups. Excess depth returns a diagnostic. Store nodes in
   a flat postorder vector to avoid recursive evaluation and destruction.
4. Evaluate integer expressions with checked operations and collect validated
   print values before writing. Format integers in decimal without locale rules.
5. Extend error codes, language reference, examples, and cross-platform tests.

## Verified acceptance cases

| Case | Required result |
| --- | --- |
| `1 + 2 * 3` | `7` |
| `(1 + 2) * 3` | `9` |
| `8 - 3 - 1` | `4` |
| `-7 / 2` | `-3` |
| `7 / -2` | `-3` |
| `1 / 0` | Diagnostic, no output |
| Maximum integer plus one | Overflow diagnostic, no output |
| Minimum integer divided by `-1` | Overflow diagnostic, no output |
| Incomplete `1 +` or unmatched parentheses | Syntax diagnostic |
| Excessive parenthesis nesting | Depth diagnostic, no panic |
| Existing strings, comments, escapes, and empty files | Existing behavior preserved |

Both debug and release tests verify that overflow behavior does not depend on
compiler settings. CI also verifies the minimum toolchain. The implementation
adds no dependencies.

## Deferred work

Bindings, scope, booleans, comparisons, strings as general expressions, functions,
modules, a type checker, and compilation remain separate later milestones. This
milestone introduces none of those features. Prerelease publication is managed
separately from the language implementation.
