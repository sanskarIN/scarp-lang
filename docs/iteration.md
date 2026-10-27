# Iteration and safe control flow

v0.0.3 adds counted `repeat` alongside the integrated `while` implementation.
Published v0.0.2 does not have either loop. Keep the release's existing `bind`,
`var`, `print`, and `run/check/tokens` spellings.

## A counted loop

```scarp
var total = 0;
repeat 5 as index {
    total += index;
}
print(total);
```

Output:

```text
10
```

The integer count is evaluated exactly once before entering the body. The index
runs from zero through count minus one. Changing a variable used by the count
does not change that saved count. Zero runs no iterations; negative counts fail
with E0019. Booleans and strings are static type errors. Floats are not part of
this release. The body is always checked for names/types, even when count is zero.

The loop owns an immutable integer index and a lexical body scope. The index can
shadow an outer name; the count still reads the outer scope. It cannot escape the
loop or be assigned to. Use another `var` variable for a local accumulator.
A same-scope declaration cannot duplicate the index, while an explicit inner
block can shadow it. Body declarations initialize on each reached iteration.

Only `repeat expression as identifier { statements }` is accepted. `as` and the
braces are mandatory. There is no trailing semicolon or alternative counted-loop
keyword. Both repeat/as stay contextual identifier tokens, preserving variables
such as `var repeat = 2; repeat += 1;` and `bind as = 3;`.

## While and nearest-loop control

```scarp
var index = 0;
while index < 5 {
    index += 1;
    if index == 2 { continue; }
    if index == 4 { break; }
    print(index);
}
```

Output:

```text
1
3
```

While reevaluates a boolean guard before every iteration. There is no implicit
truthiness. `break;` exits the nearest enclosing loop. `continue;` reevaluates a
while guard or advances a repeat index, including inside nested conditionals and
blocks. Mixed/nested loops keep their own targets. Both statements require a
semicolon and an enclosing loop; an unreachable use outside a loop still fails.

Existing mutable outer state persists across iterations. Loop-local bindings
cannot escape their scope. All arithmetic remains checked. A later runtime or
budget failure discards every pending print, including output before the loop.
An I/O failure during successful replay can leave bytes already written.

## Bounds and implementation

Loops use a flat instruction plan and fixed storage proportional to source
declarations. Repeat saves count/index in two integer slots; it never builds an
array of indices. Even i64::MAX counts stop under the execution budget rather
than allocating a range or overflowing the index. Compiled programs are reusable
with independent state and budgets on every execution.

Expression and block nesting limits remain 64, independent of iteration count.
New syntax adds `Statement::Repeat` to the public AST. Binding metadata includes
its two slots. Existing Rust API signatures are retained.

Run the exact-output examples with:

```sh
scarp run examples/loops.srp
scarp run examples/repeat.srp
scarp check examples/repeat.srp
```

See [budgets](budgets.md) for limits, [diagnostics](reference/diagnostics.md) for
repairs, and [the design](proposals/0007-counted-repeat.md) for lowering details.
