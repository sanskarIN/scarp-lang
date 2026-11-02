# Contributing to Scarp

Scarp is at the prototype stage. Small, tested changes are welcome. Read the
[README](README.md) for the behavior that currently exists before proposing a
language feature.

## Local setup

Use the [feature matrix](docs/development/feature-matrix.md) to select coherent
work. Update its implementation status together with tests, runnable examples and
documentation. Review historical branches against current main before reusing them.
Development does not authorize a release, version change or tag.

Install Rust and Cargo with rustfmt and Clippy. Clone the repository, create a
feature branch, and run the checks before changing anything:

```sh
git switch -c feature/short-description
rustup component add rustfmt clippy
cargo test --locked
```

Run the complete validation script from any working directory:

```sh
sh scripts/check.sh --offline
```

On Windows with PowerShell:

```powershell
./scripts/check.ps1 -Offline
```

Omit the offline option if a future change adds dependencies that are not cached.
The current project has no third-party Rust dependencies. The scripts stop on
the first failure and do not install tools or change Git settings.

## Changes and tests

Keep tokenization, parsing, execution, and command-line handling separate. Attach
half-open source byte spans to new tokens and syntax errors. Validate an entire
program before executing it. Prefer returning errors over panicking on source
input or file operations. Unsafe Rust is forbidden for this prototype.

For each language behavior, add a small positive example and relevant negative
cases. Verify UTF-8 byte boundaries when changing source offsets. Use library
tests for language semantics and process tests for arguments, exit codes, and
standard streams. Keep tests deterministic and independent of their execution
order; temporary files must be unique and cleaned up.

Check changes using formatting, Clippy with warnings denied, the test suite in
debug and release modes, builds, rustdoc, and example commands. CI repeats the checks on Windows, Linux,
and macOS, and tests the minimum Rust version listed in `Cargo.toml`.

Document new syntax and changed limitations. Features listed as planned are not
part of the implemented language until their implementation and tests land.
Discuss dependencies and changes to the minimum Rust version in the PR.

## Branch lifecycle

Keep `main` as the protected, verified release source and `development`
as an optional long-lived experimentation line. Use short-lived `feature/*`,
`fix/*`, `release/*` and `scarp/*` branches only for actual changes; do
not manufacture ten permanent branches merely to reach a fixed number.
Aim to keep **fewer than ten active working branches** while preserving any
open PR, unmerged experimental work or branch necessary for release recovery.

[Branch policy and cleanup safeguards](docs/branches.md) document which
historical branches can be removed. The protected-main cleanup workflow
checks the current ancestry and active PR references before deleting
fully merged inactive heads. It never force-pushes or prunes a divergent
branch, and must pass its own dry-run safety tests.

## Pull requests and commits

Open a PR against the appropriate base branch and explain the observable change,
why it is needed, and what you verified. Include small reproductions for bug
fixes. Use descriptive commit messages and keep independently reviewable changes
in separate commits. Avoid generated binaries, build caches, or unrelated files.

For a dependent PR, identify the prerequisite PR and target its branch until the
prerequisite merges. Merge the base PR first, then retarget the dependent PR to
`main`. If preserving individual commits matters, use a merge commit or rebase
merge instead of squashing the stack.

Releases need a separate readiness review. A version in `Cargo.toml` does not
mean a release has been published. CI builds and verifies native package
artifacts; tagging and release publication remain separate maintainer actions.
See [the release process](docs/releasing.md).

## License

Contributions are made under the repository's [Apache License 2.0](LICENSE).
Keep the existing license and preserve required third-party notices when adding
third-party code.
