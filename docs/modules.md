# Source modules

Modules are implemented in the **published 0.0.5-alpha.1 prerelease**.
Earlier 0.0.4-alpha.4 packages have no imports. See the
[publication record](releases/v0.0.5-alpha.1-validation.md) for verified downloads.
Start with the [import tutorial](tutorials/modules.md) for a complete example.
The [design contract](proposals/0029-source-modules.md) records the grammar.

## Import and visibility

An entry file can import another UTF-8 `.srp` file with an explicit alias:

```scarp
import "libraries/math.srp" as math;
print(math.twice(math.factor));
```

The corresponding `libraries/math.srp` contains:

```scarp
pub bind factor = 2.0;
pub pure funcra twice(value: float) -> float { return value * factor; }
```

The output is `4.0` followed by a newline. `pub` makes a function or immutable
scalar constant accessible to importers. All other declarations are private to
their file. Functions can call private helpers and use private constants in the
same module. All function bodies are checked, including unused private helpers.

Use `math.twice(...)` to call a public function and `math.factor` to read a public
constant. Qualification has exactly one alias and one member. Imports are not
re-exported; `math.other.member`, wildcard imports, unqualified imported names
and module values are unsupported. Imported members cannot be assigned to.

Aliases must be unique within a file and cannot collide with its file-level
functions or bindings. Each library has its own function and constant namespace,
so two libraries can both define `twice`. Existing lexical shadowing applies to
parameters and local values. A nested value named `math` does not change the
meaning of the qualified name `math.factor`.

`import`, `pub` and `as` are contextual identifiers. Ordinary old bindings and
function calls using those names keep working. Imports and public declarations
are permitted only at file scope.

## Declarations and execution

Library files contain imports, functions and immutable scalar constants.
Published alpha.1 accepts literals, grouping and numeric negation. Unreleased
development also supports [constant expressions](constant-expressions.md): scalar
arithmetic, comparisons, logic and earlier/imported public constant references.
Folded typed values are substituted at uses without runtime module initialization.
A library cannot contain file-level prints, calls, loops, mutable bindings or
vector/tile constants. Calls are forbidden in constant initializers too.

Entry files retain ordinary script statements. An entry's `pub bind` is a
declarative scalar constant too; ordinary `bind` statements keep their existing
runtime behavior and cannot be captured by functions. A function can refer to
its own module constants, local helpers and explicitly imported public members.
It cannot capture values from its caller's file.

Typed arguments/results, named/default arguments, purity, needs/ensures,
forward calls and mutual recursion within a file retain the existing
[function rules](functions.md). Local functions take precedence over built-ins
within their own file. A function in a different library does not change an
unrelated file's built-in calls.

The complete source graph is loaded, parsed, linked and checked before execution.
Discovery does not execute Scarp code. A canonical file is loaded once even when
several aliases or diamond-shaped dependencies reach it. Libraries have no
initialization side effects to repeat. Cycles are rejected even when their
members are unused. Program output is still prepared before either stream is
written; source, type, contract and resource failures emit no program output.

## Paths and trusted sources

The CLI trusts the entry file's parent directory. Imports are relative to the
**importing file**, using portable `/` separators and a `.srp` suffix. For example,
`libraries/linear.srp` can import `"settings.srp"` from the same subdirectory.
The working directory only determines how the CLI finds the entry file.

`.` and `..` normalize within the trusted root. Absolute paths, escaping `..`,
backslashes, drive/URI prefixes such as `C:` or `https:`, empty components, and
NUL/control characters are rejected. Filenames may contain spaces and Unicode.
Canonicalization rejects symlinks resolving outside the root and deduplicates
aliases of the same canonical path. Distinct hard-link paths are distinct
identities. File case follows the host filesystem's canonicalization behavior.
The entry path uses native OS encoding and need not have a `.srp` extension.

The filesystem loader assumes a trusted source tree. Its path checks are not a
race-proof sandbox against someone concurrently replacing directories or
symlinks. Embedders handling adversarial trees should supply an isolated or
in-memory loader. There are no network imports, package downloads or search paths.

## Source limits

| `ModuleLimits` field | Default | Accounting |
| --- | --- | --- |
| `max_files` | 128 | Unique canonical sources, including the entry |
| `max_depth` | 32 | Import edges on the longest path; entry depth is zero |
| `max_file_bytes` | 1,048,576 | UTF-8 bytes in each source |
| `max_total_bytes` | 8,388,608 | UTF-8 bytes across unique sources |
| `max_imports` | 1,024 | Import declarations, including repeated aliases |
| `max_expansion_bytes` | 8,388,608 | New symbol names and substituted string text |

Filesystem reads are bounded; custom loaders receive a byte allowance and their
returned source lengths are checked again. Cached diamond dependencies still
count toward the longest path. Zero limits are valid and enforced.
These limits bound logical loading/linking resources, not all parser/container
allocations or process memory. Runtime work, calls, slots, snapshots and numerical
payload retain their independent [budgets](budgets.md).

The CLI uses the defaults above. Rust hosts can supply `ModuleLimits` to
`modules::compile_file` or `modules::compile_with_loader`. Source-only APIs such
as `scarp::compile` and `scarp::run` never read files implicitly; use the explicit
[module embedding API](embedding.md#load-modules-explicitly).

## Diagnose an import

Unreleased development validates a file's portable import spelling and alias
namespace before resolving any dependency from that file. Duplicate/reserved
aliases and declaration collisions therefore precede child-file read failures.
Root containment remains a loader check. Cycles identify actual import edges;
unknown alias/member errors can suggest nearby valid names, using public members
only. See [module inspection](module-inspection.md) for the new static command,
metadata queries and precise compatibility/ordering rules. Published alpha.1
does not include these tooling changes.

| Code | Typical problem | Repair |
| --- | --- | --- |
| E0037 | Nested import, unsupported library statement or source-only API | Move declarations to file scope, move computation inside a function, or select a module API |
| E0038 | Missing/unreadable file or invalid UTF-8 | Check the path relative to its importing file and save UTF-8 |
| E0039 | Import or constant cycle | Follow related references; remove the cyclic dependency |
| E0040 | Source graph/expansion limit | Reduce the graph or explicitly choose suitable host limits |
| E0041 | Private member | Mark the intended function/constant `pub`, or call a public wrapper |
| E0042 | Unknown alias/member or wrong symbol use | Check alias spelling, the member's declaration and whether it is callable |
| E0043 | Path policy violation | Use a relative `/`-separated `.srp` path inside the root |

Duplicate functions retain E0020; alias/constant collisions use E0011 with a
related declaration. Cross-file type errors include the parameter declaration;
contract and arithmetic failures retain their bounded call traces. Each excerpt
is rendered against its owning file. Exact examples live in
[`tests/fixtures/modules`](../tests/fixtures/modules/).

To break `a.srp -> b.srp -> a.srp`, move the shared declarations into `common.srp`
and have both files import it. Local mutual function recursion remains supported;
cross-module cyclic dependencies are deferred. Runtime globals, vector/tile
exports, re-exports, packages and a standard library require separate designs.
