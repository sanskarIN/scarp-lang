# Scarp repository migration - completed

The maintainer transferred and renamed the existing repository to
**[scarp-lang/scarp](https://github.com/scarp-lang/scarp)**. The canonical
repository name is lowercase `scarp`. This follow-up verified the destination
on **2026-11-01T08:08:32Z** and updated the local Git remote and current source
metadata. It did not perform another transfer or create a replacement repository.

## Verified identity and history

| Item | Before transfer | Verified destination |
| --- | --- | --- |
| Repository | `sanskarIN/scarp-lang` | `scarp-lang/scarp` |
| Repository ID | `1432976395` | `1432976395` |
| Owner | Personal account `sanskarIN` | Organization `scarp-lang`, ID `350973173` |
| Default branch / visibility | `main` / public | `main` / public |
| Development head | `05b4c40f63567267f5d9662ba8108d403d682f88` | Unchanged |
| Main before this follow-up | `8755ffeefb37d28d11ca3b2b182882ca1226a95b` | Unchanged by transfer |
| CLI / Cargo package | `scarp` | `scarp` |
| Latest prerelease | `v0.0.5-alpha.1`, release ID `430432379` | Same tag, release and asset identities |
| Release source | `afe8b5380c9d6c4774162e51c6c9bc8b90901671` | Unchanged |
| Annotated tag object | `c2c943e134bcb70179a34a69c2089575dd5ed6f2` | Unchanged |

The [pre-transfer baseline](scarp-repository-baseline.json) remains an unchanged,
dated snapshot, including its original planned spelling `scarp-lang/Scarp` and
`transfer_performed: false`. The separate
[post-transfer receipt](scarp-post-transfer-verification.json) records the actual
canonical path, verified preservation and relocated asset URLs.

All prior non-main refs matched the baseline. The 11 historical open PRs kept
their original numbers. All 10 releases, including the nine preceding this
prerelease, retained their release and asset IDs, names, sizes and digests.
Main may advance through reviewed follow-up commits; the published source and
existing history remain reachable. No history was replaced by a new copy/fork.

## Release and redirect verification

The relocated [prerelease](https://github.com/scarp-lang/scarp/releases/tag/v0.0.5-alpha.1)
contains the same seven uploaded assets. All seven were independently downloaded
without authentication from the new URLs and matched their pre-transfer SHA-256
values and API sizes/digests. The exact-source archive verifier checked clean
build records, executable permissions and every packaged documentation/example
Git blob against the original release source. The combined manifest matched its
canonical bytes. [Publication evidence](../releases/v0.0.5-alpha.1-validation.md)
retains the original publication context and checksums.

The old repository page, release page and SHA256SUMS download were also checked:
they redirect and return the expected destination/content. Keep the old
`sanskarIN/scarp-lang` location unused so those redirects remain available, as
explained in [GitHub's transfer documentation](https://docs.github.com/en/repositories/creating-and-managing-repositories/transferring-a-repository).
Published tags, binaries, release-note bodies and packaged historical URLs were
not rewritten to change ownership metadata.

## Current clone and remote

```sh
git clone https://github.com/scarp-lang/scarp.git
cd scarp
cargo build --locked --offline
```

For an existing checkout:

```sh
git remote set-url origin https://github.com/scarp-lang/scarp.git
git fetch origin --tags
gh api repos/scarp-lang/scarp --jq '{id,full_name,default_branch,visibility}'
gh release view v0.0.5-alpha.1 --repo scarp-lang/scarp
```

The local workspace can keep its existing directory name. `Cargo.toml` now names
the canonical repository URL; the package, command, version and archive names
remain unchanged. Current setup, maintenance and release links use the new path.
Historical release/design records and the pre-transfer baseline retain their
original URLs and context.

## Organization and workflow checks

Repository Actions are enabled and allow the existing pinned actions. Default
workflow-token permission is read-only and approving PR reviews is disabled.
Existing explicit per-job write scopes remain in the publisher and maintenance
workflows. The active main-branch ruleset `25815628` requires a PR and prohibits
branch deletion and non-fast-forward updates; its rules were inspected.
Authenticated repository access and the GitHub connector worked at the new path.

The pruning guard already uses stable repository ID `1432976395` rather than an
owner/name string. Other workflows obtain the repository path from GitHub
context. No workflow permission or protection was relaxed by this follow-up.
The completion PR and its exact destination main revision must pass all four
Rust jobs and three native package jobs before the follow-up is declared complete;
run outcomes are recorded in that PR and the
[Actions history](https://github.com/scarp-lang/scarp/actions).

Future publication still requires a fresh exact-version/source authorization.
The completed 0.0.5-alpha.1 request remains retired. No new release, crate rename,
package-registry publication, website or Pages deployment is part of this follow-up.
