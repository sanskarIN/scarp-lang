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
fn loop_budgets_are_available_to_run_and_check_without_partial_output() {
    let fixture = Fixture::new();
    let infinite = fixture.file("infinite.srp", "println(\"before\"); while true {}");
    let prints = fixture.file("prints.srp", "while true { println(1); }");
    let valid = fixture.file(
        "valid.srp",
        "let mut x = 0; while x < 3 { x += 1; } println(x);",
    );
    for mode in ["run", "check"] {
        let result = command()
            .args([mode, "--max-steps", "100", "--max-prints", "1"])
            .arg(&valid)
            .output()
            .unwrap();
        assert!(result.status.success(), "{}", stderr(&result));
        assert_eq!(
            result.stdout,
            if mode == "run" {
                b"3\n".as_slice()
            } else {
                b"".as_slice()
            }
        );
        let result = command()
            .args([mode, "--max-steps", "9"])
            .arg(&infinite)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(1));
        assert!(result.stdout.is_empty());
        assert!(stderr(&result).contains("E0017"));
        let result = command()
            .args([mode, "--max-prints", "2"])
            .arg(&prints)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(1));
        assert!(result.stdout.is_empty());
        assert!(stderr(&result).contains("E0018"));
    }
}

#[test]
fn malformed_execution_budget_options_are_usage_errors() {
    let fixture = Fixture::new();
    let file = fixture.file("valid.srp", "println(1);");
    for args in [
        vec!["run", "--max-steps", "-1"],
        vec!["run", "--max-steps", "+1"],
        vec!["run", "--max-steps", "1.0"],
        vec!["run", "--max-steps", "18446744073709551616"],
        vec!["run", "--max-steps", "1", "--max-steps", "2"],
        vec!["tokens", "--max-steps", "1"],
        vec!["check", "--max-prints", "x"],
    ] {
        let result = command().args(&args).arg(&file).output().unwrap();
        assert_eq!(
            result.status.code(),
            Some(2),
            "{args:?}: {}",
            stderr(&result)
        );
        assert!(result.stdout.is_empty());
    }
    let dash = fixture.file("-program.srp", "println(1);");
    let result = command()
        .args(["run", "--max-steps", "2", "--", "-program.srp"])
        .current_dir(&fixture.directory)
        .output()
        .unwrap();
    assert!(dash.exists());
    assert!(result.status.success(), "{}", stderr(&result));
    assert_eq!(result.stdout, b"1\n");
}

#[test]
fn conditional_programs_run_and_skipped_static_errors_reach_stderr() {
    let fixture = Fixture::new();
    let valid = fixture.file(
        "decisions.srp",
        "let mut x = 1; if x < 2 and not false { x += 2; } else { println(1 / 0); } println(x);",
    );
    let invalid = fixture.file(
        "invalid.srp",
        "println(\"before\"); if false { println(missing); }",
    );
    for mode in ["run", "check"] {
        let success = command().arg(mode).arg(&valid).output().unwrap();
        assert!(success.status.success(), "{}", stderr(&success));
        assert_eq!(
            success.stdout,
            if mode == "run" {
                b"3\n".as_slice()
            } else {
                b"".as_slice()
            }
        );
        assert!(success.stderr.is_empty());
        let failure = command().arg(mode).arg(&invalid).output().unwrap();
        assert_eq!(failure.status.code(), Some(1));
        assert!(failure.stdout.is_empty());
        assert!(stderr(&failure).contains("E0010"));
    }
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
    for subcommand in ["run", "tokens", "check"] {
        let output = command().arg(subcommand).arg(&path).output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(stderr(&output).contains("error[E0001]"));
    }
}

