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
