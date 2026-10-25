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
| native_dot | Two fresh vectors of 4,096 elements | A compensated native dot kernel |
| indexed_dot | The same input values and expected output | An indexed Scarp loop with a scalar accumulator |

Record the commit, compiler, target, profile, workload sizes, and command with
results. Compare identical workloads and stages on the same host. Repeat runs
when scheduling noise is material. Do not turn a timing threshold into a CI test.
The quick mode is suitable for checking that the harness still works on minimum
Rust and other operating systems.

## Execution work buffers

The workspace branch allocates task and value vectors once per execution and
reuses their capacity between expressions. Validation discards each print value
immediately. Execution still captures the selected path's prints before writing,
preserving empty output on source or arithmetic errors. Each call has fresh
variable slots and scratch state; compiled programs remain immutable and can be
used concurrently. There is no shared mutable cache.

Work-buffer memory follows the largest expression encountered in that execution.
The AST, resolved slots, literal storage, and captured execution output still have
costs. String literals still create immutable runtime values. This optimization
does not add a compiler backend, constant folding, or a general optimization pass.

## Paired snapshot comparison

Separate process runs on this host showed substantial timing drift even in the
unchanged compilation stage. The controlled comparison links two separately named
libraries into one process and alternates measurement order for 21 paired batches.
It checks both engines' workload outputs before timing. Each stage has ten paired
warmups; compile batches have ten repetitions, and runtime batches have 200.

Use PowerShell with Git and Rust installed, from a checkout containing both refs:

```powershell
./scripts/compare-runtime.ps1 -Baseline 54e6056bb429eeceb27b904cb0451fc5c53e5bd5 -Candidate 11880811ea0cb23f8ebda573fed63a3a466b4cd3 -OutputDirectory dist/comparison-review
```

The script exports both immutable Git snapshots, compiles their dependency-free
libraries with `rustc -O --edition=2021`, and runs the comparison harness. It writes
the resolved commits, compiler details, and CSV to a fresh output directory. It
refuses to overwrite existing records and does not change either checkout.
The tool targets compatible, dependency-free snapshots using the current public
compile/execute/validate APIs; future library dependencies may require updating it.

## Recorded Windows result

Recorded on 2026-10-25 with an Intel Core i5-1235U, Windows x64 MSVC, and
`rustc 1.99.1 (58cf8f9c3 2026-10-13)`. Baseline is the benchmark commit `54e6056`;
candidate `1188081` changes the evaluator workspace and validation capture policy.
The [complete CSV](benchmarks/2026-10-25-workspace.csv) includes stage timings and
sample percentiles. Values below are median **paired before/after ratios**;
they are not ratios of the independently calculated timing medians.

| Workload | Compile ratio | Validation speedup | Execution speedup |
| --- | --- | --- | --- |
| assignments | 0.987× | 4.205× | 4.132× |
| branches | 1.021× | 2.886× | 3.195× |
| prints | 0.975× | 3.598× | 3.828× |
| long expression | 1.000× | 1.005× | 1.074× |

The unchanged compilation stage serves as a control and stays near 1.0×.
Repeated small expressions benefit from amortized allocation. One long expression
has little opportunity to reuse capacity, so its result is close to unchanged.
Sample ranges remain wide on this shared host; the CSV's 5th/95th sample
percentiles describe timing variation, not confidence intervals. These results
apply to these workloads, snapshots, compiler, and machine. They do not establish
relative performance against C, C++, Rust, or other language implementations.

## Native numerical workload sample

The vector extension adds two verified dot-product workloads. Both create fresh
4,096-element vectors filled with 1.0 and 2.0, and both must output `8192.0` before
timing begins. The native reduction uses compensation; the indexed loop uses
ordinary scalar accumulation. These exactly representable inputs make both
results agree without hiding their different numerical algorithms.

Recorded with engine commit `f29a3c899ceeb1e20619374606006af67e7b3ba7`, Rust 1.99.1,
Windows x64 MSVC, and the same i5-1235U host. The standard harness uses seven
samples and 200 repetitions for runtime stages. Input allocation and fresh state
are included. [Raw CSV](benchmarks/2026-10-25-numeric-kernels.csv) retains all six
workloads, their dimensions, and all measured stages.

| Workload | Validation, median µs | Execution to sink, median µs |
| --- | --- | --- |
| native dot | 163.759 | 156.025 |
| indexed loop | 4576.287 | 6375.687 |

This sample illustrates avoiding interpreter dispatch per element. It is a
single-host observation, with scheduling variation and no timing threshold.
It is not a native-language comparison or evidence that all numerical programs
have the same relative speed. The earlier paired snapshot measurements remain
specific to their recorded commits and four original workloads.
