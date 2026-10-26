# Design 0003: string-valued bindings

**Status: implemented on main for v0.0.2.** This extension builds on the
integer-variable rules and is included in the variable prerelease.
Examples use development `bind`/`var`; published v0.0.2 uses `let`/`let mut`.
See the [syntax migration decision](0013-binding-syntax.md).

Bindings infer either `integer` or `string` from their initializer. Mutable
assignment must preserve that type. Inner scopes may shadow a binding with a
different type because each declaration has a distinct slot.

```scarp
var greeting = "Hello";
bind original = greeting;
println(greeting);
greeting = "Hello from Scarp!";
println(original); // Hello
println(greeting); // Hello from Scarp!
```

Strings are immutable values. Copying a binding or capturing print output may
share immutable storage, but assigning another string replaces the value and
does not alter earlier copies or pending output. Escapes and Unicode preserve
their existing meanings. Empty and parenthesized string values are valid.

Arithmetic, negation, and compound assignment remain integer-only. No implicit
coercion, string concatenation, indexing, interpolation, or string mutation is
introduced. A primitive type mismatch is a compile-time diagnostic at the
responsible expression/operator; assignment errors also identify the declaration.

Name resolution and primitive type inference complete before arithmetic is
evaluated. The executable plan is immutable and retains its slot types. Runtime
evaluation also checks operand types for standalone library expressions. Existing
integer programs keep their checked arithmetic behavior.

This is a small primitive-value check, not a general type system with user types,
generics, inference across functions, or annotations. Those remain later work.
