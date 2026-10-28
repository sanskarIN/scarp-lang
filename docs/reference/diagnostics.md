# Diagnostic reference

Use the error code to identify the class of failure, then use the source location
and help to repair it. Codes `E0001` through `E0009` exist in v0.0.1; the remaining
codes through `E0015` are available in v0.0.2. Beta.1 adds `E0016` through
`E0019`; published beta.2 retains those assignments and adds contextual
declaration/assignment repair help. This page covers beta.2 and the unreleased function checkout;
E0020–E0029 are new in that checkout and are absent from beta.2 binaries.

## Codes and first repairs

| Code | Cause | First repair to try |
| --- | --- | --- |
| `E0001` | Invalid source character | Remove unsupported punctuation or a character outside a valid token |
| `E0002` | Unterminated string | Close the quote; use `\n` instead of a physical line break |
| `E0003` | Unsupported escape | Use `\\`, `\"`, `\n`, `\t`, or `\r` in v0.0.3 |
| `E0004` | Unexpected token or missing syntax | Add the named punctuation or correct the statement form |
| `E0005` | Unknown statement or legacy declaration | Use a supported statement; migrate `let` to `bind` and `let mut` to `var` |
| `E0006` | Integer literal outside the range | Use a representable signed 64-bit value |
| `E0007` | Arithmetic overflow | Keep every intermediate integer result within range |
| `E0008` | Division by zero | Make the divisor evaluate to a nonzero integer |
| `E0009` | Expression nesting exceeds 64 groups | Remove unnecessary nested parentheses |
| `E0010` | Unknown variable | Declare it before use in a visible scope |
| `E0011` | Duplicate declaration in one scope | Rename it, or use a deliberate nested scope |
| `E0012` | Reserved variable name | Avoid statement words, boolean literals, and logical operator words |
| `E0013` | Assignment to an immutable binding | Declare the intended target with `var` |
| `E0014` | Block nesting exceeds 64 levels | Flatten unnecessary statement blocks |
| `E0015` | Primitive type mismatch | Keep assignment types fixed; arithmetic/order use integers and logic uses booleans |
| `E0016` | Loop control outside a loop | Place break/continue in the while/repeat body it should control |
| `E0017` | Execution work budget exhausted | Check loop progress or raise the explicit budget |
| `E0018` | Selected print count exhausted | Reduce prints or raise the explicit print budget |
| `E0019` | Negative repeat count | Supply zero to skip the loop or a positive integer count |

| `E0020` | Invalid or conflicting function definition | Check signature, duplicate names, and binding collisions |
| `E0021` | Invalid return or unit value use | Match the declared result; use unit calls as statements |
| `E0022` | Missing return path | Cover every branch or add a final return after loops |
| `E0023` | Unknown or non-callable function | Declare a funcra with that exact name |
| `E0024` | Invalid argument mapping | Check labels, duplicates, required values, and positional order |
| `E0025` | Purity violation | Keep output and impure calls outside pure functions/contracts/defaults |
| `E0026` | Contract failure | Satisfy needs/ensures and inspect the bounded call trace |
| `E0027` | Function resource limit | Check the named ExecutionOptions field and recursion/frame size |
| `E0028` | Unsupported function form | Use file-level named functions without captures |
| `E0029` | Output snapshot limit | Reduce selected output or change max_capture_bytes |

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
print(answer);
bind answer = 42;
```

Move the declaration before the use. A later declaration does not make a name
visible earlier. A declaration inside a completed block is also invisible outside.

### Duplicate declaration: E0011

```scarp-invalid
bind answer = 40;
bind answer = 42;
```

If the value should change, declare one mutable binding and assign to it. If a
separate local is intended, introduce a nested scope. Do not remove the first
declaration without checking uses between the two locations.

### Immutable write: E0013

```scarp-invalid
bind answer = 40;
answer += 2;
```

For intended mutation, change the declaration to `var answer = 40;`. If an
inner declaration shadows an outer mutable binding, the inner declaration
controls the write; resolution does not skip it.

### Assignment type mismatch: E0015

```scarp-invalid
var label = "Scarp";
label = 42;
```

Keep the string binding and introduce a separate integer binding, or assign
another string. `var` permits replacement, not a change of primitive type.
There are no implicit conversions.

### Arithmetic type mismatch: E0015

```scarp-invalid
bind label = "Scarp";
print(label + 1);
```

Use `print(label, 1);` to display separate values on one line. String
concatenation and mixed string/integer arithmetic are unsupported.

### Incomplete variable statements: E0004

```scarp-invalid
bind total = ;
```

Supply an initializer after `=` and finish with a semicolon, for example
`bind total = 0;`. A missing name or terminator receives different repair help.

```scarp-invalid
var total = 0;
total += ;
```

Supply the assignment's right-hand expression, such as `total += 2;`.
More specific failures retain their original code: an out-of-range integer is
E0006, while a division by zero is E0008 after static checks succeed.

### Negative repeat count: E0019

```scarp-invalid
repeat -1 as index { print(index); }
```

Use zero to skip the loop or a positive integer count. Repeat evaluates its
count once; the immutable index ranges from zero through count minus one.

## Why a later error can appear first

The pipeline validates the complete file in phases: lexing, parsing, name/type
resolution, then checked evaluation. The first failure in a phase prevents later
phases. This example reports `E0010` rather than first evaluating the division:

```scarp-invalid
print(1 / 0);
print(missing);
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
