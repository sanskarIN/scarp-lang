# Scarp prototype language reference

This reference describes the development language following v0.0.1. It is an
experimental contract, not a stable language specification. Future proposals do
not change this reference until their implementation is accepted.

## Source and lexical rules

Source is UTF-8. A single leading UTF-8 byte order mark is accepted and occupies
three source bytes. A mark elsewhere is an invalid character. The lexer skips
Unicode whitespace outside string literals and `//` comments through the next
LF, CR, or end of file. It recognizes comments only outside strings.

Identifiers match the ASCII pattern `[A-Za-z_][A-Za-z0-9_]*`. Statement forms use
`let`, `println`, and `if`, with exactly that spelling and case. `elif` and `else`
extend a preceding conditional. Variable names and assignment targets are
resolved after parsing. Braces can also begin a standalone block.

Punctuation tokens are `(`, `)`, `[`, `]`, `,`, `{`, `}`, `;`, and `=`. Compound assignment tokens use
longest match: `+=`, `-=`, `*=`, `/=`, `%=`, `&=`, `|=`, `^=`, `<<=`, `>>=`,
and `>>>=`. Arithmetic tokens are `+`, `-`, `*`, `/`, and `%`. Integer bit tokens
are `&`, `|`, `^`, `~`, `<<`, `>>`, and `>>>`. Comparisons use longest match:
`==`, `!=`, `<`, `<=`, `>`, and `>=`. Integers use decimal digits, or hexadecimal
`0x`/`0X` and binary `0b`/`0B` prefixes. Leading zeros without a prefix remain
decimal. A minus sign is a separate token. Other characters
outside strings, comments, identifiers, and whitespace are lexical errors.
`//` starts a comment even immediately after an expression. A single `/` is the
division operator. There are no block comments.

## Strings

Strings begin and end with `"`. Contents preserve Unicode text without
normalization. The lexer decodes exactly three escape sequences:

| Source spelling | Decoded character |
| --- | --- |
| `\\` | Backslash |
| `\"` | Double quote |
| `\n` | LF line break |

A physical CR or LF, or end of input before a closing quote, produces `E0002`.
Other escapes, including `\t`, `\r`, numeric escapes, and Unicode escapes, produce
`E0003`. Unicode text can be written directly in UTF-8 instead. A final backslash
without another character produces `E0002`.

Empty strings are valid. The characters `//`, `(`, `)`, and `;` inside strings
are ordinary text. Embedded control characters other than CR and LF are currently
preserved; token dumps escape string values for readability.

## Grammar

Trivia means whitespace and line comments. Trivia can appear between tokens.
The following grammar applies after trivia has been removed:

```ebnf
program         = { statement }, EOF ;
statement       = print_statement | declaration | assignment | block | conditional | loop | loop_control ;
loop            = "while", expression, block ;
loop_control    = ("break" | "continue"), ";" ;
conditional     = "if", expression, block,
                  { "elif", expression, block }, [ "else", block ] ;
block           = "{", { statement }, "}" ;
declaration     = "let", [ "mut" ], IDENTIFIER, "=", expression, ";" ;
assignment      = IDENTIFIER, assign_operator, expression, ";" ;
assign_operator = "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "&=" | "|=" | "^="
                  | "<<=" | ">>=" | ">>>=" ;
print_statement = "println", "(", argument, ")", ";" ;
argument        = expression ;
expression      = logical_or ;
logical_or      = logical_and, { "or", logical_and } ;
logical_and     = logical_not, { "and", logical_not } ;
logical_not     = { "not" }, comparison ;
comparison      = bit_or, [ ("==" | "!=" | "<" | "<=" | ">" | ">="), bit_or ] ;
bit_or          = bit_xor, { "|", bit_xor } ;
bit_xor         = bit_and, { "^", bit_and } ;
bit_and         = shift, { "&", shift } ;
shift           = sum, { ("<<" | ">>" | ">>>"), sum } ;
sum             = product, { ("+" | "-"), product } ;
product         = unary, { ("*" | "/" | "%"), unary } ;
unary           = [ "-" | "~" ], postfix ;
postfix         = primary, { "[", expression, "]" } ;
primary         = INTEGER | FLOAT | STRING | "true" | "false" | IDENTIFIER
                  | "(", expression, ")" | vector | builtin_call ;
vector          = "[", [ expression, { ",", expression }, [ "," ] ], "]" ;
builtin_call    = BUILTIN, "(", [ expression, { ",", expression }, [ "," ] ], ")" ;
```

