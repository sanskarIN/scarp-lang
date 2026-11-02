# Compile-time scalar constant expressions

This is unreleased development toward 0.0.5-alpha.2. The package version remains
0.0.5-alpha.1; published alpha.1 supports scalar literal module constants only.
Build the development checkout to try these expressions. No release is prepared.

```scarp
pub bind base = 10;
pub bind multiplier = 4;
pub bind result = base * multiplier;
print(result);
```

Running this entry prints `40` followed by LF. Public bindings are evaluated
during compilation. Ordinary entry-script `bind` and `var` still execute at
runtime. Use `scarp run <file>` or module compilation APIs. Source-only
`scarp::compile` still rejects module declarations; use the module APIs instead.

## Declarations and ordering

Every immutable file-level binding in an imported file is a module constant.
In the entry, only `pub bind` declares a module constant. Functions and nested
bindings retain their existing execution and lexical scope rules.

Initializers may reference earlier module constants in the same file, whether
public or private, and public constants through an explicit import alias. Imports
are file-scoped and need not precede constants textually. Dependencies are compiled
before importers; shared files are compiled once. Functions can use constants
declared later, including in defaults/contracts. Initializers themselves follow
declaration order. Ordinary entry bindings cannot supply constants. Imported
private declarations remain inaccessible; aliases are not re-exported.

The [complete runnable example](../examples/constant_expressions.srp) spans three
files and prints:

```text
constant expressions 42 84 true 0.75 42
```

## Expressions and types

| Forms | Result and checks |
| --- | --- |
| Scalar literals, constant references, groups | Preserve integer/finite-float/boolean/string type |
| Numeric unary `-`, binary `+`, `-`, `*`, `/` | Same numeric kind; no implicit integer/float mixing |
| `<`, `<=`, `>`, `>=` | Same numeric kind; boolean result |
| `==`, `!=` | Same scalar kind; boolean result, including string equality |
| `not`, `and`, `or` | Boolean operands and result |

Precedence, integer division, overflow and finite-float rules match
[runtime values and operators](reference/values.md). Strings have no concatenation.
Signed floating zero is preserved; division by either sign of zero is an error.
Every evaluated intermediate result must fit its type.

Boolean operators short-circuit: `pub bind safe = true or (1 / 0 > 0);` succeeds.
Both operands still undergo static name/type checking: `true or missing` and
`true or (1 + 2.0 > 0)` fail. Skipped references appear in dependency metadata.

Calls, including pure functions and built-ins, vectors and indexing are forbidden
in every initializer branch. Construct vectors/tiles and call functions in an
entry script or function body. Constant evaluation cannot reach user execution
frames, printing or file I/O. Unused constants are validated too. `modules`
evaluates these restricted initializers during compilation; it executes no entry
statements, function bodies, defaults or contracts.

## Diagnostics

| Code | Constant expression failure |
| --- | --- |
| E0007 / E0008 | Integer overflow / division by zero, at the operator |
| E0010 | Unknown name, including a runtime binding used as a constant |
| E0015 | Incompatible operands; numeric mismatches include operand type notes |
| E0031 | Nonfinite float intermediate result |
| E0037 | Call, vector or indexing initializer |
| E0039 | Local constant dependency cycle or an import cycle |
| E0040 | Constant node/task limit or existing module storage/loading limit |
| E0041 / E0042 | Private member / invalid or later constant reference |

Forward references identify the use and later declaration. Local cycles are found
before forward references; notes show actual cycle references, omit acyclic
prefixes and are capped at 16 related sites. Evaluation/type failures identify
the owning constant. Locations preserve files and Unicode-scalar line/columns.
Function argument errors consistently relate the parameter declaration in single
and multi-file compilation. Return errors relate the declared result and the
returned expression's inferred type, with an actionable suggestion.

## Host APIs and allowances

Existing module entry points use `ConstantLimits::default()`: 1,000,000 initializer
nodes and 2,000,000 evaluator tasks cumulatively across the graph. Node validation
includes skipped branches and precedes initializer cloning/typing. Selected
evaluation uses a separate budget: a literal takes one task, `1 + 2` four, and
`true or false` three. Parsing/loading retain source/nesting bounds. These are
deterministic work/storage allowances, not physical process heap caps.

Runtime `ExecutionOptions` and CLI runtime flags do not alter compile-time limits.
Existing expansion limits charge constant-reference string copies and new
reference-derived folded strings before cloning. Direct literal storage remains
covered by source limits, preserving previous literal-only expansion boundaries.

```rust
use scarp::modules::{self, ConstantLimits, MemorySourceLoader, ModuleLimits};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut loader = MemorySourceLoader::new();
    loader.insert("main.srp", "pub bind base=6; pub bind answer=base*7;")?;
    let program = modules::compile_with_loader_and_constant_limits(
        "main.srp", &loader, ModuleLimits::default(),
        ConstantLimits { max_nodes: 4, max_steps: 5 },
    )?;
    let edge = &program.constant_dependencies()[0];
    assert_eq!((edge.source, edge.target), (0, 0));
    assert_eq!(program.sources().locate_span(edge.span).unwrap().start.line, 1);
    assert_eq!(program.constant_dependencies().len(), 1);
    Ok(())
}
```

`resolve_with_constant_limits` accepts a previously loaded graph;
`compile_file_with_constant_limits` accepts a native path. Existing ModuleLimits
and compile signatures remain unchanged. `modules::load` only loads/parses;
`resolve` performs constant compilation and ordinary type checking.

`constant_dependencies()` borrows direct reference records in module discovery,
declaration and reference order. Repeated uses are separate records. Records carry
owning/target source IDs, declaration spans and the use span. IDs belong to that
compilation, not a persistent cache. No values are added to text inspection output.
See [Proposal 0031](proposals/0031-constant-expressions.md).
