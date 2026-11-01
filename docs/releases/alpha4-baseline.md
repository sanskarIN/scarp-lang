# Alpha.4 integration baseline

Audit date: 1 November 2026 (Asia/Calcutta).

- Original checkout: `scarp/funcra-inspection`, `251de8fb4d8a54c7120915b606edb870a553b5e5`,
  package `0.0.3-dev`. The untracked `docs/funcra-validation.md` was preserved.
- Fetched main: `466ae7027ec5e977b9402c8acffa6074b1f60577`, package `0.0.4-alpha.3`.
  Created `codex/alpha4-integration` from this main rather than replaying the
  superseded native/compiler stack from the old inspection branch.
- Main already contains shared vector windows, stitching, classification helpers,
  log-space reductions, selection, L2 normalization, terminal self-transfer and
  the E0036 cumulative numerical allocation ledger.
- Reviewed the unique source diffs and commit lists of draft
  [tiles #53](https://github.com/sanskarIN/scarp-lang/pull/53), head
  `6b8d887323607ed67371b2f3348a092763709a2d`, and
  [scope retirement #51](https://github.com/sanskarIN/scarp-lang/pull/51), head
  `0f115f78e2f3c6d0bae903ba46d2d6e1d35b1bd3`. Both were based on older
  `a4069d1`; tiles conflict with current kernel signatures. Their original
  commits are preserved as integration merge parents.
- [Issue #59](https://github.com/sanskarIN/scarp-lang/issues/59) is open and
  specifies the allocation/window/retirement integration acceptance criteria.
  Older stacked PRs also remain open; their branches were retained. There were
  27 remote work branches excluding `origin/HEAD`; no branches were deleted.
- Alpha.3 was published on 31 October 2026 at 09:08:50 UTC with three archives,
  three sidecar checksums and `SHA256SUMS`. The alpha.3 request remained within
  its window ending 2 November at 18:30 UTC, although publication had completed.
  The publisher already leaves an existing published prerelease unchanged.
  Retire the completed request before integrating alpha.4.
- Alpha.4 had no tag, published release, draft release or matching remote branch
  in the authenticated release/branch inventory. Recheck before publication.
- Existing CI defines stable Windows, Linux and Apple Silicon macOS plus Rust
  1.74, and three native build/extract/smoke package jobs. Old PR runs are not
  validation evidence for this candidate.

Implementation sequence: integrate tiles with visible slices and prepaid
allocation; integrate retirement and its invalid-plan audit; implement bounded
matrix kernels; verify numerical edge cases, diagnostics and executable examples;
measure runtime changes; prepare alpha.4 docs/version/package checks; submit and
validate exact-source CI. Publication requires a separately authorized source SHA.
