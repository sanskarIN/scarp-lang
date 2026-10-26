# Getting started

Use this guide to choose a Scarp version, build it, and confirm that the command
you are running matches the documentation. Start with
[compatibility](compatibility.md) if you already have a downloaded executable.

## Choose a version

| Goal | Choose | What to expect |
| --- | --- | --- |
| Try variables and the current guides | The v0.0.2 prerelease or `main` | Variables, scopes, strings, booleans, and decisions |
| Reproduce the first interpreter | The v0.0.1 tag | Printing and checked integer expressions |
| Inspect one proposed feature | Its individual feature branch | Only that branch and its prerequisites |

The package version is `0.0.2`. This is an experimental prerelease, with
syntax and APIs subject to change. Later feature PRs are outside this version.

## Build the development checkout

Install Rust with Cargo using the instructions linked from the
[project README](../README.md). The declared minimum Rust version is 1.74;
the project also validates a current stable toolchain. Scarp has no third-party
Rust dependencies.

```sh
git clone https://github.com/sanskarIN/scarp-lang.git
cd scarp-lang
git switch main
cargo build --locked --offline
cargo run -- --version
```

Expected version:

```text
scarp 0.0.2
```

Run commands from the repository root so relative example paths resolve:

```sh
cargo run -- run examples/hello.srp
cargo run -- run examples/string_bindings.srp
cargo run -- check examples/scopes.srp
cargo run -- tokens examples/arithmetic.srp
```

`check` is silent on success. `tokens` prints debugging information instead of
program output. See the [CLI reference](reference/cli.md) for exit codes and paths.

## Create a source file

Save this as a UTF-8 file named `demo.srp`:

```scarp
println("Hello from Scarp!");
println(1 + 2 * 3);
```

Output:

```text
Hello from Scarp!
7
```

Execute it with `cargo run -- run demo.srp`. The same program works in v0.0.1.
The `.srp` suffix is conventional; the CLI validates file contents rather than
requiring a particular extension.

## Use an installed command

From the selected checkout, `cargo install --path . --locked` installs `scarp`
through Cargo. Verify the result with `scarp --version`. If several copies are on
your `PATH`, the first matching command may be an older release. During source
development, `cargo run -- ...` selects the current checkout explicitly.

Downloaded release archives already contain an executable and do not require
Rust for running programs. Their platform and checksum instructions are in the
[v0.0.1 release notes](releases/v0.0.1.md).

Next, follow [your first program](tutorials/first-program.md), then the
[variable tutorial](tutorials/variables.md) on main or in the v0.0.2 package.
