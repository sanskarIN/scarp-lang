# Conditional statements

Status: implemented on main for v0.0.2, following the boolean-expression work.

## Readable decisions

```ebnf
conditional = "if", expression, block,
              { "elif", expression, block }, [ "else", block ] ;
```

Conditions require booleans. There is no integer/string truthiness. Braces remain
mandatory, including for a single statement or an empty body. Newlines are
whitespace and there is no semicolon after a block. Use `elif`, rather than
`else if`, for another condition. `if`, `elif`, and `else` are reserved names.
Conditionals are statements, not expression values.

Conditions are evaluated in source order until the first true result. Only that
body executes. Later conditions are not evaluated. If every condition is false,
the optional else body executes. Without else, execution continues afterward.

## Static checks and scope

All conditions and all bodies are parsed, resolved, and type-checked before
execution. Unknown names, invalid operations, duplicate declarations, and
immutable writes remain errors even in an unselected body. Arithmetic is checked
only along the selected execution path. A skipped `1 / 0` therefore does not
fail execution, while a skipped unknown name or type mismatch fails compilation.

Each body has its own lexical scope. Branch-local names cannot escape their
body or feed another condition/body. Different bodies may reuse a local name
with different primitive types. Initializers can read visible outer bindings.
Assignments to an outer mutable binding persist after the selected body ends.

Source errors and arithmetic errors on the selected path still prevent all
program output. Print values are captured before output, including prints made
before a later selected failure. The Rust library's `check` API selects the same
path without writing; CLI `verify` only performs static checks.

## Flat execution plan

The parser stores one vector of conditional branches, so long `elif` chains do
not become recursively nested syntax. Each brace body counts toward the same
64-level block limit; sibling `elif` bodies do not add nesting levels.
The resolver also checks this limit for public, manually constructed syntax.

Resolution lowers the branches to private forward jumps with absolute targets.
Variable slots remain unique and immutable. Execution uses a program counter,
with no runtime scope stack or recursive statement traversal. There are no loops
or backward jumps in this feature.

`Statement` gains `If` and the public `ConditionalBranch` structure describes
conditions, bodies, and body spans. Hosts matching statement variants must add
this case. `statement_count` includes conditional guards and all body operations,
but excludes block containers and unconditional bookkeeping jumps.
`binding_count` includes declarations from unselected bodies too.

## Validation

Tests cover first-match selection, optional/empty bodies, nested branches,
state changes, lexical scopes, static errors in skipped paths, skipped arithmetic,
atomic output, original spans, malformed syntax, public AST construction,
concurrent reuse, generated branch outcomes, 64/65-level nesting, and 20,000
`elif` clauses. Source/token mutation tests and the packaged decisions example
exercise the feature through the existing process boundary.
