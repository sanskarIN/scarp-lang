use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let directory = std::env::temp_dir().join(format!(
            "scarp-foundation-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).unwrap();
        Self(directory)
    }
    fn source(&self, source: &str) -> PathBuf {
        let path = self.0.join("source café.srp");
        fs::write(&path, source).unwrap();
        path
    }
    fn entries(&self) -> Vec<std::ffi::OsString> {
        let mut names: Vec<_> = fs::read_dir(&self.0)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        names.sort();
        names
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
fn success(output: &Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn exec_routes_program_output_streams_and_static_verification_stays_silent() {
    let fixture = Fixture::new();
    let path = fixture.source(
        "write(\"Loading\"); ewrite(\"Warning\"); print(\"...\", true); eprint(\"!\", 2,);",
    );
    let result = command().arg("exec").arg(&path).output().unwrap();
    assert!(result.status.success());
    assert_eq!(result.stdout, b"Loading... true\n");
    assert_eq!(result.stderr, b"Warning! 2\n");
    let checked = command().arg("verify").arg(&path).output().unwrap();
    assert!(checked.status.success());
    assert!(checked.stdout.is_empty() && checked.stderr.is_empty());
    let tree = command()
        .args(["inspect", "ast"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(tree.status.success() && tree.stderr.is_empty());
    let tree = String::from_utf8(tree.stdout).unwrap();
    assert!(tree.contains("Output") && tree.contains("Stderr") && tree.contains("newline: false"));
}

#[test]
fn cli_failures_suppress_both_program_streams_and_keep_diagnostics_separate() {
    let fixture = Fixture::new();
    for tail in ["print(missing);", "print(1 / 0);"] {
        let path = fixture.source(&format!(
            "print(\"suppress stdout\"); eprint(\"suppress program stderr\"); {tail}"
        ));
        let result = command().arg("exec").arg(&path).output().unwrap();
        assert_eq!(result.status.code(), Some(1));
        assert!(result.stdout.is_empty());
        let expected = scarp::check(&fs::read_to_string(&path).unwrap())
            .unwrap_err()
            .render(&path.to_string_lossy(), &fs::read_to_string(&path).unwrap())
            + "\n";
        assert_eq!(result.stderr, expected.as_bytes());
    }
    let path = fixture.source("eprint(1 / 0);");
    let result = command().arg("verify").arg(&path).output().unwrap();
    assert!(result.status.success() && result.stdout.is_empty() && result.stderr.is_empty());
}

#[test]
fn guide_help_and_flags_agree_without_requiring_files() {
    let expected = command().arg("--help").output().unwrap();
    success(&expected);
    for args in [
        vec![],
        vec!["-h"],
        vec!["guide"],
        vec!["guide", "--help"],
        vec!["guide", "-h"],
    ] {
        let output = command().args(&args).output().unwrap();
        success(&output);
        assert_eq!(output.stdout, expected.stdout, "{args:?}");
        assert!(output.stderr.is_empty());
    }
    let text = String::from_utf8(expected.stdout).unwrap();
    for implemented in ["exec", "compile", "verify", "inspect", "guide", "version"] {
        assert!(text.contains(implemented));
    }
    assert_eq!(command().output().unwrap().status.code(), Some(0));
}

#[test]
fn short_version_aliases_agree() {
    for argument in ["--version", "-V", "version"] {
        let output = command().arg(argument).output().unwrap();
        success(&output);
        assert_eq!(
            output.stdout,
            format!("scarp {}\n", env!("CARGO_PKG_VERSION")).as_bytes()
        );
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn contextual_help_matches_each_command_and_nested_topic() {
    for verb in ["exec", "compile", "verify", "inspect", "guide", "version"] {
        let expected = command().args([verb, "--help"]).output().unwrap();
        success(&expected);
        assert!(String::from_utf8_lossy(&expected.stdout).contains("Usage:"));
        {
            let alias = "guide";
            let output = command().args([alias, verb]).output().unwrap();
            success(&output);
            assert_eq!(output.stdout, expected.stdout, "{alias} {verb}");
            assert!(output.stderr.is_empty());
        }
    }
    for subcommand in ["tokens", "ast"] {
        let expected = command()
            .args(["inspect", subcommand, "--help"])
            .output()
            .unwrap();
        success(&expected);
        assert!(
            String::from_utf8_lossy(&expected.stdout).contains(&format!("inspect {subcommand}"))
        );
        let guided = command()
            .args(["guide", "inspect", subcommand])
            .output()
            .unwrap();
        success(&guided);
        assert_eq!(guided.stdout, expected.stdout);
    }
}

#[test]
fn exec_executes_source_and_preserves_runtime_diagnostics() {
    let fixture = Fixture::new();
    for (source, expected, code) in [
        ("var n=3; n+=2; println(n);", b"5\n".as_slice(), None),
        ("println(1); println(1/0);", b"".as_slice(), Some("E0008")),
        ("println(missing);", b"".as_slice(), Some("E0010")),
    ] {
        let path = fixture.source(source);
        let exec = command().arg("exec").arg(&path).output().unwrap();
        assert_eq!(exec.stdout, expected);
        if let Some(code) = code {
            assert_eq!(exec.status.code(), Some(1));
            assert!(String::from_utf8_lossy(&exec.stderr).contains(code));
        } else {
            success(&exec);
            assert!(exec.stderr.is_empty());
        }
    }
    let path = fixture.source("println(1);");
    let output = command()
        .args(["exec", "--max-steps", "0"])
        .arg(path)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("E0017"));
    assert!(output.stdout.is_empty());
}

#[test]
fn verify_is_static_and_does_not_execute_arithmetic_or_loops() {
    let fixture = Fixture::new();
    for (source, runtime_code) in [
        ("println(1); println(1/0);", "E0008"),
        ("while true { continue; }", "E0017"),
    ] {
        let path = fixture.source(source);
        let before = fixture.entries();
        let output = command().arg("verify").arg(&path).output().unwrap();
        success(&output);
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
        assert_eq!(fixture.entries(), before);
        assert_eq!(fs::read_to_string(&path).unwrap(), source);
        let checked = command()
            .args(["exec", "--max-steps", "30"])
            .arg(path)
            .output()
            .unwrap();
        assert_eq!(checked.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&checked.stderr).contains(runtime_code));
    }
}

#[test]
fn verify_reports_actual_static_errors_and_unicode_crlf_locations() {
    let fixture = Fixture::new();
    for (source, code) in [
        ("println(missing);", "E0010"),
        ("println(1+true);", "E0015"),
        ("println(1)", "E0004"),
        ("// π\r\nbind x=true;\r\nx=false;", "E0013"),
    ] {
        let path = fixture.source(source);
        let output = command().arg("verify").arg(&path).output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        let errors = String::from_utf8_lossy(&output.stderr);
        assert!(errors.contains(code));
        assert!(errors.contains("source café.srp"));
        if code == "E0013" {
            assert!(errors.contains(":3:1"));
        }
        assert_eq!(fs::read_to_string(path).unwrap(), source);
    }
}

#[test]
fn inspect_tokens_shows_lexer_spans_without_requiring_valid_syntax() {
    let fixture = Fixture::new();
    let path = fixture.source("println(missing) // parsed only by the AST command");
    let inspected = command()
        .args(["inspect", "tokens"])
        .arg(&path)
        .output()
        .unwrap();
    success(&inspected);
    assert!(inspected.stderr.is_empty());
    let tokens = String::from_utf8(inspected.stdout).unwrap();
    assert!(tokens.contains("1:1..1:8  identifier(println)  [0..7]"));
    assert!(tokens.contains("identifier(missing)"));
    assert!(tokens.contains("<eof>"));
    let parsed = command()
        .args(["inspect", "ast"])
        .arg(path)
        .output()
        .unwrap();
    assert_eq!(parsed.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&parsed.stderr).contains("E0004"));
}

#[test]
fn ast_inspection_parses_real_syntax_without_resolution_or_execution() {
    let fixture = Fixture::new();
    let source = "bind count=missing; println(1/0);";
    let path = fixture.source(source);
    let before = fixture.entries();
    let output = command()
        .args(["inspect", "ast"])
        .arg(&path)
        .output()
        .unwrap();
    success(&output);
    assert!(output.stderr.is_empty());
    let tree = String::from_utf8(output.stdout).unwrap();
    for fragment in [
        "Program",
        "Declaration",
        "name: \"count\"",
        "missing",
        "Println",
        "Divide",
        "Span",
    ] {
        assert!(tree.contains(fragment), "{fragment}: {tree}");
    }
    assert_eq!(fixture.entries(), before);
    assert_eq!(fs::read_to_string(path).unwrap(), source);
}

#[test]
fn filename_separators_and_unicode_paths_work_in_all_new_file_commands() {
    let fixture = Fixture::new();
    let name = "-café.srp";
    fs::write(fixture.0.join(name), "println(7);").unwrap();
    for prefix in [
        vec!["exec"],
        vec!["verify"],
        vec!["inspect", "tokens"],
        vec!["inspect", "ast"],
    ] {
        let output = command()
            .current_dir(&fixture.0)
            .args(&prefix)
            .args(["--", name])
            .output()
            .unwrap();
        success(&output);
        assert!(output.stderr.is_empty());
        if prefix == ["exec"] {
            assert_eq!(output.stdout, b"7\n");
        }
        if prefix == ["verify"] {
            assert!(output.stdout.is_empty());
        }
    }
}

#[test]
fn unsupported_options_project_modes_and_program_arguments_fail_explicitly() {
    let fixture = Fixture::new();
    let path = fixture.source("println(7);");
    for args in [
        vec!["guide", "made-up"],
        vec!["guide", "inspect", "ir"],
        vec!["version", "--verbose"],
        vec!["version", "--json"],
        vec!["verify", "--strict"],
        vec!["verify", "--json"],
        vec!["verify", "--max-steps", "10"],
        vec!["verify"],
        vec!["exec"],
        vec!["inspect"],
        vec!["inspect", "types"],
        vec!["inspect", "ir"],
        vec!["compile", "--release"],
        vec!["ast"],
    ] {
        let output = command().args(&args).output().unwrap();
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("Usage:"));
    }
    let output = command()
        .arg("exec")
        .arg(&path)
        .args(["--", "alice", "42"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
}

#[test]
fn command_suggestions_never_execute_or_change_the_command() {
    let fixture = Fixture::new();
    let path = fixture.source("println(42);");
    let output = command().arg("verfy").arg(path).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("did you mean 'verify'"));
}

#[test]
fn production_help_lists_exactly_the_supported_canonical_groups() {
    let output = command().arg("guide").output().unwrap();
    success(&output);
    let text = String::from_utf8(output.stdout).unwrap();
    let commands: Vec<_> = text
        .lines()
        .filter_map(|line| line.strip_prefix("  "))
        .filter_map(|line| line.split_whitespace().next())
        .filter(|word| *word != "scarp")
        .collect();
    assert_eq!(
        commands,
        ["exec", "compile", "verify", "inspect", "guide", "version"]
    );
}

#[test]
fn retired_command_names_fail_without_dispatching_or_creating_artifacts() {
    let fixture = Fixture::new();
    let source = fixture.source("println(42);");
    let before = fixture.entries();
    for (old, preferred) in [
        ("run", "exec"),
        ("build", "compile"),
        ("check", "verify"),
        ("tokens", "inspect tokens"),
        ("emit-c", "compile --emit c"),
        ("help", "guide"),
    ] {
        let output = command()
            .current_dir(&fixture.0)
            .arg(old)
            .arg(&source)
            .args(["-o", "must-not-exist"])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{old}");
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains(&format!("scarp {preferred}")));
        let help = command().args([old, "--help"]).output().unwrap();
        assert_eq!(help.status.code(), Some(2));
        assert!(help.stdout.is_empty());
        assert_eq!(
            command()
                .args(["guide", old])
                .output()
                .unwrap()
                .status
                .code(),
            Some(2)
        );
        assert_eq!(fixture.entries(), before);
    }
}

#[test]
fn proposed_unimplemented_groups_are_not_advertised_as_working_commands() {
    let fixture = Fixture::new();
    let help = command().arg("guide").output().unwrap();
    let help = String::from_utf8(help.stdout).unwrap();
    let planned = [
        "create",
        "init",
        "launch",
        "test",
        "clean",
        "fmt",
        "lint",
        "polish",
        "shell",
        "eval",
        "watch",
        "debug",
        "bench",
        "coverage",
        "profile",
        "explain",
        "fix",
        "doc",
        "examples",
        "generate",
        "diagnose",
        "env",
        "config",
        "metadata",
        "workspace",
        "task",
        "migrate",
        "ci",
        "packages",
        "registry",
        "publish",
        "bundle",
        "audit",
        "toolchain",
        "target",
        "cache",
        "completions",
        "lsp",
        "self",
        "artifacts",
    ];
    for command_name in planned {
        assert!(!help
            .lines()
            .any(|line| line.trim_start().starts_with(&format!("{command_name} "))));
        let output = command()
            .current_dir(&fixture.0)
            .args([command_name, "--help"])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{command_name}");
        assert!(output.stdout.is_empty());
    }
    assert!(fixture.entries().is_empty());
}
