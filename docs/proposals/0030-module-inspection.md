# Proposal 0030 - Static module inspection

Status: focused, unreleased development toward 0.0.5-alpha.2. The Cargo version
remains 0.0.5-alpha.1. This work does not authorize a release, tag or publication.

## Scope

Add `scarp modules [--] <entry>` and `scarp modules --help`. The command uses the
explicit bounded module compiler, including visibility and static type checking,
then prints the loaded files, import aliases/specifiers/targets and locations,
direct dependencies/dependents and public/private declarations. Function summaries
include original names, parameter names/types/default presence, purity and result
types. Constant values and default expressions are not printed. Defaults are not
evaluated; [Proposal 0031](0031-constant-expressions.md) extends compilation with
restricted constant initializer evaluation.
Ordinary script bindings are not module constants and are omitted.

Inspection never executes runtime statements, defaults or needs/ensures.
It can therefore inspect a statically valid program that would fail or loop when
run. Loading/linking failures produce diagnostics and no partial inventory.
Writer failures retain the existing CLI broken-pipe/exit-code contract.
All runtime-budget flags are rejected; existing default ModuleLimits still apply.
The existing `run`, `check`, `tokens`, help and version behavior is preserved.

Text uses module/source discovery order, import/declaration source order and
escaped loader-provided filenames. Filesystem dependency labels remain canonical
native paths; text is deterministic for the same sources/loader, not promised
byte-identical across different checkout directories or operating systems.

## Reusable metadata

Keep existing public ModuleInfo, SymbolInfo and SymbolKind fields unchanged.
Add immutable import-edge records with owning source ID, specifier, alias, target
ID, statement span, path span and alias span. Add direct and transitive forward/
reverse dependency queries. Direct edges deduplicate repeated aliases in first
declaration order; reverse edges use source discovery order. Transitive results
exclude the queried file and use source discovery order, with iterative traversal.
Invalid source IDs return None. No incremental compiler or mutable graph cache
is introduced.

Reuse the existing resolved function metadata for rich parameter/default/contract
inspection through a borrowed iterator pairing original module symbols with
function information. Maintain private per-module ranges rather than cloning a
second set of signatures. Expose validated source-map span locations for tools;
invalid/cross-file/non-UTF-8-boundary spans return None. Diagnostic rendering keeps
its existing defensive clamping behavior.

## Resolution and diagnostic improvements

Validate every file's portable import spelling, reserved/duplicate aliases and collisions with
file-level declarations before resolving/loading any child. Import count failures
point to the first exceeding declaration. This avoids unnecessary loader calls
for an already-invalid import list. Root containment/canonical path resolution
remain the explicit loader's responsibility. Ordinary single-file semantic diagnostics
remain the existing resolver's responsibility.

Remove repeated alias/declaration scans from the linker once graph loading has
validated them. Report actual cyclic import edges and omit unrelated acyclic
prefixes. Unknown alias/member guidance can suggest nearby valid names, with
deterministic bounded matching and public-only imported-member candidates.
Never weaken private access checks or suggest private members as public repairs.
Existing diagnostic code assignments remain unchanged.

## Validation and deferred work

Test exact metadata/UTF-8/CRLF spans, signature/default details, repeated and
diamond dependencies, forward/reverse queries, deterministic rendering, failed
writers, concurrent reuse, no-execution inspection, validation before I/O, cycles,
visibility and typo repairs. Add runnable sources and stable in-memory inventory
fixtures, plus CLI/package smoke coverage. Preserve Rust 1.74, no dependencies,
unsafe forbid, source limits, runtime budgets and published artifacts.

JSON output, watch mode, incremental recompilation, LSP, package resolution,
re-exports, module initialization and the rest of the alpha.2 roadmap are deferred.
