# Inspect source modules

This is an **unreleased development feature toward 0.0.5-alpha.2**. Published
0.0.5-alpha.1 has imports but no `modules` command or these new inspection APIs.
The package version intentionally remains 0.0.5-alpha.1 during this phase; build
the current development checkout and identify its commit when sharing results.
No alpha.2 release or complete alpha.2 roadmap is implied.

```sh
cargo build --locked --offline
cargo run --locked --offline -- modules examples/module_inspection.srp
cargo run --locked --offline -- modules --help
```

`modules` loads, links and statically checks the same bounded graph as run/check,
then writes an inventory to stdout. It does **not** execute statements, defaults,
contracts or arithmetic. A valid `print(1/0);` or nonterminating while loop can
be inspected. Literal validity, names, visibility, types and purity still undergo
compilation checks. A missing/private/invalid declaration fails with status 1,
diagnostics on stderr and no partial inventory.

Runtime-budget options are rejected with usage status 2. Default ModuleLimits
and the trusted source-root/path policy still apply. Filenames beginning with a
dash use `scarp modules -- -entry.srp`. The command has the same native-path,
UTF-8-source and broken-pipe behavior as the existing CLI. See [modules](modules.md)
for loading limits; this command does not add filesystem/network privileges.

## Read the inventory

The [runnable entry](../examples/module_inspection.srp) imports
[`modules/inspection.srp`](../examples/modules/inspection.srp). Running it prints
`module tools 42` followed by LF. Inspecting it lists source files, aliases,
specifier strings, target IDs, import locations, direct dependencies/dependents,
and public/private constants/functions. Signatures show parameter names/types,
default presence, purity and results. Locations are one-based Unicode-scalar
line/columns. Constants' values and default expressions are omitted.

The checked [in-memory snapshot](../examples/module_inspection.modules) includes:

```text
modules: 2, imports: 1
module [0] "module_inspection.srp"
  import tools = "modules/inspection.srp" -> [1] @ 1:8
  dependencies: [1]
  dependents: none
  symbols: none
module [1] "modules/inspection.srp"
  imports: none
  dependencies: none
  dependents: [0]
  pub bind label: string @ 1:10
  private bind offset: integer @ 2:6
  pub pure funcra adjusted(value: integer, step: integer = <default>) -> integer @ 4:17
  private pure funcra helper(value: integer) -> integer @ 11:13
```

This is a summary format, not executable Scarp syntax. Ordinary entry-script
bindings are omitted because they are not module constants. Private declarations
are visible to source inspection; they remain inaccessible to importing code.
The tool neither exports them nor changes function call resolution.

The CLI uses loader-provided file labels; filesystem dependencies show canonical
paths, so paths differ across hosts/checkouts. Labels and specifiers use escaped
quoted strings, preventing raw terminal control injection. Non-Unicode native
paths remain usable where the filesystem accepts them; their display labels can
be lossy, while loader identity is preserved. Filesystems that reject such names
produce the normal read diagnostic. In-memory sources use their stable virtual
labels as shown above.
Ordering is deterministic for the same sources/loader: module discovery order,
then source-ordered imports and declarations.

## Embed the metadata

```rust
use scarp::modules::{self, MemorySourceLoader, ModuleLimits};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut loader = MemorySourceLoader::new();
    loader.insert("main.srp", "import \"math.srp\" as math; print(math.twice(3));")?;
    loader.insert("math.srp", "pub pure funcra twice(n: integer=2)->integer {return n*2;}")?;
    let program = modules::compile_with_loader("main.srp", &loader, ModuleLimits::default())?;
    let edge = &program.imports()[0];
    assert_eq!((edge.source, edge.target), (0, 1));
    assert_eq!(program.dependencies(0), Some([1].as_slice()));
    assert_eq!(program.transitive_dependents(1), Some(vec![0]));
    let at = program.sources().locate_span(edge.path_span).unwrap();
    assert_eq!((at.source, at.start.line, at.start.column), (0, 1, 8));
    let (symbol, signature) = program.functions_in(1).unwrap().next().unwrap();
    assert_eq!(symbol.name, "twice");
    assert_eq!(signature.parameters[0].name, "n");
    assert!(signature.parameters[0].default.is_some());
    let mut report = Vec::new();
    program.write_inspection(&mut report)?;
    assert!(String::from_utf8(report)?.contains("pub pure funcra twice"));
    Ok(())
}
```

`imports_in(id)` returns that source's import records with statement/path/alias
spans. `dependencies` deduplicates repeated aliases in first-import order;
`dependents` uses source discovery order. The transitive versions traverse
iteratively, exclude the queried source and return IDs in source discovery
order. Invalid IDs return None, while a valid leaf returns an empty result.
Queries and repeated/concurrent inspection do not mutate the compiled program.
Source IDs belong to that compilation; compare loader identities when tracking
files across separately compiled graphs.
Transitive queries allocate a result/visited set on demand; they are not caches
or a process-memory budget.

`functions_in` borrows the resolver's existing metadata, paired with original
module symbols. Use `SymbolInfo.name` for the source name; `FunctionInfo.name`
is an internal execution identity. Parameter/default/contract spans retain their
original source coordinates. `SourceMap::locate_span` validates a span belongs to
one source at UTF-8 boundaries, including zero-width EOF spans; invalid/reversed/
cross-file spans return None. Diagnostic rendering retains defensive clamping.

## Repairs and limits

Import spelling, reserved/duplicate aliases and alias/declaration collisions are
checked before resolving any child of that file. Root containment/canonicalization
remain loader checks. With multiple defects, an invalid import list can now be
reported before a missing dependency. Ordinary single-file semantic diagnostics
and all runtime budgets retain their established behavior. Import count failures
point to the first exceeding path.

Cycle notes point to actual import edges in the cycle and omit acyclic prefixes.
Unknown alias/member diagnostics offer best-effort spelling repairs using public
members only. Matching examines at most 256 eligible source-ordered names of at
most 64 bytes and distance two, with lexical tie-breaking. Long names and larger
candidate sets retain ordinary guidance; no hint grants access to a private name.

Existing public ModuleInfo/SymbolInfo/SymbolKind fields remain unchanged. JSON
output, watch mode, incremental recompilation, LSP, package resolution, re-exports
and module initialization remain separate work. See
[Proposal 0030](proposals/0030-module-inspection.md) for this slice's contract.
