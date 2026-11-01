use scarp::{
    diagnostics::DiagnosticKind as Kind,
    modules::{self, ModuleLimits},
    ExecutionOptions,
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);

#[cfg(windows)]
#[test]
fn windows_case_aliases_share_the_canonical_file_identity() {
    let fixture = Fixture::new();
    let entry = fixture.file(
        "main.srp",
        "import \"library.srp\" as a; import \"LIBRARY.srp\" as b; print(a.value,b.value);",
    );
    fixture.file("library.srp", "pub bind value=1;");
    let program = modules::compile_file(entry, ModuleLimits::default()).unwrap();
    assert_eq!(program.sources().files().len(), 2);
}
struct Fixture {
    directory: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let directory = std::env::temp_dir().join(format!(
            "scarp modules {} {}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&directory).unwrap();
        Self { directory }
    }
    fn file(&self, name: &str, source: impl AsRef<[u8]>) -> PathBuf {
        let path = self.directory.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, source).unwrap();
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}
#[test]
fn unicode_filenames_relative_imports_and_crlf_work_from_any_working_directory() {
    let fixture = Fixture::new();
    let entry = fixture.file(
        "hello café.srp",
        "// नमस्ते 🌍\r\nimport \"lib/मूल्य.srp\" as library;\r\nprint(library.answer);\r\n",
    );
    fixture.file("lib/मूल्य.srp", "pub bind answer=\"नमस्ते 🌍\";\r\n");
    for mode in ["run", "check"] {
        let process = Command::new(env!("CARGO_BIN_EXE_scarp"))
            .current_dir(std::env::temp_dir())
            .arg(mode)
            .arg(&entry)
            .output()
            .unwrap();
        assert!(
            process.status.success(),
            "{}",
            String::from_utf8_lossy(&process.stderr)
        );
        assert_eq!(
            process.stdout,
            if mode == "run" {
                "नमस्ते 🌍\n".as_bytes()
            } else {
                b""
            }
        );
        assert!(process.stderr.is_empty());
    }
}
#[test]
fn tokens_never_follows_imports_and_invalid_utf8_modules_are_source_errors() {
    let fixture = Fixture::new();
    let entry = fixture.file(
        "main.srp",
        "print(\"hidden\"); eprint(\"hidden\"); import \"invalid.srp\" as invalid;",
    );
    fixture.file("invalid.srp", [0xff, 0xfe]);
    let tokens = Command::new(env!("CARGO_BIN_EXE_scarp"))
        .arg("tokens")
        .arg(&entry)
        .output()
        .unwrap();
    assert!(tokens.status.success() && tokens.stderr.is_empty());
    for mode in ["run", "check"] {
        let process = Command::new(env!("CARGO_BIN_EXE_scarp"))
            .arg(mode)
            .arg(&entry)
            .output()
            .unwrap();
        assert_eq!(process.status.code(), Some(1));
        assert!(process.stdout.is_empty());
        let rendered = String::from_utf8(process.stderr).unwrap();
        assert!(rendered.starts_with("error[E0038]"));
        assert!(rendered.contains("not valid UTF-8"));
        assert!(rendered.contains("main.srp:1:"));
    }
}
#[test]
fn filesystem_limits_bound_reads_and_source_graph_ownership_outlives_files() {
    let fixture = Fixture::new();
    let source = "print(123);";
    let entry = fixture.file("main.srp", source);
    let exact = ModuleLimits {
        max_file_bytes: source.len(),
        max_total_bytes: source.len(),
        ..ModuleLimits::default()
    };
    let program = modules::compile_file(&entry, exact).unwrap();
    assert_eq!(
        modules::compile_file(
            &entry,
            ModuleLimits {
                max_file_bytes: source.len() - 1,
                ..exact
            }
        )
        .unwrap_err()
        .diagnostic
        .kind,
        Kind::ModuleLimit
    );
    fs::remove_file(entry).unwrap();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    program
        .execute(&mut out, &mut err, ExecutionOptions::default())
        .unwrap();
    assert_eq!(out, b"123\n");
}
#[test]
fn filesystem_parent_traversal_is_restricted_to_the_explicit_root() {
    let fixture = Fixture::new();
    let entry = fixture.file("project/main.srp", "import \"../outside.srp\" as outside;");
    fixture.file("outside.srp", "THIS FILE MUST NOT BE PARSED");
    assert_eq!(
        modules::compile_file(entry, ModuleLimits::default())
            .unwrap_err()
            .diagnostic
            .kind,
        Kind::ModulePath
    );
}
#[cfg(unix)]
#[test]
fn canonical_symlinks_share_identity_and_cannot_escape_the_root() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    let entry = fixture.file(
        "project/main.srp",
        "import \"lib.srp\" as a; import \"alias.srp\" as b; print(a.value,b.value);",
    );
    let library = fixture.file("project/lib.srp", "pub bind value=9;");
    symlink(library, fixture.directory.join("project/alias.srp")).unwrap();
    assert_eq!(
        modules::compile_file(&entry, ModuleLimits::default())
            .unwrap()
            .sources()
            .files()
            .len(),
        2
    );
    let outside = fixture.file("outside.srp", "MUST NOT PARSE");
    fs::remove_file(fixture.directory.join("project/alias.srp")).unwrap();
    symlink(outside, fixture.directory.join("project/alias.srp")).unwrap();
    assert_eq!(
        modules::compile_file(entry, ModuleLimits::default())
            .unwrap_err()
            .diagnostic
            .kind,
        Kind::ModulePath
    );
}
#[test]
fn cross_module_types_and_contracts_retain_both_file_locations() {
    let fixture = Fixture::new();
    let entry = fixture.file(
        "main.srp",
        "import \"lib.srp\" as lib; print(\"hidden\"); print(lib.positive(0));",
    );
    fixture.file(
        "lib.srp",
        "pub pure funcra positive(x: integer)->integer needs x>0; ensures result>0; {return x;}",
    );
    let program = modules::compile_file(&entry, ModuleLimits::default()).unwrap();
    let error = program.check(ExecutionOptions::default()).unwrap_err();
    assert_eq!(error.diagnostic.kind, Kind::ContractViolation);
    let rendered = error.render();
    assert!(rendered.contains("lib.srp:1:"));
    assert!(rendered.contains("main.srp:1:"));
}

