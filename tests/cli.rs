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
        "var x = 0; while x < 3 { x += 1; } println(x);",
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
fn function_budget_flags_enforce_exact_boundaries_in_run_and_check() {
    let fixture = Fixture::new();
    let file = fixture.file(
        "recursion.srp",
        "pure funcra f(n: integer) -> integer { if n == 0 { return 0; } return f(n - 1) + 0; } print(f(4));",
    );
    for mode in ["run", "check"] {
        for (flag, accepted, rejected) in [
            ("--max-call-depth", "5", "4"),
            ("--max-calls", "5", "4"),
            ("--max-live-slots", "10", "9"),
        ] {
            let success = command()
                .args([mode, flag, accepted])
                .arg(&file)
                .output()
                .unwrap();
            assert!(success.status.success(), "{flag}: {}", stderr(&success));
            assert_eq!(
                success.stdout,
                if mode == "run" {
                    b"0\n".as_slice()
                } else {
                    b"".as_slice()
                }
            );
            assert!(success.stderr.is_empty());
            let failure = command()
                .args([mode, flag, rejected])
                .arg(&file)
                .output()
                .unwrap();
            assert_eq!(failure.status.code(), Some(1));
            assert!(failure.stdout.is_empty());
            let diagnostic = stderr(&failure);
            assert!(diagnostic.contains("E0027"), "{diagnostic}");
            assert!(diagnostic.contains(flag), "{diagnostic}");
            assert!(
                diagnostic.contains(&format!("limit of {rejected}")),
                "{diagnostic}"
            );
        }
    }
}

#[test]
fn snapshot_budget_counts_values_separators_empty_lines_and_utf8() {
    let fixture = Fixture::new();
    for (source, expected, bytes) in [
        ("print(1);", "1\n", 72),
        ("print();", "\n", 64),
        ("print(1, 2);", "1 2\n", 209),
        ("print(\"न\");", "न\n", 67),
    ] {
        let file = fixture.file("snapshot.srp", source);
        for mode in ["run", "check"] {
            let success = command()
                .args([mode, "--max-capture-bytes", &bytes.to_string()])
                .arg(&file)
                .output()
                .unwrap();
            assert!(success.status.success(), "{source}: {}", stderr(&success));
            assert_eq!(
                success.stdout,
                if mode == "run" {
                    expected.as_bytes()
                } else {
                    b""
                }
            );
            assert!(success.stderr.is_empty());
            let failure = command()
                .args([mode, "--max-capture-bytes", &(bytes - 1).to_string()])
                .arg(&file)
                .output()
                .unwrap();
            assert_eq!(failure.status.code(), Some(1));
            assert!(failure.stdout.is_empty());
            let diagnostic = stderr(&failure);
            assert!(diagnostic.contains("E0029"), "{diagnostic}");
            assert!(diagnostic.contains("--max-capture-bytes"), "{diagnostic}");
            assert!(
                diagnostic.contains(&format!("limit of {}", bytes - 1)),
                "{diagnostic}"
            );
        }
    }
}

#[test]
fn function_budget_errors_discard_both_program_streams() {
    let fixture = Fixture::new();
    let file = fixture.file(
        "atomic.srp",
        "funcra f() -> unit { f(); } print(\"OUT\"); eprint(\"ERR\"); f();",
    );
    for mode in ["run", "check"] {
        for (flag, count, code) in [
            ("--max-call-depth", "0", "E0027"),
            ("--max-calls", "0", "E0027"),
            ("--max-capture-bytes", "100", "E0029"),
        ] {
            let result = command()
                .args([mode, flag, count])
                .arg(&file)
                .output()
                .unwrap();
            assert_eq!(result.status.code(), Some(1));
            assert!(result.stdout.is_empty());
            assert!(result.stderr.starts_with(b"error["));
            assert!(stderr(&result).contains(code));
        }
    }
    let slots = fixture.file(
        "slots.srp",
        "print(\"OUT\"); eprint(\"ERR\"); bind value = 1;",
    );
    for mode in ["run", "check"] {
        let result = command()
            .args([mode, "--max-live-slots", "0"])
            .arg(&slots)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(1));
        assert!(result.stdout.is_empty());
        assert!(result.stderr.starts_with(b"error[E0027]"));
    }
}

