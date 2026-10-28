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

## Function branch options (unreleased)

On `scarp/funcra-main`, the additive `ExecutionOptions` selects call depth,
cumulative calls, live binding slots, and cumulative output snapshot storage.
Existing two-field ExecutionLimits literals remain valid; old entry points use
default additional caps. These apply during run and check, including snapshot
limits for programs without functions. CLI flags still select only steps/prints;
use the embedding options API for other limits. See [functions](functions.md).
