# Local application projects

Status: experimental implementation for review on `scarp/project-workflow`,
dependent on the canonical CLI in PR #24. Main and published releases are unchanged.

## Manifest profile

The repository has no existing Scarp project manifest. This proposal introduces
`scarp.toml` for one local application entrypoint, without dependencies, tasks,
workspace discovery, or network access:

```toml
format = 1

[project]
name = "hello-world"
entry = "src/main.srp"
```

This is a deliberately restricted TOML profile, not a general TOML parser. It
accepts the exact format/table/keys above, basic or literal single-line strings,
basic-string escapes including Unicode scalars, comments, and LF/CRLF. Unknown
fields, duplicate keys/tables, other format versions, multiline strings, arrays,
and other TOML features are errors. The manifest is UTF-8 and at most 16 KiB.

Names contain 1–64 ASCII letters/digits/hyphens/underscores, beginning with a
letter, and exclude reserved Windows device names. Entrypoints use relative,
forward-slash paths with no empty, dot, parent, drive, or reserved components.
Unicode path components are allowed. Resolved entrypoints must be regular files
inside the canonical project root. Linked manifests and scaffolding directories
are rejected. Existing source links may resolve within the same project.

## Canonical commands

`scarp create <directory>` creates a new application directory. `--name <name>`
overrides the basename used for the manifest, allowing Unicode/space-containing
directory paths. The parent must exist; existing destinations are refused.

`scarp init` initializes the current directory. It preserves unrelated files and
allows an ordinary existing `src` directory. Existing `scarp.toml`, `src/main.srp`,
or a linked/non-directory `src` causes a conflict. Filesystem roots are refused.
Both commands support `--template app` and read-only `--dry-run`; library
templates, force/overwrite modes, and the optional `new` alias are unsupported.

The generated source is the real, executable `println("Hello from Scarp!");`
program. Files use exclusive creation, with the manifest written last. A failed
creation makes a best effort to remove only its own unchanged files and empty
directories; unrelated content is preserved. No recursive removal, Git setup,
credential changes, dependency installation, or external commands occur.

`scarp launch` loads only the current directory's manifest and executes its
entrypoint. It is project-only and rejects file arguments. Runtime budget flags
match `exec`. Program arguments remain unsupported and are rejected explicitly.

Without an explicit file, `exec`, static `verify`, and `compile` use that same
entrypoint. `compile` still requires a fresh explicit output path and retains
its existing language/backend limits. Explicit file commands do not load a
manifest. No parent-directory search or implicit source-file guessing occurs.
Inspection continues to require an explicit file.

## Observable behavior

Creation reports the actual created project. Dry runs report the planned files
and perform the same conflict/path checks without writing. Launch/exec keep
stdout for program output and retain source diagnostics and atomic semantic
failures. Verify stays static and silent; compilation never executes the result.
Malformed options return 2; manifest/filesystem/source failures return 1.

Project loading is read-only. Generated paths are checked again when applying a
plan, and exclusive file creation prevents normal concurrent invocations from
overwriting each other. This is not a sandbox for untrusted concurrent filesystem
mutation. No commands execute arbitrary manifest-defined shell tasks.

Verification covers end-to-end creation/initialization, dry runs, conflicts,
manifest syntax and version errors, portable paths, links, entrypoint confinement,
file-vs-project precedence, budgeted launch, static checks, native/C output,
concurrent creation, and extracted package use. A complete TOML implementation,
modules/library templates, program arguments, and project build profiles remain
future work.