#[test]
fn all_budget_flags_allow_zero_and_mixed_order_without_changing_path_rules() {
    let fixture = Fixture::new();
    fixture.file("--max-calls", "// empty program\n");
    for mode in ["run", "check"] {
        let result = command()
            .args([
                mode,
                "--max-calls",
                "0",
                "--max-prints",
                "0",
                "--max-capture-bytes",
                "0",
                "--max-steps",
                "0",
                "--max-live-slots",
                "0",
                "--max-call-depth",
                "0",
                "--",
                "--max-calls",
            ])
            .current_dir(&fixture.directory)
            .output()
            .unwrap();
        assert!(result.status.success(), "{}", stderr(&result));
        assert!(result.stdout.is_empty());
        assert!(result.stderr.is_empty());
    }
}

#[test]
fn function_budget_option_errors_are_rejected_before_reading_source() {
    let fixture = Fixture::new();
    let missing = fixture.directory.join("missing.srp");
    for flag in [
        "--max-call-depth",
        "--max-calls",
        "--max-live-slots",
        "--max-capture-bytes",
    ] {
        for mode in ["run", "check"] {
            for invalid in [
                "",
                "-1",
                "+1",
                "1.0",
                " 1",
                "1_000",
                "१",
                "18446744073709551616",
            ] {
                let result = command()
                    .args([mode, flag, invalid])
                    .arg(&missing)
                    .output()
                    .unwrap();
                assert_eq!(
                    result.status.code(),
                    Some(2),
                    "{mode} {flag} {invalid:?}: {}",
                    stderr(&result)
                );
                assert!(result.stdout.is_empty());
                assert!(!stderr(&result).contains("could not read"));
            }
            let repeated = command()
                .args([mode, flag, "1", "--max-steps", "100", flag, "2"])
                .arg(&missing)
                .output()
                .unwrap();
            assert_eq!(repeated.status.code(), Some(2));
            assert!(stderr(&repeated).contains("cannot be repeated"));
            let absent = command().args([mode, flag]).output().unwrap();
            assert_eq!(absent.status.code(), Some(2));
        }
        let tokens = command()
            .args(["tokens", flag, "1"])
            .arg(&missing)
            .output()
            .unwrap();
        assert_eq!(tokens.status.code(), Some(2));
        assert!(stderr(&tokens).contains("only for run and check"));
    }
}

#[test]
fn help_lists_all_function_budget_flags_and_defaults() {
    for mode in ["run", "check"] {
        let result = command().args([mode, "--help"]).output().unwrap();
        assert!(result.status.success());
        assert!(result.stderr.is_empty());
        let help = String::from_utf8(result.stdout).unwrap();
        for entry in [
            "--max-call-depth 256",
            "--max-calls 100000",
            "--max-live-slots 1000000",
            "--max-capture-bytes 16777216",
        ] {
            assert!(help.contains(entry), "{entry}: {help}");
        }
    }
}

