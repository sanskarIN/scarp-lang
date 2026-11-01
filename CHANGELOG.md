# Changelog

## 0.0.5-alpha.1 — prerelease, 2026-11-01

- Add explicit relative source imports, qualified aliases, public/private
  functions and immutable scalar literal constants, with declaration-only
  libraries and deterministic initialization-free linking.
- Add filesystem/in-memory source loaders, independent graph/expansion limits,
  canonical deduplication, cycle detection and source-aware E0037–E0043 errors.
- Preserve function typing, local shadowing, defaults, purity, contracts,
  numerical budgets, ordinary single-file diagnostics and atomic output.
- Add cross-file type locations, owned source maps, public typed symbol metadata,
  reusable compiled module programs and separate load/resolve/check/execute APIs.
- Add API/CLI/path/limit regressions, exact diagnostic fixtures and a three-file
  numerical example. Refresh introductory docs and require new/inherited package
  contents. Publish the explicitly authorized source through the guarded
  exact-source publisher. See the [verification record](docs/releases/v0.0.5-alpha.1-validation.md).

## 0.0.4-alpha.4 — prerelease, 2026-11-01

- Integrate immutable shape-aware tiles and typed tile parameters/results from
  PR #53 with visible vector windows and the cumulative allocation ledger.
- Add `matvec`, `matmul`, `tileadd` and `tilescale`, checked shapes, compensated
  products, finite intermediates and prepaid work/payload limits.
- Integrate scope retirement from PR #51 across lexical exits, loop transfers,
  function completion and shared snapshots. Strengthen invalid-plan tests for
  postcondition parameter lifetimes, slots, defaults and control flow.
- Add precise dimension/index spans, contextual tile argument errors, reviewed
  diagnostics, independent matrix reference tests and seven runnable examples.
- Include the post-alpha.3 window/stitch, log-space, selection and classification
  kernels already integrated on main. Preserve existing language/CLI behavior.
- Retire completed publication requests. Publish alpha.4 from the separately
  authorized source after exact-source CI, native-package, checksum, draft and
  public-download gates. See the
  [verification record](docs/releases/v0.0.4-alpha.4-validation.md).

## 0.0.4-alpha.3 — prerelease candidate, 2026-10-31

- Add pure, overflow-resistant `norm(v)` and `normalize(v)` kernels with
  scaled numerical reductions, checked finite results and regression coverage.
- Reuse the current `funcra` frame on eligible direct terminal self-returns
  without bypassing total-call/step budgets or needs/default contracts.
  Postcondition functions and nonterminal calls retain ordinary frames.
- Introduce cumulative numerical payload allocation quota
  `--max-vector-bytes`/`ExecutionOptions.max_vector_bytes` (default 8 MiB),
  E0036 errors, exact-byte host/kernel/CLI tests and atomic-output validation.
  `normalize` participates in this same quota.
- Add three exact-output examples and a memory-control specification.
- Define minimal branch naming, auto-prune only merged inactive branches,
  and retain open PR heads or divergent experiments.
- Require exact-main multi-OS CI, native assets, checksum/source verification,
  and explicitly requested gated publication. See
  [release notes](docs/releases/v0.0.4-alpha.3.md).


## 0.0.4-alpha.2 — scheduled prerelease, 2026-10-31

- Immutable finite vectors, literals/indexing, whole-value assignment and typed
  funcra defaults/arguments/results/contracts with shared snapshots.
- Ten pure kernels with compensated reductions, scaled mean/MSE and shifted
  softmax. Preserve user-defined kernel names; fill uses value then count.
- Prepay native work and payload allocations; account vector equality, output
  preparation and snapshot bytes under the unchanged six execution controls.
- Add E0032–E0035 without changing established codes, independent numerical
  references, adversarial/function/CLI tests and three exact-output examples.
- Record numerical benchmark results and require native vector smoke tests.
- Schedule the authorized alpha.2 publication for 31 October at 10:00 AM IST,
  with an explicit UTC start/expiry window and unchanged source/artifact gates.
  Add numerical tutorials, error repairs and checkpoint evidence. See
  [release notes](docs/releases/v0.0.4-alpha.2.md).

## 0.0.4-alpha.1 — 2026-10-30 — prerelease

- Finite binary64 float scalars with decimal/scientific literals, same-type
  arithmetic/comparisons, checked nonfinite results and signed-zero divisors.
- Float parameters/defaults/results use existing funcra frames, purity/contracts,
  all six budgets, atomic streams and value snapshots without implicit coercion.
- Append E0030/E0031 without changing existing codes; add numeric examples and
  boundary, rounding, source-span, concurrency and native-package regressions.
- No vectors, native backend, CLI rename, or third-party dependencies.
- Require float guides/example fixtures in native archives and retire the
  completed beta.3 recovery request. Publication uses a fresh, explicitly
  authorized request and exact-main gates. See [release notes](docs/releases/v0.0.4-alpha.1.md).

## 0.0.3-beta.3 — 2026-10-29 — prerelease

- Adapt typed file-level `funcra` functions onto beta.2 main without importing
  the experimental numeric, native, or CLI stacks. Forward calls, recursion,
  named/default arguments, unit returns, purity, and needs/ensures contracts.
