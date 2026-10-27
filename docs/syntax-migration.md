# Declaration syntax migration

Development main uses `bind` for immutable bindings and `var` for mutable
variables. Published v0.0.2 and v0.0.3-beta.1 binaries retain their original
`let` and `let mut` syntax. Check `scarp --version` before copying examples.

| Published source | Current main |
| --- | --- |
| `let size = 6;` | `bind size = 6;` |
| `let mut count = 0;` | `var count = 0;` |

```scarp
bind size = 6;
var count = 0;
repeat size as index {
    count += index;
}
println(count);
```

Both forms require an initializer and a semicolon. Scope, shadowing, checked
arithmetic, type consistency, and execution budgets retain their existing rules.
Assignment to a `bind` binding or a repeat index remains an error.

Old declaration forms report E0005 with replacement help before any output.
Replace declaration tokens only: text inside strings and comments is unchanged.
`bind`, `var`, `let`, and `mut` are reserved names; longer identifiers such as
`letter` remain ordinary names. The lexer still reports keywords as identifiers.

Rust hosts inspecting syntax must replace `Statement::Let` matches and
constructors with `Statement::Declaration`. Its fields retain the same meaning.
The resolved-program and execution entry points keep their existing signatures.

The old example sources and expected bytes are preserved under
`tests/fixtures/releases/v0.0.2/`. Tests check their published hashes and run a
token-based declaration migration against their original output snapshots.
Current `examples/` files intentionally use the new syntax. Existing tags and
published packages remain immutable. This development change creates no release.
