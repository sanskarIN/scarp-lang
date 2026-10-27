# Troubleshooting

Start with the exact command, version, working directory, stderr, and exit code.
This guide separates environment and invocation problems from language errors so
you can fix the failing layer first.

## A documented feature is rejected

Run `scarp --version`. The published v0.0.1 interpreter does not support variables,
blocks, or `check`. The variable prerelease reports `scarp 0.0.2`.
See [compatibility](compatibility.md) for the feature matrix.

An older executable on `PATH` can differ from your checkout. Compare:

```sh
scarp --version
cargo run -- --version
```

In a POSIX shell, `command -v scarp` shows the selected command. In PowerShell,
`Get-Command scarp` shows its location. Use `cargo run -- ...` to select the
current source checkout explicitly while developing.

## Cargo or a build component is unavailable

Check `rustc --version` and `cargo --version` in the same terminal. The declared
minimum is Rust 1.74. Follow the Rust installation instructions linked in the
[README](../README.md) if the toolchain is absent.

The full checks also need rustfmt and Clippy. Install those components through
rustup with `rustup component add rustfmt clippy`. An offline Cargo build cannot
install missing compiler components or native build tools.

If compilation reports a missing native linker, check the selected Rust host
with `rustc --version --verbose` and follow that toolchain's installation
requirements. On Windows the documented package target is MSVC; using a different
host is not the same as having the required MSVC build environment.

## The file cannot be read

Relative paths start at the process working directory, not the executable's
directory. Check it with `pwd` in a POSIX shell or `Get-Location` in PowerShell.
Quote filenames with spaces, and pass one file rather than a directory.

A file beginning with a dash needs a separator:

```sh
cargo run -- run -- -hello.srp
```

The first `--` belongs to Cargo; the second belongs to Scarp. Source files must
contain UTF-8 bytes. An editor or shell can save another encoding, so choose
UTF-8 explicitly when an apparently readable file produces a UTF-8 read error.
A single leading UTF-8 byte order mark is accepted.

## There is no program output

`check` is intentionally silent on success. Use `run` to print values. Empty and
comment-only files also succeed without output.

Read stderr and the exit code before assuming a program ran. Any lexical,
syntax, resolution, mutability, type, or arithmetic error prevents all program
output, including prints before the failing statement. The
[diagnostic reference](reference/diagnostics.md) explains the codes and phase order.

## A variable will not change, or changes the wrong value

Assignment requires `var`. An inner declaration can hide an outer variable,
and the nearest declaration controls both type and mutability. Scope exit does
not undo assignments to an outer binding. See the
[scope tutorial](tutorials/scopes.md) for examples of these different cases.

Repeated declarations in the same scope are rejected. Use assignment for one
changing value or a nested block for a deliberately separate local.

## Strings do not format or combine as expected

Placeholders inside a string are literal text. There is no interpolation,
concatenation, `print` function, or multiple-argument `print`. Print labels and
values with separate statements. `+=` is integer arithmetic, not string append.

Use only `\\`, `\"`, and `\n` escapes. To represent a line break inside a string,
write `\n` rather than placing a physical newline between its quotes.

## Package creation refuses to continue

The packaging script intentionally refuses to overwrite an archive or checksum.
Choose a fresh output directory, for example
`./scripts/package.ps1 -Target x86_64-pc-windows-msvc -OutputDirectory dist/review-2`.

Packaging requires a native toolchain matching the target. Inspect the `host:`
line from `rustc --version --verbose`. A successful ordinary build with another
host does not establish that the native package target can run on this machine.
See [the release process](releasing.md) for supported targets and build records.

If these steps do not explain a failure, report the smallest source and exact
command through the repository's bug-report template, including the complete
diagnostic and version information.
