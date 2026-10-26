# Your first Scarp program

This tutorial works with both the published v0.0.1 interpreter and the development
branch. It teaches the complete file-to-output path before introducing variables.
Use [getting started](../getting-started.md) to select an executable.

## Statements, comments, and output

Save the following as `first.srp`:

```scarp
// A comment explains the program without producing output.
println("Build Beyond Boundaries.");
println(6 * 7);
```

Output:

```text
Build Beyond Boundaries.
42
```

From a checkout, run `cargo run -- run first.srp`. With an installed command, use
`scarp run first.srp`. Each `println` adds one LF line break. Statements run in
source order, and every print statement needs a terminating semicolon.

`//` starts a comment outside a string. Inside a string, it is ordinary text:

```scarp
println("https://example.invalid is text here");
```

Output:

```text
https://example.invalid is text here
```

The text is printed; it does not open a connection or fetch a page.

## Strings and escapes

Strings preserve UTF-8 text. Three escape sequences are supported: `\n` for a
line break, `\"` for a double quote, and `\\` for a backslash. A physical CR or LF
inside a string is invalid; write `\n` instead.

```scarp
println("Line one\nLine two");
println("नमस्ते 🌍");
println("C:\\Scarp");
```

Output:

```text
Line one
Line two
नमस्ते 🌍
C:\Scarp
```

An empty string is valid and prints a blank line. Empty or comment-only files
are also valid and print nothing.

## Find a syntax mistake

This file is missing its final semicolon and fails with `E0004`:

```scarp-invalid
println("Hello")
```

Add `;` after the closing parenthesis. The diagnostic identifies the filename,
line, column, and expected punctuation. A caret at the end of a line can identify
a missing token rather than an invalid character.

The entire file is validated before output is written. A syntax error in a later
statement therefore prevents earlier print statements from writing anything.
This makes it useful to fix the reported error and rerun the complete file.

## Inspect what the lexer sees

`scarp tokens first.srp` prints identifiers, string or integer tokens,
punctuation, source locations, and an EOF token. Comments and whitespace do not
produce tokens. A token listing can succeed for an incomplete program because
tokenization does not check statement grammar.

On the development branch, `scarp check first.srp` additionally validates the
whole program without printing its output. For the next step, read
[variables and mutation](variables.md).
