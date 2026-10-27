# Output in the development language

This guide describes the unreleased output feature in the development stack,
based on the `bind`/`var` development stack. Published v0.0.2 supports only
single-argument `println`; its packages and language contract are unchanged.

| Statement | Stream | Adds one LF |
| --- | --- | --- |
| `print(...)` | stdout | Yes; preferred spelling |
| `println(...)` | stdout | Yes; permanent equivalent spelling |
| `write(...)` | stdout | No |
| `eprint(...)` | stderr | Yes |
| `ewrite(...)` | stderr | No |

## Values, separators, and lines

```scarp
bind name = "Scarp";
print("Hello,", name, 42, true,);
write("Loading");
write("...");
print(" done");
print();
```

Stdout (including the blank final line):

```text
Hello, Scarp 42 true
Loading... done

```

Every argument uses its existing value representation. Exactly one ASCII space
separates arguments, including empty strings: `print("", "x");` produces
` x\n`. Distinct statements insert no separator: `write("A"); print("B");`
produces `AB\n`. A final raw write need not end with LF. All generated line
breaks are LF bytes on Windows, Linux, and macOS.

`print();` and `println();` each produce one LF. `write();` and `write("");`
produce no bytes. Decoded `\n` inside a string remains an additional line break.
Unicode and NUL are preserved; plain `{braces}` and `%s` remain literal text.
Integers use signed decimal, booleans lowercase, and existing finite decimals
and vectors retain their normal display. No new value types or string escapes
are introduced.

A trailing comma is optional, including across lines. Empty middle arguments
such as `print(1,,2);`, comma expressions, missing `)`, and missing semicolons
remain errors. Output parentheses do not count toward expression grouping depth.
Names are case-sensitive and the five output names are reserved binding names.
They are statements, not callable expression functions. No other output aliases
or macro spellings are supported.
Previously valid variable names `print`, `write`, `eprint`, or `ewrite` must be
renamed when upgrading; they now identify built-in output statements.

## Program stderr and diagnostics

```scarp
write("result:");
print(42);
ewrite("Warning:");
eprint("configuration", "missing");
```

Stdout:

```text
result:42
```

Stderr:

```text
Warning:configuration missing
```

`scarp exec file.srp` supplies both streams. These stderr bytes are program
output; source diagnostics separately include `error[E....]` and a location.
All names/types are checked before evaluation. Only selected branches execute,
and every selected argument evaluates left to right. Captured values preserve
snapshots across later mutations. A later reachable source, arithmetic, or budget
failure prevents writes to both program streams. A writer or flush failure
during replay can leave earlier output and is propagated.

Function calls may produce nested output. All arguments of the caller's output
statement finish before that statement's own events are appended, so nested
output precedes the caller's line. Failed contracts discard both streams too.
The interpreter applies a default 16 MiB logical snapshot-storage cap, including
pending arguments; runtime validation enforces it without retaining output.
See [function budgets](functions.md#frames-budgets-and-output) for accounting.

Stream switches flush the previous destination. The host's handling of separate
streams can still alter how their text is presented together. Cross-stream
output is not globally atomic. Embedding callers own final flushing.

## Embedding, limits, and native output

Use `scarp::run_with_io(source, &mut stdout, &mut stderr)` or
`interpreter::execute_with_io(&program, &mut stdout, &mut stderr)`. Their
`*_with_io_and_limits` variants take `ExecutionLimits`. The library never writes
to global process streams. Existing single-writer APIs remain valid; if a
selected statement produces nonempty stderr, they return
`RunError::Output(Unsupported)` after evaluation and before any stdout. Unselected
stderr and zero-byte raw writes require no second writer. A runtime diagnostic
takes precedence over a missing destination.

`scarp verify` performs static checks without arithmetic. Rust `scarp::check`
and `interpreter::validate` evaluate the selected path, including stderr
arguments, without capturing, formatting, or writing output.

`max_prints` counts selected output statements of every spelling and destination,
including empty writes. Additional arguments cost one work step per separator,
plus ordinary expression/vector costs. Existing one-argument budgets are intact.

The C/native subset supports the five output statements for integers, booleans,
and direct string literals. Other value kinds remain E0027. Generated units add
`scarp_io` / `scarp_run_io`; `scarp_host`, `scarp_run`, and `scarp_validate`
retain their source API. Recompile generated units and workspace consumers;
their layout is program-specific, not a stable binary ABI. See
[the backend guide](proposals/0010-freestanding-c.md) and
[embedding](embedding.md).

Phase A and Phase B are implemented for review. Phase C remains design only in
[the output proposal](proposals/0014-output-statements.md): interpolation,
formatting specifications, formatting functions, and logging require separate
designs. `print(f"Hello, {name}!");` is not valid today.
