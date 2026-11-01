# Module milestone baseline — 1 November 2026

Main: `8521f5adeb2ce580c39d23782552979865a6278a`. Latest published prerelease:
`v0.0.4-alpha.4`, source `26e2fedc5ad492cdfd6ea986ca5b3017c286d310`.
The source package remains Rust 2021, Rust 1.74 minimum, dependency-free,
Apache-2.0, publish=false and unsafe_code=forbid. No active release request exists.
Final-main Rust checks and all three native packages were verified successful.

Tiles, matrix arithmetic, shared windows, allocation accounting, classification
kernels, terminal calls and scope retirement are already integrated. They will
be preserved rather than ported again. No module syntax/loading API exists.
Getting-started and compatibility guides still claim alpha.2 is current.

Eleven historical stacked PRs (#16–#27, excluding absent numbers) remain open,
including old function, vector, native-C and CLI stacks. Actual code comparisons
of the old function/vector branches show substantial incompatible differences:
the function stack adds obsolete C/backend files and lacks the current tile
module and retirement audit. Current main implements newer functions/vectors;
these older branches still contain distinct backend/tooling work. They are not
blindly merged, closed or deleted. No open issue was returned by the live audit.

Work uses one new branch, codex/modules. The original untracked
docs/funcra-validation.md is preserved. The module contract is specified in
proposal 0029 before implementation. Publication of a future version requires
new exact-version/source authorization; prior alpha.4 approval does not apply.
