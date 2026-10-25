# Scarp prototype language reference

This reference describes the implemented v0.0.1 development language. It is an
experimental contract, not a stable language specification. Future proposals do
not change this reference until their implementation is accepted.

## Source and lexical rules

Source is UTF-8. A single leading UTF-8 byte order mark is accepted and occupies
three source bytes. A mark elsewhere is an invalid character. The lexer skips
Unicode whitespace outside string literals and `//` comments through the next
LF, CR, or end of file. It recognizes comments only outside strings.

Identifiers match the ASCII pattern `[A-Za-z_][A-Za-z0-9_]*`. The only supported
statement identifier is `println`, with exactly that spelling and case. Unknown
identifiers are tokens but produce a parser error when used as statement names.

Punctuation tokens are `(`, `)`, and `;`. Any other character outside strings,
comments, identifiers, and whitespace is a lexical error. A slash is valid only
as the beginning of a `//` comment. There are no block comments.

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
program         = { print_statement }, EOF ;
print_statement = "println", "(", STRING, ")", ";" ;
```

`"println"` denotes an identifier with that exact value, not a substring of a
longer identifier. `STRING` is one complete string token, not an expression.
There is no argument list: a call takes exactly one string literal. Semicolons
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
strings, multiple arguments, and nested calls are invalid.

## Execution

The complete source file is lexed, then parsed. The first lexical error prevents
parsing, and the first parser error prevents execution. Consequently, a lexical
error later in the file can be reported before an earlier syntax error. No
statement executes until the whole file has passed both stages.

Each statement writes its decoded string followed by one LF to the supplied
output stream. Strings containing `\n` can therefore write several lines.
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

Diagnostic wording and library types are experimental. Codes are intended to be
more useful for tests than full platform-dependent file-error strings. Read
errors, invalid UTF-8, and output errors are CLI errors without source spans.

## Command-line contract

| Command | Behavior |
| --- | --- |
| `scarp run <path>` | Read, validate, and execute a file |
| `scarp tokens <path>` | Lex a file and print tokens without parsing |
| `scarp --help` or `scarp -h` | Show command usage |
| `scarp --version` or `scarp -V` | Show the package development version |

The `run` and `tokens` commands accept an optional `--` before the path. Exactly
one path is required, and stdin is not a source input. The `.srp` extension is a
convention rather than a filename check. Native non-Unicode paths are accepted
where supported by the operating system; the file contents must remain UTF-8.

Program output, help, versions, and token listings use stdout. Diagnostics use
stderr. Exit codes are `0` for success, `1` for source/file/output failures, and
`2` for invalid arguments. No commands install packages or publish releases.

## Scope

Expressions, numbers, bindings, functions, types, modules, loops, a compiler
backend, and a package manager are not implemented. The next proposed slice is
documented in [the integer expression proposal](proposals/0001-integer-expressions.md).
