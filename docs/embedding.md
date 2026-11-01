# Embed Scarp in a Rust application

Published alpha.4 adds public `value::NumericTile`, `Value::Tile` and
`ValueType::Tile`; exhaustive enum matches need an additional arm. Construct a
checked tile with `NumericTile::new` or `from_vector`. The latter shares a visible
window without copying. `numeric::evaluate_with_vector_limit` accepts the new
`Builtin::MatVec`, `MatMul`, `TileAdd` and `TileScale` variants. Host-provided
inputs do not spend that call's new-allocation quota, while materialized outputs
do. Existing entry-point signatures and `ExecutionLimits` literals remain valid.
See [tiles](tiles.md) and [memory lifetimes](memory-lifetimes.md).

This guide targets the 0.0.5-alpha.1 development API. It helps a host application choose
between parsing, compilation, validation, and execution while retaining control
of output and diagnostics. See [compatibility](compatibility.md) for v0.0.1 API
differences.

## Add the local library

Use a source checkout of main for the examples below. In a sibling Rust
application, a path dependency can select that checkout:

```toml
[dependencies]
scarp = { path = "../scarp-lang" }
```

Adjust the path to your directory layout. The package has no third-party Rust
dependencies and requires Rust 1.74 or later. Its registry publication is disabled;
these instructions use a local dependency rather than a registry release.

## Select a pipeline stage

The source-string stages below never access files implicitly. For imports,
select the explicit module API in the next section.

| API | Input | Result | Evaluates arithmetic? | Writes program output? |
| --- | --- | --- | --- | --- |
| `lexer::lex` | Source text | Tokens with byte spans | No | No |
| `parser::parse` | Tokens | Syntax `Program` | No | No |
| `parse` | Source text | Syntax `Program` | No | No |
| `resolver::resolve` | Owned syntax | `ResolvedProgram` | No | No |
| `compile` | Source text | `ResolvedProgram` | No | No |
| `check` | Source text | Success or `Diagnostic` | Yes | No |
| `interpreter::validate` | Resolved program | Success or `Diagnostic` | Yes | No |
| `interpreter::execute` | Resolved program and writer | Success or `RunError` | Yes | Yes |
| `run` | Source text and writer | Success or `RunError` | Yes | Yes |
| `run_with_io` | Source text and two writers | Success or `RunError` | Yes | Both streams |
| `interpreter::execute_with_io` | Resolved program and two writers | Success or `RunError` | Yes | Both streams |

Compilation resolves names, scopes, mutability, and primitive types. Arithmetic
failures such as division by zero require validation or execution. Parsing alone
can therefore succeed for a program whose names or operations are invalid.
All branches undergo name and type checks, while validation and execution
evaluate only selected conditions and bodies.

## Load modules explicitly

For a trusted source tree, `modules::compile_file(entry_path, limits)` roots
imports at the entry file's parent. For virtual sources or isolated embedding,
supply a `SourceLoader`. No global search path or mutable runtime cache is used.

```rust
use scarp::{modules::{self, MemorySourceLoader, ModuleLimits}, ExecutionOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut loader = MemorySourceLoader::new();
    loader.insert("main.srp", r#"import "math.srp" as math; print(math.twice(3));"#)?;
    loader.insert("math.srp", "pub pure funcra twice(n: integer)->integer {return n*2;}")?;
    let program = modules::compile_with_loader("main.srp", &loader, ModuleLimits::default())?;
    let (mut out, mut err) = (Vec::new(), Vec::new());
    program.execute(&mut out, &mut err, ExecutionOptions::default())?;
    assert_eq!(out, b"6\n");
    assert!(err.is_empty());
    Ok(())
}
```

`modules::load` performs bounded discovery and parsing, returning `ModuleGraph`;
`modules::resolve` links and type-checks it into `ModuleProgram`. The convenience
compiler combines those stages, without evaluating Scarp. `program.check(options)`
evaluates without output, and `program.execute(stdout, stderr, options)` prepares
and writes both streams. Compiled programs are immutable and reusable across
threads, with independent execution state.

