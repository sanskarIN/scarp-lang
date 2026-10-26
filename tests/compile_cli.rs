use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let directory = std::env::temp_dir().join(format!(
            "scarp-build-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).unwrap();
        Self(directory)
    }
    fn source(&self, source: &str) -> PathBuf {
        let path = self.0.join("program $ 名前.srp");
        fs::write(&path, source).unwrap();
        path
    }
    fn output(&self, name: &str) -> PathBuf {
        self.0.join(if cfg!(windows) {
            format!("{name}.exe")
        } else {
            name.into()
        })
    }
    fn clean_staging(&self) {
        assert!(fs::read_dir(&self.0).unwrap().all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".scarp-build-")));
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_scarp"))
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
        eprintln!("native build checks skipped: no C compiler");
        None
    }
}
fn success(output: &Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn native_function_rejection_does_not_create_or_replace_artifacts() {
    let fixture = Fixture::new();
    let source = fixture.source("funcra f() -> integer { return 4; } print(f());");
    for emit in ["c", "exe", "obj"] {
        let destination = fixture.0.join(format!("function-output.{emit}"));
        let result = command()
            .args(["compile", "--emit", emit])
            .arg(&source)
            .arg("-o")
            .arg(&destination)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(1));
        assert!(result.stdout.is_empty());
        assert!(String::from_utf8_lossy(&result.stderr).contains("E0035"));
        assert!(!destination.exists());
        fixture.clean_staging();
    }
    let destination = fixture.0.join("existing.c");
    fs::write(&destination, b"preserve me").unwrap();
    let result = command()
        .args(["compile", "--emit", "c"])
        .arg(&source)
        .arg("-o")
        .arg(&destination)
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert_eq!(fs::read(destination).unwrap(), b"preserve me");
}
fn build(cc: &OsString, source: &Path, destination: &Path, flags: &[&str]) -> Output {
    command()
        .arg("compile")
        .args(flags)
        .arg(source)
        .arg("-o")
        .arg(destination)
        .arg("--cc")
        .arg(cc)
        .output()
        .unwrap()
}

#[test]
fn build_help_and_invalid_invocations_do_not_read_source() {
    {
        let verb = "compile";
        for help in ["--help", "-h"] {
            let output = command().args([verb, help]).output().unwrap();
            success(&output);
            assert!(String::from_utf8_lossy(&output.stdout).contains("--emit exe|obj|c"));
        }
    }
    for args in [
        vec![],
        vec!["missing.srp"],
        vec!["-o", "app"],
        vec!["missing.srp", "-o"],
        vec!["missing.srp", "-o", "app", "--emit", "llvm"],
        vec![
            "missing.srp",
            "-o",
            "app",
            "--target",
            "aarch64-unknown-none-elf",
        ],
        vec![
            "missing.srp",
            "-o",
            "app",
            "--emit",
            "obj",
            "--target",
            "bogus",
        ],
        vec![
            "missing.srp",
            "-o",
            "app",
            "--emit",
            "obj",
            "--max-steps",
            "1",
        ],
        vec!["missing.srp", "-o", "app", "--max-steps", "-1"],
        vec![
            "missing.srp",
            "-o",
            "app",
            "--max-prints",
            "18446744073709551616",
        ],
        vec!["missing.srp", "-o", "app", "--cc", ""],
        vec!["missing.srp", "-o", "app", "--output", "other"],
        vec!["missing.srp", "-o", "app", "--unknown"],
        vec!["missing.srp", "extra.srp", "-o", "app"],
        vec!["-o", "app", "--"],
        vec!["-o", "app", "--", "one", "two"],
    ] {
        let output = command().arg("compile").args(&args).output().unwrap();
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("could not read"));
    }
}

