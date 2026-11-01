# Scarp documentation

Development declarations use `bind` and `var`; see the [migration guide](syntax-migration.md). Published tags retain their original syntax.

Choose a guide by the task you want to complete. Current tutorials target the
**0.0.5-alpha.1 prerelease**, adding [source modules](modules.md)
to the finite floats, numerical vectors/tiles, typed functions, contracts, loops
and output streams already implemented. The latest published prerelease is
[0.0.5-alpha.1](releases/v0.0.5-alpha.1-validation.md), with verified native
packages and imports. [Preparation notes](releases/v0.0.5-alpha.1.md) preserve the
original candidate contract. [Repository migration](migrations/scarp-lang-Scarp.md)
records the verified maintainer transfer to scarp-lang/scarp. Historical release
notes describe fixed snapshots. Tensors, native
backends, a package manager and website deployment remain separate.

## Guides and what each adds

| Document | Use it to |
| --- | --- |
| [Getting started](getting-started.md) | Select a version, build the intended checkout, and verify your executable |
| [Your first program](tutorials/first-program.md) | Learn the file-to-output workflow, comments, escapes, and a first syntax repair |
| [Source modules](modules.md) | Organize files, choose public/private declarations and understand loading limits |
| [Import tutorial](tutorials/modules.md) | Run a three-file numerical program and repair private-member/cycle errors |
| [Finite floats](floats.md) | Use the alpha's binary64 scalars, numeric diagnostics, and function integration |
| [Numerical vectors](vectors.md) | Use immutable numerical data, pure kernels, typed vector calls and bounded allocation |
| [Your first numerical program](tutorials/numerical-program.md) | Build a small typed numerical calculation and compare probabilities safely |
| [Numerical errors](reference/numerical-errors.md) | Repair type, shape, index, domain, overflow and quota failures |
| [Functions](functions.md) | Learn typed calls, defaults, contracts, and bounded invocation resources |
| [Output](output.md) | Choose print/write, control newlines, and route explicit stderr |
| [Variables and mutation](tutorials/variables.md) | Introduce names, copy values, choose mutability, and avoid forward references |
| [Scopes and shadowing](tutorials/scopes.md) | Predict which declaration a name uses and what survives a block |
| [Decisions](tutorials/decisions.md) | Select an if/elif/else body and distinguish static checks from executed work |
| [Loops and budgets](proposals/0006-bounded-loops.md) | Repeat computations with scoped state, loop control, and per-call bounds |
| [Iteration](iteration.md) | Choose while/repeat, understand count snapshots, immutable indices, and nested control |
| [Execution budgets](budgets.md) | Set work/print limits, interpret failures, and embed bounded execution |
| [Command-line reference](reference/cli.md) | Look up commands, version availability, paths, streams, and exit codes |
| [Diagnostic reference](reference/diagnostics.md) | Interpret all error codes and repair common failures |
| [Values and operators](reference/values.md) | Check precedence, numeric boundaries, assignment rules, and string limits |
| [Rust embedding](embedding.md) | Select a library stage, capture output, reuse compiled programs, and render errors |
| [Testing changes](testing.md) | Choose relevant tests, add exact-output examples, and validate documentation |
| [Troubleshooting](troubleshooting.md) | Separate installation, path, encoding, version, language, and package failures |
| [Compatibility](compatibility.md) | Compare released/development behavior and migrate an embedding host |
| [Performance](performance.md) | Reproduce stage benchmarks and assess measured runtime improvements |

## Alpha.4 numerical and runtime prerelease

Read [tiles and linear algebra](tiles.md) for immutable two-axis values,
matrix products, arithmetic, shape rules, costs and seven executable examples.
[Automatic value retirement](memory-lifetimes.md) covers blocks, loops,
function completion and shared immutable snapshots.

The release also integrates windows/stitching, log-space kernels, selection,
and categorical loss/gradient helpers developed since alpha.3. The
[publication record](releases/v0.0.4-alpha.4-validation.md) identifies the approved
source, passing CI and verified public packages. Published alpha.3 assets remain unchanged.

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
[contribution guide](../CONTRIBUTING.md). The variable milestone is integrated
into main; further work uses separate branches and PRs.

## Exact rules and project records

The tutorials teach complete tasks. The existing [language reference](language.md)
and [variable reference](variables.md) spell out the implemented semantics.
[Architecture notes](architecture.md) explain the module and execution boundaries.

Design records cover [integer expressions](proposals/0001-integer-expressions.md),
[variables](proposals/0002-variables.md), and
[string values](proposals/0003-string-bindings.md). Read each record's status;
a proposed capability is not implemented merely because it appears in a roadmap.
The [boolean design](proposals/0004-boolean-expressions.md) records implemented
comparison, precedence, and short-circuit rules.
The [conditional design](proposals/0005-conditional-statements.md) explains scoped
bodies, flat branch execution, and checks in unselected paths.

Published behavior is recorded in the [v0.0.1 notes](releases/v0.0.1.md).
The [v0.0.2 notes](releases/v0.0.2.md) describe the variable prerelease.
The [v0.0.3-beta.1 notes](releases/v0.0.3-beta.1.md) describe the iteration release;
the [v0.0.3-beta.2 notes](releases/v0.0.3-beta.2.md) cover declaration migration,
and output streams. The [v0.0.3-beta.3 notes](releases/v0.0.3-beta.3.md) cover
functions and CLI resource controls. Current downloads are recorded in the
[alpha.4 publication record](releases/v0.0.4-alpha.4-validation.md). Use [compatibility](compatibility.md)
before running a current tutorial with an older installed executable.
[Release preparation](releasing.md) describes the separate packaging and
publication process. The root [changelog](../CHANGELOG.md) distinguishes released
work from unmerged development changes.

## Example conventions

`scarp` code fences contain complete runnable programs for the stated version,
or explicitly named files in a documented multi-file example.
An accompanying output block shows exact text, with a final line break after
each print. Fences marked `scarp-invalid` intentionally fail with the discussed
diagnostic; they are examples of repairs, not runnable success cases.

Shell commands assume the described checkout and working directory. Complete
Rust programs in the embedding guide target the development API. Source examples
under `examples/` have `.stdout` companions and are checked by the test suite.

If a guide disagrees with an executable, first confirm the version and branch.
For a documentation bug, include the page, command, source, actual result, and
version so the mismatch can be reproduced.
