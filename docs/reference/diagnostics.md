# Diagnostic reference

Use the error code to identify the class of failure, then use the source location
and help to repair it. Codes `E0001` through `E0009` exist in v0.0.1; the remaining
codes belong to the development variable stack.

## Codes and first repairs

| Code | Cause | First repair to try |
| --- | --- | --- |
| `E0001` | Invalid source character | Remove unsupported punctuation or a character outside a valid token |
| `E0002` | Unterminated string | Close the quote; use `\n` instead of a physical line break |
| `E0003` | Unsupported escape | Use `\\`, `\"`, `\n`, `\t`, or `\r` in v0.0.3 |
| `E0004` | Unexpected token or missing syntax | Add the named punctuation or correct the statement form |
| `E0005` | Unknown statement | Use a supported declaration, assignment, block, or `println` call |
| `E0006` | Integer literal outside the range | Use a representable signed 64-bit value |
| `E0007` | Arithmetic overflow | Keep every intermediate integer result within range |
| `E0008` | Division by zero | Make the divisor evaluate to a nonzero integer |
| `E0009` | Expression nesting exceeds 64 groups | Remove unnecessary nested parentheses |
| `E0010` | Unknown variable | Declare it before use in a visible scope |
| `E0011` | Duplicate declaration in one scope | Rename it, or use a deliberate nested scope |
| `E0012` | Reserved variable name | Avoid statement words, boolean literals, and logical operator words |
| `E0013` | Assignment to an immutable binding | Declare the intended target with `let mut` |
| `E0014` | Block nesting exceeds 64 levels | Flatten unnecessary statement blocks |
| `E0015` | Primitive type mismatch | Keep assignment types fixed; arithmetic/order use integers and logic uses booleans |
| `E0016` | Loop control outside a loop | Place break/continue in a while body |
| `E0017` | Execution work budget exhausted | Check loop progress or raise the explicit budget |
| `E0018` | Selected print count exhausted | Reduce prints or raise the explicit print budget |
| `E0019` | Negative repeat count | Supply zero to skip the loop or a positive integer count |

File-read failures, invalid UTF-8, and output I/O failures are process errors
without source error codes. They still use stderr and exit code `1`. Invalid
command-line usage uses exit code `2`; see the [CLI reference](cli.md).

## Read the location

The arrow identifies a filename and one-based line/column. The caret points at
the unexpected token, the responsible operator, or an empty position at EOF
where punctuation is missing. It can identify where something belongs rather
than a character that must be deleted.

Internal spans are half-open UTF-8 byte ranges. Columns count Unicode scalar
values. Multibyte text, combining marks, wide glyphs, and tabs can therefore make
byte offsets, columns, and terminal cells differ. Error excerpts expand tabs to
four spaces and escape control characters.

Duplicate bindings, immutable writes, and assignment type errors can include a
related source location. Read that declaration as well as the primary caret:
the declaration determines which variable, mutability, or type the later use has.

## Worked repairs

### Unknown name: E0010

```scarp-invalid
println(answer);
let answer = 42;
```

Move the declaration before the use. A later declaration does not make a name
visible earlier. A declaration inside a completed block is also invisible outside.

### Duplicate declaration: E0011

```scarp-invalid
let answer = 40;
let answer = 42;
```

If the value should change, declare one mutable binding and assign to it. If a
separate local is intended, introduce a nested scope. Do not remove the first
declaration without checking uses between the two locations.

### Immutable write: E0013

```scarp-invalid
let answer = 40;
answer += 2;
```

For intended mutation, change the declaration to `let mut answer = 40;`. If an
inner declaration shadows an outer mutable binding, the inner declaration
controls the write; resolution does not skip it.

### Assignment type mismatch: E0015

```scarp-invalid
let mut label = "Scarp";
label = 42;
```

Keep the string binding and introduce a separate integer binding, or assign
another string. `mut` permits replacement, not a change of primitive type.
There are no implicit conversions.

### Arithmetic type mismatch: E0015

```scarp-invalid
let label = "Scarp";
println(label + 1);
```

Print the label and number in separate statements. String concatenation and
mixed string/integer arithmetic are unsupported.

## Why a later error can appear first

The pipeline validates the complete file in phases: lexing, parsing, name/type
resolution, then checked evaluation. The first failure in a phase prevents later
phases. This example reports `E0010` rather than first evaluating the division:

```scarp-invalid
println(1 / 0);
println(missing);
```

After correcting the name, the division error can become the next diagnostic.
Scarp reports one diagnostic at a time and does not write partial program output
for a source or semantic failure.

## Report a useful bug

Include the smallest source file, exact command, complete stderr, exit code,
Scarp version, and operating system. For source builds, include the commit and
`rustc --version`. Avoid relying on an exact file-error sentence: those messages
can differ between operating systems.

If the caret or code appears wrong, preserve the original file's UTF-8 contents
and line endings when reporting it. See [testing](../testing.md) for turning a
reproduction into a focused regression test.
