# Counted repeat and the v0.0.3 iteration scope

Base: main `62ee83a1f5b98a04a54a1e5349d0145a476a9958`, after PR #28 integrated
the exact source/test work of PR #16. Direct retargeting had five documentation
conflicts; the focused integration preserved published v0.0.2 notes and left all
existing feature branches/PRs unchanged. Baseline: 368 local test executions,
zero failures/ignored tests, all seven hosted jobs, and five release-verifier tests.
Rust 2021/MSRV 1.74, no registry dependencies, unsafe code forbidden.

This milestone retains let/let mut, println, run/check/tokens. It excludes the
later numerical, function, native backend, command/syntax redesign, and project
workflow stacks. Existing tags/assets and the separate function checkout remain
untouched. Only a new v0.0.3-beta.1 prerelease is intended after final gates.

## Grammar and lexical compatibility

`repeat integer_expression as identifier { statements }` is the sole counted
loop form. Both words remain identifier tokens. `as` remains a legal variable
name everywhere. `repeat` is contextual at statement start except when followed
by an assignment operator, preserving existing variables/assignments named
repeat. Prefixes and case variants remain ordinary identifiers.

Resolve and evaluate the count exactly once in the enclosing scope. Non-integer
counts use existing E0015. Negative counts use new E0019 (the next free code in
this release baseline) at the complete count expression; all older codes retain
their meanings. Zero counts skip execution but still check every body statement.

The body has one lexical scope containing an immutable integer index, from zero
through count minus one. It may shadow an outer name, including a name read by
the count. A declaration cannot duplicate that index in the same body scope;
an explicit inner block may shadow it under the existing scope rules. No index
escapes the body. Count and index storage cannot be mutated through user aliases.

## Lowering and budgets

Add one AST variant and three private flat operations: initialize saved count
and index, test index against count, advance index. Two integer slots per repeat
are allocated at compile time and reused on every entry. No range is materialized.
Break targets the nearest loop exit. Continue targets a repeat's advance operation
or a while's condition, using patched pending loop-control jumps.

Initialization and the count expression run once per entry; guards, advances,
backward jumps, body operations, and expressions share the existing work budget.
An empty repeat with literal count n uses 3 + 3*n steps in this implementation.
Advancing the last valid index reaches count, which is at most i64::MAX, without
overflow. Giant counts stop through the budget. The runtime remains iterative.
Binding metadata includes the saved count and index slots; existing API signatures
and ExecutionLimits literals are unchanged. Exhaustive public AST matches must
add Statement::Repeat. The default budgets remain 1,000,000 steps/100,000 prints.

## Delivery and tests

Implement counted loops separately from the three bounded improvements: tab/CR
escapes, actionable loop/budget diagnostics, and CLI/docs/examples. Each PR must
pass review, local checks and hosted tests/packages before merging. Verify mixed
loop control, count snapshots, integer boundaries, zero/exact budgets, scopes,
64/65 nesting, source spans, invalid grammar, atomic output, parallel reuse, and
fixed storage on long runs. Final release QA must verify all three archives,
checksums, clean build records, exact source commit, and the beta executable version.
