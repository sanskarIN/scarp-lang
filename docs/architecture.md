# Prototype architecture

The front end and interpreter are a reusable Rust library. The CLI is a thin
process boundary that owns argument parsing, source-root selection, standard streams,
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
| `modules` | Explicit loaders, bounded source graphs, typed module symbols, linking and owned source maps |
| `evaluator` | Checked signed 64-bit arithmetic with operator diagnostics |
| `value` | Integer/finite-float/string/boolean values, immutable string storage, primitive type names |
| `interpreter` | Evaluate all arguments, then write values and propagate I/O failures |
| `diagnostics` | Error codes, source locations, excerpts, and help |
| `lib` | Public convenience functions and combined error type |
| `main` | Native arguments, file loading, streams, and process status |

## Source graphs (0.0.5-alpha.1)

The source-string pipeline above remains free of implicit I/O. The CLI's run/check
commands use `modules::compile_file`; tokens still lexes just the requested file.
An embedder can instead supply `SourceLoader`, including `MemorySourceLoader`.
The filesystem loader canonicalizes relative paths within an explicitly trusted
root and performs bounded UTF-8 reads. It assumes a trusted tree, not concurrently
hostile filesystem mutation; see the [path policy](modules.md#paths-and-trusted-sources).

`modules::load` discovers a graph with iterative, source-ordered depth-first
traversal. Stable loader identities deduplicate repeated imports, the active path
detects cycles, and a postorder longest-path pass enforces depth even when cached
diamond edges hide a longer path. File count, source bytes and import declarations
have independent caps. No discovery step executes Scarp statements.

`modules::resolve` builds per-file symbol/alias tables, checks visibility and
declaration-only libraries, rewrites qualified names, substitutes typed scalar
literal constants and gives every function in a multi-file graph a distinct
module-qualified internal identity, including entry functions. This keeps entry
function names from affecting a library's built-in calls or local declarations.
Local lexical bindings and local function/built-in precedence are preserved.
Expansion text is charged before cloning. The resulting syntax goes through the
existing typed resolver and immutable execution plan; there is no second runtime.
Public `ModuleInfo`/`SymbolInfo` expose types, visibility and owning source IDs.

Unreleased tooling validates each import list before child-loader callbacks,
using borrowed name maps instead of repeated alias/declaration scans. Cycle
related spans identify actual edges and omit acyclic prefixes. Bounded spelling
repairs iterate source-ordered candidates and filter imported names by visibility.

`ModuleProgram` additionally owns import-site records and forward/reverse
adjacency lists. Dependency edges deduplicate aliases; source order remains
explicit rather than depending on hash iteration. Transitive queries traverse
iteratively on demand. Per-module ranges pair original symbols with borrowed
resolved signatures, avoiding a second cloned signature table. The static
inventory writer uses separate incremental location trackers for source-ordered
imports and symbols, avoiding a full prefix scan for every printed declaration.
`SourceMap::locate_span` provides a strict UTF-8/file-membership query for hosts;
defensive diagnostic rendering keeps its earlier clamping behavior.
See [the inspection contract](proposals/0030-module-inspection.md).

`ModuleProgram` owns the resolved plan and an immutable `Arc<SourceMap>`.
Sources share one coordinate space: tokens are relocated before parsing, so all
AST, resolver and runtime spans keep their existing public representation.
Rendering maps each primary/related span to its own file and UTF-8 excerpt.
Diagnostics own the source map and remain renderable after the loader/files are
dropped. Existing single-file diagnostic formatting remains unchanged.
Each execution uses independent runtime state, writers and options. Loading caps
are separate from runtime budgets. There are no runtime module globals or
initialization side effects; all library values are scalar literal constants.

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
state therefore needs no scope stack and never reuses an
inner slot for a surviving outer binding. Parsed block trees are bounded to 64
levels; resolved programs and expression evaluation remain flat.

Diagnostics accept public spans, so rendering clamps out-of-range offsets and
rounds offsets inside UTF-8 characters down to valid boundaries. Source control
characters are escaped in excerpts. This keeps diagnostic rendering safe for
callers that construct their own diagnostics.

## Compiler-planned scope retirement (alpha.4)

`NumericTile` wraps a visible-window `NumericVector` and checked axes. Shared
built-in signature metadata serves static resolution and host validation;
`numeric/tiles.rs` owns shape checks and execution without parser/frame coupling.
Source argument spans are borrowed from resolved expressions without per-call
span buffers. This is a boundary for future typed lowering, without an unused
IR or a native-backend claim. See [tiles](tiles.md) and
[memory lifetimes](memory-lifetimes.md).

Alpha.4 records lexical binding retirement
in the compiled operation stream. Scope exits clear their declared storage
slots; `break` and `continue` clear all locally exited scopes, retaining the
current repeat index and enclosing-loop values. Function completion drops
local slots while retaining parameters/results for executable postconditions.

No source syntax or public binding/type/CLI APIs are changed. Existing snapshot
semantics and work-limit boundaries are preserved. The Rust host still uses
`Arc` for immutable payload sharing, and total process-memory usage is
**not** bounded by the slot limit. See the
[scope-retirement ledger](proposals/0019-scope-retirement-ledger.md) for
the invariants, test strategy and explicit limitations.

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
path. General expressions infer an integer, float, string, or boolean type; numeric
operators require matching numeric kinds. The resolver fixes each slot's type,
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

## Output replay

General output forms lower to an argument list, destination, and newline flag.
Single-argument stdout lines retain a compact operation and avoid argument-list
allocations during parsing and resolution. Both paths share value evaluation,
limit checks, and event replay. Preparation captures values in source order.
Replay uses supplied writers and explicit LF bytes; stream changes flush the
previous writer. Validation discards values immediately and creates no replay
events. Single-writer hosts reject required stderr before replay. See the
[output guide](output.md) for the additive APIs and AST compatibility rules.

## Function integration (beta.3)

`resolver/functions.rs` resolves parameter defaults, contracts, return paths,
and an iterative strongly-connected-component call graph. It creates one flat
plan per function, preserving lexical isolation from the file frame. Immutable
public metadata has no mutable execution caches.

`evaluator::Workspace` suspends its task/value vectors at user-call nodes.
`interpreter/machine.rs` manages heap frames and resumes the caller after defaults,
needs, body, and ensures. Repeat counts use a dedicated completion that saves the
count once. All loop targets stay inside their invocation. Return changes the
current frame's stage instead of relying on Rust unwinding.

The outer interpreter module owns stream replay and I/O failures. Output snapshot
and frame caps are logical accounting, including during silent validation; they
are not a general allocation sandbox. See [functions](functions.md).

## Finite scalar invariant

The alpha stores float values in a private checked FiniteFloat wrapper.
Literal parsing and every arithmetic result enforce finiteness. Numeric Eq is
valid without NaN; signed zero is retained in bits/output but compares equal.
No raw mutable f64 reference is exposed. Float literals retain their source
spelling in tokens and become finite scalar AST nodes. Existing expression tasks,
heap frames, result slots, contracts, and output snapshots handle the scalar
without additional recursion or dependencies. See [the contract](proposals/0016-finite-floats.md).

## Numerical vector layer

The numerical layer uses NumericVector with private Arc<Vec<FiniteFloat>>
storage. Elements are contiguous and immutable; clones share storage and moving
an owned buffer into Arc avoids a second element copy. No rank/stride abstraction
is added before tensors have their own reviewed design.

The flat AST gains vector/index nodes. Generic call syntax resolves to a private
User/Builtin call identity, preserving old user names and keeping kernels out of
the user call graph. Resumable expression tasks evaluate arguments/elements in
source order and survive nested user calls. Existing heap frames/defaults/results
need no vector-specific frame representation.

numeric.rs owns signatures, checked kernels, compensated/scaled reductions and
fallible buffers; value.rs owns finite storage and host access. Work is prepaid
before element loops/allocations. At least one step per allocated element bounds
cumulative runtime vector payload by 8 * max_steps bytes, excluding container,
task, parser and host memory. Snapshots account 64 + 8n bytes per value even when
shared; output preparation charges n in both run/check. No new option field or
hidden mutable global state is introduced. See [the contract](proposals/0017-numerical-vectors.md).

Parser container state is separated from leaf-statement and atom temporaries.
Private boxed parse failures keep recursive Result values small without changing
public diagnostics or adding valid-path error allocations. This preserves the
independent 64 block/expression limits on small parser stacks; a 512 KiB
regression covers grouped scalars/vectors and nested calls at the combined bounds.