- Heap call frames, additive resource options, bounded traces, immutable metadata,
  and function-aware repeat counts preserve atomic stdout/stderr preparation.
- New function codes E0020–E0029 preserve existing E0001–E0019. Unknown call names
  now report E0023; unknown non-call statements retain E0005.
- Expose call depth/count, live slots, and output snapshot caps as CLI options
  for both run and check, with exact-boundary tests and actionable diagnostics.
- Verify bundled documentation/examples against the exact Git tree; require all
  function guides/examples and smoke-test every resource flag in native archives.
- Add a separately gated tag publisher using successful final-main checks and
  native packages. See [release notes](docs/releases/v0.0.3-beta.3.md).

## 0.0.3-beta.2 — 2026-10-28 — prerelease

- Contextual repairs for incomplete bind/var declarations and assignments,
  preserving precise expression errors and output atomicity.

- Canonical `print`, raw `write`, and explicit stderr `eprint`/`ewrite`, with
  zero/multiple arguments and optional trailing commas. `println` remains an
  exact alias. Output events preserve snapshots, budgets, and atomic diagnostics.
- Additive dual-writer execution APIs; the CLI routes program stderr explicitly.
  Single-writer hosts receive an actionable error before stdout when a program
  requires stderr. Current examples and guides use `print` by default.

- Immutable `bind` and mutable `var` declarations replace `let`/`let mut`.
  Legacy declarations report E0005 with migration help; Rust syntax uses
  `Statement::Declaration` instead of `Statement::Let`.
- Current examples and guides use the new declarations; the v0.0.2 source
  examples remain preserved as exact historical compatibility fixtures.

## 0.0.3-beta.1 — 2026-10-27 — prerelease

- Actionable loop/count/index diagnostics, configured limits in budget errors,
  contextual run/check/tokens help, and iteration/budget guides.
- Tab `\t` and carriage-return `\r` string escapes, with exact-byte fixture checks.
- Counted `repeat count as index` with a one-time count snapshot, immutable
  zero-based index, fixed storage, and nearest-loop control shared with while.
- Scoped while loops, nearest-loop break/continue, per-call work/print budgets,
  and CLI/embedding controls with atomic budget diagnostics.


## 0.0.2 — 2026-10-26 — prerelease

- Execution-local expression work buffers, immediate disposal of validation print
  values, and reproducible stage benchmarks with verified workloads.

- Conditional `if`/`elif`/`else` statements with boolean guards, scoped bodies,
  static checking of all paths, and execution of the selected path only.

- Boolean values, same-type equality, integer ordering, and short-circuit `and`,
  `or`, and `not` expressions.

- Immutable integer bindings and names in expressions.
- Explicit mutable bindings, assignment, and checked `+=`, `-=`, `*=`, `/=`.
- Immutable-assignment diagnostics and value snapshots for deferred output.
- Lexical block scopes, shadowing, and bounded block parsing/resolution.
- Name resolution before evaluation, with fresh storage for every execution.
- Unknown, duplicate, and reserved-name diagnostics with related declarations.
- A separate parsed-syntax API and immutable resolved-program API.
- Silent `check` validation of syntax, names, mutability, scopes, and arithmetic.
- Sequential token locations without repeated source-prefix scans.
- Generated-program conformance cases and malformed source/token mutation corpora.
- Package version `0.0.2`, native archives, and verified source/checksum records.
- String variables, primitive type inference,
  same-type mutable assignment, and immutable string value snapshots.

The variable milestone is integrated into main. Loops, floating-point/vector
arithmetic, bit operations, and freestanding C emission remain in separate PRs
and are excluded from this prerelease.

## 0.0.1 — 2026-10-25 — prerelease

The first experimental Scarp interpreter. Syntax and library APIs may change.

### Language

- UTF-8 `.srp` files with whitespace, line comments, and escaped string literals.
- `println` statements accepting strings or signed 64-bit integer expressions.
- Arithmetic precedence, grouping, unary minus, and left-associative operators.
- Checked arithmetic, literal range checks, and division-by-zero diagnostics.
- Full validation and evaluation before program output is written.
- Source byte spans, line/column locations, carets, error codes, and help.
- A nesting limit with diagnostics and flat expression storage for long chains.

### Tools and distribution

- `scarp run`, `scarp tokens`, help, and version commands.
- A reusable Rust library with explicit output destinations and error handling.
- Debug/release tests, cross-platform CI, and minimum Rust 1.74.0 checks.
- Runnable examples, a language reference, and contributor documentation.
- Native binary archives with SHA-256 checksums and recorded build information.
- Packaging smoke tests performed on the extracted deliverables.

### Known limits

No bindings, user-defined functions, floating-point values, type checker, modules,
loops, package manager, or native code generation. Source and intermediate values
are held in memory. Only the first diagnostic is reported. Expression grouping
is limited to 64 levels. The prerelease includes Windows x64, Linux x64 with
GNU libc, and Apple Silicon macOS builds; other platforms can build from source
where a compatible Rust toolchain is available.
