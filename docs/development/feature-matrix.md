# Feature completeness and development roadmap

Audit date: 2026-11-02. This inventory is the source of truth for this development
cycle; update a row when its tested implementation lands. No roadmap row implies
a release or an available CLI command. Cargo remains 0.0.5-alpha.1.

Initial main: `1d3b69a79bb0f656b4da3517560ce59e1e41f3bc`. Inspected development:
`05b4c40f63567267f5d9662ba8108d403d682f88` (148 main-only and 31 development-only
commits). That branch contains older native/project/CLI experiments and lacks
later numerical, module and safety work. It must not be merged wholesale.

PR #70 (inspection) and #71 (constants) were independently checked against their
seven successful CI jobs, then integrated in that order. Retargeting #71 to main
preserved its verified source tree. Integration commits are `4ec11a3` and `93ae364`.
Automatic prerelease publication is disabled for this cycle; normal CI remains on.
No release request, version change, tag, registry or published-artifact mutation.

Priorities: P0 current foundation; P1 next enabling work; P2 later; P3 long term.
Complexity: S local; M several compiler layers; L substantial design/runtime; XL
requires separate architecture and multiple implementation phases.

| Feature | Current status | Missing subfeatures | Dependencies | Priority | Complexity | Required tests | Documentation |
| --- | --- | --- | --- | --- | --- | --- | --- |
| UTF-8 source and lexical rules | Implemented; ASCII identifiers | Unicode identifier policy, additional escapes | Lexer/source spans | P2 | M | BOM, invalid UTF-8, boundaries, truncation | Lexical grammar |
| Parser and AST | Flat expressions, bounded statements, typed declarations | Composite type grammar | Existing colon/type grammar | P0 | S | AST spans, malformed annotations, depth | Grammar/API migration |
| Immutable/mutable bindings | Implemented bind/var and optional annotations | Deferred initialization remains intentionally absent | Parser, typed resolver | P0 | S | Inferred/annotated parity, mismatch, shadowing | Variables/tutorial |
| Initialization analysis | Initializers mandatory; resolve before insertion | Deferred/conditional initialization not designed | CFG/data-flow first | P2 | L | Self-use, branch joins, loops | Initialization contract |
| Lexical shadowing | Implemented | Rich pattern bindings | Resolver scopes | P2 | M | Shadowing initializer, duplicates, captures | Scope rules |
| Assignments | =, +=, -=, *=, /= | Additional operators after bitwise design review | Operator/type contract | P2 | M | Type preservation, overflow, atomic output | Operators |
| Integer and finite-float math | Checked i64/binary64 and exact explicit conversions | Additional scalar math after domain design | Built-in registry and evaluator | P0 | S | Bounds, signed zero, loss of precision | Numeric reference |
| Scalar constants | Implemented by #71; annotations checked before evaluation | Approved constant intrinsics only after separate design | Shared expression typing/evaluation | P0 | M | Order/cycles, import visibility, task/node bounds | Constant guide |
| Bitwise/radix/remainder | Historical PR #19 only | Port reviewed semantics to current AST/codes | Reconcile old branch and diagnostics | P1 | M | All counts, overflow, precedence, parity | New current proposal |
| Strings | UTF-8 literals/equality, shared storage and seven bounded scalar queries | Construction, slicing, normalization with allocation policy | Typed pure built-ins and work charging | P0 | M | Combining marks, emoji, empty input, limits | Text indexing contract |
| Characters and bytes | Missing | Distinct types, decoding and conversion rules | Type representation and allocation limits | P2 | L | Encoding/invalid sequences, host parity | Type proposal |
| Tuples/fixed arrays | Missing | Types, literals, destructuring, indexing | Composite type representation | P1 | L | Nested types, length/index/type errors | Grammar/types |
| Dynamic typed lists | Numeric vectors only | General element types and mutation policy | Composite types, allocation budgets | P2 | XL | Aliasing, shape, limits, retirement | Collection contract |
| Maps/sets | Missing | Key equality/hash policy, deterministic iteration | Composite types and allocation accounting | P2 | XL | Collisions/order/limits, Unicode keys | Collection contract |
| Optional/structured errors | Missing | Tagged values and propagation | Variants and match typing | P1 | L | Success/error paths, exhaustive handling | Error model |
| Records/structures | Missing | Declaration, fields, construction, access | Nominal/structural type decision | P1 | XL | Recursive/nested fields, visibility | Type-system proposal |
| Enums/tagged variants | Missing | Constructors, pattern typing | Composite types and exhaustiveness | P1 | L | Exhaustive/redundant patterns, recursion | Variant/match design |
| Generics/type aliases | Missing | Type parameters, constraints, monomorphization policy | Composite types and stable IR | P2 | XL | Substitution, recursion bounds, diagnostics | Type-system design |
| Arithmetic/comparison/logic | Implemented; strict primitive kinds | Conditional expression form | Typed expression nodes | P1 | M | Precedence, short circuit, dead-branch typing | Expressions |
| Indexing/slicing | Numeric vectors/windows and tile cells/lanes | General strings/collections and multidimensional syntax | Composite types; text scalar policy | P1 | L | Bounds, visible windows, backing lifetime | Collection guides |
| If/while/repeat | Implemented and bounded | General collection iteration/ranges | Iterator/range type design | P1 | L | Empty/range bounds, controls, budgets | Iteration grammar |
| Break/continue/early return | Implemented with retirement | Labeled control only if justified | CFG and scope cleanup | P2 | M | Nested exits, contracts, retirement | Control flow |
| Pattern matching | Missing | Patterns, exhaustiveness, binding analysis | Variants/composite types | P1 | L | Missing/redundant arms, scope joins | Match design |
| Typed functions | Implemented | First-class callables and coherent overload design | Callable types and captures | P1 | XL | Resolution, nested calls, captured lifetime | Functions/effects |
| Named/default arguments | Implemented | Variadics only after collection/type design | Call signature representation | P2 | L | Ordering, duplicates, defaults, source sites | Argument rules |
| Closures/higher-order functions | Missing | Capture policy and escape/lifetime semantics | Callable values, effect rules | P2 | XL | Recursive capture, frame cleanup, quotas | Closure design |
| Pure functions/contracts | Implemented | General effect system | Explicit external capabilities first | P1 | L | Transitive purity, defaults/postconditions | Effects/contracts |
| Recursion/tail handling | Bounded heap frames; terminal self transfer | Mutual-tail optimization and profiling | Call-graph/IR analysis | P2 | L | Depth/call parity, postconditions | Runtime trade-offs |
| Module loading/visibility | Implemented; trusted roots and bounds | Explicit re-exports and standard modules | Export/alias contract | P1 | M | Re-export collisions/private access, cycles | Modules |
| Import/dependency inspection | #70 integrated; #71 constant edges | Versioned JSON output in this cycle | Existing immutable metadata | P0 | M | Exact schema/order/escaping/I/O/CLI | Inspection schema |
| Source navigation | Import and constant-use locations | All symbol references, go-to-definition | Reference index and stable source identity | P1 | L | Cross-file/Unicode/reference ownership | Tooling API |
| Incremental checking/cache | Missing | Identity, invalidation and immutable result reuse | Dependency/reference graph; cache contract | P1 | L | Changed/deleted dependencies, stale diagnostics | Invalidation design |
| Module initialization | Intentionally absent | Effectful initialization requires separate design | Capability/effect and cycle policy | P3 | XL | Failure/ordering/deduplication | Initialization design |
| Typed execution plan | Implemented lowered slot/operation plan | Explicit stable IR and CFG views | Resolver plan invariants | P1 | L | IR/source mapping and control edges | IR architecture |
| Optimizations | Constant modules, slot retirement, terminal self transfer | General folding, DCE and CFG passes | IR/effect/budget-equivalence contract | P1 | L | Differential output/errors/budgets | Optimization semantics |
| Native backend | Historical PR #20 only | Rebase scalar subset on current functions/modules/IR | Bitwise branch review, current safety budgets | P1 | XL | Differential interpreter/C, all rejected forms | Experimental subset |
| Native build/compile CLI | Historical PR #22 only; unavailable on main | Current backend integration and toolchain boundary | Backend, platform compiler/error handling | P2 | L | Artifact overwrite, subprocess/path/ABI | Build contract |
| Math/linear algebra library | 35 checked vector/tile built-ins | Scalar conversions now; later scalar math catalogue | Pure intrinsic dispatch | P0 | M | Numeric domains, finiteness, exact budgets | Built-in reference |
| General text/algorithms | Unicode length/codepoint/search/prefix/suffix implemented | General collection sorting/search | Allocation/work policy, collection types | P0 | M | Unicode semantics, deterministic search, limits | Standard library |
| Formatting/JSON/serialization | No language JSON library | Module JSON tool now; data JSON later | Error/composite values; bounded parsing | P1 | L | Escaping, schemas, invalid data, limits | Format/version policy |
| Filesystem/network/process I/O | Explicit source loader only | External capability model and structured errors | Effects, permissions, errors | P2 | XL | Denied access, roots, no ambient authority | Security model |
| Dates/time/random/environment | Missing | Explicit nondeterministic host capabilities | Effects/host interfaces | P3 | L | Seed/replay, denied host calls | Capability APIs |
| Numerical AI kernels | Vectors/tiles, stable softmax/loss/gradients | Generalized tensors/broadcasting and shape types | Composite numeric type/shape design | P1 | XL | Reference models, shapes, budgets, precision | Tensor proposal |
| CPU training/inference | Small classification examples implemented | Parameter/layer APIs and general models | Tensor/shape and serialization design | P2 | XL | Numerical oracles, convergence, reproducibility | AI tutorials |
| Automatic differentiation | Loss gradient kernels only | Graph/tape and chain-rule architecture | Tensor representation and lifetime budgets | P2 | XL | Finite differences, graph reuse, memory | AD design |
| GPU/backend abstraction | Missing | Explicit devices, transfer and backend contracts | General tensors and stable IR | P3 | XL | CPU differential, deterministic error rules | No GPU-support claim |
| Scope/frame retirement | Implemented, shared immutable numerical storage | Profiling and measured live-memory reporting | Runtime instrumentation contract | P1 | M | Nested scopes, exits, recursion and aliases | Memory trade-offs |
| Allocation budgets | Work/output/calls/slots/vector payload bounded | Unified measured statistics and string construction budget | Distinguish cumulative/live/process memory | P1 | L | Exact boundary and no-partial-output tests | Budgets/profiling |
| CLI run/check/tokens/modules | Implemented | JSON modules and explain in this cycle | Metadata/code catalogue | P0 | M | Help/usage/native paths/status/output failures | CLI reference |
| fmt/test/bench/doc/doctor | Not CLI commands | Complete contracts/tool discovery before advertising | Parser/trivia or runner/host capabilities | P2 | L | Idempotence/discovery/status/side effects | CLI proposals |
| LSP/IDE/debugger | Missing | JSON consumers, reference index, protocol and stepping | Stable diagnostics/source/reference metadata | P1 | XL | Protocol, edits/cancellation, UTF-16 mapping | IDE design |
| Package/build tooling | Cargo builds Scarp; source modules only | Scarp package manifest, dependency trust/lockfiles | Module/package identity design | P2 | XL | Reproducibility, traversal, offline cache | Package design |
| Interoperability/FFI | Rust host APIs only | Safe value/ABI and ownership boundaries | Stable IR/types/capabilities | P3 | XL | Lifetime/ABI/error/host permission tests | FFI design |
| Concurrency/async | Programs reusable across host threads | Language scheduling, synchronization, cancellation | Effects, values and runtime model | P3 | XL | Races/replay/deadlocks and quotas | Concurrency design |
| Performance/benchmarks | Runtime/numerics harnesses and examples | Compile-time/tooling workloads and statistics | Stable workload/reference recording | P1 | M | Repeated measurements, correctness oracles | Benchmark methodology |
| Security | Unsafe forbidden; bounded input/runtime; trusted source roots | External capabilities, hostile concurrent FS policy | Effects and host design | P1 | L | Escape/symlink/race/threat-specific tests | Threat model |
| Tests/CI/distribution | Stable/minimum debug/release; three native platforms | Broader feature corpus as rows land | Reproducible source/artifact evidence | P0 | M | Fmt/Clippy/docs/exact fixtures/native verification | Contribution/testing |
| Documentation/compatibility | In-repo guides and fixed release records | Synchronize separate website after scope/access established | Implemented contracts only | P0 | S | Relative links, runnable examples, snippets | Feature inventory stays current |

## Historical branch findings

Open PRs #16–#18 contain old loops/floats/vectors now independently implemented on
main. #19 adds radix/bit/remainder operations that are still missing. #20 adds a
freestanding C11 scalar backend; #22 wraps it in build/compile commands. They use
old AST/type/operator/diagnostic conventions, reject newer types/functions/modules,
and need a deliberate port plus differential tests, not a wholesale merge. #23–#27
contain historical CLI/bind/output/function stacks already superseded on main.
Development also has project-manifest/CLI experiments. No historical branches were
deleted, and no claim of native, project, tensor or GPU support is made.

## This cycle's complete vertical slices

1. Integrate verified inspection/constants (#70 then #71).
2. Optional binding/constant type annotations with strict inference compatibility.
3. Exact numeric conversions and bounded Unicode text queries, without new text
   allocation or ambient external effects.
4. Versioned module JSON inventories and diagnostic explanations for developer tools.

Each implementation batch must update its status, tests, examples and docs, use a
focused PR and pass CI before integration. Remaining rows are explicit future work.
