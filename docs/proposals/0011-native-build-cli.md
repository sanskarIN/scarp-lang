# Native build commands

Status: implemented on `scarp/canonical-cli`, dependent on the freestanding
C backend in PR #20. Unreleased and outside main; v0.0.2 is unchanged.

## Commands and language roles

The compiler, interpreter, and `scarp` CLI remain Rust programs. This first
native path emits C11 and invokes a Clang/GCC-style compiler. Scarp users write
Scarp source; C++ and assembly are not required to use these commands. A future
direct machine-code backend could replace the C step. Kernel startup, interrupt
entry, and CPU instructions may eventually need small target-specific assembly
adapters; none are implemented or required for this milestone.

Install the CLI from this branch with `cargo install --path . --locked`.
`exec`, `verify`, and `inspect` use the interpreter/frontend and need no C
compiler. `compile --emit c` writes source without invoking a compiler.

```sh
scarp exec examples/bit_fields.srp
scarp verify examples/bit_fields.srp
scarp inspect tokens examples/bit_fields.srp
scarp compile --emit c examples/bit_fields.srp -o program.c
```

`compile` produces an artifact without executing the result. It requires a
source path and a fresh `-o`/`--output` path. The parent directory
must already exist. Options may appear before or after the source path:

```sh
# Linux/macOS
scarp compile examples/bit_fields.srp -o ./bit-fields
./bit-fields

# Windows
scarp compile examples/bit_fields.srp -o ./bit-fields.exe
./bit-fields.exe
```

Both binaries print `3`, `10`, `-9223372036854775808`, and `-1` on separate
lines. The generated hosted entry point supplies stdout and fixed workspace.
Windows output and diagnostics use binary stream mode for consistent LF bytes.

## Compiler selection and build modes

`--cc <executable>` selects one compiler executable, not a shell command or a
list of flags. Otherwise `SCARP_CC` wins, followed by `clang` on Windows or for
explicit cross targets, and `cc` for native Unix builds. Paths with spaces and
native paths are supported, including non-Unicode bytes on Linux filesystems
that permit them. The checked macOS filesystem rejects non-Unicode names.
There is no shell interpolation and
`CFLAGS` is not consumed. MSVC's `cl.exe` driver syntax is not supported.

The default `--emit exe` uses C11, `-O2`, `-Wall`, `-Wextra`, and `-Werror`.
It links against the host's C runtime. `--target` is rejected for executables;
cross-linking hosted programs and choosing platform SDKs remain future work.

`--emit obj` adds `-ffreestanding -fno-builtin -fno-stack-protector -c` and emits
the computation unit without a hosted entry point. It uses the native target
unless one of the two checked Clang targets is supplied:

```sh
scarp compile examples/bit_fields.srp --emit obj --target x86_64-unknown-none-elf -o ./bits-x64.o --cc clang
scarp compile examples/bit_fields.srp --emit obj --target aarch64-unknown-none-elf -o ./bits-arm.o --cc clang
```

Objects retain the caller-owned workspace/output interface from the
[freestanding guide](0010-freestanding-c.md). The caller supplies execution
budgets at runtime. Inspect unresolved symbols when embedding; these objects
are not bootable kernels or complete operating systems.

## Semantics, budgets, and errors

Only the backend's integer/boolean subset and direct literal prints compile.
Unsupported values produce E0027 before a compiler starts. Source emission
does not evaluate arithmetic: `println(1/0);` can compile, then fails when run.
Use `verify` for static checks or `exec` to evaluate the selected source path.

`--emit c` writes the same freestanding unit used by the backend to an explicit
file. It is silent on success, needs no C compiler, and refuses existing outputs.
Explicit compiler, target, and runtime-budget options are rejected for this mode
because they would not affect C emission.

Hosted executables default to 1,000,000 logical steps and 100,000 selected
prints. Build options `--max-steps <decimal>` and `--max-prints <decimal>` bake
different limits into the executable. They do not limit compiler work and are
rejected for object and C-source outputs. Zero budgets are valid. Counts must fit u64 and the
host's usize respectively. A generated program validates before output, so
arithmetic and budget failures leave stdout empty. Diagnostics report the
original E-code and source byte span. Host/output errors may leave earlier bytes.

Build exit codes are 0 for success, 1 for source/tool/filesystem failure, and 2
for malformed arguments. The compiled executable returns 0 for success and 1
for a runtime/output failure. Successful builds are silent; the artifact is
never automatically executed. `compile --help` needs no input. Only the canonical
`compile` command is accepted; former aliases return usage errors.

## Output preservation and verification

Existing destinations, including dangling symlinks, are refused. Each build
owns a fresh staging directory beneath the output parent and removes it when
done. The compiler writes there before publication; compilation failures create
no destination. Hard links publish atomically where supported. Other filesystems
reserve the destination with exclusive creation before copying, preserve its
permissions, and remove it on copy failure. A partial file can be visible during
that fallback copy; consumers must wait for a successful build exit.
Abrupt process termination can leave staging files; process supervision and a
managed cleanup command remain future work.

Tests cover native output, UTF-8/NUL bytes, canonical names, spaces, source diagnostics,
runtime spans, zero/full-width budgets, missing compilers/parents, existing and
racing outputs, cleanup, failed compilers, native paths, and both checked ELF
architectures. These commands add no Rust dependencies and do not alter main,
tags, or published releases.
