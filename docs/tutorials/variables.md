# Variables and mutation

This tutorial requires the development stack. Published v0.0.1 does not support
declarations. It shows how to name values, choose mutability, and keep earlier
results when values change. The precise rules are in the
[variable reference](../variables.md).

## Name a result

```scarp
let columns = 8;
let rows = 5;
let cells = columns * rows;
println(cells);
```

Output:

```text
40
```

`let` introduces a binding after its initializer. `cells` can use `columns` and
`rows` because they are already declared. Names are case-sensitive ASCII
identifiers; `rows` and `Rows` are different names.

By default, a binding is immutable. A declaration requires an initializer and a
semicolon. There are no implicit declarations or uninitialized variables.

## Change a value explicitly

```scarp
let mut remaining = 10;
let starting = remaining;
remaining -= 3;
remaining = remaining * 2;
println(starting);
println(remaining);
```

Output:

```text
10
14
```

`let mut` permits assignment. The immutable `starting` keeps the value it copied;
it is not an alias for the storage of `remaining`. Compound assignment supports
`+=`, `-=`, `*=`, and `/=` and follows the checked integer rules in the
[value and operator reference](../reference/values.md).

This attempt to change an immutable binding fails with `E0013`:

```scarp-invalid
let remaining = 10;
remaining -= 3;
```

Use `let mut` only when later assignment is needed. The diagnostic also points
to the declaration that established immutability.

## Bind and replace strings

The string extension infers a binding's primitive type from its initializer:

```scarp
let mut label = "Draft";
let original = label;
println(label);
label = "Ready for review";
println(original);
println(label);
```

Output:

```text
Draft
Draft
Ready for review
```

Reassignment replaces a string value without changing copies or earlier pending
prints. A mutable binding must retain its inferred type. Changing `label` to an
integer fails with `E0015`; string concatenation and string compound assignment
are also unsupported.

## Declare before use

This forward reference fails with `E0010`:

```scarp-invalid
println(answer);
let answer = 42;
```

Move the declaration before the print. Self-reference in an initializer also
fails unless an outer binding with that name is already visible.

Use `scarp verify variables.srp` to check syntax, names, mutability, primitive
types without evaluating arithmetic or printing. Every execution starts with fresh
variable storage, including repeated executions of one compiled library program.

Next, use [scopes and shadowing](scopes.md) to reuse a name within a nested block.