#[test]
fn c_source_output_is_exclusive_and_rejects_options_that_would_be_ignored() {
    let fixture = Fixture::new();
    let source = fixture.source("println(1);");
    let destination = fixture.0.join("source.c");
    let result = command()
        .args(["compile", "--emit", "c"])
        .arg(&source)
        .arg("-o")
        .arg(&destination)
        .env("SCARP_CC", "compiler-must-not-run")
        .output()
        .unwrap();
    success(&result);
    assert!(result.stdout.is_empty() && result.stderr.is_empty());
    let bytes = fs::read(&destination).unwrap();
    assert!(String::from_utf8_lossy(&bytes).contains("uint32_t scarp_run"));
    let duplicate = command()
        .args(["compile", "--emit", "c"])
        .arg(&source)
        .arg("-o")
        .arg(&destination)
        .output()
        .unwrap();
    assert_eq!(duplicate.status.code(), Some(1));
    assert_eq!(fs::read(destination).unwrap(), bytes);
    for flags in [
        vec!["--cc", "clang"],
        vec!["--target", "x86_64-unknown-none-elf"],
        vec!["--max-steps", "1"],
        vec!["--max-prints", "1"],
    ] {
        let rejected = fixture.0.join("rejected.c");
        let result = command()
            .args(["compile", "--emit", "c"])
            .arg(&source)
            .arg("-o")
            .arg(&rejected)
            .args(flags)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(2));
        assert!(!rejected.exists());
    }
    fixture.clean_staging();
}

#[test]
fn native_compile_preserves_exact_output() {
    let Some(cc) = compiler() else { return };
    let fixture = Fixture::new();
    let source = fixture.source("println(\"compiled 🌍\0\"); var x=3; while x>0 { println(x); x-=1; } println(false and 1/0==0); println(-1 >>> 1);");
    let mut expected = Vec::new();
    scarp::run(&fs::read_to_string(&source).unwrap(), &mut expected).unwrap();
    {
        let verb = "compile";
        let destination = fixture.output(&format!("{verb} ; 应用"));
        let output = command()
            .arg(verb)
            .arg(&source)
            .arg("-o")
            .arg(&destination)
            .arg("--cc")
            .arg(&cc)
            .env("SCARP_CC", "missing-compiler-that-must-be-overridden")
            .output()
            .unwrap();
        success(&output);
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
        let executed = Command::new(destination).output().unwrap();
        success(&executed);
        assert_eq!(executed.stdout, expected);
        assert!(executed.stderr.is_empty());
    }
    fixture.clean_staging();
}

#[test]
fn native_output_arguments_and_both_streams_match_the_interpreter() {
    let Some(cc) = compiler() else { return };
    let fixture = Fixture::new();
    let source = fixture.source("write(\"Loading\"); ewrite(\"Warning\"); print(\"...\", 42, true,); eprint(\"!\", false); print(); write();");
    let destination = fixture.output("dual-stream");
    success(&build(&cc, &source, &destination, &[]));
    let executed = Command::new(&destination).output().unwrap();
    assert!(executed.status.success());
    assert_eq!(executed.stdout, b"Loading... 42 true\n\n");
    assert_eq!(executed.stderr, b"Warning! false\n");
    fixture.clean_staging();
}

#[test]
fn native_runtime_failure_does_not_leak_either_program_stream() {
    let Some(cc) = compiler() else { return };
    let fixture = Fixture::new();
    let source =
        fixture.source("print(\"suppress stdout\"); eprint(\"suppress stderr\"); write(1, 1 / 0);");
    let destination = fixture.output("dual-error");
    success(&build(&cc, &source, &destination, &[]));
    let executed = Command::new(&destination).output().unwrap();
    assert_eq!(executed.status.code(), Some(1));
    assert!(executed.stdout.is_empty());
    let text = fs::read_to_string(&source).unwrap();
    let operator = text.find('/').unwrap();
    assert_eq!(
        executed.stderr,
        format!(
            "error[E0008] at source bytes {operator}..{}\n",
            operator + 1
        )
        .as_bytes()
    );
    fixture.clean_staging();
}

