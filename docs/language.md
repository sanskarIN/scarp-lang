# Scarp prototype language reference

Published alpha.4 adds the `tile` parameter/result type and immutable
two-axis values. `height`, `width`, `cell`, `lane`, `unroll`, `turn`, `matvec`,
`matmul`, `tileadd` and `tilescale` follow [the tile contract](tiles.md).
There is no broadcasting, implicit vector conversion or mutable cell indexing.

This reference targets the published **0.0.5-alpha.1 prerelease**,
including [source modules](modules.md). Published alpha.4 does not include imports;
published beta.3 does not include floats.
Published v0.0.3-beta.2 remains the earlier experimental baseline without functions. See the
[syntax migration](syntax-migration.md) before using older programs. It is an
experimental contract, not a stable language specification. Future proposals do
not change this reference until their implementation is accepted.

## Source and lexical rules

Source is UTF-8. A single leading UTF-8 byte order mark is accepted and occupies
three source bytes. A mark elsewhere is an invalid character. The lexer skips
Unicode whitespace outside string literals and `//` comments through the next
LF, CR, or end of file. It recognizes comments only outside strings.

Identifiers match the ASCII pattern `[A-Za-z_][A-Za-z0-9_]*`. Statement forms use
`bind`, `var`, output names, `if`, `while`, and `repeat`, with exact spelling and case. `elif` and `else`
extend a preceding conditional. Variable names and assignment targets are
resolved after parsing. Braces can also begin a standalone block.

Punctuation tokens include `(`, `)`, `[`, `]`, `{`, `}`, `,`, `;`, `.`, `:`, `->` and `=`. Compound assignment tokens use
longest match: `+=`, `-=`, `*=`, `/=`. Arithmetic operator tokens are `+`, `-`,
`*`, and `/`. Comparison tokens use longest match: `==`, `!=`, `<`, `<=`, `>`,
and `>=`. Integers contain one or more ASCII decimal digits; leading zeros
are accepted as decimal. A minus sign is a separate token. Other characters
outside strings, comments, identifiers, and whitespace are lexical errors.
`//` starts a comment even immediately after an expression. A single `/` is the
division operator. There are no block comments.

## Strings

Strings begin and end with `"`. Contents preserve Unicode text without
normalization. The lexer decodes exactly five escape sequences in v0.0.3:

| Source spelling | Decoded character |
| --- | --- |
| `\\` | Backslash |
| `\"` | Double quote |
| `\n` | LF line break |
| `\t` | Tab |
| `\r` | Carriage return |

A physical CR or LF, or end of input before a closing quote, produces `E0002`.
Other escapes, including numeric escapes and Unicode escapes, produce
`E0003`. Unicode text can be written directly in UTF-8 instead. A final backslash
without another character produces `E0002`.

Empty strings are valid. The characters `//`, `(`, `)`, and `;` inside strings
are ordinary text. Embedded control characters other than CR and LF are currently
preserved; token dumps escape string values for readability.

## Grammar

Unreleased development extends declarations with optional primitive type
annotations and [computed scalar module constants](constant-expressions.md).
Published alpha.1 lacks these extensions. An annotation must match inference;
`unit` is a function-result type only and does not permit an uninitialized binding.

Trivia means whitespace and line comments. Trivia can appear between tokens.
The following base grammar applies after trivia has been removed.
Beta.3 extends it with typed function declarations, call
expressions/statements, returns, and contracts; see [functions](functions.md).
Those extensions additionally recognize `:` and `->` punctuation.
The module extension adds file-level `import STRING as IDENTIFIER;`,
`pub [pure] funcra ...` and `pub bind name = scalar-literal;`, with contextual
`import`/`pub` words and `alias.member` expressions/calls. Imported library files
are declaration-only. The exact grammar, visibility and path semantics are in
[Proposal 0029](proposals/0029-source-modules.md) and [modules](modules.md).

