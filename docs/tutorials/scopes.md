# Scopes and shadowing

This development-only tutorial explains which declaration a name refers to,
what survives a block, and how a shadowing initializer behaves. A block contains
statements between `{` and `}` and does not need a trailing semicolon.

## Introduce a local name

```scarp
bind value = 2;
{
    bind value = value + 3;
    print(value);
}
print(value);
```

Output:

```text
5
2
```

The initializer of the inner `value` reads the outer declaration. The inner
declaration becomes visible only after that initializer. On leaving the block,
the outer declaration is visible again.

Duplicate declarations in the same scope are rejected. Shadowing is therefore
an explicit nested-scope choice rather than silent replacement of a declaration.

## Assign to an outer mutable binding

```scarp
var count = 1;
{
    count += 2;
    bind local = count * 10;
    print(local);
}
print(count);
```

Output:

```text
30
3
```

The assignment finds the nearest visible `count`. Because that declaration is
outside the block, its changed value is still visible afterward. Scope exit
restores name visibility; it does not undo writes to an outer variable.

`local` is invisible after the block. Reading it afterward fails with `E0010`:

```scarp-invalid
{
    bind local = 30;
}
print(local);
```

Sibling blocks cannot see each other's local declarations either.

## The nearest declaration controls mutability

An immutable inner declaration hides a mutable outer one. This fails with
`E0013`, and the related location identifies the inner declaration:

```scarp-invalid
var count = 1;
{
    bind count = 2;
    count += 1;
}
```

Resolution does not skip the inner declaration in search of another mutable
binding. Rename the local, make that declaration mutable, or assign before the
shadowing declaration, according to the behavior you intend.

## Shadow with a different primitive type

In the string extension, separate declarations can have different types:

```scarp
bind label = "outside";
{
    bind label = 7;
    print(label);
}
print(label);
```

Output:

```text
7
outside
```

This differs from assigning a new type to an existing mutable binding, which
is rejected. Each declaration receives a distinct storage slot.

Blocks are statements, not values: they cannot be used as initializers or print
arguments. Block nesting is limited to 64 levels, independently of the
64-level expression-parenthesis limit. See [diagnostics](../reference/diagnostics.md)
for missing braces and excessive nesting.
