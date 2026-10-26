# CLI implementation inventory

This inventory compares the supplied command proposal with the repository and
the `scarp/cli-foundation` branch. It records capabilities rather than treating
the proposal as implemented. Main contains the v0.0.2 variable milestone; native
compilation depends on unmerged PRs #20 and #22. Nothing here merges or publishes.

The six implemented proposed groups are `guide`, `version`, file-only `exec`,
subset-only `compile`, static `verify`, and `inspect tokens/ast`. Traditional
`run`, `build`, `check`, `tokens`, help/version flags, and `emit-c` are retained.
Other proposed groups are absent from production help and return usage status 2.

## Catalog audit

"Existing" identifies a real implementation available before this foundation
slice, including prerequisite development branches. Remaining compiler work
includes runtime or tooling engines; infrastructure includes project models and
services. Priorities follow the proposal and do not imply an implementation date.

| Proposed command | Existing implementation | Implemented subset here | Compiler/runtime/tooling work remaining | Project/service work remaining | Priority |
| --- | --- | --- | --- | --- | --- |
| `guide` | Global help flags | Contextual guide/help | None for current commands | None | P0 |
| `version` | Version flags | Exact short-version alias | Component/verbose reporting | None | P0 |
| `create` | None | None | Valid app templates | Adopt project layout/manifest | P0 |
| `init` | None | None | Conflict-safe scaffolding | Adopt project layout/manifest | P0 |
| `exec` | File `run` | File execution alias | Program argument API | Project entrypoint resolution | P0 |
| `launch` | None | None | Reuse execution engine | Project-only entrypoint model | P0/P1 |
| `compile` | C backend and native build driver | Native executable/object subset | More value kinds and profiles | Project build model | P0 |
| `verify` | Resolver; check also evaluates | Static frontend checks | Warnings/JSON schema | Project source discovery | P0 |
| `test` | Rust contributor tests | None | Scarp test engine/discovery | Project test model | P1 |
| `clean` | None | None | Managed-artifact ownership checks | Adopt build root/records | P1 |
| `fmt` | Rust contributor formatting | None | Scarp source formatter | File/project selection | P1 |
| `lint` | Rust contributor Clippy | None | Scarp lint engine | Configuration policy | P1 |
| `polish` | None | None | Formatter/linter integration | Configuration policy | P1 |
| `inspect` | Lexer token dump and syntax API | Tokens and real parsed AST | Symbols/types/IR reports; JSON | Project/dependency graphs | P1 |
| `shell` | None | None | Stateful interpreter sessions | None | P2 |
| `eval` | Whole-source library execution | None | Snippet/expression contract | None | P2 |
| `watch` | None | None | Watcher/debounce/process supervision | Generated-path policy | P2 |
| `debug` | None | None | Debug information and debugger adapter | Platform debugger integration | P3 |
| `bench` | Real Rust stage benchmarks | None | Scarp benchmark engine | Workload discovery | P2 |
| `coverage` | None | None | Runtime instrumentation/source maps | Report format | P3 |
| `profile` | None | None | Profiling backend/source maps | Platform integration | P3 |
| `explain` | E-code reference documentation | None | CLI diagnostic catalog | None | P1 |
| `fix` | None | None | Safe edit rules and dry run | Backup/change reporting | P2 |
| `doc` | Markdown and Rustdoc | None | Scarp API documentation generator | Project discovery | P2 |
| `examples` | Tested repository samples | None | CLI sample catalog | Packaging/location policy | P2 |
| `generate` | None | None | Supported source templates | Adopt project layout | P2 |
| `diagnose` | Compiler spawn errors | None | Explicit readiness probes/reporting | Environment policy | P1 |
| `env` | SCARP_CC override | None | Selected-value reporting | Configuration/privacy policy | P1 |
| `config` | No Scarp settings file | None | Settings parser/validation | Adopt precedence/storage | P2 |
| `metadata` | Resolved binding metadata API | None | Project report/schema | Adopt manifest | P2 |
| `workspace` | None | None | Member/graph validation | Adopt workspace model | P2 |
| `task` | Contributor scripts | None | Named-task executor | Adopt manifest/trust policy | P2 |
| `migrate` | None | None | Versioned reversible transforms | Adopt format versions | P3 |
| `ci` | Real contributor check scripts | None | Scarp project check preset | Project test/lint model | P2 |
| `packages` | No Scarp dependencies | None | Resolver and integrity checks | Manifest/lockfile/cache/registry | P3 |
| `registry` | None | None | Registry client | Trusted registry protocol/storage | P3 |
| `publish` | Maintainer GitHub releases only | None | Package validation | Registry/authentication/confirmation | P3 |
| `bundle` | Native CLI release archives | None | Scarp application bundler | Distribution manifest | P3 |
| `audit` | None | None | Dependency integrity/advisory checks | Advisory and package sources | P3 |
| `toolchain` | External Rust/C installation | None | Toolchain management | Trusted installation/rollback | P3 |
| `target` | Native build target flags | Group absent; two checked object targets | More target backends | Target installation/model | P2/P3 |
| `cache` | No Scarp build cache | None | Owned cache records/cleaning | Cache policy | P2 |
| `completions` | None | None | Shell-specific generation | Installed-version synchronization | P1 |
| `lsp` | Frontend APIs | None | Language Server Protocol implementation | Editor integration | P2 |
| `self` | None | None | Installation manager | Signed distribution/update/rollback | P3 |
| `artifacts` | Real native output files | None | Artifact records/inspection | Adopt build ownership model | P2 |

## Compatibility decisions

- Bare `scarp` retains usage status 2. `scarp guide`, `scarp help`, and help
  flags provide successful discovery. This preserves the existing no-argument
  status instead of changing it to the proposal's suggested zero.
- `exec` and `run` have identical file behavior, budgets, and diagnostics.
  A standalone `--` still introduces one filename; program arguments and
  implicit project entrypoints are rejected rather than silently ignored.
- `compile` and `build` are equivalent. A fresh explicit output path is required;
  no implicit build directories or source overwrites are introduced.
- `verify` is deliberately distinct from `check`. The former stops after name
  and type resolution; the latter evaluates the selected path with budgets.
  An infinite loop or `1/0` passes static verification if otherwise well typed.
- Source and handled tool failures retain status 1; malformed/unsupported CLI
  usage retains status 2. The proposal's additional statuses are not adopted.
- `--release`, `--strict`, `--json`, watch mode, manifests, and registry flags are
  rejected. Compilation currently uses the documented fixed optimized profile.
- AST inspection prints the actual parsed tree using an experimental readable
  Debug format. It is not a stable JSON schema and performs no name resolution.
- Close unambiguous command spellings may receive a suggestion; execution is
  never corrected or retried automatically.

## Verification and next milestone

Integration tests exercise aliases, contextual help, version agreement, true
static checking, unchanged files, byte/line diagnostics, AST/token inspection,
unsupported commands/flags, native output, and path/separator behavior. Native
build tests also run through CI on Windows, Linux, and macOS.

The next coherent project milestone is a reviewed local manifest/entrypoint
design, followed by conflict-safe app creation and launch. Functions/modules,
package management, and networked services need their own designs and engines.
