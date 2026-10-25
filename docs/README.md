# Scarp documentation

Choose a guide by the task you want to complete. These pages document the
published v0.0.1 snapshot and the complete `0.0.2-dev` feature stack, including
the optional string-binding extension. Development examples are labeled;
they are not claims that a v0.0.2 release has been published.

## Guides and what each adds

| Document | Use it to |
| --- | --- |
| [Getting started](getting-started.md) | Select a version, build the intended checkout, and verify your executable |
| [Your first program](tutorials/first-program.md) | Learn the file-to-output workflow, comments, escapes, and a first syntax repair |
| [Variables and mutation](tutorials/variables.md) | Introduce names, copy values, choose mutability, and avoid forward references |
| [Scopes and shadowing](tutorials/scopes.md) | Predict which declaration a name uses and what survives a block |
| [Command-line reference](reference/cli.md) | Look up commands, version availability, paths, streams, and exit codes |
| [Diagnostic reference](reference/diagnostics.md) | Interpret all error codes and repair common failures |
| [Values and operators](reference/values.md) | Check precedence, numeric boundaries, assignment rules, and string limits |
| [Rust embedding](embedding.md) | Select a library stage, capture output, reuse compiled programs, and render errors |
| [Testing changes](testing.md) | Choose relevant tests, add exact-output examples, and validate documentation |
| [Troubleshooting](troubleshooting.md) | Separate installation, path, encoding, version, language, and package failures |
| [Compatibility](compatibility.md) | Compare released/development behavior and migrate an embedding host |

## Reading paths

For a first program, follow getting started, the first-program tutorial, then
variables and scopes on the development branch. Keep the CLI reference open
when switching between installed binaries and Cargo commands.

For a failure, identify the executable and exit code in troubleshooting, then
look up the diagnostic. Read both primary and related source locations before
changing a declaration.

For a Rust host, begin with compatibility and embedding. Parsing, compilation,
validation, and execution intentionally have different results and effects.

For a contribution, read testing and the root
[contribution guide](../CONTRIBUTING.md). The current variable work remains a
feature-branch stack, with integration and release decisions left to the maintainer.

## Exact rules and project records

The tutorials teach complete tasks. The existing [language reference](language.md)
and [variable reference](variables.md) spell out the implemented semantics.
[Architecture notes](architecture.md) explain the module and execution boundaries.

Design records cover [integer expressions](proposals/0001-integer-expressions.md),
[variables](proposals/0002-variables.md), and
[string values](proposals/0003-string-bindings.md). Read each record's status;
a proposed capability is not implemented merely because it appears in a roadmap.

Published behavior is recorded in the [v0.0.1 notes](releases/v0.0.1.md).
The [v0.0.2 notes](releases/v0.0.2.md) are a development draft.
[Release preparation](releasing.md) describes the separate packaging and
publication process. The root [changelog](../CHANGELOG.md) distinguishes released
work from unmerged development changes.

## Example conventions

`scarp` code fences contain complete runnable programs for the stated version.
An accompanying output block shows exact text, with a final line break after
each print. Fences marked `scarp-invalid` intentionally fail with the discussed
diagnostic; they are examples of repairs, not runnable success cases.

Shell commands assume the described checkout and working directory. Complete
Rust programs in the embedding guide target the development API. Source examples
under `examples/` have `.stdout` companions and are checked by the test suite.

If a guide disagrees with an executable, first confirm the version and branch.
For a documentation bug, include the page, command, source, actual result, and
version so the mismatch can be reproduced.