#[test]
fn reports_missing_files_for_all_file_commands() {
    let fixture = Fixture::new();
    let missing = fixture.directory.join("missing.srp");
    for subcommand in ["run", "tokens", "check"] {
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
    for subcommand in ["run", "tokens", "check"] {
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
        vec!["check"],
        vec!["check", "--unknown"],
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

#[test]
fn runs_integer_expressions_and_existing_strings_together() {
    let fixture = Fixture::new();
    let path = fixture.file(
        "expressions.srp",
        "println(1 + 2 * 3); println((1 + 2) * 3); println(\"done\");",
    );
    let output = command().arg("run").arg(path).output().unwrap();
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(output.stdout, b"7\n9\ndone\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn reports_arithmetic_errors_without_partial_program_output() {
    let fixture = Fixture::new();
    for (argument, code) in [
        ("1 / 0", "E0008"),
        ("9223372036854775807 + 1", "E0007"),
        ("9223372036854775808", "E0006"),
    ] {
        let path = fixture.file(
            "arithmetic-error.srp",
            format!("println(\"first\");\nprintln({argument});"),
        );
        let output = command().arg("run").arg(path).output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(stderr(&output).contains(code));
        assert!(stderr(&output).contains("arithmetic-error.srp:2:"));
        assert!(!stderr(&output).contains("panicked"));
    }
}

#[test]
fn token_inspection_does_not_evaluate_or_range_check_integers() {
    let fixture = Fixture::new();
    let path = fixture.file("tokens.srp", "println(9223372036854775808 / 0);");
    let output = command().arg("tokens").arg(path).output().unwrap();
    assert!(output.status.success(), "{}", stderr(&output));
    let tokens = String::from_utf8(output.stdout).unwrap();
    assert!(tokens.contains("integer(9223372036854775808)"));
    assert!(tokens.contains("  /  "));
}

#[test]
fn runs_bindings_and_reports_semantic_errors_on_stderr() {
    let fixture = Fixture::new();
    let valid = fixture.file("bindings.srp", "let value = 6 * 7; println(value);");
    let output = command().arg("run").arg(valid).output().unwrap();
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(output.stdout, b"42\n");
    for (source, code) in [
        ("println(\"first\"); println(missing);", "E0010"),
        ("let x = 1; let x = 2;", "E0011"),
        ("let println = 1;", "E0012"),
    ] {
        let invalid = fixture.file("names.srp", source);
        let output = command().arg("run").arg(invalid).output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(stderr(&output).contains(code));
    }
}

#[test]
fn runs_mutable_bindings_and_diagnoses_immutable_writes() {
    let fixture = Fixture::new();
    let path = fixture.file(
        "mutable.srp",
        "let mut value = 1; value += 41; println(value);",
    );
    let output = command().arg("run").arg(path).output().unwrap();
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(output.stdout, b"42\n");
    let path = fixture.file("immutable.srp", "let value = 1; println(value); value = 2;");
    let output = command().arg("run").arg(path).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(stderr(&output).contains("E0013"));
    assert!(stderr(&output).contains("declared here"));
}

#[test]
fn runs_scoped_variables_and_reports_names_that_leave_scope() {
    let fixture = Fixture::new();
    let path = fixture.file(
        "scopes.srp",
        "let x = 1; { let x = x + 1; println(x); } println(x);",
    );
    let output = command().arg("run").arg(path).output().unwrap();
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(output.stdout, b"2\n1\n");
    let path = fixture.file("leak.srp", "{ let x = 1; } println(x);");
    let output = command().arg("run").arg(path).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(stderr(&output).contains("E0010"));
}

#[test]
fn check_is_silent_on_success_and_validates_arithmetic() {
    let fixture = Fixture::new();
    let path = fixture.file("check.srp", "let mut x = 1; x += 2; println(x);");
    let output = command().arg("check").arg(path).output().unwrap();
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(output.stdout.is_empty() && output.stderr.is_empty());
    for (source, code) in [
        ("println(1); println(missing);", "E0010"),
        ("let x = 1; x = 2;", "E0013"),
        ("println(1); let mut x = 1; x /= 0;", "E0008"),
        ("let x = 9223372036854775807 + 1;", "E0007"),
    ] {
        let path = fixture.file("check-error.srp", source);
        let output = command().arg("check").arg(path).output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(stderr(&output).contains(code));
    }
}

#[test]
fn check_supports_unicode_paths_and_an_explicit_separator() {
    let fixture = Fixture::new();
    fixture.file("-café.srp", "println(\"नमस्ते\");");
    let output = command()
        .args(["check", "--", "-café.srp"])
        .current_dir(&fixture.directory)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(output.stdout.is_empty());
}

#[test]
fn large_single_line_token_dumps_retain_exact_eof_locations() {
    let fixture = Fixture::new();
    let source = "let v = 1;".repeat(5_000);
    let path = fixture.file("large.srp", &source);
    let output = command().arg("tokens").arg(path).output().unwrap();
    assert!(output.status.success(), "{}", stderr(&output));
    let tokens = String::from_utf8(output.stdout).unwrap();
    assert_eq!(tokens.lines().count(), 25_001);
    assert!(tokens.ends_with(&format!(
        "1:{}..1:{}  <eof>  [{}..{}]\n",
        source.len() + 1,
        source.len() + 1,
        source.len(),
        source.len()
    )));
}

#[test]
fn each_file_command_exposes_help_before_requiring_a_path() {
    for subcommand in ["run", "tokens", "check"] {
        for flag in ["--help", "-h"] {
            let output = command().args([subcommand, flag]).output().unwrap();
            assert!(output.status.success());
            assert!(output.stderr.is_empty());
            assert!(String::from_utf8(output.stdout)
                .unwrap()
                .contains("scarp check"));
        }
    }
}

#[test]
fn iteration_help_explains_evaluation_and_budget_placement() {
    for subcommand in ["run", "check"] {
        let result = command().args([subcommand, "--help"]).output().unwrap();
        assert!(result.status.success() && result.stderr.is_empty());
        let help = String::from_utf8(result.stdout).unwrap();
        assert!(
            help.contains("--max-steps")
                && help.contains("--max-prints")
                && help.contains("Options precede the path")
        );
    }
    let result = command().args(["check", "--help"]).output().unwrap();
    assert!(String::from_utf8(result.stdout)
        .unwrap()
        .contains("Loops and arithmetic execute"));
    let result = command().args(["run", "--help"]).output().unwrap();
    assert!(String::from_utf8(result.stdout)
        .unwrap()
        .contains("repeat count as index"));
}

#[test]
fn counted_loop_cli_has_atomic_budgets_and_tokens_still_only_lexes() {
    let fixture = Fixture::new();
    let path = fixture.file(
        "iteration café.srp",
        "println(\"hidden\"); repeat 9223372036854775807 as i { continue; }",
    );
    for subcommand in ["run", "check"] {
        let result = command()
            .args([subcommand, "--max-steps", "20"])
            .arg(&path)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(1));
        assert!(result.stdout.is_empty());
        let stderr = String::from_utf8(result.stderr).unwrap();
        assert!(
            stderr.contains("E0017")
                && stderr.contains("limit of 20")
                && stderr.contains("--max-steps")
        );
    }
    let result = command().arg("tokens").arg(&path).output().unwrap();
    assert!(result.status.success() && result.stderr.is_empty());
    assert!(String::from_utf8(result.stdout)
        .unwrap()
        .contains("identifier(repeat)"));
}

#[test]
fn iteration_milestone_keeps_later_commands_and_syntax_out() {
    for subcommand in ["exec", "verify", "inspect", "compile"] {
        let result = command()
            .args([subcommand, "missing.srp"])
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(2));
    }
    for source in [
        "bind x=1;",
        "var x=1;",
        "print(1);",
        "funcra f() -> unit {}",
        "println(1.25);",
        "println([1,2]);",
    ] {
        assert!(scarp::compile(source).is_err(), "{source}");
    }
}

#[test]
fn string_variables_run_and_type_errors_are_checked_before_output() {
    let fixture = Fixture::new();
    let path = fixture.file(
        "strings.srp",
        "let mut text = \"first\"; println(text); text = \"second\"; println(text);",
    );
    let output = command().arg("run").arg(&path).output().unwrap();
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(output.stdout, b"first\nsecond\n");
    fixture.file(
        "strings.srp",
        "println(1); let mut text = \"hello\"; text = 1;",
    );
    for subcommand in ["run", "check"] {
        let output = command().arg(subcommand).arg(&path).output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(stderr(&output).contains("E0015"));
        assert!(stderr(&output).contains("type is established here"));
    }
}

#[cfg(unix)]
#[test]
fn handles_non_unicode_native_paths_according_to_the_filesystem() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};

    let fixture = Fixture::new();
    let path = fixture
        .directory
        .join(OsString::from_vec(b"file-\xff.srp".to_vec()));
    let creation = fs::write(&path, "println(\"native path\");");
    let output = command().arg("run").arg(path).output().unwrap();
    match creation {
        Ok(()) => {
            assert!(output.status.success(), "{}", stderr(&output));
            assert_eq!(output.stdout, b"native path\n");
        }
        // macOS can reject the filename itself with EILSEQ (illegal byte sequence).
        Err(error) if cfg!(target_os = "macos") && error.raw_os_error() == Some(92) => {
            assert_eq!(output.status.code(), Some(1));
            assert!(output.stdout.is_empty());
            assert!(stderr(&output).contains("could not read"));
            assert!(!stderr(&output).contains("panicked"));
        }
        Err(error) => panic!("could not create native-path fixture: {error}"),
    }
}
