# Preparing a prerelease

The package workflow builds and verifies archives. It has read-only repository
permissions and never creates tags or GitHub Releases. Publication is a separate
maintainer action after the final source revision has passed validation.

## Build inputs and packages

The package version comes from `Cargo.toml`; use a three-part version. The
published beta.2 version and tag are `0.0.3-beta.2` and `v0.0.3-beta.2`.
That tag and its assets already exist and must remain unchanged. For a future
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
   `python scripts/verify-release.py dist/release-assets --version <version> --commit <full-final-main-sha> --write-manifest`.
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