#[test]
fn reviewed_module_diagnostic_fixtures_match_library_and_cli() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/modules");
    for (name, kind) in [
        ("private", Kind::ModuleVisibility),
        ("cycle", Kind::ModuleCycle),
        ("unknown", Kind::ModuleSymbol),
        ("type", Kind::TypeMismatch),
        ("syntax", Kind::ModuleSyntax),
    ] {
        let path = directory.join(format!("{name}.srp"));
        let source = fs::read_to_string(&path).unwrap();
        let mut loader = modules::MemorySourceLoader::new();
        loader.insert(format!("{name}.srp"), source).unwrap();
        loader
            .insert(
                "library.srp",
                fs::read_to_string(directory.join("library.srp")).unwrap(),
            )
            .unwrap();
        loader
            .insert(
                "loop.srp",
                fs::read_to_string(directory.join("loop.srp")).unwrap(),
            )
            .unwrap();
        let error =
            modules::compile_with_loader(&format!("{name}.srp"), &loader, ModuleLimits::default())
                .unwrap_err();
        assert_eq!(error.diagnostic.kind, kind);
        let expected = fs::read_to_string(directory.join(format!("{name}.stderr"))).unwrap();
        assert_eq!(format!("{}\n", error.render()), expected, "{name}");
        for mode in ["run", "check"] {
            let process = Command::new(env!("CARGO_BIN_EXE_scarp"))
                .current_dir(&directory)
                .args([mode, &format!("{name}.srp")])
                .output()
                .unwrap();
            assert_eq!(process.status.code(), Some(1));
            assert!(process.stdout.is_empty());
            // The filesystem loader identifies dependencies by their canonical
            // paths; normalize only that known fixture path for the golden.
            let mut actual = String::from_utf8(process.stderr).unwrap();
            for file in ["library.srp", "loop.srp"] {
                actual = actual.replace(
                    directory
                        .join(file)
                        .canonicalize()
                        .unwrap()
                        .to_string_lossy()
                        .as_ref(),
                    file,
                );
            }
            assert_eq!(actual, expected, "{name} {mode}");
        }
    }
}

#[test]
fn module_inspection_cli_is_static_while_run_and_check_keep_evaluating() {
    let fixture = Fixture::new();
    let entry=fixture.file("main.srp","import \"lib.srp\" as lib; print(\"PROGRAM OUTPUT\"); eprint(\"PROGRAM ERROR\"); print(lib.f()); while true {continue;}");
    fixture.file(
        "lib.srp",
        "pub pure funcra f(n: integer=1/0)->integer needs false; {return n;}",
    );
    let result = Command::new(env!("CARGO_BIN_EXE_scarp"))
        .arg("modules")
        .arg(&entry)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stderr.is_empty());
    let text = String::from_utf8(result.stdout).unwrap();
    assert!(text.starts_with("modules: 2, imports: 1\n"));
    assert!(text.contains("pub pure funcra f(n: integer = <default>) -> integer"));
    assert!(!text.contains("PROGRAM OUTPUT") && !text.contains("PROGRAM ERROR"));
    for mode in ["run", "check"] {
        let result = Command::new(env!("CARGO_BIN_EXE_scarp"))
            .arg(mode)
            .arg(&entry)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(1));
        assert!(result.stdout.is_empty());
        assert!(result.stderr.starts_with(b"error[E0008]"));
    }
}

