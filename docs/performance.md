# Measure performance before changing execution

Scarp currently interprets programs. Native compilation and performance
comparisons with compiled C, C++, or Rust programs remain future work. These
benchmarks measure Scarp pipeline stages, not native-language parity.

## Reproduce the workloads

Run `cargo bench --locked --offline --bench runtime` on an otherwise idle machine.
The standard release profile is used. `-- --quick` selects a shorter smoke run;
use the default run for comparisons. No third-party benchmark dependency is
required. The executable checks every workload's expected output before timing.

The CSV reports median microseconds per call across seven batches. Each compile
batch has ten repetitions; each runtime batch has 200. Each stage has three
untimed warmup calls. Compile includes lexing, parsing, resolution, and dropping
the resulting program. It does not generate machine code. Validation and
execution reuse a previously compiled program with fresh state on every call.
Execution includes formatting into `std::io::sink`, rather than terminal I/O.

| Workload | Shape | What it exercises |
| --- | --- | --- |
| assignments | 4,096 updates, one final print | Repeated small expressions and mutable slots |
| branches | 1,024 decisions, one final print | Boolean guards, short-circuiting, and forward jumps |
| prints | 4,096 arithmetic print statements | Evaluation, captured values, and output formatting |
| long_expression | One sum of 4,096 literals | A deep flat expression and iterative traversal |

Record the commit, compiler, target, profile, workload sizes, and command with
results. Compare identical workloads and stages on the same host. Repeat runs
when scheduling noise is material. Do not turn a timing threshold into a CI test.
The quick mode is suitable for checking that the harness still works on minimum
Rust and other operating systems.

## Current execution costs

Each expression currently allocates fresh task and value vectors. Validation
also retains captured print values until the selected execution path completes,
even though it never writes them. The next optimization reuses execution-local
scratch capacity and discards validation print values immediately. State must
remain isolated between calls and across threads, and all diagnostic and output
ordering guarantees must remain unchanged.