#[test]
fn compiled_failures_preserve_empty_output_and_diagnostic_spans() {
    let Some(cc) = compiler() else { return };
    let fixture = Fixture::new();
    for (index, source) in [
        "println(1); println(1/0);",
        "println(9223372036854775807 + 1);",
        "println(1 << 64);",
    ]
    .iter()
    .enumerate()
    {
        let path = fixture.source(source);
        let destination = fixture.output(&format!("error-{index}"));
        success(&build(&cc, &path, &destination, &[]));
        let result = Command::new(destination).output().unwrap();
        let error = scarp::check(source).unwrap_err();
        assert_eq!(result.status.code(), Some(1));
        assert!(result.stdout.is_empty());
        assert_eq!(
            String::from_utf8(result.stderr).unwrap(),
            format!(
                "error[{}] at source bytes {}..{}\n",
                error.kind.code(),
                error.span.start,
                error.span.end
            )
        );
    }
    fixture.clean_staging();
}

#[test]
fn executable_budgets_and_full_width_limit_constants_work() {
    let Some(cc) = compiler() else { return };
    let fixture = Fixture::new();
    for (index, (source, flags, code)) in [
        (
            "println(1); while true { continue; }",
            vec!["--max-steps", "20"],
            "E0017",
        ),
        (
            "println(1); println(2);",
            vec!["--max-prints", "1"],
            "E0018",
        ),
        ("println(1);", vec!["--max-steps", "0"], "E0017"),
    ]
    .iter()
    .enumerate()
    {
        let destination = fixture.output(&format!("budget-{index}"));
        success(&build(&cc, &fixture.source(source), &destination, flags));
        let output = Command::new(destination).output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains(code));
    }
    let destination = fixture.output("maximum-budget");
    let output = command()
        .arg("compile")
        .arg(fixture.source("println(7);"))
        .arg("-o")
        .arg(&destination)
        .arg("--cc")
        .arg(&cc)
        .args([
            "--max-steps",
            &u64::MAX.to_string(),
            "--max-prints",
            &usize::MAX.to_string(),
        ])
        .output()
        .unwrap();
    success(&output);
    assert_eq!(Command::new(destination).output().unwrap().stdout, b"7\n");
    fixture.clean_staging();
}

#[test]
fn invalid_source_wins_before_tool_invocation_and_leaves_no_artifact() {
    let fixture = Fixture::new();
    for (source, code) in [
        ("println(missing);", "E0010"),
        ("println(1.0);", "E0027"),
        ("let x = 1;", "E0005"),
        ("let mut x = 1;", "E0005"),
        ("bind mut x = 1;", "E0004"),
    ] {
        let destination = fixture.output("invalid");
        let output = build(
            &"missing-compiler-that-must-not-run".into(),
            &fixture.source(source),
            &destination,
            &[],
        );
        assert_eq!(output.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&output.stderr).contains(code));
        assert!(!destination.exists());
        fixture.clean_staging();
    }
}

#[test]
fn existing_outputs_and_source_files_are_never_replaced() {
    let fixture = Fixture::new();
    let source = fixture.source("println(1);");
    let destination = fixture.output("existing");
    fs::write(&destination, b"keep this content").unwrap();
    for output in [&source, &destination] {
        let result = build(&"missing-compiler".into(), &source, output, &[]);
        assert_eq!(result.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&result.stderr).contains("already exists"));
    }
    assert_eq!(fs::read(destination).unwrap(), b"keep this content");
    assert_eq!(fs::read(source).unwrap(), b"println(1);");
    fixture.clean_staging();
}

#[test]
fn missing_compilers_and_output_parents_have_clear_failures_and_cleanup() {
    let fixture = Fixture::new();
    let source = fixture.source("println(1);");
    let destination = fixture.output("missing-cc");
    let output = build(
        &"compiler-that-does-not-exist".into(),
        &source,
        &destination,
        &[],
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("could not start the C compiler"));
    assert!(!destination.exists());
    let output = build(
        &"compiler-that-does-not-exist".into(),
        &source,
        &fixture.0.join("missing-parent/app"),
        &[],
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("staging directory"));
    fixture.clean_staging();
}

