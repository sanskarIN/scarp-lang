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
resolver::resolve -> ResolvedProgram
    |
    v
interpreter::execute -> supplied Write destination
```

`compile` combines lexing, parsing, and name resolution. `parse` exposes the
unresolved syntax tree. `evaluator::evaluate` performs checked
integer evaluation. `interpreter::execute` evaluates every print argument before
writing any output. `run` combines compilation and execution.
They do not read files or assume a global stdout destination. The language layer
returns errors; the CLI renders them using the source and path it already owns.

## Modules and responsibilities

| Module | Responsibility |
| --- | --- |
| `lexer` | Skip trivia, decode strings, retain original byte spans |
| `parser` | Precedence parsing, literal validation, and nesting limits |
| `ast` | Ordered statements and flat postorder expression nodes with spans |
| `resolver` | Lexical scopes, declaration ordering, mutability, immutable slot plans |
| `evaluator` | Checked signed 64-bit arithmetic with operator diagnostics |
| `value` | Integer/float/string/boolean values, immutable string storage, primitive type names |
| `numeric` | Typed immutable vector kernels, compensated reductions, and native element work |
| `limits` | Per-call deterministic execution and print budgets |
| `interpreter` | Evaluate all arguments, then write values and propagate I/O failures |
| `diagnostics` | Error codes, source locations, excerpts, and help |
| `lib` | Public convenience functions and combined error type |
| `main` | Native arguments, file loading, streams, and process status |

The lexer cursor always advances by UTF-8 character boundaries. The parser uses
an index into tokens and does not recursively process the top-level statement
list. Syntax errors refer to the unexpected token, or an empty EOF span when
punctuation is missing at the end of a file. The AST owns its strings, allowing
the source and token vector to be dropped after successful compilation.

Each integer argument owns a vector of expression nodes and a root index. Nodes
refer only to earlier children. Parser construction is private to the crate,
while callers can inspect immutable node slices. Binary chains are parsed with
loops and evaluated with an explicit task stack; dropping the vector has no recursive tree
walk. Grouping uses bounded recursion, with a maximum of 64 nested groups.
Unary minus is represented explicitly except for the minimum integer literal,
whose positive magnitude cannot be represented as an `i64`.

The resolver keeps a stack of lexical name maps and assigns unique slots to
declarations. Initializers resolve before insertion, allowing inner declarations
to read outer names. Block structure is flattened into storage and print
operations. Conditional statements lower to forward branch guards and jumps;
execution uses a program counter and evaluates only the selected path. Runtime
loops add budgeted backward jumps and nearest-loop control. Numeric calls use
native finite-vector kernels; they do not dispatch interpreted expressions per
element. All instructions retain spans for execution-budget diagnostics. Runtime
state therefore needs no scope stack and never reuses an
inner slot for a surviving outer binding. Parsed block trees are bounded to 64
levels; resolved programs and expression evaluation remain flat.

Diagnostics accept public spans, so rendering clamps out-of-range offsets and
rounds offsets inside UTF-8 characters down to valid boundaries. Source control
characters are escaped in excerpts. This keeps diagnostic rendering safe for
callers that construct their own diagnostics.

## Failure boundaries

Lexing, parsing, resolution, primitive type checks, and arithmetic evaluation
finish before any writes. Both source
and evaluation diagnostics are carried by `RunError::Diagnostic`.
Output failures are different from
source errors: earlier writes can succeed before a sink fails. Library callers
choose a `Write` implementation and handle `RunError::Output` themselves. The CLI
uses buffered stdout, explicitly flushes it, and reports flush errors too.
Stderr write failures are ignored because there is no further error destination.

The CLI keeps paths as native OS strings. It uses lossy display conversion only
for messages, never for filesystem lookup. Invalid UTF-8 source is a file-read
error. File extensions do not select parsers.

## Current costs and constraints

Source, token strings, and AST strings are owned in memory. There is no streaming
parser, incremental compilation, error recovery, or optimization pass. Random
diagnostic location lookup scans source prefixes. Sequential token listings use
a constant-space location tracker that scans source characters once in token
order, avoiding repeated prefix scans on large single-line files.

Prototype APIs and error wording may change. Keep new semantics in the library
and new process behavior in the CLI. Direct string literal prints retain a fast
path. General expressions infer integer, float, string, or boolean types; arithmetic
and compound assignment require same-kind numbers. The resolver fixes each slot's type,
and strings share immutable storage so snapshots survive later reassignment.
Boolean logic traverses only the required operands at runtime. Both operands
still participate in static name and type checks. No runtime traversal recurses
with flat logical-chain length.

## Validation

The test suite separates language semantics from CLI process behavior. It checks
byte spans, Unicode boundaries, truncations of valid source, malformed token
streams, output sink failures, and long flat statement lists. Arithmetic tests
exercise boundaries, precedence, long chains, excessive nesting, and agreement
with a bounded reference calculation. CLI tests create
unique temporary directories and compare observable streams and exit codes.
The example corpus pairs each `.srp` file with exact expected `.stdout` bytes.

`check` uses the same preparation pass as execution and discards each print
value immediately. It does not retain, format, or write pending output. Syntax, resolution, and
arithmetic failures therefore agree between validation and execution. Tests also
generate 2,000 scoped programs against an independent value model and mutate
source/token streams to verify failure handling and diagnostic boundaries.

The evaluator reuses execution-local task and value buffers between expressions.
Scratch storage is never shared with another execution or stored in an immutable
compiled program. This removes repeated vector allocation on common statement
sequences while keeping fresh slots, concurrent reuse, and captured print values.
The [performance guide](performance.md) describes reproducible stage benchmarks.

Local shell and PowerShell scripts run formatting, Clippy, tests, builds, rustdoc,
and smoke commands in debug and release modes. CI repeats validation on three operating systems and tests
the declared minimum Rust version. A separate package workflow builds archives,
extracts them, and smoke-tests the delivered executables. Release publication
remains a separate maintainer action.