```ebnf
program         = { statement }, EOF ;
statement       = output_statement | declaration | assignment | block | conditional | loop | loop_control ;
loop            = "while", expression, block | "repeat", expression, "as", IDENTIFIER, block ;
loop_control    = ("break" | "continue"), ";" ;
conditional     = "if", expression, block,
                  { "elif", expression, block }, [ "else", block ] ;
block           = "{", { statement }, "}" ;
declaration     = ("bind" | "var"), IDENTIFIER, [ ":", binding_type ], "=", expression, ";" ;
binding_type    = "integer" | "float" | "boolean" | "string" | "vector" | "tile" ;
assignment      = IDENTIFIER, assign_operator, expression, ";" ;
assign_operator = "=" | "+=" | "-=" | "*=" | "/=" ;
output_statement = output_name, "(", [ argument_list ], ")", ";" ;
output_name      = "print" | "println" | "write" | "eprint" | "ewrite" ;
argument_list    = expression, { ",", expression }, [ "," ] ;
expression      = logical_or ;
logical_or      = logical_and, { "or", logical_and } ;
logical_and     = logical_not, { "and", logical_not } ;
logical_not     = { "not" }, comparison ;
comparison      = sum, [ ("==" | "!=" | "<" | "<=" | ">" | ">="), sum ] ;
sum             = product, { ("+" | "-"), product } ;
product         = unary, { ("*" | "/"), unary } ;
unary           = [ "-" ], postfix ;
postfix         = primary, { "[", expression, "]" } ;
primary         = INTEGER | FLOAT | STRING | "true" | "false" | IDENTIFIER | vector | "(", expression, ")" ;
vector          = "[", [ argument_list ], "]" ;
```

`"print"` denotes an identifier with that exact value, not a substring of a
longer identifier. `STRING` is one complete string token and a primitive value.
Output statements accept zero or more expressions, separated by one ASCII space
in displayed output. A trailing comma is accepted; empty middle arguments are
errors. Semicolons are mandatory. Newlines do not terminate statements. See
[output](output.md) for raw writes, stderr, and the permanent `println` alias.

```scarp
// Valid: comments and whitespace can separate tokens.
print
(
    "one"
)
;
print("two");
```

Empty and comment-only files are valid programs. Extra semicolons, concatenated
strings are invalid. Output forms remain statements and cannot be nested as
expressions; user-function calls can be nested. Multiple output arguments are
valid. Binary operators
associate left to right. Multiplication and division have higher precedence
than addition and subtraction. One unary minus is allowed before a primary:
`--1` is invalid, while `-(-1)` and `1--2` are valid. Unary plus is unsupported.

## Integers and arithmetic

Values range from `-9223372036854775808` to `9223372036854775807`. Literal range
validation happens during parsing. The magnitude `9223372036854775808` is accepted
only directly after a unary minus, ignoring trivia, to represent the minimum
integer. `-(9223372036854775808)` is rejected because its positive literal is
out of range. Leading zeros do not change these rules.

All operations use checked signed 64-bit arithmetic in debug and release builds.
Division truncates toward zero. Division by zero, overflowing intermediate
results, negating the minimum integer, and dividing it by `-1` are errors.
Diagnostics highlight the responsible operator. Evaluation follows source order
and postorder within each expression, stopping at the first arithmetic error.

Grouping is limited to 64 nested pairs of parentheses, excluding the `print`
call itself. Greater depth produces `E0009`. Flat operator chains have no fixed
language limit and are processed without recursive evaluation or destruction.
Available memory still limits the size of a program. Numeric separators,
base prefixes, strings as arithmetic operands, and string concatenation are unsupported.

## Finite floats (alpha)

FLOAT follows `digits "." digits [exponent]` or `digits exponent`, with an
exponent of `e`/`E`, optional sign, and required digits. `1.`, `.5`, `1e`, suffixes,
and repeated points/exponents are E0030. Plain digits retain i64 semantics.
Literals must parse to finite binary64. Unary minus and same-type arithmetic/
comparisons support floats without conversions. Operations producing nonfinite
results are E0031; either signed-zero divisor is E0008. Subnormal underflow is
allowed, including signed zero. See [floats](floats.md) for rounding, exact
formatting rules, host invariants, examples, and budget behavior.

## Execution

Booleans are explicit `true`/`false` values, with no integer/string truthiness.
Equality requires matching primitive kinds; ordering requires same-type numeric operands. `not`
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

