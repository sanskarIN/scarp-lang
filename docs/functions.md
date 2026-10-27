# Functions with funcra

This unreleased feature adds file-level named functions to the interpreter. The
declaration keyword is exactly `funcra`; calls use the function name. Parameter
and result types are mandatory, while local bindings keep inferred types.

```scarp
funcra square(value: integer) -> integer {
    return value * value;
}

bind answer = square(6);
print(answer);
```

Output is `36` followed by LF. Existing `bind`/`var`, checked arithmetic,
short-circuit booleans, numerical kernels, and all five output operations work
inside functions. See [the executable examples](../examples/functions.srp).

## Types, names, and return paths

Parameter types are `integer`, `float`, `boolean`, `string`, and `vector`. A vector
is the existing immutable finite-float vector, without element type parameters.
No implicit integer/float conversions occur. Results may also be `unit`:

```scarp
funcra greet(name: string) -> unit {
    print("Hello", name);
    return;
}
greet("Scarp");
```

Unit functions may fall through or use a bare return. They cannot return a value.
A unit call is legal only as a standalone statement, never an initializer,
argument, output value, or operand. A call returning a value may also stand alone;
its result is discarded. Non-unit functions must return the declared type on
every path. Cover all conditional branches or add a final return. A loop alone
never establishes a definite return, even with a literal `true` condition.
Return exits only the current invocation, including from nested blocks/loops.
Break/continue apply only to loops in the same function. Unreachable statements
and unused function bodies still undergo name and type checks.

Signatures are collected first, so forward calls, recursion, and mutual recursion
work. Duplicate definitions are errors with the first definition's location.
Function names cannot collide with ordinary bindings or parameters anywhere in
the file. Functions cannot implicitly read file-level variables. Parameters are
immutable values; strings/vectors may share immutable storage internally. A local
`var` can replace its own value without changing the caller's variable. Function
body locals occupy an inner scope and may deliberately shadow parameters.

An explicit function declaration takes precedence over a numerical built-in of
the same name throughout the file, without overload selection. For example,
`funcra sum(a: integer, b: integer) -> integer` replaces built-in vector `sum`
in that file. This reconciles the function examples with the existing numerical
library. Without a function declaration, legacy built-in resolution is unchanged:
`bind sum = sum([1.0, 2.0]);` remains valid. Output/control words cannot be
overridden. `funcra`, `pure`, `return`, `needs`, and `ensures` are newly reserved
binding names. Type words remain contextual, so `bind integer = 1;` is legal.

## Named arguments and defaults

```scarp
funcra endpoint(start: integer, length: integer = 5) -> integer {
    return start + length;
}
print(endpoint(7), endpoint(length: 2, start: 10));
```

Output is `12 12` followed by LF. Positional arguments precede named arguments.
Named arguments may appear in any parameter order, but each parameter can be
supplied once. Unknown labels, duplicate values, missing required values, and
wrong types are static errors. Trailing commas are accepted in parameter lists
and calls. Semicolons are still mandatory after return and call statements.

Parameters with defaults form a trailing suffix. A default sees only earlier
parameters and supported pure calls; it cannot access a later parameter, its own
parameter, a caller local, or a file-level variable. A default is checked even
when every call supplies that argument. Each invocation proceeds in this order:

1. Evaluate explicit arguments exactly once, left to right in source order.
2. Bind their results by parameter identity in a fresh frame.
3. Evaluate omitted defaults once, in declaration order.
4. Check preconditions, execute the body, and check postconditions.
5. Resume the caller with the result, or propagate a diagnostic.

Providing an argument skips its default at runtime. Defaults are not cached.
There are no user-defined variadic arguments, overloads, or generic functions.

## Purity and executable contracts

```scarp
pure funcra clipped(x: integer, lo: integer = 0, hi: integer = 10) -> integer
    needs lo <= hi;
    ensures result >= lo and result <= hi;
{
    if x < lo { return lo; }
    if x > hi { return hi; }
    return x;
}
print(clipped(x: 15));
```

Output is `10` followed by LF. An explicit `pure` declaration is checked:
the function cannot output or call a user function without `pure`. Existing
numerical built-ins are pure. Local `var` mutation is allowed because frame
storage cannot escape or be observed by another invocation. Recursive pure calls
are allowed. An unmarked function remains impure even if its body happens to
have no effects. Purity does not promise termination or freedom from errors.

