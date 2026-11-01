# Module-tooling baseline

Inspected main: `1d3b69a79bb0f656b4da3517560ce59e1e41f3bc`.
Published version: 0.0.5-alpha.1. Rust 2021 / minimum 1.74, no third-party
dependencies, publish=false and unsafe forbid remain in force. Existing module
loading, scalar constants, function visibility/types/contracts and numerical
runtime features are implemented and will be reused.

The library currently exposes module/symbol lists and alias-target pairs, but
not import-site metadata, dependency queries or rich per-module signatures.
The CLI only exposes run/check/tokens. Alias validation occurs after all child
loads and scans prior aliases/declarations repeatedly. Cycle related locations
point to file starts and can include an acyclic prefix rather than actual import
edges. The source map renders owned diagnostics but lacks a public validated
span-to-file/location query.

Proposal 0030 defines the bounded first inspection slice before implementation.
The package version and released source/notes/assets will not change. Historical
stacked PRs remain outside this focused branch. The unrelated untracked
`docs/funcra-validation.md` is preserved.
