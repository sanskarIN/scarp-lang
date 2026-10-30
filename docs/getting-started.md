# Getting started

Use this guide to choose a Scarp version, build it, and confirm that the command
you are running matches the documentation. Start with
[compatibility](compatibility.md) if you already have a downloaded executable.

## Choose a version

| Goal | Choose | What to expect |
| --- | --- | --- |
| Follow the float guides | Current `main` or the v0.0.4-alpha.1 prerelease | Finite binary64 scalars integrated with typed functions and six resource budgets |
| Reproduce the function beta | v0.0.3-beta.3 | Typed functions, contracts, bind/var, output streams, loops; no floats |
| Reproduce the iteration beta | The v0.0.3-beta.1 tag | Original let/let mut and println, loops, budgets, and five escapes |
| Reproduce the variable milestone | The v0.0.2 tag | Variables, scopes, strings, booleans, and decisions |
| Reproduce the first interpreter | The v0.0.1 tag | Printing and checked integer expressions |
| Inspect one proposed feature | Its individual feature branch | Only that branch and its prerequisites |

Current main identifies as `0.0.4-alpha.1`. The
[prerelease page](https://github.com/sanskarIN/scarp-lang/releases/tag/v0.0.4-alpha.1)
records publication status and downloads. It includes [finite floats](floats.md) and
[typed functions](functions.md). Published beta.3 identifies as `0.0.3-beta.3`
and includes functions without floats. The temporary `0.0.4-dev` integration
identifier was never published.
Syntax and APIs remain subject to change. Published packages retain their
original syntax. Vectors, backends, project tooling, and CLI redesign remain separate.

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
scarp 0.0.4-alpha.1
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
print("Hello from Scarp!");
print(1 + 2 * 3);
```

Output:

```text
Hello from Scarp!
7
```

Execute it with `cargo run -- run demo.srp`. For v0.0.1 through beta.1, use
`println` instead of `print`; those releases do not recognize `print`.
The `.srp` suffix is conventional; the CLI validates file contents rather than
requiring a particular extension.

## Use an installed command

From the selected checkout, `cargo install --path . --locked` installs `scarp`
through Cargo. Verify the result with `scarp --version`. If several copies are on
your `PATH`, the first matching command may be an older release. During source
development, `cargo run -- ...` selects the current checkout explicitly.

Downloaded release archives already contain an executable and do not require
Rust for running programs. Their platform and checksum instructions are in the
[v0.0.4-alpha.1 release notes](releases/v0.0.4-alpha.1.md). Select Windows x64 MSVC,
Linux x64 GNU, or Apple Silicon macOS to match the host. Verify the archive's
SHA-256 against its companion checksum before extracting, keep the supplied
`examples/` and `docs/` directories, and run commands from the extracted root.
Use `./scarp.exe` on Windows or `./scarp` on Unix when the directory is not on
`PATH`. A source checkout includes the Rust sources and tests; a binary archive
contains the runnable interpreter, documentation, and examples.

Next, follow [your first program](tutorials/first-program.md), then the
[variable tutorial](tutorials/variables.md) on main. Older packages use the
original syntax described in their release notes.