Multiple `needs` and `ensures` clauses are boolean, statically checked, and pure.
Preconditions see parameters; postconditions additionally see read-only `result`
for a non-unit result. Body locals are never in contract scope. Parameters cannot
be named `result`; ordinary body/file bindings can. Unit contracts cannot refer
to `result`. The selected preconditions and postconditions each execute in source
order and stop at the first failure. These are runtime checks, not proofs.

Failures use E0033 at the clause and include a bounded function name plus at most
16 inner-to-outer call locations. Argument values are not dumped. Arithmetic and
other runtime failures retain their existing codes with the same call locations.
Calls inside defaults and contracts obey the ordinary global budgets.

## Frames, budgets, and output

Call-limit diagnostics identify the exhausted resource and configured value:
`max_call_depth` bounds active function frames, `max_calls` bounds cumulative
user calls, and `max_live_slots` bounds live variable storage. Their repair help
names the corresponding `ExecutionOptions` field. The limits and boundary
behavior are unchanged; a failure still discards both pending output streams.

When a default value has the wrong type, the primary location is the default
expression and a related location identifies its parameter declaration. Unused
functions receive the same static checks. Incomplete local bind/var statements
and assignments also receive statement-specific repair help.

The interpreter suspends expression tasks and uses an explicit heap frame stack.
Calls do not recurse through Rust. Each invocation has independent slots and a
program counter. Reusing a compiled program creates fresh execution storage.
This removes host-stack recursion as the depth limit; it does not make recursion
free or make the interpreter comparable to optimized native compilers.

| Default resource limit | Value |
| --- | --- |
| Logical work steps | 1,000,000 |
| Selected output statements on both streams | 100,000 |
| Active user function frames, excluding the file frame | 256 |
| Total selected user calls, including defaults/contracts | 100,000 |
| Live binding slots across file and function frames | 1,000,000 |
| Cumulative output snapshot storage estimate | 16 MiB |

The call limit counts user functions; numerical kernels retain their existing
element/work accounting. A call charges expression tasks plus one entry step
and one step per allocated slot. Slots include parameters, locals on all paths,
and one non-unit result slot. Omitted defaults and selected contracts each add
one step before their normal expression work. Return/call statements consume an
instruction step. Programs without calls retain previous work/print accounting.

Snapshot storage charges payload size plus 64 bytes per value/event; vector
payloads use eight bytes per element and scalars eight bytes. Separators and
empty line events are counted. Shared payloads count for each snapshot. This is
a conservative logical cap, not an exact process-memory or encoded-output-byte
measurement. Pending outer output arguments count before a nested call returns.
Runtime validation applies the same cap without retaining or formatting output.

All explicit arguments of an output statement finish before that statement's
own events are appended. Output produced by nested calls therefore precedes
the caller's line. Static, arithmetic, contract, and budget failures discard
both streams. Replay preserves stream order and boundary flushes; writer failures
can still leave earlier bytes. Single-writer APIs reject required stderr before
writing stdout. See [output semantics](output.md).

`ExecutionLimits` keeps its existing two-field shape. New `ExecutionOptions`
combines it with the additional limits. Old APIs use the additional defaults;
`run_with_options`, `run_with_io_and_options`, `check_with_options`, and matching
interpreter APIs accept caller-selected limits. See [embedding](embedding.md).

## Support and next steps

| Capability | Interpreter | Native C/executable/object |
| --- | --- | --- |
| Typed named functions, recursion, unit, returns | Implemented and tested | E0035 before emission |
| Positional/named/default arguments | Implemented and tested | E0035 before emission |
| Purity, needs/ensures, bounded traces | Implemented and tested | E0035 before emission |
| Read-only library signature/call-graph metadata | Implemented and tested | Not a runtime feature |
| Function values/types, higher-order calls, closures/captures | Explicitly unsupported | Unsupported |
| Per-function resource annotations | Not implemented | Not implemented |

Native compilation rejects even unused function declarations. It never silently
interprets or emits a partial C program. Existing integer/boolean native programs
keep their ABI and their step/print limits; the interpreter-only snapshot limit
does not apply to the native backend's two-pass, queue-free output mechanism.

The highest-risk follow-up is native frame/call/contract parity, followed by typed
callable values and effect compatibility, then immutable closure captures and
allocation/lifetime limits. These require complete vertical implementations and
tests before their syntax can be accepted. See [the design](proposals/0015-funcra-functions.md).
