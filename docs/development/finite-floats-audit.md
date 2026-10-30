# Finite float integration audit

Audit date: 2026-10-30. Source baseline:
`c3bf53896e89ea5e421d06742e754b9feccbef2f` on main. Latest public prerelease:
v0.0.3-beta.3, published 2026-10-29 from its immutable tagged source. Local
unfinished beta.3 metadata was preserved in stash
`6c3c3e5070783313e6a436fb76f2ee6da82ea7e2`; it is not part of this feature.

## Existing implementation and adaptation

[PR #17](https://github.com/sanskarIN/scarp-lang/pull/17), head
`9a14eb18ea2123c034b0eb465322a4e0701a89e8`, targets scarp/bounded-loops. Its
475-line change contains a useful finite wrapper, decimal/exponent scanner,
checked arithmetic, and eleven numeric tests. It is binary64 arithmetic despite
the historical decimal terminology. Its old E0019/E0020 allocations conflict
with main; its let syntax and resolver predate repeat and typed functions.

Adapt only scalar semantics and relevant tests. Preserve main's heap call
machine, explicit streams, six budgets, diagnostics, CLI, and publisher gates.
Add float parameter/result/default parsing and same-type resolution to current
funcra rather than merging the old branch. No newer float-specific PR or issue
was found in the live audit; vector/native PRs contain dependent work.

The three current workflows were inspected. Stable checks and native packages
pin Apple Silicon to macOS 15; minimum Rust is 1.74. The separate publisher only
acts on a release-request file after exact-main source/package gates. Its beta.3
recovery fields refer to an already published release. Retire that request in
the alpha preparation; never reuse its release ID or move its tag. An alpha
candidate alone does not authorize a new publication request.

## Older PR triage

| PR | Current assessment | Action |
| --- | --- | --- |
| #16 | While/control/budget behavior is integrated in main | Superseded contract; retain pending maintainer archive review |
| #17 | Scalar float candidate on an old baseline | Fresh port in this feature; close only after replacement acceptance |
| #18 | Numerical vectors and kernels are absent from main | Keep open as design/source reference; port after scalar floats |
| #19 | Bit operations and radix literals are absent | Keep separate; needs a fresh main-based port |
| #20 | Freestanding C backend is absent | Keep separate; needs interpreter differential tests |
| #22 | Native compilation CLI depends on #20 | Keep separate until a backend is accepted |
| #23 | CLI aliases/inspection differ from current command contract | Keep as proposal; do not import aliases |
| #24 | Breaking canonical command rename differs from shipped CLI | Keep as proposal; requires explicit migration decision |
| #25 | bind/var is integrated through #34 | Superseded contract; retain pending maintainer archive review |
| #26 | Output statements and streams are integrated through #35 | Superseded contract; retain pending maintainer archive review |
| #27 | Function core is integrated through #41; old numeric/vector contexts differ | Retain until dependent extensions are independently reviewed |

No old PR is closed, merged, or rewritten as a side effect of this audit.
Main protection requires PRs and resolved threads, blocks force pushes/deletion,
and requires zero approving reviews. Recommend requiring all four Rust and
three package check contexts. Keep independent-review requirements compatible
with the available maintainer; no protection is weakened or changed here.

## Release scope

Target candidate: 0.0.4-alpha.1. Finite scalars only: no vectors/tensors,
conversions, bit operations, native backend, CLI rename, modules, or packages.
See [the numeric contract](../proposals/0016-finite-floats.md). Exact-final-main
green checks and three native archives are required before public tagging.
Public alpha tagging/publication additionally requires explicit authorization
for v0.0.4-alpha.1; beta.3 authorization is not reused.
