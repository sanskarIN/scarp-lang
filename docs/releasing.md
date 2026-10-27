# Preparing a prerelease

The package workflow builds and verifies archives. It has read-only repository
permissions and never creates tags or GitHub Releases. Publication is a separate
maintainer action after the final source revision has passed validation.

## Build inputs and packages

The package version comes from `Cargo.toml`; use a three-part version. For the
iteration beta, the version and tag are `0.0.3-beta.1` and `v0.0.3-beta.1`, and the GitHub
Release is marked as a prerelease. `v0.0.0.1` is not a three-part version.

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

1. Review the changelog, release notes, platform support, and known limitations.
2. Merge verified implementation and packaging PRs, preserving contributor
   identity and useful commits. Confirm the final `main` revision's Rust checks
   and package jobs all pass.
3. Download the package artifacts from that exact workflow run with
   `gh run download`. Verify every archive's `.sha256` file and `BUILD-INFO.txt`,
   then combine the archive checksums into `SHA256SUMS`. Python 3.11 or later can
   verify all three archives, required contents, executable permissions, clean
   source records, and digests with the dependency-free helper:
   `python scripts/verify-release.py dist/release-assets --version 0.0.3-beta.1 --commit <full-sha> --write-manifest`.
   Omit `--write-manifest` to recheck an existing manifest.
4. Create an annotated `v0.0.3-beta.1` tag on the verified revision with the maintainer's
   commit email and push only that tag. Do not move an existing published tag.
5. Create a draft GitHub Release using the existing tag, the versioned release
   notes, the three archives, their checksum files, and `SHA256SUMS`.
6. Confirm the tag target, prerelease flag, notes, and all uploaded asset sizes
   and digests. Publish the draft as a prerelease, without marking it the latest
   stable release. Verify the public release and download links afterward.

Release notes are stored in `docs/releases/`. Publication does not upload a crate
to a package registry; `publish = false` remains set in `Cargo.toml`.
