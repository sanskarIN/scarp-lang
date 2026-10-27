# Embed Scarp in a Rust application

This guide targets the development API. It helps a host application choose
between parsing, compilation, validation, and execution while retaining control
of output and diagnostics. See [compatibility](compatibility.md) for v0.0.1 API
differences.

## Add the local library

Use a source checkout of main or the v0.0.2 tag. In a sibling Rust
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
[draft migration notes](releases/v0.0.2.md) before updating an embedding host.
