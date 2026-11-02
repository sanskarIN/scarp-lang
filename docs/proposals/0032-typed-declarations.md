# Proposal 0032 - Optional declaration type annotations

Status: unreleased source development, Rust 2021 / 1.74. No version or release change.

Extend `bind name = expression;` and `var name = expression;` with optional
`: integer`, `: float`, `: boolean`, `: string`, `: vector` or `: tile` before `=`.
The annotation must equal the initializer's inferred type; it never performs a
conversion. Initializers remain mandatory. Resolve a shadowing initializer in the
outer scope before inserting the new binding. Assignment and retirement rules
remain unchanged. Functions/defaults and module scalar constants follow the same
type rules; vector/tile module constants remain unsupported.

Parse into `Statement::Declaration.annotation: Option<TypeAnnotation>` retaining
the type token's source span. This is an intentional additive AST schema change:
hosts constructing Declaration literals must supply `annotation: None`, and
exhaustive destructuring should add the field or `..`. Valid existing source and
compile API signatures are preserved. Shared primitive type-name lookup avoids
different accepted types in parameters and annotations. Annotation compatibility
uses one resolver helper in both runtime and compile-time constant paths; constants
are checked before arithmetic evaluation. E0015 points to the initializer with a
related annotation site; unknown/unit binding types use E0004 and repair help.

Tests cover all six kinds, malformed syntax, unit/unknown types, exact spans,
Unicode/CRLF, nested shadowing, function parameters/returns, imports/constants,
no implicit conversion, mandatory initialization, assignment preservation and
existing debug/release/CLI/module behavior. Add a runnable annotated numerical
example and document the AST migration. No uninitialized values, type aliases,
coercions, structural types or deferred-initialization analysis are introduced.