#[test]
fn module_inspection_help_and_usage_are_validated_before_file_io() {
    for flag in ["--help", "-h"] {
        let result = Command::new(env!("CARGO_BIN_EXE_scarp"))
            .args(["modules", flag])
            .output()
            .unwrap();
        assert!(result.status.success() && result.stderr.is_empty());
        assert!(String::from_utf8(result.stdout)
            .unwrap()
            .contains("No statements, defaults, contracts"));
    }
    for args in [
        vec!["modules"],
        vec!["modules", "one.srp", "two.srp"],
        vec!["modules", "--unknown"],
        vec!["modules", "--max-steps", "0", "missing.srp"],
        vec!["modules", "--max-vector-bytes", "0", "missing.srp"],
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_scarp"))
            .args(args)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(!String::from_utf8(result.stderr)
            .unwrap()
            .contains("could not read"));
    }
}

#[test]
fn inspection_cli_errors_have_no_partial_inventory_and_keep_cross_file_locations() {
    let fixture = Fixture::new();
    fixture.file("lib.srp", "bind secret=7;");
    for (source, code) in [
        ("import \"lib.srp\" as lib; print(lib.secret);", "E0041"),
        ("import \"lib.srp\" as lib; print(lib.absent);", "E0042"),
        ("import \"missing.srp\" as missing;", "E0038"),
        ("import \"/outside.srp\" as outside;", "E0043"),
    ] {
        let entry = fixture.file("main.srp", source);
        let result = Command::new(env!("CARGO_BIN_EXE_scarp"))
            .arg("modules")
            .arg(entry)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(1));
        assert!(result.stdout.is_empty());
        let text = String::from_utf8(result.stderr).unwrap();
        assert!(text.starts_with(&format!("error[{code}]")), "{text}");
        assert!(text.contains("main.srp:1:"));
        if code == "E0041" {
            assert!(text.contains("lib.srp:1:"));
        }
    }
}

#[test]
fn inspection_cli_supports_unicode_crlf_spaces_and_explicit_dash_paths() {
    let fixture = Fixture::new();
    fixture.file(
        "-café.srp",
        "// 🌍\r\nimport \"lib/値.srp\" as lib;\r\nprint(lib.answer);",
    );
    fixture.file("lib/値.srp", "pub bind answer=42;\r\n");
    let result = Command::new(env!("CARGO_BIN_EXE_scarp"))
        .current_dir(&fixture.directory)
        .args(["modules", "--", "-café.srp"])
        .output()
        .unwrap();
    assert!(result.status.success() && result.stderr.is_empty());
    let text = String::from_utf8(result.stdout).unwrap();
    assert!(
        text.contains("lib/値.srp") && text.contains("@ 2:8"),
        "{text}"
    );
    assert!(text.contains("pub bind answer: integer @ 1:10"));
}

#[test]
fn inspection_cli_rejects_invalid_utf8_and_enforces_default_source_limits() {
    let fixture = Fixture::new();
    let entry = fixture.file("main.srp", "import \"lib.srp\" as lib;");
    fixture.file("lib.srp", [0xff]);
    let result = Command::new(env!("CARGO_BIN_EXE_scarp"))
        .arg("modules")
        .arg(&entry)
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1));
    assert!(result.stdout.is_empty() && result.stderr.starts_with(b"error[E0038]"));
    fixture.file("main.srp", vec![b' '; 1024 * 1024 + 1]);
    let result = Command::new(env!("CARGO_BIN_EXE_scarp"))
        .arg("modules")
        .arg(&entry)
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1));
    assert!(result.stdout.is_empty() && result.stderr.starts_with(b"error[E0040]"));
}

#[cfg(unix)]
#[test]
fn inspection_cli_preserves_native_non_unicode_entry_paths() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let fixture = Fixture::new();
    let entry = fixture
        .directory
        .join(OsString::from_vec(b"entry-\xff.srp".to_vec()));
    fs::write(&entry, "import \"lib.srp\" as lib; print(lib.answer);").unwrap();
    fixture.file("lib.srp", "pub bind answer=42;");
    let result = Command::new(env!("CARGO_BIN_EXE_scarp"))
        .arg("modules")
        .arg(entry)
        .output()
        .unwrap();
    assert!(result.status.success() && result.stderr.is_empty());
    assert!(String::from_utf8(result.stdout)
        .unwrap()
        .contains("pub bind answer: integer"));
}
