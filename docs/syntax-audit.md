# Binding syntax implementation and validation

Reviewed locally on 2026-10-26, Windows x64. Feature branch:
`scarp/bind-var-syntax`. Base: `scarp/canonical-cli` at
`984c59e1cc1c40626949633f8ac165fd2ae5ca1d` (open PR #24). Its Rust and package
workflow runs `40262309618` and `40262309564` were rechecked as successful.
The implementation/tooling validation snapshot is
`ea9bab93f4228cfefa5da0b2c29c333e81e0eb0b`; this report was appended afterward.
Hosted checks on the syntax PR validate its latest commit separately.

The earlier application-project work is preserved on `development` at
`05b4c40f63567267f5d9662ba8108d403d682f88`. That checkpoint compiled before
preservation, but its project-specific integration tests and documentation are
unfinished. It is not part of this syntax PR and is not ready to merge.

## Decision and observable behavior

The [syntax decision](proposals/0013-binding-syntax.md) selects `bind` for
immutable bindings and `var` for mutable variables. See the
[versioned migration table](compatibility.md#declaration-syntax-migration).

```scarp
bind width = 6;
var area = width * 7;
area += 1;
println(area); // 43
```

| Input or interface | Verified result |
| --- | --- |
| `bind x = 1; x = 2;` | E0013, related declaration span, help naming `var`. |
| `var x = 1; x += 2; println(x);` | Prints `3`, using checked same-type mutation. |
| `let x = 1;` | E0005, help to replace `let` with `bind`. |
| `let mut x = 1;` | E0005, help to replace `let mut` with `var`. |
| `bind mut x = 1;` / `var mut x = 1;` | E0004, repair using `var name = expression;`. |
| `var x;` | E0004, required-initializer help. |
| Reserved `bind`, `var`, `let`, `mut` names | E0012; ordinary `binder`/`variable` names work. |
| `exec` / `verify` / `inspect ast` | Shared new parser; legacy declarations fail consistently. |
| `inspect tokens` | Generic identifier tokens, including legacy words; lexing intentionally does not enforce statement grammar. |
| `verify` on `var x = 1 / 0;` | Success without evaluating arithmetic; execution returns E0008. |
| C source, native executable, freestanding object | New declarations work within the existing integer/boolean backend subset. |
| Rust syntax AST | Public `Statement::Declaration` replaces `Statement::Let`; fields retain their meaning. |

Strings and comments retain declaration words as text. Tests check Unicode
trivia, CRLF, BOM, keyword/reference spans, shadowing initializers, snapshots,
all compound assignments, duplicate/self/forward references, execution budgets,
fresh repeated storage, and failure before output or artifact publication.

## Findings, limitations, and deferred work

| Classification | Finding and disposition |
| --- | --- |
| Fixed | Old declaration grammar and mutability advice now use `bind`/`var`, with explicit legacy migration errors and no aliases. |
| Fixed | Syntax inspection and Rust AST consumers now use a neutral declaration variant. |
| Fixed | Current Scarp fixtures, Rustdoc source strings, examples, benchmark workloads, tutorials, reference grammar, and package smoke checks agree. |
| Fixed | Variable documentation now distinguishes published v0.0.2 from unreleased v0.0.3-dev; README no longer duplicates functions in its roadmap. |
| Verified unchanged | Scope, type inference, arithmetic, snapshots, output prevalidation, budgets, and existing C backend restrictions. All `.stdout` companions retain their bytes. |
| Observed | PRs #16–#20 and #22–#24 remain open, in that dependency order. This syntax PR targets #24's branch rather than bypassing its prerequisites. |
| Observed | The `development` project-workflow checkpoint needs tests, inventory updates, and integration with the new declaration syntax. |
| Breaking change | v0.0.2 source needs declaration migration; old variables named `bind` or `var` need renaming; embedding hosts matching `Statement::Let` must update. |
| Deferred | Functions, formatter, project discovery/scaffolding completion, modules, richer native types, package registry, editor services, raw memory, and kernel startup. |
| Validation boundary | Windows checks ran locally. Linux/macOS, minimum Rust 1.74, and C sanitizers are hosted-CI gates; local success alone does not establish them. |

Identifier spelling remains ASCII, string escapes remain the existing three,
and standard control-flow/operator syntax is unchanged. The C backend does not
compile arbitrary floats, vectors, or string variables. This work makes no
performance-parity or keyword-uniqueness claims.

## Reproduction and results

Local tools: Rust 1.99.1 MSVC, Clang 16.0.6, GCC 13.2.0, Python 3.14, and
PowerShell. The full check required C compilation and both freestanding targets:

```powershell
$env:SCARP_CC = 'C:\MinGW\bin\clang.exe'
$env:SCARP_REQUIRE_C = '1'
$env:SCARP_FREESTANDING = '1'
./scripts/check.ps1 -Offline
```

That script ran `cargo fmt --all -- --check`, standalone benchmark `rustfmt`,
`cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked`,
`cargo test --locked --release`, `cargo build --locked`, and
`cargo doc --locked --no-deps`, with offline mode enabled. It also ran all shipped
examples via `cargo run --locked -- exec`, scopes via `verify` and `inspect ast`,
and token inspection. Final results: **524 passed test executions** (262 per
mode), **0 failed, 0 ignored**. Twelve new syntax tests run in each mode. Initial
migration runs exposed three old keyword-offset assertions; their corrected
expectations and the added byte-span cases pass. OS-specific test branches
remain conditional; hosted CI covers the other platforms.

| Additional command/check | Result |
| --- | --- |
| `python dist/docs-qa/validate_docs.py` | 12 selected guides: 14 valid programs, 11 invalid programs, 3 compiled Rust embedding programs, 7 published-v0.0.1 compatibility programs, and 6 CLI assertions passed; relative links passed. |
| `python dist/docs-qa/check_decisions.py` | All 3 decisions tutorial programs and relative links passed. |
| `cargo bench --locked --offline --bench runtime -- --quick` | All 6 workloads verified output and emitted measurements; no timing threshold asserted. |
| `cargo test --locked --offline --test c_backend emitted_c_matches_interpreter_output_errors_spans_and_budgets` with GCC | 1 passed; the 20-case differential matrix matched at `-O0` and `-O2` (3 other tests filtered). |
| `cargo test --locked --offline --test compile_cli native_compile_preserves_exact_output` with GCC | 1 passed, 9 filtered. |
| `./scripts/package.ps1 -Target x86_64-pc-windows-msvc -OutputDirectory dist/binding-syntax-package` | Fresh Windows package extracted and checked: examples, errors, static verification, C output, declaration AST, legacy syntax hints, retired CLI commands, and default guidance passed. |
| `./scripts/compare-runtime.ps1 -Baseline 984c59e1cc1c40626949633f8ac165fd2ae5ca1d -Candidate ea9bab93f4228cfefa5da0b2c29c333e81e0eb0b -OutputDirectory dist/binding-runtime-comparison` | All 4 equivalent workloads verified output on both snapshots; 12 measurement rows produced across the syntax change. |
| Published-v0.0.2 archive compatibility probe | Archive checksum verified; 2 old declaration programs printed `42` on v0.0.2 and produced E0005 migration errors on development. |
| `sh -n scripts/check.sh`; PowerShell parser on all scripts; `git diff --check` | Passed. The full shell check was not duplicated locally because the PowerShell check already ran the complete suite. |

The `dist/docs-qa` scripts and historical archive probe are local review tools,
not installed Scarp commands. Cargo regression tests and package checks are
tracked and reproducible. The additional GCC runs had the Clang-only
`SCARP_FREESTANDING` flag unset. Local sanitizer and minimum-toolchain runs were
not performed; their configured CI jobs must pass.

The local Windows ZIP (validation snapshot above) has SHA-256
`070014aed938a9d581ed245bbbcaef0448cc9918113c56d373e8d752c4f4f66c`.
It was built for inspection only and was not published.

## Integration and release gates

Review the syntax migration and public AST break, then review/integrate the
existing dependency stack in order only with authorization. Complete the
project-workflow checkpoint on another branch before presenting it as finished.
A future release needs successful CI for its exact head, fresh platform package
checks, versioned migration notes, and explicit permission to merge, tag, and
publish. This task authorizes none of those release actions.

Remote `main` remains `4ddace1c4753bca8889271deb7580290c8707158`; v0.0.1 and
v0.0.2 tags, published packages, license, publisher identity, and user data were
not changed. All new commits use `sanskarin@outlook.in`.