`program.modules()` exposes module identities, imports and typed symbols.
`program.sources()` retains file names and source text. Module errors own this
map, so `ModuleDiagnostic::render` and Display identify each primary/related
location even after the loader or source files disappear. `ModuleRunError`
distinguishes source-aware diagnostics from writer failures.

Custom loaders provide stable source identities, respect the byte allowance and
own their trust policy. The compiler validates portable import spellings, graph
limits and returned source sizes. Source limits and execution options are
independent. Read [modules](modules.md) before selecting a filesystem trust root
or changing loading caps. Passing only `program.program()` to old interpreter
APIs gives raw combined-coordinate diagnostics; use ModuleProgram's wrappers
when rendering multi-file errors.

## Capture output explicitly

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
        var count = 1;
        count += 2;
        print("Count:");
        print(count);
    "#;
    let mut output = Vec::new();
    scarp::run(source, &mut output)?;
    assert_eq!(output, b"Count:\n3\n");
    Ok(())
}
```

The library does not select process stdout for you. Any suitable `std::io::Write`
destination can be supplied. The CLI is the layer that reads files and chooses
standard streams.

## Capture both streams with explicit limits

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = "print(\"ready\"); eprint(\"warning\", 2);";
    let limits = scarp::ExecutionLimits { max_steps: 100, max_prints: 2 };
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    scarp::run_with_io_and_limits(source, &mut stdout, &mut stderr, limits)?;
    assert_eq!(stdout, b"ready\n");
    assert_eq!(stderr, b"warning 2\n");
    scarp::check_with_limits(source, limits)?;
    Ok(())
}
```

The selected-output limit counts both statements, including stderr statements
and empty writes. `check_with_limits` evaluates without either writer and
enforces the same work/output limits. Its success does not emit the warning.
For a compiled program, use `execute_with_io_and_limits` or
`validate_with_limits` in `interpreter`.

Single-writer `run`/`execute` return `RunError::Output` with error kind
`Unsupported` if a selected stderr event has bytes, before writing stdout.
Both writers remain untouched for source/evaluation/budget diagnostics. During
successful replay, stream changes flush the previous writer; callers own final
flushing. An I/O failure can leave earlier bytes. The limits count logical work
and output statements, not all parser memory or the size of a single string;
they are not a general sandbox. See [output](output.md) and [budgets](budgets.md).

## Compile once and execute repeatedly

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let program = scarp::compile(
        "var count = 1; count += 2; print(count);"
    )?;
    assert_eq!(program.binding_count(), 1);
    assert_eq!(program.binding_types(), &[scarp::value::ValueType::Integer]);
    scarp::interpreter::validate(&program)?;

    for _ in 0..2 {
        let mut output = Vec::new();
        scarp::interpreter::execute(&program, &mut output)?;
        assert_eq!(output, b"3\n");
    }
    Ok(())
}
```

`ResolvedProgram` keeps its slot plan private and immutable. Each execution and
validation allocates fresh variable state. Reusing a compiled program does not
preserve previous assignments. `statement_count` counts executable operations,
including branch guards but excluding block containers and bookkeeping jumps;
`binding_count` includes distinct declarations in
all scopes. `binding_types` follows declaration/slot order.

## Preserve source for readable errors

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = "bind value = 1 / 0; print(value);";
    let mut output = Vec::new();
    match scarp::run(source, &mut output) {
        Err(scarp::RunError::Diagnostic(error)) => {
            assert_eq!(error.kind, scarp::diagnostics::DiagnosticKind::DivisionByZero);
            let rendered = error.render("embedded.srp", source);
            assert!(rendered.contains("E0008"));
            eprintln!("{rendered}");
        }
        Err(scarp::RunError::Output(error)) => return Err(error.into()),
        Ok(()) => panic!("this example should fail validation"),
    }
    assert!(output.is_empty());
    Ok(())
}
```

Diagnostics retain byte spans, not the source file. Supply the original source
and the filename you want displayed when rendering. Related locations identify
earlier declarations where relevant.

Source and semantic failures occur before program output. Output errors are
different: a writer can fail after accepting earlier bytes. The library returns
`RunError::Output` for a broken pipe too; only the CLI treats it as normal
termination. Select a host-side buffering or persistence policy appropriate to
your writer if writes must be committed separately.

