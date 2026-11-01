# Work and print budgets

In the alpha.4 candidate, vectors and tiles share the cumulative numerical
payload ledger. Shape binding, windows and unroll share existing storage without
new element charges. Lane extraction, transpose, tile arithmetic and matrix
products prepay new payload. Retirement does not refund earlier allocations.
The [tile cost table](tiles.md#resource-contract) specifies matrix work, including
zero-inner products. Selected tile output costs cell count plus height in work,
and `80 + 8*cell_count` logical snapshot bytes. Live retained backing allocations,
visible cells, cumulative new payload and process RSS remain separate quantities.

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

Seven options are available on both `run` and `check` in the alpha.3
candidate; the published alpha.2 has six. The seventh is an independent
cumulative vector-allocation limit.
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
| `--max-vector-bytes` | 8,388,608 | `max_vector_bytes` | Cumulative newly allocated finite-vector element payload (alpha.3) |

```sh
scarp run --max-call-depth 512 --max-calls 200000 examples/function_recursion.srp
scarp check --max-live-slots 10000 --max-capture-bytes 1048576 examples/functions.srp
```

`ExecutionOptions` is additive: existing two-field `ExecutionLimits` literals
remain valid and old entry points use the additional defaults. Both run and
check enforce all caps, even for programs without functions. An empty program
succeeds with all configured limits at zero. A call with zero depth/call allowance or
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

## Independent cumulative vector allocation quota (alpha.3 candidate)

The alpha.3 candidate `--max-vector-bytes N` option limits cumulative
newly materialized numerical-element payload, separate from the step budget.
Each finite element costs 8 bytes. This cap is prepaid before reserving a
vector literal or a fresh buffer for fill/add/scale/clamp/softmax. Clones,
function argument binding and read-only numerical kernels do not charge a
new allocation. Empty vectors cost zero. Allocations in nested functions and
loops all share the same ledger; dropping or replacing a value **does not
refund** an earlier charge.

```sh
scarp run --max-vector-bytes 16 examples/vectors.srp
scarp check --max-vector-bytes 8388608 examples/vectors.srp
```

Exceeding it reports **E0036**, with the configured byte cap and a source
location, before output is committed. The cap does not measure `Arc`,
`Vec` overhead, host allocations, parser memory, task stacks or resident
process memory. It is not a complete sandbox or a cap on peak live bytes.
The previous output snapshot and frame-slot limits remain independent. See
[the design](proposals/0023-vector-allocation-ledger.md). This option is
**not present in published alpha.2**.

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
