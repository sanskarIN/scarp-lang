use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    directory: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let id = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "scarp-cli-{}-{id}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&directory).unwrap();
        Self { directory }
    }

    fn file(&self, name: &str, content: impl AsRef<[u8]>) -> PathBuf {
        let path = self.directory.join(name);
        fs::write(&path, content).unwrap();
        path
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

fn command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_scarp"))
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn runs_the_repository_hello_example() {
    let output = command()
        .args(["run", "examples/hello.srp"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(output.stdout, b"Hello from Scarp!\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn inspects_tokens_with_spans_and_locations() {
    let fixture = Fixture::new();
    let path = fixture.file("tokens.srp", "println(\"hé\\n\");\n");
    let output = command().arg("tokens").arg(path).output().unwrap();
    assert!(output.status.success(), "{}", stderr(&output));
    let tokens = String::from_utf8(output.stdout).unwrap();
    assert!(tokens.contains("1:1..1:8  identifier(println)  [0..7]"));
    assert!(tokens.contains("string(\"hé\\n\")"));
    assert!(tokens.contains("2:1..2:1  <eof>"));
    assert!(output.stderr.is_empty());
}

#[test]
fn tokens_does_not_require_a_syntactically_complete_program() {
    let fixture = Fixture::new();
    let path = fixture.file("incomplete.srp", "println(");
    let output = command().arg("tokens").arg(path).output().unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout).unwrap().contains("<eof>"));
}

#[test]
fn reports_syntax_errors_on_stderr_with_a_failure_code() {
    let fixture = Fixture::new();
    let path = fixture.file("invalid.srp", "// comment\r\nprintln(\"hello\")");
    let output = command().arg("run").arg(path).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let errors = stderr(&output);
    assert!(errors.contains("error[E0004]"), "{errors}");
    assert!(errors.contains("invalid.srp:2:17"), "{errors}");
    assert!(errors.contains("expected ';'"));
    assert!(errors.contains('^'));
    assert!(!errors.contains("panicked"));
}

#[test]
fn lexical_errors_never_leak_partial_output() {
    let fixture = Fixture::new();
    let path = fixture.file("invalid.srp", "println(\"first\"); @");
    for subcommand in ["run", "tokens"] {
        let output = command().arg(subcommand).arg(&path).output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(stderr(&output).contains("error[E0001]"));
    }
}

#[test]
fn reports_missing_files_for_both_commands() {
    let fixture = Fixture::new();
    let missing = fixture.directory.join("missing.srp");
    for subcommand in ["run", "tokens"] {
        let output = command().arg(subcommand).arg(&missing).output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(stderr(&output).contains("could not read"));
        assert!(stderr(&output).contains("missing.srp"));
    }
}

#[test]
fn reports_non_utf8_files_as_read_errors() {
    let fixture = Fixture::new();
    let path = fixture.file("bytes.srp", [0xff, 0xfe, 0x80]);
    for subcommand in ["run", "tokens"] {
        let output = command().arg(subcommand).arg(&path).output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(stderr(&output).contains("could not read"));
        assert!(!stderr(&output).contains("panicked"));
    }
}

#[test]
fn reports_a_directory_instead_of_a_source_file() {
    let fixture = Fixture::new();
    let output = command()
        .arg("run")
        .arg(&fixture.directory)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("could not read"));
}

#[test]
fn supports_spaces_unicode_paths_and_crlf() {
    let fixture = Fixture::new();
    let path = fixture.file(
        "hello café.srp",
        "// Windows file\r\nprintln(\"नमस्ते 🌍\");\r\n",
    );
    let output = command().arg("run").arg(path).output().unwrap();
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "नमस्ते 🌍\n");
}

#[test]
fn supports_an_explicit_filename_separator() {
    let fixture = Fixture::new();
    fixture.file("-hello.srp", "println(\"ok\");");
    let output = command()
        .args(["run", "--", "-hello.srp"])
        .current_dir(&fixture.directory)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(output.stdout, b"ok\n");
}

#[test]
fn help_and_version_succeed_without_reading_a_file() {
    for flag in ["--help", "-h", "--version", "-V"] {
        let output = command().arg(flag).output().unwrap();
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        let text = String::from_utf8(output.stdout).unwrap();
        if flag == "--help" || flag == "-h" {
            assert!(text.contains("Usage:"));
            assert!(text.contains("scarp tokens"));
        } else {
            assert_eq!(text, format!("scarp {}\n", env!("CARGO_PKG_VERSION")));
        }
    }
}

#[test]
fn rejects_invalid_command_lines_with_usage_exit_code() {
    for args in [
        vec![],
        vec!["run"],
        vec!["tokens"],
        vec!["build", "example.srp"],
        vec!["run", "one.srp", "two.srp"],
        vec!["run", "--unknown"],
        vec!["--version", "extra"],
    ] {
        let output = command().args(&args).output().unwrap();
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty());
        assert!(stderr(&output).contains("Usage:"));
        assert!(!stderr(&output).contains("panicked"));
    }
}

#[cfg(unix)]
#[test]
fn accepts_non_unicode_native_paths() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};

    let fixture = Fixture::new();
    let path = fixture
        .directory
        .join(OsString::from_vec(b"file-\xff.srp".to_vec()));
    fs::write(&path, "println(\"native path\");").unwrap();
    let output = command().arg("run").arg(path).output().unwrap();
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(output.stdout, b"native path\n");
}
