# Freestanding C11 backend

Status: implemented on `scarp/freestanding-c-backend`, dependent on the integer
bit-operation branch. Experimental, unreleased, and outside main.

## Supported computation unit

`scarp compile --emit c file.srp -o program.c` writes a complete C11 translation
unit to a fresh file without invoking a C compiler. It uses
the normal parser/resolver and never executes the source while emitting. Integers,
booleans, declarations, assignment, scopes, conditions, loops, loop control, bit
operations, and direct string-literal prints are supported. All branches must fit
the subset. Floats, vectors, built-in calls, and general string expressions use
E0027 rather than being dropped or silently reinterpreted.

The generated source uses exact-width integer headers and fixed workspace arrays.
It does not explicitly call a heap allocator, stdio, an operating-system service,
or the Rust runtime. Expressions lower iteratively; signed overflow, zero divisors,
and invalid shift counts are checked before performing dangerous C operations.
Bit conversion and arithmetic right shift avoid implementation-defined signed
conversions/shifts. The interpreter's source spans and logical execution/print
budgets are preserved.

## Host interface

Include the generated `program.c` in one C translation unit. Allocate a
`struct scarp_workspace` according to its generated definition, usually in
caller-owned static or other suitably aligned storage. No execution-sized C
arrays are placed on the function call stack. The unit exports:

```c
uint32_t scarp_validate(struct scarp_workspace *, uint64_t steps, size_t prints);
uint32_t scarp_run(struct scarp_workspace *, const struct scarp_host *,
                   uint64_t steps, size_t prints);
uint32_t scarp_run_io(struct scarp_workspace *, const struct scarp_io *,
                      uint64_t steps, size_t prints);
```

A host contains `context` and `write(context, const uint8_t *, size_t)`. The
callback consumes/copies bytes before returning, returns zero on success, and
can connect to a kernel's own output driver. The hosted example instead uses
stdio. Required destinations are checked after the full selected path validates
and before replay. Zero-byte raw writes need no destination. Zero status means
success; source codes are their numeric diagnostic suffixes. 65535 means a host
or output failure. `error_code`, `error_start`, and `error_end` describe the result
in the workspace. Slot/temp fields are internal working storage, not a data ABI.

`scarp_io` contains `stdout_host`, `stderr_host`, `context`, and an optional
`flush(context, stream)` callback. Stream 0 means stdout; 1 means stderr.
At a nonempty stream switch, flush the previous stream before writing the next.
Callers own final flushing. `scarp_run` wraps this interface with no stderr host;
required stderr fails with 65535 before any stdout, rather than being redirected
or dropped. `scarp_validate` needs no hosts and formats/writes nothing.
The five [output statements](../output.md) share these rules, retaining the
integer/boolean/direct-literal-string subset. Recompile generated units and
workspace consumers after upgrading; no stable binary layout is promised.

Run first validates without output and then replays from fresh state with the
writer. This preserves empty output on arithmetic/budget failures without a
heap-backed print queue. It costs two computation passes. Output failures may
leave previously written bytes, as with the interpreter. A callback must not
mutate or reenter the same workspace. Separate workspaces allow independent calls;
callback synchronization belongs to the host. Fixed public symbol names currently
support one generated unit per component; modules and a stable ABI are future work.

## Resource bounds and reproducible object builds

Emission allows 256 bindings, 4,096 lowered operations, 1,024 expression nodes
per expression, 16,384 combined expression nodes/additional argument separators,
and 65,536 literal-text bytes.
Exceeded bounds produce E0027 before any source is written. Workspace dimensions
are compile-time constants; callers can inspect `sizeof(struct scarp_workspace)`.

With Rust and LLVM tools installed, PowerShell can create verified ELF objects:

```powershell
./scripts/build-freestanding.ps1 -OutputDirectory dist/object-x64
./scripts/build-freestanding.ps1 -Target aarch64-unknown-none-elf -OutputDirectory dist/object-arm
```

The script refuses existing output directories, records its target/compiler/flags,
and rejects unresolved symbols. The checked preset uses `-ffreestanding`,
`-fno-builtin`, `-fno-stack-protector`, and `-O2`. This is an object build, not a
bootable kernel. A target compiler may introduce memory/compiler-runtime helpers;
inspect every object and supply platform support where needed. Clang describes
these obligations in its [freestanding build guidance](https://clang.llvm.org/docs/UsersManual.html#freestanding-builds).

For a hosted smoke run, use the generated `program.c` with
`examples/hosted_output.c`, adding its directory to the C include search path.
The adapter is separate from the freestanding unit and owns stdout behavior.
It deliberately retains the single-host API. Use the dual-host API or the native
`scarp compile` executable driver for stderr; the latter supplies both streams.

## Verification and remaining systems work

Tests compare native output, diagnostics, source spans, and exact work budgets
with the interpreter at -O0 and -O2. They cover integer extremes, division,
overflow, shifts, short-circuiting, scoped mutation, loops, UTF-8/NUL bytes,
repeated execution, and output failure. CI also enables undefined-behavior
sanitization and checks x86-64/AArch64 ELF objects for unresolved symbols.

This provides compilable computation components for low-level hosts. Bootloader
protocols, startup/interrupt assembly, raw pointers, volatile memory/device access,
unsigned scalar types, structure layout, exported language functions, linker
scripts, and a kernel runtime remain separate milestones. No boot or hardware
execution is claimed by the object checks, and no release/tag is created.