Line output appends one LF; raw output appends none. Strings, signed decimal
integers, finite floats, and lowercase booleans use their documented display form. One ASCII space
separates arguments. `eprint` and `ewrite` target the explicit stderr writer.
Strings containing `\n` can therefore write several lines.
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
| `E0016` | Break or continue outside a loop body |
| `E0017` | Execution work limit exceeded |
| `E0018` | Selected output-statement limit exceeded |
| `E0019` | Negative repeat count |
| `E0030` | Malformed or nonfinite float literal |
| `E0031` | Nonfinite float arithmetic result |
| `E0037` | Invalid module declaration or source-loader API required |
| `E0038` | Module loading or UTF-8 failure |
| `E0039` | Circular imports |
| `E0040` | Module loading/linking resource limit |
| `E0041` | Private imported member |
| `E0042` | Unknown module alias/member or wrong symbol kind |
| `E0043` | Invalid or escaping module path |

Diagnostic wording and library types are experimental. Codes are intended to be
more useful for tests than full platform-dependent file-error strings. Read
errors for imported modules carry the import span. Tokens file-read errors and
output errors remain process errors. The [full diagnostic reference](reference/diagnostics.md)
also covers functions and numerical E0032–E0036 errors.

## Command-line contract

| Command | Behavior |
| --- | --- |
| `scarp run <path>` | Read, validate, and execute a file |
| `scarp check <path>` | Validate syntax, names, and arithmetic without printing |
| `scarp tokens <path>` | Lex a file and print tokens without parsing |
| `scarp --help` or `scarp -h` | Show command usage |
| `scarp --version` or `scarp -V` | Show the package development version |

The `run`, `check`, and `tokens` commands accept an optional `--` before the path. Exactly
one path is required, and stdin is not a source input. The `.srp` extension is a
convention rather than a filename check. Native non-Unicode paths are accepted
where supported by the operating system; the file contents must remain UTF-8.
`run` and `check` accept `--max-steps` and `--max-prints` before the path;
`tokens` accepts neither. `check` evaluates the selected path, including loops,
and is not a static-only checker. See the [CLI reference](reference/cli.md).

Ordinary output, help, versions, and token listings use stdout. Explicit
`eprint`/`ewrite` output and diagnostics use stderr. Exit codes are `0` for
success, `1` for source/file/output failures, and
`2` for invalid arguments. No commands install packages or publish releases.

## Scope

Integer bindings, explicit mutability, assignment, and lexical scopes are implemented on main and described
in [the variable reference](variables.md). Beta.3 adds typed
`funcra` calls as described in [functions](functions.md). The 0.0.5-alpha.1 prerelease
adds source modules with public/private functions and scalar literal constants.
User-defined types, runtime module globals, re-exports, a compiler backend and
a package manager are not implemented. The implemented expression semantics are also
recorded in [the expression design](proposals/0001-integer-expressions.md).

The v0.0.3-beta.1 iteration milestone adds scoped `while`, `break`, and `continue`
after v0.0.2, with per-call deterministic work and print budgets. The published
v0.0.2 packages do not contain loops. See [the loop design](proposals/0006-bounded-loops.md).

`repeat count as index { ... }` evaluates an integer count once in the enclosing
scope, rejects negative counts with E0019, and iterates from zero through count
minus one. The index is immutable and scoped to the body. Continue advances it;
break exits the nearest loop, including mixed while/repeat nesting. A zero count
still checks the body statically. The runtime uses two fixed slots per repeat
and the same work/print budgets as while; it does not allocate an index array.
`repeat` and `as` are contextual identifiers: existing bindings keep working,
including assignment to a variable named repeat. Same-scope index redeclaration
is E0011; explicit inner scopes may shadow it. See [the repeat design](proposals/0007-counted-repeat.md).

## Numerical vectors (since alpha.2)

`[1.0, 2.0]` and `[]` infer vector. Only float elements are accepted; a vector
has at most 65,536 elements. `expression[integer]` returns a float or E0033,
with no negative/reverse indices. Postfix indexing binds before unary minus.
Vector elements, call arguments, indices and groups share the 64-level nesting
limit; flat nodes and postfix chains do not use recursive destruction.

Typed function signatures accept vector. User functions keep precedence over
new builtin names. All ten positional kernels are pure; their shape/domain,
finite-result and work checks execute only on selected paths. Static type checks
still inspect unused bodies. Whole-vector var replacement is allowed, while
element mutation, broadcasting and vector arithmetic operators are not.
See [vectors](vectors.md) for signatures, algorithms, resource costs and examples.
