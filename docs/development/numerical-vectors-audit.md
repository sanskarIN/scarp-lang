# Numerical vector integration audit

Audited 2026-10-30 against main `3b65b7dc93d57ff47966faaeb192b09d57f65a8a`,
the immutable v0.0.4-alpha.1 source. Four source/MSRV and three native package
jobs passed. Rust 2021, minimum Rust 1.74, zero dependencies, unsafe-forbid and
the six existing execution options remain the constraints.

[PR #18](https://github.com/sanskarIN/scarp-lang/pull/18), head
`90be3968799becba2e35eb3bf74428f8c3605b8d`, targets the obsolete scalar branch
from #17. Its immutable vector wrapper, parser/index task design, compensated
reductions and numerical tests are useful. Its let syntax, diagnostic numbering,
call-only parser and interpreter predate current funcra frames and atomic dual
streams. No old branch or historical release file is merged wholesale.

Current scalar floats already have a checked FiniteFloat wrapper and E0030/31.
Retain it. The old vector proposal allocates conflicting E0021–25 codes, lacks
typed user-function integration and does not account vector snapshots in the
current capture cap. Its Arc slice conversion copies an owned buffer, and naive
sum-then-divide mean / square-then-divide MSE can overflow unnecessarily.
The old `fill(count, value)` signature also differs from the requested milestone.

Implementation sequence:

1. Specify grammar, immutable storage, user/builtin name precedence, numerical
   algorithms, work/allocation bounds and new diagnostics before changing code.
2. Adapt literals/indexing and add checked shared vector values; resolve numeric
   calls alongside user calls without putting kernels into the user call graph.
3. Integrate all ten pure kernels, function defaults/results/contracts, resumable
   expression tasks, vector equality, output preparation and snapshot accounting.
4. Add edge, reference-model, resource, CLI, recursion/concurrency and malformed
   source regressions; preserve every existing scalar/function test.
5. Measure small/large scalar-loop and native-kernel workloads, repeated calls
   and allocation-heavy programs with verified outputs and recorded environment.
6. Update guides/examples, prepare alpha.2 metadata and native artifacts, and
   open a main-based PR. Require complete fresh CI and repository review rules.

The published alpha.1 release, tag and assets remain unchanged. Alpha.2 has no
tag/publication authorization. Its preparation removes the previous active
publication request and never creates a new alpha.2 request.

## Stack regression found during validation

The first [PR source run](https://github.com/sanskarIN/scarp-lang/actions/runs/41027960413)
found a macOS debug stack overflow in the existing test combining 64 blocks
with 64 expression levels. Large statement and atom temporaries were retained
through recursive parser calls. Container/list parsing is now separated from
leaf statements and atom parsing, and private boxed parser errors keep recursive
Result values small. Public diagnostics and both depth limits are unchanged.

A new regression parses scalar groups, vector groups and nested vector calls
at the combined limits on an explicit 512 KiB thread, then resolves/executes
those plans on the normal test worker. All earlier nesting tests remain intact;
no environment stack increase or weakened limit is used. Final cross-platform
rechecks are recorded in the milestone PR.
