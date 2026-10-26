# Explicit binding and variable declarations

Status: implemented for review on `scarp/bind-var-syntax`, unreleased v0.0.3-dev.
Base: `scarp/canonical-cli` at `984c59e1cc1c40626949633f8ac165fd2ae5ca1d`.
The application-project checkpoint is preserved independently on `development`
at `05b4c40`; project scaffolding is outside this syntax change.

## Decision and alternatives

Use `bind` for an immutable binding and `var` for a mutable variable. The two
words communicate different capabilities without an optional modifier:

```scarp
bind width = 6;
var area = width * 7;
area += 1;
println(area); // 43
```

| Alternative | Trade-off |
| --- | --- |
| `let` / `let mut` | Familiar, but repeats the Rust declaration surface that this project wants to move away from. |
| `val` / `var` | Compact, but matches the declaration pair used by Kotlin and Scala. |
| `const` / `var` | Suggests compile-time constants, whereas Scarp evaluates reachable initializers at runtime. |
| `bind` / `var` | Clearly distinguishes a fixed binding from an assignable variable; costs one extra character for immutable declarations. |

These comparisons concern syntax, not identical semantics. See the official
[Rust variables guide](https://doc.rust-lang.org/book/ch03-01-variables-and-mutability.html),
[Kotlin basic syntax](https://kotlinlang.org/docs/basic-syntax.html#variables), and
[Scala basics](https://docs.scala-lang.org/tour/basics.html#values).
Neither keyword is claimed to be unique to Scarp. Standard `if`, `while`, braces,
semicolons, operators, `println`, `and`, `or`, `not`, and `elif` remain unchanged.

## Grammar and semantics

```ebnf
declaration = ("bind" | "var"), IDENTIFIER, "=", expression, ";" ;
```

An initializer is required. `bind` is always immutable; `var` permits assignment
and existing compound operations subject to the inferred type. There are no
`bind mut`, `var mut`, `const`, `val`, or implicit-declaration alternatives.
`bind`, `var`, `let`, and `mut` are reserved variable names. This breaks programs
that previously used `bind` or `var` as names. `binder` and `variable` are ordinary
identifiers. Strings and line comments retain their contents.

Scope, duplicate checks, outer-name access in shadowing initializers, type
inference, snapshot values, checked arithmetic, unused initializer evaluation,
execution budgets, fresh execution storage, and output prevalidation do not
change. The lexer still emits generic identifier tokens. The shared parser and
resolver handle interpreter, static verification, AST inspection, and C/native
compilation; backend value restrictions remain unchanged.

## Diagnostics and migration

| Input | Result |
| --- | --- |
| `let x = 1;` | E0005: unsupported statement, help to replace `let` with `bind`. |
| `let mut x = 1;` | E0005: unsupported statement, help to replace `let mut` with `var`. |
| `bind mut x = 1;` | E0004: malformed declaration, help to use `var x = ...;`. |
| `var mut x = 1;` | E0004: malformed declaration, help to remove `mut`. |
| `var x;` | E0004: required initializer is absent. |
| `bind x = 1; x = 2;` | E0013 with the declaration location and advice to use `var`. |
| `bind var = 1;` | E0012: reserved variable name. |

E0005 retains its unknown/unsupported-statement meaning, and E0004 retains its
unexpected-token meaning. No new diagnostic code is needed. Migration spans
cover the old declaration keyword, including `mut` where present; all spans
remain byte offsets into the original UTF-8 source, including CRLF trivia.

Published v0.0.2 continues to use `let` and `let mut`. Its tags, packages, and
release notes remain unchanged. For this development branch, replace declaration
`let` with `bind` and declaration `let mut` with `var`; assignments are unchanged.
There are no compatibility aliases. Do not use a global text replacement: quoted
text, comments, and the native Rust implementation can contain those words.
Review token-aware edits or migrate declarations manually.

The Rust embedding AST variant becomes `Statement::Declaration` (formerly
`Statement::Let`); its fields and meaning are unchanged. Consumers matching or
constructing that public enum variant must update. Rust library `run`, `check`,
and `compile` APIs retain their names. CLI names stay `exec`, `compile`, `verify`,
`inspect`, `guide`, and `version`.

## Design consistency and review gates

Future syntax should be readable, orthogonal, unambiguous, statically checkable,
documented, and accompanied by helpful diagnostics. Avoid unnecessary resemblance
where a clear alternative exists, while prioritizing semantics over cosmetic
renaming. Require positive and negative examples, scope/type rules, migration
policy, and interpreter/backend coverage before adopting another syntax change.

Review requires the full debug/release suite, formatting, Clippy, rustdoc,
examples, documentation checks, native differential tests, and extracted-package
smoke checks. Hosted CI must cover Windows, Linux, macOS, minimum Rust, and C
sanitizers. Merge dependent PRs only in order after separate authorization.
Functions, formatter, modules, richer backend types, registry, and additional
systems capabilities belong in later proposals. No release is authorized here.