#[test]
fn filename_separator_and_native_object_mode_work() {
    let Some(cc) = compiler() else { return };
    let fixture = Fixture::new();
    fs::write(fixture.0.join("-input.srp"), "println(1);").unwrap();
    let output = command()
        .current_dir(&fixture.0)
        .args(["compile", "-o", "native.o", "--emit", "obj", "--cc"])
        .arg(cc)
        .args(["--", "-input.srp"])
        .output()
        .unwrap();
    success(&output);
    assert!(fs::metadata(fixture.0.join("native.o")).unwrap().len() > 0);
    fixture.clean_staging();
}

#[test]
fn freestanding_cli_cross_compiles_checked_elf_architectures() {
    if std::env::var_os("SCARP_FREESTANDING").is_none() {
        return;
    }
    let cc = compiler().expect("freestanding compiler required");
    let fixture = Fixture::new();
    let source = fixture.source(include_str!("../examples/bit_fields.srp"));
    for (target, machine) in [
        ("x86_64-unknown-none-elf", 62u16),
        ("aarch64-unknown-none-elf", 183u16),
    ] {
        let destination = fixture.0.join(format!("{target}.o"));
        success(&build(
            &cc,
            &source,
            &destination,
            &["--emit", "obj", "--target", target],
        ));
        let bytes = fs::read(&destination).unwrap();
        assert_eq!(&bytes[..4], b"\x7fELF");
        assert_eq!(bytes[5], 1);
        assert_eq!(u16::from_le_bytes([bytes[18], bytes[19]]), machine);
        let symbols = Command::new("llvm-nm")
            .arg("--undefined-only")
            .arg(destination)
            .output()
            .unwrap();
        success(&symbols);
        assert!(symbols.stdout.is_empty());
    }
    fixture.clean_staging();
}

#[cfg(unix)]
#[test]
fn compiler_failure_missing_artifact_and_racing_output_are_handled() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    let source = fixture.source("println(1);");
    for (name, body, message) in [
        ("failure", "printf 'compiler evidence\\n' >&2; exit 7".to_owned(), "compiler evidence"),
        ("empty", "exit 0".to_owned(), "without producing"),
        ("race", format!("for artifact in \"$@\"; do :; done\nprintf compiled > \"$artifact\"\nprintf competitor > '{}'", fixture.output("race").display()), "already exists"),
    ] {
        let script = fixture.0.join(format!("{name}.sh"));
        fs::write(&script, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).unwrap();
        let destination = fixture.output(name);
        let result = build(&script.into_os_string(), &source, &destination, &[]);
        assert_eq!(result.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&result.stderr).contains(message));
        if name == "race" { assert_eq!(fs::read(destination).unwrap(), b"competitor"); }
        else { assert!(!destination.exists()); }
        fixture.clean_staging();
    }
}

#[cfg(unix)]
#[test]
fn non_unicode_modes_and_existing_symlinks_are_rejected() {
    use std::os::unix::{ffi::OsStringExt, fs::symlink};
    let fixture = Fixture::new();
    let source = fixture.source("println(9);");
    let output = command()
        .arg("compile")
        .arg(&source)
        .arg("-o")
        .arg(fixture.output("bad-mode"))
        .arg("--emit")
        .arg(OsString::from_vec(vec![255]))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let link = fixture.output("dangling");
    symlink(fixture.0.join("not-created"), &link).unwrap();
    let output = build(&"compiler-must-not-run".into(), &source, &link, &[]);
    assert!(String::from_utf8_lossy(&output.stderr).contains("already exists"));
    fixture.clean_staging();
}

#[cfg(target_os = "linux")]
#[test]
fn linux_native_paths_can_contain_non_unicode_bytes() {
    use std::os::unix::ffi::OsStringExt;
    let Some(cc) = compiler() else { return };
    let fixture = Fixture::new();
    let source = fixture
        .0
        .join(OsString::from_vec(b"source-\xff.srp".to_vec()));
    let destination = fixture.0.join(OsString::from_vec(b"program-\xfe".to_vec()));
    fs::write(&source, "println(9);").unwrap();
    success(&build(&cc, &source, &destination, &[]));
    assert_eq!(Command::new(destination).output().unwrap().stdout, b"9\n");
    fixture.clean_staging();
}
