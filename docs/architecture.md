# Prototype architecture

The front end and interpreter are a reusable Rust library. The CLI is a thin
process boundary that owns argument parsing, filesystem access, standard streams,
and exit codes. There are no external Rust dependencies.

```text
UTF-8 source
    |
    v
lexer::lex -> Vec<Token>
    |
    v
parser::parse -> Program
    |
    v
interpreter::execute -> supplied Write destination
```

`compile` combines lexing and parsing. `run` combines compilation and execution.
They do not read files or assume a global stdout destination. The language layer
returns errors; the CLI renders them using the source and path it already owns.

## Modules and responsibilities

| Module | Responsibility |
| --- | --- |
| `lexer` | Skip trivia, decode strings, retain original byte spans |
| `parser` | Validate token order and construct print statements |
| `ast` | Store ordered statements and their spans |
| `interpreter` | Write decoded text and propagate I/O failures |
| `diagnostics` | Error codes, source locations, excerpts, and help |
| `lib` | Public convenience functions and combined error type |
| `main` | Native arguments, file loading, streams, and process status |

The lexer cursor always advances by UTF-8 character boundaries. The parser uses
an index into tokens and does not recursively process the top-level statement
list. Syntax errors refer to the unexpected token, or an empty EOF span when
punctuation is missing at the end of a file. The AST owns its strings, allowing
the source and token vector to be dropped after successful compilation.

Diagnostics accept public spans, so rendering clamps out-of-range offsets and
rounds offsets inside UTF-8 characters down to valid boundaries. Source control
characters are escaped in excerpts. This keeps diagnostic rendering safe for
callers that construct their own diagnostics.

## Failure boundaries

Lexing and parsing finish before any writes. Output failures are different from
source errors: earlier writes can succeed before a sink fails. Library callers
choose a `Write` implementation and handle `RunError::Output` themselves. The CLI
uses buffered stdout, explicitly flushes it, and reports flush errors too.
Stderr write failures are ignored because there is no further error destination.

The CLI keeps paths as native OS strings. It uses lossy display conversion only
for messages, never for filesystem lookup. Invalid UTF-8 source is a file-read
error. File extensions do not select parsers.

## Current costs and constraints

Source, token strings, and AST strings are owned in memory. There is no streaming
parser, incremental compilation, error recovery, or optimization pass. Location
lookup scans source prefixes; large token listings can be slower than execution.
This is acceptable for the current tiny slice and should be profiled before
adding source-map caching.

Prototype APIs and error wording may change. Keep new semantics in the library
and new process behavior in the CLI. Add distinct expression AST nodes and value
types when expressions are accepted; do not turn print statements into ad hoc
string evaluation.

## Validation

The test suite separates language semantics from CLI process behavior. It checks
byte spans, Unicode boundaries, truncations of valid source, malformed token
streams, output sink failures, and long flat statement lists. CLI tests create
unique temporary directories and compare observable streams and exit codes.
The example corpus pairs each `.srp` file with exact expected `.stdout` bytes.

Local shell and PowerShell scripts run formatting, Clippy, tests, builds, rustdoc,
and smoke commands. CI repeats validation on three operating systems and tests
the declared minimum Rust version. There is no release automation.
