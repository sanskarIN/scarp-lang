# Typed funcra functions

Status: core interpreter implemented and tested; unreleased. Base: `scarp/output-statements`
at `4b1b3e2532c7769c5fe011f693fc47c42de1a5a5`, dependent on PR #26 and its
ordered prerequisites. The separate `development` application-workflow checkpoint
is not integrated. Main, tags, releases, and dependency versions are unchanged.

## Initial audit

Rust 2021, minimum 1.74, no third-party Rust dependencies, unsafe code forbidden.
The base supports bind/var, lexical scopes, branches/loops, checked i64, finite
f64, booleans, shared immutable strings/vectors, and five output statements.
Commands are exec, compile, verify, inspect, guide, version. Expressions are flat
postorder nodes; resolved statements use jumps and distinct local slots. Runtime
preparation captures ordered output before replay; validate discards output.
Limits are 1,000,000 work steps and 100,000 selected output statements. Native C
supports integer/boolean expressions and literal strings, with bounded storage.
Existing public ExecutionLimits has two fields and is commonly constructed with
struct literals; extending it directly would break callers.

Audit covered source stages, public APIs, native emission/runtime, examples,
current proposals, contribution rules, and Windows/Linux/macOS/minimum-Rust CI.
The complete offline baseline (`./scripts/check.ps1 -Offline`) passed on this
base: 580 test executions, zero failures/ignored tests, formatting, Clippy,
debug/release builds, rustdoc, and all 14 existing examples. Clang was required,
including x86_64-none-elf and aarch64-none-elf checks. No baseline failure was
found. Local log: `dist/funcra-baseline.log` (ignored build evidence).

## Decisions

- Canonical declaration: `funcra name(parameters) -> type { statements }`.
  Keywords remain contextual lexer identifiers; colon and adjacent arrow are
  punctuation tokens. No aliases. Parameters and result types are mandatory.
- Gather file-level signatures before any body resolution. Function names cannot
  collide with ordinary bindings anywhere. No implicit global captures. Function
  parameters are immutable; body locals may shadow parameters in an inner scope.
- The supplied sum/add examples collide with existing numerical built-ins. An
  explicit file-level function declaration takes precedence over that built-in
  throughout that file, without overload selection. Files without such a
  declaration retain existing built-in behavior, including local names like sum.
  Output/control keywords cannot be overridden. Document this deliberate choice.
- Unit is a result-only type: bare return/fallthrough; no value initializer,
  output, argument, comparison, or arithmetic use. Non-unit functions require
  conservative definite return; loops never establish definite return. All
  syntactically present bodies are checked, including unused/unreachable code.
- Preserve source-order explicit argument evaluation; map values to parameter
  identities afterward. Positional arguments precede named ones. Defaults form
  a trailing suffix and see only earlier parameters and pure known calls.
- Explicit pure functions may mutate frame-local variables but cannot output or
  call a function lacking the pure declaration. No inferred-purity promises.
  Needs and ensures must be boolean and pure. Result exists only in non-unit
  postconditions. Contracts execute in source order and consume work.
- Suspend expression evaluation at user calls and use a heap-owned frame stack;
  never recurse through the Rust evaluator. Fresh frames isolate slots and
  return control. Bounded related diagnostic entries show inner-to-outer calls
  without printing argument values.
- Add ExecutionOptions alongside the unchanged ExecutionLimits. Defaults add
  depth 256, total calls 100,000, live slots 1,000,000, and captured-value storage
  16 MiB. Existing APIs select these defaults; new APIs and exec flags configure
  them. Counts apply to runtime validation as well as execution.
- Evaluate every output statement's arguments before appending its own output;
  nested function output precedes its caller's statement. Runtime errors still
  discard both streams. Bound captured storage conservatively without formatting
  strings during validation. Existing programs retain their work accounting.
- Expose immutable function metadata and deterministic `inspect functions`.
  The inspector resolves all bodies but does not execute defaults or contracts.
- First-class functions, anonymous/nested functions, captures, function types,
  per-function annotations, and native function lowering are follow-up work.
  Reject unsupported forms explicitly. Native emission rejects any user function
  before producing C; existing supported native programs retain their ABI.

## Delivery gates

Complete interpreter slices must include grammar, resolution, runtime, tests,
examples, CLI/library integration, and documentation. Record actual support by
phase rather than treating a proposal as working syntax. Native function support
requires an explicit-frame C design, differential limits/contracts/recursion tests,
and freestanding checks before this restriction can be removed. Closure support
requires typed callable values, immutable captures, effect compatibility, and
allocation budgets before accepting its syntax.

## Follow-up checklist

The current value-kind enum and flat expression plan describe primitive results.
First-class functions need a signature/effect table and callable value identities
before closures can carry environments safely. Do not accept their grammar
until all of the following are implemented together:

- Typed callable IDs, explicit function parameter/result signatures, and pure to
  impure-capable compatibility without the reverse conversion.
- Named function values, indirect calls, returnable callables, and a static
  noncallable-value diagnostic that cannot depend on runtime string lookup.
- Anonymous expression parsing with bounded nesting, immutable captures by value,
  rejection of external mutable captures, and frame-independent lifetimes.
- Allocation/work limits for captured environments, nested-return tests, effect
  checks for callbacks, scope-exit tests, and native rejection for unsupported kinds.

Native named functions require explicit generated frames (including return PCs),
forward prototypes/identities, deterministic argument/default/contract ordering,
checked primitive arithmetic, and the same call/work failure categories. Preserve
freestanding output validation/replay and document any host ABI extension. Add
interpreter/native differential recursion, contracts, budgets, spans, and both
streams at O0/O2, plus the existing freestanding/undefined-symbol/sanitizer gates.

No release tag, PR merge, per-function annotations, closure syntax, or partial
native function emission is authorized by this implementation proposal.