## Standalone values and API boundaries

`value::Value` represents integers, booleans, and immutable strings. It exposes
`kind`, `as_integer`, `as_boolean`, and `as_str`, and implements display formatting. Constructing a
host `Value` does not inject a variable into an existing resolved program.

`evaluator::evaluate` retains an integer-result API. `evaluate_value` returns
either primitive kind from a standalone syntax expression. Without a program's
binding context, references to names return `E0010`; use compilation and
execution for complete programs with declarations.

Run `cargo doc --no-deps --open` in the selected checkout to inspect Rust API
documentation. The API remains experimental; review the
[syntax migration](syntax-migration.md) before updating an embedding host.

Development output uses `print`/`write` with `println` retained as a permanent
alias. `eprint`/`ewrite` require explicit stderr in embedding hosts. See
[output and dual-writer migration](output.md) for semantics and new AST variants.

## Function options and immutable metadata (beta.3)

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = "pure funcra double(n: integer) -> integer { return n * 2; } print(double(3));";
    let program = scarp::compile(source)?;
    assert_eq!(program.functions()[0].name, "double");
    let options = scarp::ExecutionOptions {
        max_call_depth: 32,
        max_calls: 1_000,
        ..scarp::ExecutionOptions::default()
    };
    let mut output = Vec::new();
    scarp::interpreter::execute_with_options(&program, &mut output, options)?;
    assert_eq!(output, b"6\n");
    Ok(())
}
```

`functions()` exposes read-only signatures, source spans, declared purity,
contracts, sorted callees, and recursive-component membership. Inspecting it
does not execute defaults or user code. `write_function_insight` formats it into
a caller-owned writer. Top-level binding counts exclude function-local slots;
live-slot limits count all active frames, including their saved repeat counts,
indices, and result slots. See [functions](functions.md) for precise budgets and
`check_with_options`, `run_with_options`, and dual-stream options entry points.

## Finite float values (alpha)

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let value = scarp::value::FiniteFloat::new(-0.0).expect("finite zero");
    assert!(value.get().is_sign_negative());
    assert!(scarp::value::FiniteFloat::new(f64::NAN).is_none());
    let mut output = Vec::new();
    scarp::run("pure funcra half(v: float) -> float { return v / 2.0; } print(half(1.5));", &mut output)?;
    assert_eq!(output, b"0.75\n");
    Ok(())
}
```

Use `Value::as_float()` to inspect a float result, and `evaluate_value` for
standalone float expressions. The integer evaluate helper intentionally rejects
float results. Both streams use identical scalar formatting. See [floats](floats.md).

## Numerical vector values and capped kernels (since alpha.2)

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use scarp::{numeric::{self, Builtin}, value::{NumericVector, Value}};
    let values = Value::Vector(NumericVector::new(&[1.0, 2.0, 3.0]).unwrap());
    let sum = numeric::evaluate(Builtin::Sum, std::slice::from_ref(&values), 3)?;
    assert_eq!(sum.as_float(), Some(6.0));
    assert!(numeric::evaluate(Builtin::Sum, &[values], 2).is_err());
    assert!(NumericVector::new(&[f64::NAN]).is_none());
    let mut output = Vec::new();
    scarp::run("pure funcra twice(v: vector)->vector{return scale(v,2.0);} print(twice([0.5]));", &mut output)?;
    assert_eq!(output, b"[1.0]\n");
    Ok(())
}
```

`new(&[f64])` checks finite elements/length and reserves fallibly. `from_finite`
consumes an owned checked buffer without copying it. `as_slice()` returns only
immutable FiniteFloat elements; `Value::as_vector()` borrows the wrapper. Inputs
are not injected into source bindings, and clones share immutable storage.
The host kernel API validates arity/types and uses an explicit work cap, excluding
language dispatch tasks; its diagnostics have an empty source span. Compiled
program APIs apply all six limits. Legacy evaluate_value has unbounded standalone
work, so prefer capped program/kernel APIs for controlled execution.
