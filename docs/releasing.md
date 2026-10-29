# Preparing a prerelease

The package workflow builds and verifies archives. It has read-only repository
permissions and never creates tags or GitHub Releases. A separate publisher
executes an explicit, reviewed request in `.github/release-request.json` after
the final source revision has passed validation. Removing that request disables
publication; changing a package version alone does not authorize a release.

## Build inputs and packages

The package version comes from `Cargo.toml`; use a three-part version. The
beta.3 version and tag are `0.0.3-beta.3` and `v0.0.3-beta.3`.
Earlier published tags/assets must remain unchanged. For a future
release, choose an unused version, update Cargo/lockfile/docs consistently, and
use its exact tag and archive names throughout the process. An unpublished
development identifier is not evidence that a public release exists.
`v0.0.0.1` is not a three-part version.

Run local checks, then package on a machine with the matching native Rust
toolchain and PowerShell 7:

```powershell
./scripts/check.ps1 -Offline
./scripts/package.ps1 -Target x86_64-pc-windows-msvc
```

The other supported targets are `x86_64-unknown-linux-gnu` and
`aarch64-apple-darwin`. Install the matching target through rustup if needed.
Package creation uses locked, offline Cargo builds and needs no registry access
for the current dependency-free project. Unix hosts also need `tar` and `chmod`.

Output is written to ignored `dist/`; temporary staging directories remain under
ignored `target/packages/`. Existing archive/checksum files are never overwritten;
choose a fresh `-OutputDirectory` for another run. Packages are built natively,
not cross-compiled. The workflow runs on each supported platform.

Each package includes a build record with its source commit and working-tree
state. Local experiments may be marked `modified`; published packages must say
`clean` and identify the exact release commit. Archives record their inputs but
are not promised to be byte-for-byte reproducible across hosts or build times.

## Publication process

1. Inspect existing tags/releases and select an unused version. Review its
   changelog, versioned release notes, platform support, and known limitations.
2. Merge verified implementation and packaging PRs, preserving contributor
   identity and useful commits. Confirm the final `main` revision's Rust checks
   and package jobs all pass on Windows, Linux, macOS, and minimum Rust 1.74.
   A successful earlier PR run is not a substitute for final-main checks.
3. Download the package artifacts from that exact workflow run with
   `gh run download`. Verify every archive's `.sha256` file and `BUILD-INFO.txt`,
   then combine the archive checksums into `SHA256SUMS`. Python 3.11 or later can
   verify all three archives, required contents, executable permissions, clean
   source records, and digests with the dependency-free helper:
   `python scripts/verify-release.py dist/release-assets --version <version> --commit <full-final-main-sha> --source-directory . --write-manifest`.
   Omit `--write-manifest` to recheck an existing manifest.
   For a new version, review the helper's required-file rules before relying on
   it. Match packaged docs/examples against that source tree as well as hashes.
4. Recheck that the chosen tag is unused. Create an annotated `v<version>` tag
   on the verified revision with the maintainer's commit email and push only
   that tag. If it already exists, stop and investigate; do not move it.
5. Create a draft GitHub Release using the existing tag, the versioned release
   notes, the three archives, their checksum files, and `SHA256SUMS`.
6. Confirm the tag target, prerelease flag, notes, and all uploaded asset sizes
   and digests. Download the draft assets and compare them byte-for-byte with the
   verified workflow artifacts before publication. Publish the draft as a
   prerelease, without marking it the latest stable release. Confirm published
   status, then download the public assets and recheck links and digests.

For the three supported native targets, the release has seven uploaded assets:
three archives, three corresponding `.sha256` files, and combined `SHA256SUMS`.
GitHub's automatic source archives are separate from these uploaded binaries.
The native jobs extract and smoke-test their own executable, version, every
example's exact bytes, silent checks, and deliberate failures. Record those run
URLs and the exact source SHA. A checksum detects corruption; it is not a
signature or independent provenance attestation.

Release notes are stored in `docs/releases/`. Publication does not upload a crate
to a package registry; `publish = false` remains set in `Cargo.toml`.

## Requested beta publisher

`.github/workflows/publish.yml` listens for completion of the Rust/package
workflows on main; it also supports a maintainer dispatch. It executes only
trusted main push revisions and checks out the triggering SHA. The request
currently names **only 0.0.3-beta.3**, with Sanskar's tagger identity. A later
release requires a new reviewed request matching Cargo and versioned notes.
An already published release is left unchanged. A draft or orphaned tag stops
the job for inspection rather than being overwritten.

`scripts/publish-release.py` requires both latest push workflows on the exact
current main SHA to succeed, with all four Rust jobs and three native jobs. A
missing, pending, failed, cancelled, skipped, or incomplete matrix cannot pass.
It downloads artifacts from that exact package run and verifies archives, clean
BUILD-INFO, source Git blobs, checksum files, and combined SHA256SUMS. Only then
does it create an annotated tag with the request's maintainer email, create a
draft prerelease, and upload the seven verified assets.

Draft asset metadata and downloaded bytes must match before publication with
prerelease=true and latest=false. The script confirms publication and downloads
every public asset without authentication to verify its digest. Source/tag/run
URLs and digests appear in the job log. The publisher uses job-scoped contents
write and actions read permissions; the check/package jobs stay read-only.

If main advances, the older candidate is skipped; its successor must pass its
own exact-source gates. A failure after tag/draft creation preserves them for
maintainer inspection. Do not move a tag to recover a failed publication.