#[test]
fn conditional_programs_run_and_skipped_static_errors_reach_stderr() {
    let fixture = Fixture::new();
    let valid = fixture.file(
        "decisions.srp",
        "var x = 1; if x < 2 and not false { x += 2; } else { println(1 / 0); } println(x);",
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
    let valid = fixture.file("bindings.srp", "bind value = 6 * 7; println(value);");
    let output = command().arg("run").arg(valid).output().unwrap();
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(output.stdout, b"42\n");
    for (source, code) in [
        ("println(\"first\"); println(missing);", "E0010"),
        ("bind x = 1; bind x = 2;", "E0011"),
        ("bind println = 1;", "E0012"),
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
    let path = fixture.file("mutable.srp", "var value = 1; value += 41; println(value);");
    let output = command().arg("run").arg(path).output().unwrap();
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(output.stdout, b"42\n");
    let path = fixture.file(
        "immutable.srp",
        "bind value = 1; println(value); value = 2;",
    );
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
        "bind x = 1; { bind x = x + 1; println(x); } println(x);",
    );
    let output = command().arg("run").arg(path).output().unwrap();
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(output.stdout, b"2\n1\n");
    let path = fixture.file("leak.srp", "{ bind x = 1; } println(x);");
    let output = command().arg("run").arg(path).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(stderr(&output).contains("E0010"));
}

#[test]
fn check_is_silent_on_success_and_validates_arithmetic() {
    let fixture = Fixture::new();
    let path = fixture.file("check.srp", "var x = 1; x += 2; println(x);");
    let output = command().arg("check").arg(path).output().unwrap();
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(output.stdout.is_empty() && output.stderr.is_empty());
    for (source, code) in [
        ("println(1); println(missing);", "E0010"),
        ("bind x = 1; x = 2;", "E0013"),
        ("println(1); var x = 1; x /= 0;", "E0008"),
        ("bind x = 9223372036854775807 + 1;", "E0007"),
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
    let source = "bind v = 1;".repeat(5_000);
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
fn current_main_keeps_unintegrated_commands_and_syntax_out() {
    for subcommand in ["exec", "verify", "inspect", "compile"] {
        let result = command()
            .args([subcommand, "missing.srp"])
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(2));
    }
    for source in ["println(1.2.3);", "println([1,2]);"] {
        assert!(scarp::compile(source).is_err(), "{source}");
    }
}

#[test]
fn float_calls_and_both_streams_use_existing_cli_commands() {
    let fixture = Fixture::new();
    let path = fixture.file("float.srp", "pure funcra half(n: float=1.0) -> float { return n/2.0; } print(half(), half(n:2.5)); eprint(-0.0);");
    let result = command()
        .args(["run", "--max-calls", "2"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(result.status.success());
    assert_eq!(result.stdout, b"0.5 1.25\n");
    assert_eq!(result.stderr, b"-0.0\n");
    let result = command().arg("check").arg(&path).output().unwrap();
    assert!(result.status.success() && result.stdout.is_empty() && result.stderr.is_empty());
}

#[test]
fn numerical_vector_calls_keep_cli_streams_and_silent_checks() {
    let fixture = Fixture::new();
    let path=fixture.file("vectors.srp","pure funcra f(v: vector=fill(0.5,2)) -> vector { return add(v,v); } print(f()); eprint(dot(f(),[2.0,3.0]));");
    let result = command().arg("run").arg(&path).output().unwrap();
    assert!(result.status.success());
    assert_eq!(result.stdout, b"[1.0, 1.0]\n");
    assert_eq!(result.stderr, b"5.0\n");
    let result = command().arg("check").arg(&path).output().unwrap();
    assert!(result.status.success());
    assert!(result.stdout.is_empty() && result.stderr.is_empty());
    let result = command().arg("tokens").arg(&path).output().unwrap();
    assert!(result.status.success());
    assert!(String::from_utf8_lossy(&result.stdout).contains("float(0.5)"));
}

#[test]
fn numerical_errors_and_vector_budget_failures_discard_both_cli_streams() {
    let fixture = Fixture::new();
    for (expression, code) in [
        ("dot([], [1.0])", "E0032"),
        ("[1.0][-1]", "E0033"),
        ("mean([])", "E0034"),
        ("scale([1e308],2.0)", "E0031"),
        ("[1]", "E0015"),
    ] {
        let source=format!("// café 🌍\nfuncra f() -> unit {{ print({expression}); }} print(\"OUT\");eprint(\"ERR\");f();");
        let path = fixture.file("failure.srp", &source);
        let expected = format!(
            "{}\n",
            scarp::check(&source)
                .unwrap_err()
                .render(&path.to_string_lossy(), &source)
        );
        for verb in ["run", "check"] {
            let result = command().arg(verb).arg(&path).output().unwrap();
            assert_eq!(result.status.code(), Some(1));
            assert!(result.stdout.is_empty());
            assert!(stderr(&result).starts_with(&format!("error[{code}]")));
            assert_eq!(stderr(&result), expected);
        }
    }
    let path = fixture.file("budget.srp", "print([1.0,2.0]); eprint([3.0]);");
    for verb in ["run", "check"] {
        let result = command()
            .args([verb, "--max-capture-bytes", "79"])
            .arg(&path)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(1));
        assert!(result.stdout.is_empty());
        assert!(stderr(&result).starts_with("error[E0029]"));
    }
}

#[test]
fn float_cli_errors_are_source_linked_and_discard_both_pending_streams() {
    let fixture = Fixture::new();
    for (expression, code) in [
        ("1e309", "E0030"),
        ("1.2.3", "E0030"),
        ("1e308*2.0", "E0031"),
        ("1.0/-0.0", "E0008"),
        ("1.0+1", "E0015"),
    ] {
        let path = fixture.file(
            "float-error.srp",
            format!("// café 🌍\r\nprint(\"OUT\"); eprint(\"ERR\"); print({expression});"),
        );
        for verb in ["run", "check"] {
            let result = command().arg(verb).arg(&path).output().unwrap();
            assert_eq!(result.status.code(), Some(1));
            assert!(result.stdout.is_empty());
            let error = stderr(&result);
            assert!(
                error.starts_with(&format!("error[{code}]"))
                    && error.contains("float-error.srp:2:")
            );
            assert!(!error.contains("OUT\n") && !error.contains("ERR\n"));
        }
    }
}

#[test]
fn float_tokens_keep_original_spelling_without_range_evaluation() {
    let fixture = Fixture::new();
    let path = fixture.file("float-tokens.srp", "print(1e9999, 2.5E-4);");
    let result = command().arg("tokens").arg(&path).output().unwrap();
    assert!(result.status.success() && result.stderr.is_empty());
    let text = String::from_utf8(result.stdout).unwrap();
    assert!(text.contains("float(1e9999)") && text.contains("float(2.5E-4)"));
    let result = command().arg("check").arg(&path).output().unwrap();
    assert_eq!(result.status.code(), Some(1));
    assert!(stderr(&result).contains("E0030"));
}

#[test]
fn typed_calls_run_and_check_with_the_existing_cli_contract() {
    let fixture = Fixture::new();
    let path = fixture.file("functions.srp", "pure funcra size(n: integer = 2) -> integer { return n; } repeat size() as i { print(i); }");
    let result = command().arg("run").arg(&path).output().unwrap();
    assert!(result.status.success());
    assert_eq!(result.stdout, b"0\n1\n");
    assert!(result.stderr.is_empty());
    let result = command().arg("check").arg(&path).output().unwrap();
    assert!(result.status.success() && result.stdout.is_empty() && result.stderr.is_empty());
}

#[test]
fn contract_failures_render_traces_without_replaying_output() {
    let fixture = Fixture::new();
    let path = fixture.file("contract.srp", "funcra positive(n: integer) -> integer needs n > 0; { eprint(n); return n; } print(\"pending\"); print(positive(0));");
    for operation in ["run", "check"] {
        let result = command().arg(operation).arg(&path).output().unwrap();
        assert_eq!(result.status.code(), Some(1));
        assert!(result.stdout.is_empty());
        let error = String::from_utf8(result.stderr).unwrap();
        assert!(error.contains("E0026") && error.contains("called funcra positive here"));
        assert!(!error.contains("pending\n"));
    }
}

#[test]
fn output_forms_route_exact_streams_and_check_remains_silent() {
    let fixture = Fixture::new();
    let path = fixture.file("output.srp", "bind name=\"Scarp\"; write(\"ready\"); print(name, true,); ewrite(\"warning\"); eprint(2); print();");
    let output = command().arg("run").arg(&path).output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"readyScarp true\n\n");
    assert_eq!(output.stderr, b"warning2\n");
    let output = command().arg("check").arg(&path).output().unwrap();
    assert!(output.status.success());
    assert!(output.stdout.is_empty() && output.stderr.is_empty());
    let output = command().arg("tokens").arg(&path).output().unwrap();
    assert!(output.status.success() && output.stderr.is_empty());
    assert!(String::from_utf8(output.stdout).unwrap().contains("  ,  ["));
}

#[test]
fn output_failures_do_not_leak_program_stderr_before_diagnostics() {
    let fixture = Fixture::new();
    for tail in ["print(1/0);", "bind x=missing;", "let x=1;"] {
        let path = fixture.file(
            "invalid-output.srp",
            format!("write(\"hidden-out\"); eprint(\"hidden-err\"); {tail}"),
        );
        for mode in ["run", "check"] {
            let output = command().arg(mode).arg(&path).output().unwrap();
            assert_eq!(output.status.code(), Some(1));
            assert!(output.stdout.is_empty());
            let error = stderr(&output);
            assert!(error.starts_with("error[E"));
            // The source excerpt may contain these strings; no replayed bytes precede the diagnostic.
            assert!(!error.starts_with("hidden-err"));
        }
    }
}

#[test]
fn string_variables_run_and_type_errors_are_checked_before_output() {
    let fixture = Fixture::new();
    let path = fixture.file(
        "strings.srp",
        "var text = \"first\"; println(text); text = \"second\"; println(text);",
    );
    let output = command().arg("run").arg(&path).output().unwrap();
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(output.stdout, b"first\nsecond\n");
    fixture.file("strings.srp", "println(1); var text = \"hello\"; text = 1;");
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
