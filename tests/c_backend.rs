use scarp::{c_backend, diagnostics::DiagnosticKind, ExecutionLimits, RunError};
use std::{
    ffi::OsString,
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "scarp-c-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn compiler() -> Option<OsString> {
    let cc = std::env::var_os("SCARP_CC").unwrap_or_else(|| {
        if cfg!(windows) {
            "clang".into()
        } else {
            "cc".into()
        }
    });
    if Command::new(&cc).arg("--version").output().is_ok() {
        Some(cc)
    } else {
        assert!(
            std::env::var_os("SCARP_REQUIRE_C").is_none(),
            "required C compiler unavailable"
        );
        eprintln!("C execution checks skipped: no compiler");
        None
    }
}

fn native(
    cc: &OsString,
    source: &str,
    limits: ExecutionLimits,
    opt: &str,
    fail: bool,
    repeats: usize,
) -> String {
    let directory = Directory::new();
    fs::write(
        directory.0.join("program.c"),
        c_backend::emit_c(source).unwrap(),
    )
    .unwrap();
    let harness = format!(
        r#"
#include "program.c"
#include <stdio.h>
static uint8_t bytes[65536]; static size_t used;
static int capture(void *context,const uint8_t *data,size_t count) {{
 size_t i; (void)context; if({fail} || count>sizeof bytes-used) return 1;
 for(i=0;i<count;++i) {{ bytes[used++]=data[i]; }} return 0;
}}
int main(void) {{
 static struct scarp_workspace workspace; struct scarp_host host; uint32_t code=0; size_t i;
 host.context=0; host.write=capture;
 for(i=0;i<{repeats};++i) {{code=scarp_run(&workspace,&host,UINT64_C({steps}),{prints});if(code) break;}}
 printf("%u:%llu:%llu:",(unsigned)code,(unsigned long long)workspace.error_start,(unsigned long long)workspace.error_end);
 for(i=0;i<used;++i) {{ printf("%02x",(unsigned)bytes[i]); }} return 0;
}}
"#,
        fail = usize::from(fail),
        steps = limits.max_steps,
        prints = limits.max_prints
    );
    fs::write(directory.0.join("host.c"), harness).unwrap();
    let executable = directory
        .0
        .join(if cfg!(windows) { "host.exe" } else { "host" });
    let mut command = Command::new(cc);
    command
        .current_dir(&directory.0)
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", opt]);
    if std::env::var_os("SCARP_C_SANITIZE").is_some() {
        command.args(["-fsanitize=undefined", "-fno-sanitize-recover=all"]);
    }
    let built = command
        .arg("host.c")
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let result = Command::new(executable).output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).unwrap()
}
fn reference(source: &str, limits: ExecutionLimits, repeats: usize) -> String {
    let mut bytes = Vec::new();
    let mut code = 0;
    let (mut start, mut end) = (0, 0);
    for _ in 0..repeats {
        match scarp::run_with_limits(source, &mut bytes, limits) {
            Ok(()) => {}
            Err(RunError::Diagnostic(error)) => {
                code = error.kind.code()[1..].parse::<u32>().unwrap();
                start = error.span.start;
                end = error.span.end;
                break;
            }
            other => panic!("{other:?}"),
        }
    }
    format!(
        "{code}:{start}:{end}:{}",
        bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}

fn native_batch(
    cc: &OsString,
    cases: &[(&str, ExecutionLimits)],
    optimization: &str,
) -> Vec<String> {
    let directory = Directory::new();
    let mut source = String::from("#include <stdio.h>\n");
    for (index, (program, _)) in cases.iter().enumerate() {
        source.push_str(
            &c_backend::emit_c(program)
                .unwrap()
                .replace("scarp_", &format!("scarp_case{index}_")),
        );
        source.push_str(
            "\n#undef SCARP_SLOT_COUNT\n#undef SCARP_TEMP_COUNT\n#undef SCARP_OUTPUT_ERROR\n",
        );
    }
    source.push_str("static uint8_t bytes[65536]; static size_t used;\nstatic int capture(void *context,const uint8_t *data,size_t count) {size_t i; (void)context; if(count>sizeof bytes-used) {return 1;} for(i=0;i<count;++i) {bytes[used++]=data[i];} return 0;}\nint main(void) {\n");
    for (index, (_, limits)) in cases.iter().enumerate() {
        source.push_str(&format!("{{static struct scarp_case{index}_workspace w; struct scarp_case{index}_host host; uint32_t code,checked; uint64_t start,end; size_t i; host.context=0;host.write=capture;used=0;checked=scarp_case{index}_validate(&w,UINT64_C({}),{});start=w.error_start;end=w.error_end;code=scarp_case{index}_run(&w,&host,UINT64_C({}),{});if(code!=checked||start!=w.error_start||end!=w.error_end) return 2;printf(\"%u:%llu:%llu:\",(unsigned)code,(unsigned long long)w.error_start,(unsigned long long)w.error_end);for(i=0;i<used;++i)printf(\"%02x\",(unsigned)bytes[i]);printf(\"\\n\");}}\n",limits.max_steps,limits.max_prints,limits.max_steps,limits.max_prints));
    }
    source.push_str("return 0;}\n");
    fs::write(directory.0.join("host.c"), source).unwrap();
    let executable = directory
        .0
        .join(if cfg!(windows) { "host.exe" } else { "host" });
    let mut command = Command::new(cc);
    command.current_dir(&directory.0).args([
        "-std=c11",
        "-Wall",
        "-Wextra",
        "-Werror",
        optimization,
    ]);
    if std::env::var_os("SCARP_C_SANITIZE").is_some() {
        command.args(["-fsanitize=undefined", "-fno-sanitize-recover=all"]);
    }
    let built = command
        .arg("host.c")
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let run = Command::new(executable).output().unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    String::from_utf8(run.stdout)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

#[derive(Debug, PartialEq, Eq)]
struct NativeOutput {
    code: u32,
    start: usize,
    end: usize,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    flushes: String,
}

fn native_io(
    cc: &OsString,
    source: &str,
    limits: ExecutionLimits,
    flags: u32,
    optimization: &str,
) -> NativeOutput {
    let directory = Directory::new();
    fs::write(
        directory.0.join("program.c"),
        c_backend::emit_c(source).unwrap(),
    )
    .unwrap();
    let harness = format!(
        r#"
#include "program.c"
#include <stdio.h>
struct sink {{ uint8_t bytes[4096]; size_t used; unsigned stream; }};
static char flushes[64]; static size_t flushed; static unsigned mode={flags}u;
static int capture(void *context,const uint8_t *bytes,size_t count) {{
 struct sink *s=(struct sink *)context; size_t i;
 if((s->stream==0u && (mode&4u)) || (s->stream==1u && (mode&8u)) || count>sizeof s->bytes-s->used) return 1;
 for(i=0;i<count;++i) {{ s->bytes[s->used++]=bytes[i]; }} return 0;
}}
static int flush_stream(void *context,unsigned stream) {{
 (void)context; if(flushed>=sizeof flushes) return 1;
 flushes[flushed++]=stream?'e':'o'; return (mode&16u)?1:0;
}}
int main(void) {{
 static struct scarp_workspace w; static struct sink out,err;
 struct scarp_host oh,eh; struct scarp_io io; uint32_t code=0; size_t i,n;
 out.stream=0; err.stream=1; oh.context=&out; oh.write=capture; eh.context=&err; eh.write=capture;
 io.stdout_host=&oh; io.stderr_host=(mode&2u)?0:&eh; io.context=0; io.flush=flush_stream;
 for(n=0;n<((mode&32u)?2u:1u);++n) {{
  if(mode&64u) code=scarp_validate(&w,UINT64_C({steps}),{prints});
  else if(mode&1u) code=scarp_run(&w,&oh,UINT64_C({steps}),{prints});
  else code=scarp_run_io(&w,&io,UINT64_C({steps}),{prints});
  if(code) break;
 }}
 printf("%u:%llu:%llu:",(unsigned)code,(unsigned long long)w.error_start,(unsigned long long)w.error_end);
 for(i=0;i<out.used;++i) {{ printf("%02x",(unsigned)out.bytes[i]); }} putchar(':');
 for(i=0;i<err.used;++i) {{ printf("%02x",(unsigned)err.bytes[i]); }} putchar(':');
 for(i=0;i<flushed;++i) {{ putchar(flushes[i]); }} putchar('\n'); return 0;
}}
"#,
        steps = limits.max_steps,
        prints = limits.max_prints
    );
    fs::write(directory.0.join("host.c"), harness).unwrap();
    let executable = directory
        .0
        .join(if cfg!(windows) { "host.exe" } else { "host" });
    let mut build = Command::new(cc);
    build
        .current_dir(&directory.0)
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", optimization]);
    if std::env::var_os("SCARP_C_SANITIZE").is_some() {
        build.args(["-fsanitize=undefined", "-fno-sanitize-recover=all"]);
    }
    let built = build
        .arg("host.c")
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let result = Command::new(executable).output().unwrap();
    assert!(
        result.status.success() && result.stderr.is_empty(),
        "{result:?}"
    );
    let text = String::from_utf8(result.stdout).unwrap();
    let parts: Vec<_> = text.trim().split(':').collect();
    let decode = |text: &str| {
        text.as_bytes()
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect()
    };
    NativeOutput {
        code: parts[0].parse().unwrap(),
        start: parts[1].parse().unwrap(),
        end: parts[2].parse().unwrap(),
        stdout: decode(parts[3]),
        stderr: decode(parts[4]),
        flushes: parts[5].into(),
    }
}

#[test]
fn native_dual_stream_output_matches_rust_bytes_errors_budgets_and_order() {
    let Some(cc) = compiler() else { return };
    let defaults = ExecutionLimits::default();
    let cases = [
        ("write(\"A\"); ewrite(\"B\"); print(1, true,);", defaults),
        ("eprint(); ewrite(); eprint(\"\", \"x\");", defaults),
        (
            "eprint(\"π 🌍\0\", -9223372036854775808); print(\"ok\");",
            defaults,
        ),
        (
            "print(\"before\"); eprint(\"suppress\"); print(1, 1 / 0);",
            defaults,
        ),
        ("eprint(1 / 0, 9223372036854775807 + 1);", defaults),
        ("eprint(9223372036854775807 + 1, 1 / 0);", defaults),
        (
            "if false { eprint(1 / 0); } print(false and 1 / 0 == 0, true);",
            defaults,
        ),
        (
            "var n=0; while n<3 { print(n); eprint(n); n+=1; }",
            defaults,
        ),
        (
            "eprint(\"a\", \"b\", \"c\");",
            ExecutionLimits {
                max_steps: 2,
                max_prints: 1,
            },
        ),
        (
            "eprint(\"a\", \"b\", \"c\");",
            ExecutionLimits {
                max_steps: 3,
                max_prints: 1,
            },
        ),
        (
            "ewrite();",
            ExecutionLimits {
                max_steps: 100,
                max_prints: 0,
            },
        ),
    ];
    for optimization in ["-O0", "-O2"] {
        for (source, limits) in cases {
            let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
            let mut expected = NativeOutput {
                code: 0,
                start: 0,
                end: 0,
                stdout: Vec::new(),
                stderr: Vec::new(),
                flushes: String::new(),
            };
            if let Err(error) =
                scarp::run_with_io_and_limits(source, &mut stdout, &mut stderr, limits)
            {
                let RunError::Diagnostic(error) = error else {
                    panic!("{error}")
                };
                expected.code = error.kind.code()[1..].parse().unwrap();
                expected.start = error.span.start;
                expected.end = error.span.end;
            }
            expected.stdout = stdout;
            expected.stderr = stderr;
            let mut actual = native_io(&cc, source, limits, 0, optimization);
            if source == cases[0].0 {
                assert_eq!(actual.flushes, "oe");
            }
            actual.flushes.clear();
            assert_eq!(actual, expected, "{optimization}: {source}");
        }
    }
}

#[test]
fn native_legacy_host_missing_sinks_and_callback_failures_are_explicit() {
    let Some(cc) = compiler() else { return };
    let limits = ExecutionLimits::default();
    let source = "write(\"A\"); ewrite(\"B\");";
    for flags in [1, 2] {
        let result = native_io(&cc, source, limits, flags, "-O2");
        assert_eq!(result.code, 65535);
        assert!(result.stdout.is_empty() && result.stderr.is_empty());
    }
    for flags in [4, 8, 16] {
        let result = native_io(&cc, source, limits, flags, "-O2");
        assert_eq!(result.code, 65535);
        assert_eq!(
            result.stdout,
            if flags == 4 {
                b"".as_slice()
            } else {
                b"A".as_slice()
            }
        );
        assert!(result.stderr.is_empty());
    }
    let result = native_io(&cc, source, limits, 64 | 4 | 8, "-O2");
    assert_eq!(result.code, 0);
    assert!(result.stdout.is_empty() && result.stderr.is_empty() && result.flushes.is_empty());
    let result = native_io(
        &cc,
        "ewrite(); ewrite(\"\"); print(\"ok\");",
        limits,
        1,
        "-O2",
    );
    assert_eq!(result.code, 0);
    assert_eq!(result.stdout, b"ok\n");
    let result = native_io(
        &cc,
        "var n=1; print(n); n+=1; eprint(n);",
        limits,
        32,
        "-O2",
    );
    assert_eq!(result.code, 0);
    assert_eq!(result.stdout, b"1\n1\n");
    assert_eq!(result.stderr, b"2\n2\n");
}

#[test]
fn unsupported_values_and_backend_resource_caps_are_diagnosed() {
    for source in [
        "bind x = 1.0;",
        "bind x = \"text\";",
        "println((\"text\"));",
        "println(1.0 == 1.0);",
        "if false { println(1.0); }",
        "println(len([]));",
    ] {
        assert_eq!(
            c_backend::emit_c(source).unwrap_err().kind,
            DiagnosticKind::BackendUnsupported,
            "{source}"
        );
    }
    for source in [
        format!("println(0{});", "+1".repeat(1024)),
        "println(1);".repeat(4097),
        (0..257)
            .map(|index| format!("bind x{index}=1;"))
            .collect::<String>(),
        format!("println(\"{}\");", "x".repeat(65537)),
        format!("println(0{});", "+1".repeat(500)).repeat(17),
    ] {
        assert_eq!(
            c_backend::emit_c(&source).unwrap_err().kind,
            DiagnosticKind::BackendUnsupported
        );
    }
    assert_eq!(
        c_backend::emit_c("println(missing);").unwrap_err().kind,
        DiagnosticKind::UnknownName
    );
    assert!(c_backend::emit_c("println(1 / 0);").is_ok());
    assert!(c_backend::emit_c(&format!("println(0{});", "+1".repeat(500))).is_ok());
}

#[test]
fn emitted_c_matches_interpreter_output_errors_spans_and_budgets() {
    let Some(cc) = compiler() else { return };
    let defaults = ExecutionLimits::default();
    let cases=[
        ("",defaults),
        ("print(); println(); write(); print(\"\"); write(\"\");",defaults),
        ("write(\"A\"); print(\"B\",1,true,); println(\"\", \"x\");",defaults),
        ("print(1 + 2 * 3, (1 + 2) * 3, false);",defaults),
        ("print(\"a\", \"b\", \"c\");",ExecutionLimits{max_steps:2,max_prints:1}),
        ("print(\"a\", \"b\", \"c\");",ExecutionLimits{max_steps:3,max_prints:1}),
        ("write();",ExecutionLimits{max_steps:100,max_prints:0}),
        ("print(\"first\"); write(1, 1 / 0);",defaults),
        ("ewrite(); ewrite(\"\"); print(\"ok\");",defaults),
        ("println(\"Hello 🌍\\nA\0B\"); println(true); println(false);",defaults),
        ("println(-9223372036854775808); println(9223372036854775807); println(-9223372036854775808 * 1); println(-9223372036854775808 * 0);",defaults),
        ("println(-7 / 2); println(-7 % 2); println(-1 >>> 1); println(-8 >> 1); println(1 << 63); println(~0); println(3 | 8 ^ 7 & 1);",defaults),
        ("var x = 3; { bind x = x + 1; println(x); } x *= 7; x -= 2; x /= 3; x %= 5; x |= 8; x &= 15; x ^= 3; x <<= 2; x >>= 1; x >>>= 1; println(x);",defaults),
        ("var x = 0; while x < 4 { x += 1; if x == 2 { continue; } elif x == 4 { break; } else { println(x); } }",defaults),
        ("println(false and 1 / 0 == 0); println(true or 1 << 64 == 0); println(not 1 < 2); println(not not true);",defaults),
        ("println(1); println(9223372036854775807 + 1);",defaults),
        ("println(-9223372036854775808 - 1);",defaults),
        ("println(-9223372036854775808 * -1);",defaults),
        ("println(-(-9223372036854775808));",defaults),
        ("println(1); println(2 / 0);",defaults),
        ("println(-9223372036854775808 % -1);",defaults),
        ("var x = 1; x >>>= 64;",defaults),
        ("println(1 << -1);",defaults),
        ("println(1); while true { continue; }",ExecutionLimits{max_steps:40,max_prints:10}),
        ("println(\"first\"); println(2);",ExecutionLimits{max_steps:100,max_prints:1}),
        ("println(1+2*3);",ExecutionLimits{max_steps:5,max_prints:1}),
        ("println(\"x\");",ExecutionLimits{max_steps:1,max_prints:1}),
        ("",ExecutionLimits{max_steps:0,max_prints:0}),
    ];
    for optimization in ["-O0", "-O2"] {
        let results = native_batch(&cc, &cases, optimization);
        assert_eq!(results.len(), cases.len());
        for ((source, limits), actual) in cases.iter().zip(results) {
            assert_eq!(
                actual,
                reference(source, *limits, 1),
                "{optimization}: {source}"
            );
        }
    }
}

#[test]
fn compiled_programs_reset_workspace_and_validate_before_output() {
    let Some(cc) = compiler() else { return };
    let limits = ExecutionLimits::default();
    let source = "var x = 0; while x < 3 { x += 1; } println(x);";
    assert_eq!(
        native(&cc, source, limits, "-O2", false, 2),
        reference(source, limits, 2)
    );
    assert!(native(&cc, "println(1);", limits, "-O2", true, 1).starts_with("65535:"));
    let source = "println(1); println(1 / 0);";
    assert_eq!(
        native(&cc, source, limits, "-O2", true, 1),
        reference(source, limits, 1)
    );
}

#[test]
fn freestanding_objects_have_no_undefined_symbols_on_checked_targets() {
    if std::env::var_os("SCARP_FREESTANDING").is_none() {
        return;
    }
    let cc = compiler().expect("freestanding compiler required");
    let directory = Directory::new();
    fs::write(
        directory.0.join("program.c"),
        c_backend::emit_c(include_str!("../examples/bit_fields.srp")).unwrap(),
    )
    .unwrap();
    for target in ["x86_64-unknown-none-elf", "aarch64-unknown-none-elf"] {
        let object = directory.0.join(format!("{target}.o"));
        let built = Command::new(&cc)
            .args([
                "-target",
                target,
                "-std=c11",
                "-ffreestanding",
                "-fno-builtin",
                "-fno-stack-protector",
                "-O2",
                "-c",
            ])
            .arg(directory.0.join("program.c"))
            .arg("-o")
            .arg(&object)
            .output()
            .unwrap();
        assert!(
            built.status.success(),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let symbols = Command::new("llvm-nm")
            .arg("--undefined-only")
            .arg(&object)
            .output()
            .unwrap();
        assert!(symbols.status.success());
        assert!(
            symbols.stdout.is_empty(),
            "{target}: {}",
            String::from_utf8_lossy(&symbols.stdout)
        );
        let bytes = fs::read(object).unwrap();
        assert_eq!(&bytes[..4], b"\x7fELF");
    }
}
