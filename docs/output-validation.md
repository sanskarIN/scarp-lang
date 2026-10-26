# Output feature validation

Local review: 2026-10-26, Windows x64. Branch: `scarp/output-statements`.
Base: `scarp/bind-var-syntax` at
`3449af94910af2b25991a63c42254f99eb5c9667` (open PR #25).
Implementation/package snapshot:
`532cff32605115cf2854c567c5e6c600c44f18bb`. This evidence report was appended
afterward; hosted checks on the output PR validate its latest commit separately.

The supplied brief described older main APIs. This change preserves current
`bind`/`var`, numeric types and kernels, comma tokens, and the canonical CLI
(`exec`, `compile`, `verify`, `inspect`, `guide`, `version`). It does not restore
retired CLI aliases or discard any development prerequisites.

## Delivery and compatibility

| Phase | Result |
| --- | --- |
| A | Implemented: preferred `print`, permanent `println`, raw `write`, zero/one/many arguments, single-space joining, trailing comma, LF bytes, reserved names, and required semicolons. |
| B | Implemented: `eprint` / `ewrite`, explicit dual writers in Rust and CLI, ordered replay, boundary flushing, safe single-writer rejection, and additive C/native dual hosts. |
| C | Design only: interpolation, format specifications/functions, macros, and logging are not implemented. Plain braces remain literal. |

See the [output contract](output.md) and
[design decision](proposals/0014-output-statements.md).

All output forms lower to one resolved operation. Preparation evaluates selected
arguments left to right and captures one flat ordered event list without a
concatenated output string. Literal values retain a borrowed fast path. Static,
arithmetic, and budget failures write neither program stream. Writer/flush
failures during replay propagate and may leave earlier bytes.

The existing Rust single-writer APIs and single-argument `Statement::Println`
shape remain. `Statement::Output` / `OutputTarget` are additive; exhaustive AST
matches need another arm. Add `run_with_io`, `execute_with_io`, and their
`*_with_io_and_limits` variants for stderr. A single-writer call returns
`RunError::Output(Unsupported)` before stdout if selected nonempty stderr is
required. Empty raw stderr and unselected branches need no second writer.

`print`, `write`, `eprint`, and `ewrite` become reserved variable names; older
programs using those names need renaming. Existing example sources and `.stdout`
fixtures retain their exact bytes. The new `output_basics.stdout` deliberately
ends with `Tail` and no LF. Only the newly added example uses the new spellings.

Generated C retains `scarp_host`, `scarp_run`, and `scarp_validate`; it adds
`scarp_io` / `scarp_run_io`. Required destinations are checked after validation
and before replay. Workspace layout is program-specific: rebuild generated units
and consumers. Backend types remain integer/boolean/direct-string-literal only;
other types still produce E0027. The hosted adapter supplies both real streams
and flushes them; Rust embedding callers retain final-flush responsibility.

## Exact checks and results

Tools: Rust 1.99.1 MSVC, Clang 16.0.6, GCC 13.2.0, Python 3.14, PowerShell 7.
The complete check ran with C compilation and both freestanding targets required:

```powershell
$env:SCARP_CC = 'C:\MinGW\bin\clang.exe'
$env:SCARP_REQUIRE_C = '1'
$env:SCARP_FREESTANDING = '1'
./scripts/check.ps1 -Offline
```

Result: **580 passed test executions** (290 debug, 290 release), **0 failed,
0 ignored**, across 48 test-target runs. This includes 21 new output tests,
2 new CLI stream tests, 2 new native CLI tests, 2 new native dual-host tests,
and one new Rustdoc example in each applicable mode. Formatting, standalone
benchmark formatting, Clippy, builds, rustdoc, all 14 examples, static verification,
AST inspection, and output token inspection passed.

The script executed these Cargo checks with offline mode enabled:

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo test --locked --release
cargo build --locked
cargo doc --locked --no-deps
cargo run --locked -- exec examples/output_basics.srp
cargo run --locked -- verify examples/output_basics.srp
cargo run --locked -- inspect tokens examples/output_basics.srp
```

| Additional command/check | Actual result |
| --- | --- |
| `cargo test --locked --offline --test c_backend --test compile_cli` with GCC | 18 passed, 0 failed/ignored. Clang-only freestanding gates were unset for this GCC run; the complete Clang run exercised both targets. |
| C differential matrices, `-O0` and `-O2` | 28 stdout cases and 11 dual-stream cases agree with Rust bytes, error codes, spans, and budgets; callback/missing-sink/flush failures and repeated workspace reset also pass. |
| `python dist/docs-qa/validate_docs.py` | 12 guides: 14 valid programs, 11 invalid programs, 4 compiled Rust embedding programs, 7 published-v0.0.1 compatibility programs, and 6 CLI assertions passed; relative links passed. |
| `python dist/docs-qa/check_decisions.py` | All 3 tutorial programs and relative links passed. |
| `python dist/validate_output_docs.py` | Both new output-guide examples and the README's preferred greeting matched exact stdout/stderr; all 3 also passed static verification. |
| `cargo bench --locked --offline --bench runtime -- --quick` | All 6 workloads verified output and produced measurements; no timing threshold or performance-parity claim. |
| `./scripts/package.ps1 -Target x86_64-pc-windows-msvc -OutputDirectory dist/output-streams-package` | Fresh archive extracted and checked: all 14 raw-byte example fixtures, separate stderr, no-final-LF output, static checks, errors, C emission, AST, retired CLI names, and guidance passed. |
| PowerShell parser, `sh -n scripts/check.sh`, `git diff --check` | Passed. The full shell suite was not duplicated locally; the PowerShell entrypoint ran all checks. |

The local ZIP for the implementation snapshot has SHA-256
`5f761f3a9884ed724a54b421711a7b1e3f14e763e460fb6d44a8c3e1cffc3d9d`.
It is an inspection artifact and was not published as a release. Packaging
requires PowerShell 7, as documented by the existing release guide. Its process
helper drains both byte streams concurrently, preserving LF, Unicode, NUL, and
the absence of a final newline.

Local review tools under `dist/` are not installed language commands. Cargo
regressions, exact example fixtures, package checks, and CI are tracked. Initial
targeted runs exposed obsolete negative tests for newly valid output syntax and
a strict Clang warning in the new callback test harness; both were corrected
before the final passing runs.

## Scope and integration gates

Windows checks ran locally. Linux/macOS, Rust 1.74, and undefined-behavior checks
are separate hosted-CI gates; use the PR's exact-head checks for their status.
I/O replay is not globally atomic across streams, and writer failures cannot be
rolled back. Output aliases share semantics; no other syntax aliases were added.

Keep the output PR stacked on #25 and leave both unmerged until authorized.
Follow-up work should specify opt-in interpolation and its evaluation/escape
rules before adding it. Project-workflow completion remains separate from output.
No language version bump, main merge, tag, or release publication is authorized.
Main remains `4ddace1c4753bca8889271deb7580290c8707158`; historical tags, release
notes/packages, dependencies, license, publisher identity, and user data are
unchanged. All new commits use `sanskarin@outlook.in`.
