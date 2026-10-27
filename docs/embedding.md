# Embed Scarp in a Rust application

This guide targets the development API. It helps a host application choose
between parsing, compilation, validation, and execution while retaining control
of output and diagnostics. See [compatibility](compatibility.md) for v0.0.1 API
differences. This unreleased syntax branch uses `bind` and `var` in Scarp source
and renames `ast::Statement::Let` to `ast::Statement::Declaration`. Native Rust
declarations in host programs continue to use Rust syntax. See the
[migration table](compatibility.md#declaration-syntax-migration).

## Add the local library

Use a source checkout of the documented development branch. In a sibling Rust
application, a path dependency can select that checkout:

```toml
[dependencies]
scarp = { path = "../scarp-lang" }
```

Adjust the path to your directory layout. The package has no third-party Rust
dependencies and requires Rust 1.74 or later. Its registry publication is disabled;
these instructions use a local dependency rather than a registry release.

## Select a pipeline stage

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
| `run_with_io` | Source text and two writers | Success or `RunError` | Yes | Both explicit streams |
| `interpreter::execute_with_io` | Resolved program and two writers | Success or `RunError` | Yes | Both explicit streams |

Compilation resolves names, scopes, mutability, and primitive types. Arithmetic
failures such as division by zero require validation or execution. Parsing alone
can therefore succeed for a program whose names or operations are invalid.
All branches undergo name and type checks, while validation and execution
evaluate only selected conditions and bodies.

## Capture output explicitly

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
        var count = 1;
        count += 2;
        println("Count:");
        println(count);
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

## Compile once and execute repeatedly

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let program = scarp::compile(
        "var count = 1; count += 2; println(count);"
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
    let source = "bind value = 1 / 0; println(value);";
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

## Explicit stdout and stderr

```rust
fn main() -> Result<(), scarp::RunError> {
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    scarp::run_with_io(
        "write(\"answer:\"); print(42); eprint(\"warning\", true);",
        &mut stdout,
        &mut stderr,
    )?;
    assert_eq!(stdout, b"answer:42\n");
    assert_eq!(stderr, b"warning true\n");
    Ok(())
}
```

`run_with_io_and_limits` and `execute_with_io_and_limits` take explicit
`ExecutionLimits`. All destinations are caller-supplied; the library never
touches global stdout/stderr. Preparation captures one ordered event list before
replay. Source/evaluation errors affect neither writer. Stream switches flush
the previous writer, and a flush failure stops replay. Callers own final flushing
and buffering; I/O failures can leave earlier bytes written.

Old single-writer APIs remain valid for stdout-only programs. If a selected
statement needs nonempty stderr, they return `RunError::Output` with kind
`Unsupported` after evaluation and before stdout. Unselected stderr and zero-byte
raw writes require no second writer. `check` evaluates both kinds of arguments
without output; CLI `verify` remains static. See [output](output.md) for the
language rules and [compatibility](compatibility.md) for additive AST variants.

## Function programs and execution options

Existing `ExecutionLimits { max_steps, max_prints }` and all existing entry points
remain valid. Additive `ExecutionOptions` includes those limits plus call depth,
total user calls, live slots, and output snapshot storage. Runtime validation
checks the same budgets as execution. Static `compile` checks every function body
but does not evaluate calls, defaults, contracts, or arithmetic.

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = "pure funcra twice(n: integer) -> integer { return n*2; } print(twice(9));";
    let program = scarp::compile(source)?;
    assert_eq!(program.functions()[0].name, "twice");
    let options = scarp::ExecutionOptions {
        max_call_depth: 8,
        max_calls: 100,
        max_capture_bytes: 4096,
        ..scarp::ExecutionOptions::default()
    };
    scarp::interpreter::validate_with_options(&program, options)?;
    let (mut out, mut err) = (Vec::new(), Vec::new());
    scarp::interpreter::execute_with_io_and_options(&program, &mut out, &mut err, options)?;
    assert_eq!(out, b"18\n");
    assert!(err.is_empty());
    Ok(())
}
```

`run_with_options`, `run_with_io_and_options`, and `check_with_options` compile
source then use the same engine. `ResolvedProgram::functions()` returns a
source-ordered immutable slice with parameter/default spans, result types,
declared purity, contract spans, sorted direct callees, and recursive-cycle flags.
`resolver::write_function_insight` writes a deterministic human-readable listing
without evaluating the program. Built-in callees use the `builtin::` prefix.
Binding metadata on `ResolvedProgram` still describes file-frame slots; function
frame layouts remain private. Function declarations do not count as file
instructions. See [functions](functions.md) for precise resource accounting.

## Standalone value compatibility

`value::Value` represents integers, finite floats, booleans, immutable strings,
and numerical vectors, plus the `Unit` no-result marker used internally by calls.
Source programs cannot print or bind unit. It exposes `kind`, `as_integer`, `as_float`, `as_boolean`,
`as_vector`, and `as_str`, and implements display formatting. Constructing a
host `Value` does not inject a variable into an existing resolved program.

`evaluator::evaluate` retains an integer-result API. `evaluate_value` returns
any supported value kind from a standalone syntax expression. Without a program's
binding context, references to names return `E0010`; use compilation and
execution for complete programs with declarations.

Run `cargo doc --no-deps --open` in the selected checkout to inspect Rust API
documentation. The API remains experimental; review the
[migration notes](releases/v0.0.2.md) before updating an embedding host.
