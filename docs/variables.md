# Variables in the development language

This document describes implemented variable behavior on the v0.0.2 development
branches. The published v0.0.1 interpreter does not support declarations.

## Immutable integer bindings

```scarp
let width = 6;
let height = 7;
let area = width * height;
println(area); // 42
```

Declarations require `let`, an ASCII identifier, `=`, an integer expression, and
`;`. Names are case-sensitive. `_` is an ordinary name. `let`, `mut`, and `println`
are reserved and cannot be used as variables. Strings remain direct print
arguments; string variables are not implemented.

A declaration becomes visible after its initializer. Initializers and print
expressions can reference earlier bindings. Forward and self-references are
errors. Duplicate declarations in the same scope are errors. There are no
implicit declarations or uninitialized bindings.

Compilation now includes name resolution after lexing and parsing. Every name
reference is assigned a storage slot. Evaluation uses those slots and starts
with fresh storage on every execution, including repeated execution of the same
compiled program. Variables do not persist between files or library calls.

The whole file is resolved before arithmetic evaluation. A later unknown name
can therefore be reported before an earlier arithmetic error. Declarations and
print arguments are evaluated in source order before any output is written.
Unused initializers are still evaluated and can fail. Existing checked integer
arithmetic and expression nesting rules are unchanged.

## Diagnostics

| Code | Meaning |
| --- | --- |
| `E0010` | Unknown name, including forward or self-reference |
| `E0011` | Duplicate declaration in the same scope |
| `E0012` | Reserved name used as a variable |
| `E0013` | Assignment to an immutable binding |
| `E0014` | Block nesting limit exceeded |

Duplicate-name diagnostics include the original declaration as a related source
location. Reference and declaration spans refer to the original UTF-8 source.

## Library changes

`scarp::parse` returns a syntax `Program` without resolving names.
`resolver::resolve` consumes syntax and returns an immutable `ResolvedProgram`.
`scarp::compile` combines those steps. `interpreter::execute` accepts the resolved
program; `run` still accepts source and an explicit writer. `binding_count` and
`statement_count` expose executable metadata without exposing mutable slot plans.

`evaluator::evaluate` remains useful for standalone literal expressions. Without
a program's binding context, a name reference returns `E0010`. Use `compile` and
`execute` for expressions that depend on declarations.

These are development API changes from v0.0.1. See
[the variable design](proposals/0002-variables.md) for the staged scope.

## Mutable bindings and assignment

Bindings are immutable unless declared with `let mut`:

```scarp
let mut count = 2;
let original = count;
count += 5;
println(count);    // 7
println(original); // 2
```

Simple assignment uses `name = expression;`. Compound assignment supports `+=`,
`-=`, `*=`, and `/=`. The right-hand expression is evaluated against the previous
environment, and its result is combined with the target's previous value using
the same checked arithmetic as ordinary operators. Assignments require a
previous declaration and are statements, not expressions.

Initializers copy integer values; bindings do not alias one another. Print
values are captured when their statement is evaluated, so a later assignment
does not change an earlier pending print. Repeated and concurrent executions of
one resolved program still use separate storage.

Writing an immutable binding produces `E0013` and points to its declaration.
Compound overflow and division errors highlight the entire compound operator.
An arithmetic failure in the right-hand expression highlights that expression's
operator instead. Any such failure prevents all program output.

There are no implicit declarations, uninitialized bindings, increment/decrement
operators, assignment expressions, or string-valued variables.

## Lexical scopes and shadowing

```scarp
let mut total = 1;
{
    let total = total + 10;
    println(total); // 11
}
total += 2;
println(total); // 3
```

Braces introduce a statement scope and do not require a trailing semicolon.
Declarations can shadow outer bindings, but duplicate declarations in the same
scope are rejected. A shadowing initializer reads the previous outer value
before the new name becomes visible. Without an outer declaration, a reference
to the name being initialized is an unknown-name error.

Assignment searches the nearest visible binding. An immutable inner binding
hides a mutable outer binding and prevents assignment; resolution never skips
an immutable declaration to find a mutable one. Assignment to an outer mutable
binding persists after leaving the block. Inner declarations become invisible
on exit, and sibling blocks cannot see each other's locals.

Blocks are statements, not expression values. Empty blocks are valid. Block
nesting is limited to 64 levels independently of the 64-level expression limit.
Both the parser and resolver enforce the block limit. Runtime operations are
flattened, so executing a block does not recurse.

Every declaration receives a unique storage slot, including shadowed names in
different blocks. Slots are retained for the duration of execution and are not
reused on block exit. `binding_count` includes those slots; `statement_count`
counts executable declarations, assignments, and prints, excluding block
containers. Future storage optimization must preserve these semantics.