`"println"` denotes an identifier with that exact value, not a substring of a
longer identifier. `STRING` is one complete string token and a primitive value.
`println` takes exactly one value expression. Native built-ins have fixed argument
lists described in [the vector design](proposals/0008-numeric-vectors.md). Semicolons
are mandatory. Newlines do not terminate statements.

```scarp
// Valid: comments and whitespace can separate tokens.
println
(
    "one"
)
;
println("two");
```

Empty and comment-only files are valid programs. Extra semicolons, concatenated
strings and multiple arguments to `println` are invalid. Built-in calls can nest.
Binary operators
associate left to right. Multiplication, division, and remainder bind above
addition and subtraction, followed by shifts, `&`, `^`, and `|`. One unary minus
or complement is allowed before a primary:
`--1` is invalid, while `-(-1)` and `1--2` are valid. Unary plus is unsupported.

## Integers and arithmetic

The decimal extension adds a distinct finite binary64 kind. It requires explicit
decimal/exponent spelling and same-kind operands. See the
[decimal design](proposals/0007-finite-decimals.md) for literal grammar,
rounding, underflow, formatting, and checked nonfinite results.

Values range from `-9223372036854775808` to `9223372036854775807`. Literal range
validation happens during parsing. The magnitude `9223372036854775808` is accepted
only directly after a unary minus, ignoring trivia, to represent the minimum
integer. `-(9223372036854775808)` is rejected because its positive literal is
out of range. Leading zeros do not change these rules.

Arithmetic uses checked signed 64-bit operations in debug and release builds.
Division truncates toward zero. Division by zero, overflowing intermediate
results, negating the minimum integer, and dividing it by `-1` are errors.
Diagnostics highlight the responsible operator. Evaluation follows source order
and postorder within each expression, stopping at the first arithmetic error.
Remainder follows the dividend's sign and rejects zero and `MIN % -1`.
Bit operations use the integer's 64-bit representation. Left shift discards high
bits, `>>` extends the sign, and `>>>` fills with zeros. Shift counts must be
between 0 and 63 (E0026 otherwise). See the
[integer bit-operation design](proposals/0009-integer-bit-operations.md).

Grouping is limited to 64 nested pairs of parentheses, excluding the `println`
call itself. Greater depth produces `E0009`. Flat operator chains have no fixed
language limit and are processed without recursive evaluation or destruction.
Available memory still limits the size of a program. Numeric separators,
strings as arithmetic operands, and string concatenation are unsupported.

## Execution

Booleans are explicit `true`/`false` values, with no integer/string truthiness.
Equality requires matching primitive kinds; ordering requires same-kind numbers. `not`
binds after comparisons, followed by `and`, then `or`. All operands resolve and
type-check during compilation. Runtime `and` skips its right operand when false;
`or` skips it when true. Chained comparisons need explicit `and` composition.
See [the boolean design](proposals/0004-boolean-expressions.md) for exact rules.

The complete source file is lexed, parsed, resolved, and checked for primitive
type consistency in that order. Name resolution and primitive inference share
one traversal. The first
error in a phase prevents later phases. A later lexical error can therefore be
reported before an earlier syntax error; a later unknown name can be reported
before an earlier arithmetic failure. Reachable declarations, assignments, and
print arguments are then evaluated in source order before any output is written.
All lexical, syntax, name, mutability, type, and arithmetic errors produce no program
output.

