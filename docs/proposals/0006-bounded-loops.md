# While loops and deterministic execution budgets

Status: implemented on `scarp/bounded-loops`, dependent on the expression,
conditional, and execution-workspace branches. Integrated through PR #28 for
v0.0.3-beta.1; published v0.0.2 remains unchanged.

## Iteration and local state

`while condition { statements }` requires a boolean condition and braces. The
condition is checked before every iteration, including the first. `break;` exits
the nearest enclosing loop; `continue;` rechecks its condition. These words are
reserved binding names. There is no loop-else clause or implicit truthiness.

```scarp
let mut index = 0;
let mut total = 0;
while index < 5 {
    index += 1;
    if index == 3 { continue; }
    total += index;
}
println(total);
```

Output:

```text
12
```

The body has a lexical scope. Its declarations are initialized on every reached
iteration, reuse their compiled slots, and cannot escape the body. Assignments to
outer mutable bindings persist. All bodies remain statically checked, even when
the initial condition is false. `break`/`continue` outside a loop produce `E0016`,
including in an unselected conditional. Braces share the existing 64-level limit.

Resolution uses a stack of loop contexts to patch nearest-loop targets. The
immutable execution plan contains forward/backward jumps and spans. Runtime
execution remains iterative, and iteration count does not increase slot storage.
`Statement` gains `While`, `Break`, `Continue`, and a `span()` accessor. Operation
metadata includes loop guards and explicit loop control, excluding bookkeeping jumps.

## Deterministic bounds

Default execution permits 1,000,000 work steps and 100,000 selected prints.
Each instruction and each expression task consumes a step. Future native kernels
can charge their element work through the same internal budget. A budget is
fresh for every call and is never stored in the compiled program. Compilation
and static checks precede execution budgets; these are not wall-clock deadlines
or limits on parsing/source-file memory.

Exhausted steps produce `E0017` at the operation or expression that cannot run.
Exceeded selected print count produces `E0018` at that print, before its argument
is evaluated. Validation enforces the same counts despite discarding values.
Skipped branches consume no runtime work. Zero budgets are valid: an empty
program succeeds, while an operation requiring a step fails.

Limits count the current implementation's execution work; exact counts can
change when lowering or kernels change. They are not a language complexity metric.
The print bound limits value count, not the byte length of an individual string.
Existing source/arithmetic/budget failures all leave program output empty.

## CLI and Rust hosts

Before the file path, `run` and `check` accept `--max-steps <count>` and
`--max-prints <count>` once each, in either order. Counts are unsigned decimal
digits; zero is allowed. Invalid, duplicate, or overflowing options are usage
errors with exit status 2. `tokens` does not accept execution limits.

```sh
cargo run -- run --max-steps 2000000 --max-prints 10 examples/loops.srp
cargo run -- check --max-steps 2000000 examples/loops.srp
```

The explicit filename separator still works after options. Library hosts use
`ExecutionLimits { max_steps, max_prints }` with `run_with_limits`,
`check_with_limits`, `interpreter::execute_with_limits`, and
`interpreter::validate_with_limits`. Existing entry points use defaults.
Standalone expression evaluation remains bounded by its finite syntax and does
not apply program defaults; it has no looping statement access.

## Validation

Tests cover reference-model sums, nearest-loop control, per-iteration locals,
static checking of unreachable bodies, source spans, 64/65 nesting, 100,000
iterations with two fixed slots, deterministic infinite-loop termination,
exact budget boundaries, concurrent/repeated calls, CLI options, print caps,
and empty output on failure. Mutation checks use a small budget so malformed
updates cannot cause a test to hang. Native packages exercise the loop example.
