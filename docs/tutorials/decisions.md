# Make decisions with if, elif, and else

This tutorial requires the development conditional branch. It builds on
[variables](variables.md) and [the boolean rules](../proposals/0004-boolean-expressions.md).
The published v0.0.1 interpreter does not support this syntax.

## Select the first matching case

```scarp
bind score = 73;
var grade = "pending";
if score >= 90 {
    grade = "A";
} elif score >= 60 {
    grade = "B";
} else {
    grade = "C";
}
print(grade);
```

Output:

```text
B
```

Conditions are explicit booleans. `score >= 90` compares integers and produces
a boolean. An integer such as `if score {}` is a type error. Use readable `and`,
`or`, and `not` to combine decisions. No parentheses are required around a
condition, but braces are required around every body.

Scarp checks conditions in order and runs the first matching body. Later
conditions and bodies are skipped. Use `elif` for another condition. `else` is
optional and runs only when no earlier condition matched. An `if` without `else`
simply continues after all false conditions. There is no semicolon after `}`.

## Keep local names local

```scarp
bind label = "outer";
if true {
    bind label = "selected";
    print(label);
}
print(label);
```

Output:

```text
selected
outer
```

Each body introduces a lexical scope. Its local declarations do not escape and
cannot be read in a sibling body or a later `elif` condition. Assigning to an
outer mutable variable, as with `grade`, persists after the body ends. Declaring
a local with the same name, as with `label`, creates a distinct binding.

## Separate static checks from selected work

```scarp
if false {
    print(1 / 0);
} else {
    print("safe");
}
```

Output:

```text
safe
```

The unselected arithmetic is not evaluated. Names and types are checked in every
body, however: replacing `1 / 0` with an unknown variable still fails compilation.
This catches misspelled variables even in code paths not selected by this run.

`scarp check examples/decisions.srp` validates the same selected path silently.
If selected arithmetic fails, earlier print statements produce no output. The
language continues to prepare the complete selected result before writing.
See [the conditional design](../proposals/0005-conditional-statements.md) for
precise branch ordering, limits, and Rust API changes.