Conditional conditions require booleans. Every body is statically checked, but
only the first selected body evaluates arithmetic and captures prints. Each body
has a separate lexical scope. `elif` conditions stop after the first match.
Braces are mandatory and use the existing block-depth limit. See the
[conditional design](proposals/0005-conditional-statements.md) and
[decisions tutorial](tutorials/decisions.md).

Each print statement writes its decoded string or decimal integer followed by one LF
to the supplied output stream. Strings containing `\n` can therefore write several lines.
Statements execute in source order. Empty programs write nothing.

An I/O failure is returned to the caller. Previously written bytes cannot be
rolled back. The library returns broken-pipe errors like other output errors;
the CLI treats a broken output pipe as successful termination.

## Spans, locations, and diagnostics

Every token, including EOF, carries a half-open byte span `[start, end)` in the
original source. String token spans include both quotes and their original
escape spelling. Decoded text can have a different byte length. A print
statement span starts at its identifier and ends after its semicolon. EOF is an
empty span at the source's byte length, including trailing trivia.

Lines and columns are one-based. LF advances the line; CR in CRLF still occupies
a source byte. Columns count Unicode scalar values, so a multibyte character is
one column and a tab is one column. Error excerpts expand tabs to four spaces.
Combining characters and wide glyphs can differ from terminal display cells.
Carets cover the first affected line and remain visible for empty spans at EOF.

| Code | Meaning |
| --- | --- |
| `E0001` | Invalid source character |
| `E0002` | Unterminated string |
| `E0003` | Unsupported string escape |
| `E0004` | Unexpected token or missing punctuation/argument |
| `E0005` | Unknown statement identifier |
| `E0006` | Integer literal outside the signed 64-bit range |
| `E0007` | Checked integer arithmetic overflow |
| `E0008` | Division by zero |
| `E0009` | Expression nesting limit exceeded |
| `E0010` | Unknown variable |
| `E0011` | Duplicate declaration in the same scope |
| `E0012` | Reserved variable name |
| `E0013` | Assignment to an immutable binding |
| `E0014` | Block nesting limit exceeded |
| `E0015` | Primitive type mismatch |

Diagnostic wording and library types are experimental. Codes are intended to be
more useful for tests than full platform-dependent file-error strings. Read
errors, invalid UTF-8, and output errors are CLI errors without source spans.

## Command-line contract

| Command | Behavior |
| --- | --- |
| `scarp run <path>` | Read, validate, and execute a file |
| `scarp check <path>` | Validate syntax, names, and arithmetic without printing |
| `scarp tokens <path>` | Lex a file and print tokens without parsing |
| `scarp emit-c <path>` | Emit freestanding C for the supported integer/boolean subset |
| `scarp --help` or `scarp -h` | Show command usage |
| `scarp --version` or `scarp -V` | Show the package development version |

The file commands accept an optional `--` before the path. Exactly
one path is required, and stdin is not a source input. The `.srp` extension is a
convention rather than a filename check. Native non-Unicode paths are accepted
where supported by the operating system; the file contents must remain UTF-8.

Program output, help, versions, and token listings use stdout. Diagnostics use
stderr. Exit codes are `0` for success, `1` for source/file/output failures, and
`2` for invalid arguments. No commands install packages or publish releases.

## Scope

While loops, nearest-loop control, and per-call deterministic work/print budgets
are implemented on the loop branch. See [the loop design](proposals/0006-bounded-loops.md).

Integer bindings, explicit mutability, assignment, and lexical scopes are implemented on development branches and described
in [the variable reference](variables.md). An experimental
[freestanding C backend](proposals/0010-freestanding-c.md) supports integer/boolean
computation units with caller-owned storage and output. Functions, user-defined
types, modules, raw memory/device access, kernel startup, and a package manager
remain future work. The implemented expression semantics are also
recorded in [the expression design](proposals/0001-integer-expressions.md).
