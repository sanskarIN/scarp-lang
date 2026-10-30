# Work and print budgets

Every run and check starts with 1,000,000 logical work steps and 100,000 selected
output statements across both streams, including empty writes. The limits include both while and repeat, nested control
flow, and expressions. They do not measure wall time, parse memory, or the byte
length of a single string. These are deterministic execution limits rather than
a general resource sandbox.

```sh
scarp run --max-steps 2000000 --max-prints 10 examples/repeat.srp
scarp check --max-steps 2000000 --max-prints 10 examples/repeat.srp
```

The first prints `8` and `0`, each with LF; check evaluates the same program
silently. Options go before the filename, in either order, once each. Values
contain unsigned decimal digits. Zero is allowed. Missing/negative/duplicate/
overflowing options or options after the filename return usage status 2.

Use `--` after options for filenames beginning with a dash. `tokens` only lexes
and rejects execution-limit options. `check` performs static validation and then
executes selected arithmetic/loops without printing; it is not static-only.

E0017 identifies the operation/expression that cannot consume another work step,
with the configured limit and help for `--max-steps`. E0018 identifies the print
that exceeds `--max-prints`, before evaluating its arguments. Check loop progress
before raising a limit. Static errors, arithmetic failures, negative repeat
counts, and exhausted budgets all leave program output empty.

An empty program succeeds with zero limits. A selected statement still requires
work even if it produces no output. Repeat evaluates its count once; each guard,
advance, backward jump, body instruction, and expression task consumes work.
The current empty `repeat n as i {}` with literal nonnegative n takes 3 + 3*n
steps. Exact work counts describe this implementation and can change with lowering.

For Rust embedding, existing entry points choose the defaults. Use
`ExecutionLimits { max_steps, max_prints }` with `run_with_limits`,
`check_with_limits`, `interpreter::execute_with_limits`, or
`interpreter::validate_with_limits`. Compiled plans store no mutable counters;
repeated or concurrent calls start fresh. Standalone expression evaluation has
finite syntax and no loop access, and retains its existing API.

See [iteration](iteration.md) for control flow and [CLI help](reference/cli.md).

Each extra argument separator consumes one work step. A selected `write();` or
`ewrite();` consumes one output-statement allowance even though it emits no bytes.
Dual-stream hosts can use `run_with_io_and_limits` or
`interpreter::execute_with_io_and_limits` with the same limits.

## Function and snapshot options (beta.3)

All six options are available on both `run` and `check`, before the file path.
Each takes unsigned decimal digits, allows zero, and may occur once. Options
may appear in any order. Missing, negative, signed, nondecimal, overflowing, or
duplicate counts produce usage status 2 before the file is read. `tokens`
rejects execution options; `--` ends option parsing for a dash-prefixed path.

| CLI option | Default | ExecutionOptions field | Meaning |
| --- | --- | --- | --- |
| `--max-steps` | 1,000,000 | `limits.max_steps` | Selected instruction and expression work |
| `--max-prints` | 100,000 | `limits.max_prints` | Selected output statements, including empty writes |
| `--max-call-depth` | 256 | `max_call_depth` | Active user frames, excluding the file frame |
| `--max-calls` | 100,000 | `max_calls` | Cumulative selected calls, including defaults/contracts |
| `--max-live-slots` | 1,000,000 | `max_live_slots` | Slots across live file/function frames and saved loop state |
| `--max-capture-bytes` | 16,777,216 | `max_capture_bytes` | Cumulative logical output snapshot storage |

```sh
scarp run --max-call-depth 512 --max-calls 200000 examples/function_recursion.srp
scarp check --max-live-slots 10000 --max-capture-bytes 1048576 examples/functions.srp
```

`ExecutionOptions` is additive: existing two-field `ExecutionLimits` literals
remain valid and old entry points use the additional defaults. Both run and
check enforce all caps, even for programs without functions. An empty program
succeeds with all six limits at zero. A call with zero depth/call allowance or
a selected output value with insufficient snapshot allowance fails with status 1.

E0027 identifies the exhausted call/depth/slot budget and its CLI/API control.
E0029 names the configured snapshot cap. Both discard pending stdout and stderr.
Snapshot storage counts UTF-8 string payload bytes plus 64 bytes per value or
separator; other scalar payloads use eight bytes. An empty printed line costs
64 bytes, `print(1)` costs 72, and `print(1, 2)` costs 209. A newline attached to
a value has no separate snapshot charge. Empty writes have no snapshot payload
but still consume an output-statement allowance. Shared values are charged for
each snapshot; pending outer arguments count before nested calls finish.

This is a logical storage budget, not an exact encoded-output or process-memory
measurement. Parser allocations are outside these execution caps. See
[functions](functions.md) for evaluation order and [embedding](embedding.md)
for reusable immutable plans and caller-selected options.

## Vector work and allocation in alpha.2

Native kernels and literal materialization prepay deterministic element work
before looping or allocating. Each allocated element costs at least one step;
cumulative runtime vector payload allocation is bounded by 8 * max_steps bytes,
excluding container/task overhead. Each vector additionally has a 65,536-element
cap and a fallible buffer reservation. Raising work limits raises this bound;
it does not introduce a total parser/host/process-memory cap.

Vector equality prepays the larger length, and selected vector output prepays
its length. Every vector snapshot costs 64 + 8n logical bytes, even if Arc shares
the elements. Check applies identical costs without retaining/formatting output.
The existing frame-slot and user-call limits keep their meanings; native kernels
consume work but no user-call depth/count. See [the full cost table](proposals/0017-numerical-vectors.md).
