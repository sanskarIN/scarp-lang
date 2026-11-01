# Scarp repository migration record

Destination: **scarp-lang/Scarp** at `https://github.com/scarp-lang/Scarp`.
The maintainer will perform the ownership transfer and rename. They have not
been performed by the release work recorded here. Do not create an empty target
repository: the existing repository is to be transferred, preserving its history.

## Identity and preserved release

| Item | Before transfer | Intended after transfer |
| --- | --- | --- |
| Repository | `sanskarIN/scarp-lang` | `scarp-lang/Scarp` |
| Repository ID | `1432976395` | Must remain `1432976395` |
| Owner | Personal account `sanskarIN` | Organization `scarp-lang`, ID `350973173` |
| Default branch | `main` | `main` |
| Development branch | `development` at `05b4c40f63567267f5d9662ba8108d403d682f88` | Preserved |
| CLI / Cargo package | `scarp` | `scarp` |
| Latest prerelease | `v0.0.5-alpha.1` | Same tag, release ID, source and asset bytes |
| Release source | `afe8b5380c9d6c4774162e51c6c9bc8b90901671` | Unchanged |

The [publication record](../releases/v0.0.5-alpha.1-validation.md) records the
release ID, annotated tag, seven uploaded assets, SHA-256 values and verification
runs. The adjacent [baseline JSON](scarp-repository-baseline.json) records release
and ref identities for comparison after the move. The source and archive names
keep `scarp`; the requested removal of `-lang` concerns the GitHub repository name.

The pre-release inventory had nine published releases, 27 branches and 11 open
historical PRs. The new prerelease and release-record work are added to that
history. No historical release/tag was rewritten, no unique branch or open
stacked PR was removed, and no repository copy/fork was substituted for a transfer.
A complete, verified Git bundle and API snapshots were saved locally under
`target/alpha5-publication-evidence/`; the ignored bundle is not a GitHub release
asset. Git bundles contain Git history, not issues, settings or release binaries.

## Maintainer transfer

1. Confirm the release record is published, the completed release request is
   retired and current main's Rust/native-package checks are successful.
2. In `sanskarIN/scarp-lang`, open **Settings â†’ General â†’ Danger Zone â†’ Transfer**.
   Select the existing `scarp-lang` organization and use repository name `Scarp`
   when the transfer dialog offers that field. GitHub requires repository admin
   access and permission to create repositories in the organization; renaming in
   that transfer dialog requires organization-owner permission.
3. If the dialog does not offer the name field, complete the transfer with the
   existing name and then rename it to `Scarp` in repository settings. Confirm
   there is no conflicting target repository or fork in the same network.
4. Keep the repository public and retain `main` as default. Transfer the existing
   repository rather than creating a new repository and pushing a copy.

GitHub documents preservation of commits, issues, PRs, releases and related
repository data, plus redirects from the old location. Do not reuse the old
`sanskarIN/scarp-lang` location: creating a repository/fork there removes those
redirects. See [GitHub's transfer guide](https://docs.github.com/en/repositories/creating-and-managing-repositories/transferring-a-repository)
and [rename guide](https://docs.github.com/en/repositories/creating-and-managing-repositories/renaming-a-repository).

## Verify the destination

These commands are for **after** the maintainer completes the transfer:

```sh
gh api repos/scarp-lang/Scarp --jq '{id,full_name,default_branch,visibility}'
git remote set-url origin https://github.com/scarp-lang/Scarp.git
git fetch origin --tags
git ls-remote --heads --tags origin
gh release view v0.0.5-alpha.1 --repo scarp-lang/Scarp
```

Confirm repository ID `1432976395`, full name `scarp-lang/Scarp`, public visibility,
and default `main`. Compare development/other preserved branch heads, all existing
annotated tags, release/asset IDs and checksums with the baseline JSON. Main is
allowed to advance through reviewed publication/status commits; the approved
release source and recorded history must remain reachable. Check the 11 retained
open PRs and their original numbers. Download all seven release files from the
new location and recheck SHA256SUMS and the clean source build records.

The expected relocated release URL is
`https://github.com/scarp-lang/Scarp/releases/tag/v0.0.5-alpha.1`.
This is an intended destination, not a claim that transfer has already happened.
Check both the new links and old URL redirects after the move.

## Follow-up source and integration updates

- Update `Cargo.toml`'s repository URL and current README/docs clone, `cd Scarp`,
  issue/PR and release links after the destination is live. Keep the crate name,
  binary name and published archive names as `scarp`. Do not rewrite published
  tags, packaged documents or assets to change their historical URLs.
- Search current operational files with
  `rg -n 'sanskarIN/scarp-lang' Cargo.toml README.md CONTRIBUTING.md docs scripts .github`.
  Treat historical release/source records as snapshots; avoid a global replacement.
- The branch-pruning workflow's repository guard uses stable repository ID
  `1432976395`, so it stays restricted to this repository after transfer/rename
  and remains disabled on forks. It still preserves main/development, active PR
  refs, protected branches and unique unmerged work. The publisher and other
  workflows obtain the repository path from GitHub context.
- Verify the organization's Actions policies permit the existing pinned actions,
  read-only Rust/package jobs and narrowly scoped publisher/maintenance writes.
  Review branch/ruleset requirements, collaborator/team access, installed GitHub
  Apps and organization secret access. Do not assume account policy is identical.
- Run fresh Rust/native-package workflows on destination main and verify all
  seven jobs and archive source records before another release. The existing
  publisher requires a new exact-version/source request for any future version.
- Check external integrations, badges and any references to this repository as
  an action. GitHub does not redirect `uses: owner/repo@ref` for hosted actions
  when their repository is renamed. Scarp's pinned third-party actions do not
  require renaming as part of this move.

No local remote URL or Cargo repository metadata is changed to the destination
before it exists. No ownership-transfer API, website, Pages project, hosting
account, crate rename or package-registry publication is part of this record.

## Completion checklist

- [x] Source-modules prerelease published and public files verified.
- [x] Release/tag/source/checksum and history baseline recorded.
- [x] Name-dependent maintenance guard prepared for the same repository ID.
- [ ] Maintainer transfers and renames the existing repository.
- [ ] Destination identity, refs, PRs, releases, assets and redirects verified.
- [ ] Local remotes and current source links updated against the live destination.
- [ ] Organization permissions/integrations checked and seven destination jobs pass.
