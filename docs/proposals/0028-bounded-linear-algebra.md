# Proposal 0028 — Bounded tile linear algebra

Status: implemented in the alpha.4 integration candidate; publication pending.

Use the existing positional, pure built-in naming style: `matvec`, `matmul`,
`tileadd` and `tilescale`. These preserve the existing meanings of vector `dot`,
`add` and `scale`; there is no overload or broadcasting selection.

The full public specification, empty-shape rules, finite-intermediate policy,
work formulas, allocation charges and runnable examples are in [the tile
guide](../tiles.md). `Builtin::parameter_types` and `result_type` serve both
static resolution and host validation; `src/numeric/tiles.rs` owns shape checking
and execution, independent of syntax or call frames. This is a useful boundary
for future typed lowering, not an unused intermediate representation.

Validation uses exact integer reference products across 125 small shape triples,
hand-computed rectangular/identity/zero fixtures, subnormal and signed-zero
checks, compensated cancellation, nonfinite intermediate rejection, maximum
outputs, zero inner axes, exact prepaid work/payload boundaries, shared-window
inputs, typed contracts/defaults, concurrent re-execution, atomic streams and
reviewed diagnostic fixtures.

Higher ranks, broadcasting, mutation, modules, automatic differentiation,
training orchestration, GPU backends and external BLAS are outside this scope.
