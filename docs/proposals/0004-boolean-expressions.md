# Boolean expressions and comparisons

Status: implemented on main for v0.0.2, following the variable, string, and
documentation work. The v0.0.1 snapshot remains unchanged.

## Motivation

Variables need a way to describe decisions. This slice adds a third primitive,
checked comparisons, and readable logical operators. Conditional statements are
a separate dependent feature. Native compilation remains future work.

## Syntax and precedence

`true` and `false` are boolean literals. They print with those lowercase spellings.
Booleans support declarations, same-type mutable assignment, copies, and scopes.
The new words `true`, `false`, `and`, `or`, and `not` are reserved binding names.

From strongest to weakest: grouping, unary minus, multiplication/division,
addition/subtraction, one comparison, `not`, `and`, `or`. `not` therefore negates
a complete comparison: `not score < 40` means `not (score < 40)`. Repeated `not`
is allowed. Use parentheses when it appears as a comparison operand, for
example `ready == (not blocked)`.

`==` and `!=` compare two values of the same primitive kind. String equality
compares decoded contents without Unicode normalization. There is no integer to
boolean conversion. `<`, `<=`, `>`, and `>=` require integers. All comparisons
produce booleans. They never subtract operands, so comparing integer extremes
cannot overflow. Arithmetic within an operand remains checked.

Chained comparisons are rejected: write `low < x and x < high`, rather than
`low < x < high`. Explicitly grouped boolean equality is allowed. `=` remains
assignment; equality requires `==`. Compound assignments remain arithmetic-only.

## Static checks and short-circuit execution

`and`, `or`, and `not` require booleans and produce booleans. They never return
an integer or string operand as a truthy value. `and` evaluates the right operand
only if the left is true. `or` evaluates it only if the left is false.

Compilation resolves names and checks primitive types in every operand,
including skipped operands. `false and missing` is an unknown-name error;
`true or 1` is a type error. In contrast, `false and 1 / 0 == 0` is valid and
evaluates to false because the right-hand arithmetic is never executed.
`check` follows exactly the same short-circuit rules as `run`.

## Implementation and limits

Expressions retain a flat postorder syntax representation with private indices.
An explicit task stack traverses only the nodes needed at runtime. Evaluation,
parsing of flat logical chains, and destruction do not recurse with chain length.
The existing 64-level grouping limit remains unchanged.

`Value` and `ValueType` gain `Boolean`, and `Value::as_boolean` inspects it.
`ExpressionKind` gains boolean, logical, and negation nodes; `BinaryOperator`
gains comparison variants. Rust hosts matching these enums must handle the new
variants. The integer-only `evaluator::evaluate` still rejects noninteger results;
`evaluate_value` supports all primitives and statically checks the entire
standalone expression before evaluating it.

Invalid operands use the existing `E0015` code and highlight the responsible
operator. Chained comparisons use `E0004` with explicit composition guidance.
Binding declarations using reserved boolean words use `E0012`.

## Validation

Focused tests cover all six comparisons, boundary integers, Unicode string
equality, logical precedence, skipped arithmetic, static errors in skipped
operands, value snapshots, fixed binding types, public AST misuse, and concurrent
reuse. Generated comparisons are checked against an independent integer model.
20,000-term logical and negation chains exercise iterative traversal. Source and
public-token mutation corpora include the new syntax. The shipped boolean
example has exact expected output and is exercised in native packages too.
