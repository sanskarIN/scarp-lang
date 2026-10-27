# Design 0002: variables

Historical design record: current declaration and output spellings are documented
in the [syntax migration](../syntax-migration.md) and [output guide](../output.md).

**Status: implemented on main for the v0.0.2 variable milestone.** The maintainer
authorized integration of the reviewed variable stack on 2026-10-26.

This design describes the integer foundation. A separate
[string-value extension](0003-string-bindings.md) generalizes initializers and
introduces primitive type checks without changing the scope and mutation rules.

## Milestones

1. Immutable integer bindings and a distinct name-resolution phase.
2. Explicit mutability, assignment, and checked compound assignment.
3. Lexical block scopes with deliberate shadowing rules.
4. A validation command and v0.0.2 package metadata.

## Binding rules

```scarp
let width = 6;
let area = width * 7;
println(area);
```

An initializer is required and must be an integer expression. Bindings use the
existing ASCII identifier grammar, are case-sensitive, and are visible only
after their declaration. Forward references and self-reference without an outer
binding are errors. `let`, `mut`, and `println` are reserved names. `_` is an
ordinary identifier, not a discard pattern.

The compiler parses the entire file, resolves names in source order, and returns
an immutable executable program. Runtime storage uses resolved slot indices,
not repeated string lookup. Every execution starts with fresh storage. Public
syntax inspection and executable programs are separate APIs.

## Mutability and assignment

The second slice introduces `let mut count = 1;`, `count = count + 1;`, and
`+=`, `-=`, `*=`, `/=`. Only explicitly mutable bindings can be assigned.
Assignments are statements and do not produce expression values. Compound
assignment reads the previous value and applies the same checked integer
arithmetic as an ordinary binary expression. String assignment, implicit
declarations, destructuring, increment/decrement, and uninitialized bindings
are outside this design.

## Scope

The third slice introduces `{ ... }` statement blocks. Duplicate declarations
within one scope are errors; an inner scope may shadow an outer binding. A
shadowing initializer reads the previous outer binding before the new binding
becomes visible. Leaving the block restores visibility of the outer binding.
Assignment searches the nearest visible binding and obeys that binding's
mutability. Inner declarations never leak out of their block.

Parser block nesting is bounded independently of expression nesting. Scope
resolution may use bounded recursion; runtime execution stays flat and does
not recurse through blocks. Slot IDs are never reused within one compiled
program, preventing inner variables from aliasing surviving outer variables.

## Errors and phase ordering

Lexical and syntax validation happen before name resolution. All name resolution
happens before evaluation, so a later unknown name can be reported before an
earlier arithmetic failure. Declarations, assignments, and print arguments are
then evaluated in source order before any output is written. Even an unused
initializer must be valid. Arithmetic and semantic failures produce no program
output; output I/O failures can still occur after completed writes.

Diagnostics retain the reference or declaration span. Duplicate declarations
and writes to immutable bindings also identify the original declaration.
The first diagnostic is reported; recovery and warnings for unused bindings
are deferred.

## Acceptance criteria

- Existing v0.0.1 programs keep their outputs and arithmetic behavior.
- Unknown, forward, self-referential, duplicate, and reserved names are diagnosed.
- Debug and release builds agree on overflow and division semantics.
- Repeated execution of one compiled program never shares state.
- Long binding chains and flat statement lists avoid recursive execution.
- Scoped shadowing, nearest-binding assignment, and scope exit have explicit tests.
- The check command resolves and evaluates without writing program output.
- No changes are merged into `main`, and no release or tag is created, without
  a separate maintainer instruction.
