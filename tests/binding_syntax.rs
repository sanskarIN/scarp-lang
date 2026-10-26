use scarp::{
    ast::Statement,
    diagnostics::{Diagnostic, DiagnosticKind, Span},
    interpreter,
    lexer::{lex, TokenKind},
    ExecutionLimits, RunError,
};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

fn error(source: &str) -> Diagnostic {
    let mut bytes = Vec::new();
    let failure = scarp::run(source, &mut bytes).unwrap_err();
    assert!(bytes.is_empty(), "failed program emitted output: {source}");
    match failure {
        RunError::Diagnostic(error) => error,
        other => panic!("expected source diagnostic: {other}"),
    }
}

#[test]
fn declarations_expose_neutral_ast_variants_and_original_spans() {
    let source = "// café 🌍\r\nbind fixed = 1;\r\nvar changing = fixed;";
    let syntax = scarp::parse(source).unwrap();
    for (statement, expected, mutable) in [
        (&syntax.statements[0], "bind fixed = 1;", false),
        (&syntax.statements[1], "var changing = fixed;", true),
    ] {
        let Statement::Declaration {
            mutable: actual,
            name,
            name_span,
            span,
            ..
        } = statement
        else {
            panic!("{statement:?}")
        };
        assert_eq!(*actual, mutable);
        assert_eq!(&source[span.start..span.end], expected);
        assert_eq!(&source[name_span.start..name_span.end], name);
        assert_eq!(statement.span(), *span);
    }
}

#[test]
fn lexer_keeps_keywords_contextual_and_names_whole() {
    let tokens = lex("bind var let mut binder variable").unwrap();
    for (token, expected) in tokens
        .iter()
        .zip(["bind", "var", "let", "mut", "binder", "variable"])
    {
        assert_eq!(token.kind, TokenKind::Identifier(expected.into()));
    }
    let mut output = Vec::new();
    scarp::run(
        "bind binder = 6; var variable = binder; variable *= 7; println(variable);",
        &mut output,
    )
    .unwrap();
    assert_eq!(output, b"42\n");
}

#[test]
fn legacy_declarations_have_precise_migration_diagnostics() {
    for (source, old, new) in [
        ("let x = 1;", "let", "bind"),
        ("let mut x = 1;", "let mut", "var"),
        ("// café 🌍\r\nlet\tmut x = 1;", "let\tmut", "var"),
        (
            "\u{feff}let // trivia\r\n mut x = 1;",
            "let // trivia\r\n mut",
            "var",
        ),
        ("println(1); let x = 1 / 0;", "let", "bind"),
    ] {
        let diagnostic = error(source);
        assert_eq!(
            diagnostic.kind,
            DiagnosticKind::UnknownStatement,
            "{source}"
        );
        assert_eq!(diagnostic.kind.code(), "E0005");
        assert_eq!(&source[diagnostic.span.start..diagnostic.span.end], old);
        let help = diagnostic.help.unwrap();
        assert!(help.contains("replace"), "{help}");
        assert!(help.contains(new), "{help}");
        assert_eq!(
            scarp::compile(source).unwrap_err().kind,
            DiagnosticKind::UnknownStatement
        );
    }
    let source = "// नमस्ते\r\nlet mut x = 1;";
    let rendered = error(source).render("migration.srp", source);
    assert!(rendered.contains("migration.srp:2:1"), "{rendered}");
    assert!(
        rendered.contains("replace \"let mut\" with \"var\""),
        "{rendered}"
    );
}

#[test]
fn malformed_modifiers_and_missing_initializers_have_repairs() {
    for source in ["bind mut x = 1;", "var mut x = 1;", "bind\nmut\tx = 1;"] {
        let diagnostic = error(source);
        assert_eq!(diagnostic.kind, DiagnosticKind::UnexpectedToken);
        assert_eq!(&source[diagnostic.span.start..diagnostic.span.end], "mut");
        assert!(diagnostic.help.unwrap().contains("var name = expression;"));
    }
    for source in ["var x;", "bind x;", "var x", "bind x"] {
        let diagnostic = error(source);
        assert_eq!(diagnostic.kind, DiagnosticKind::UnexpectedToken);
        assert!(diagnostic.help.unwrap().contains("initializer"));
    }
    for source in ["const x = 1;", "val x = 1;", "mut x = 1;"] {
        assert_eq!(error(source).kind, DiagnosticKind::UnknownStatement);
    }
}

#[test]
fn declaration_and_legacy_words_are_reserved_names() {
    for name in ["bind", "var", "let", "mut"] {
        for source in [
            format!("bind {name} = 1;"),
            format!("var {name} = 1;"),
            format!("println({name});"),
        ] {
            let diagnostic = error(&source);
            assert_eq!(diagnostic.kind, DiagnosticKind::ReservedName, "{source}");
            assert_eq!(&source[diagnostic.span.start..diagnostic.span.end], name);
        }
    }
}

#[test]
fn immutable_assignment_preserves_all_compound_errors_and_related_spans() {
    for op in [
        "=", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "<<=", ">>=", ">>>=",
    ] {
        let source = format!("// π\r\nbind value = 1;\r\nvalue {op} 2;");
        let diagnostic = error(&source);
        assert_eq!(diagnostic.kind, DiagnosticKind::ImmutableAssignment);
        assert_eq!(&source[diagnostic.span.start..diagnostic.span.end], "value");
        let declaration = source.find("value").unwrap();
        assert_eq!(
            diagnostic.related[0].span,
            Span::new(declaration, declaration + 5)
        );
        assert!(diagnostic.help.as_ref().unwrap().contains("var"));
        let rendered = diagnostic.render("immutable.srp", &source);
        assert!(rendered.contains("immutable.srp:2:6"), "{rendered}");
        assert!(rendered.contains("immutable.srp:3:1"), "{rendered}");
    }
}

#[test]
fn shadowing_snapshots_and_same_type_mutation_remain_intact() {
    let source = r#"
        var count = 1;
        bind original = count;
        { bind count = count + 10; println(count); }
        count += 2;
        var text = "before"; bind saved = text; text = "after";
        var flag = false; flag = true;
        var decimal = 1.0; decimal += 0.5;
        var numbers = [1.0]; bind snapshot = numbers; numbers = [2.0];
        println(original); println(count); println(saved); println(text);
        println(flag); println(decimal); println(snapshot[0]); println(numbers[0]);
    "#;
    let mut output = Vec::new();
    scarp::run(source, &mut output).unwrap();
    assert_eq!(output, b"11\n1\n3\nbefore\nafter\ntrue\n1.5\n1.0\n2.0\n");
    for source in [
        "var x = 1; x = 1.0;",
        "var x = false; x = 0;",
        "var x = [1.0]; x = 1.0;",
    ] {
        assert_eq!(error(source).kind, DiagnosticKind::TypeMismatch);
    }
}

#[test]
fn declaration_order_and_full_file_validation_remain_intact() {
    for (source, kind) in [
        ("bind x = x + 1;", DiagnosticKind::UnknownName),
        (
            "var x = later; bind later = 1;",
            DiagnosticKind::UnknownName,
        ),
        ("bind x = 1; var x = 2;", DiagnosticKind::DuplicateBinding),
        (
            "println(1); var unused = 1 / 0;",
            DiagnosticKind::DivisionByZero,
        ),
        (
            "println(1); if false { bind x = missing; }",
            DiagnosticKind::UnknownName,
        ),
    ] {
        assert_eq!(error(source).kind, kind, "{source}");
    }
    let program = scarp::compile("var x = 1 / 0; println(x);").unwrap();
    assert_eq!(
        interpreter::validate(&program).unwrap_err().kind,
        DiagnosticKind::DivisionByZero
    );
}

#[test]
fn keywords_in_strings_comments_and_unicode_trivia_preserve_text() {
    let source = "// let mut bind var 🌍\r\nbind\u{2003}binder = \"let mut bind var // café\";\r\nvar // bind mut\r\nvariable = binder;\r\nprintln(variable);";
    let mut output = Vec::new();
    scarp::run(source, &mut output).unwrap();
    assert_eq!(
        String::from_utf8(output).unwrap(),
        "let mut bind var // café\n"
    );
}

#[test]
fn budgets_and_repeated_execution_keep_fresh_storage_and_atomic_output() {
    let program =
        scarp::compile("var x = 0; while x < 3 { x += 1; } bind saved = x; println(saved);")
            .unwrap();
    for _ in 0..3 {
        let mut output = Vec::new();
        interpreter::execute(&program, &mut output).unwrap();
        assert_eq!(output, b"3\n");
    }
    for (limits, kind) in [
        (
            ExecutionLimits {
                max_steps: 1,
                max_prints: 10,
            },
            DiagnosticKind::ExecutionLimit,
        ),
        (
            ExecutionLimits {
                max_steps: 1000,
                max_prints: 0,
            },
            DiagnosticKind::OutputLimit,
        ),
    ] {
        let mut output = Vec::new();
        let failure = interpreter::execute_with_limits(&program, &mut output, limits).unwrap_err();
        assert!(output.is_empty());
        let RunError::Diagnostic(diagnostic) = failure else {
            panic!("{failure}")
        };
        assert_eq!(diagnostic.kind, kind);
    }
}

static NEXT: AtomicU64 = AtomicU64::new(0);
struct SourceFile(PathBuf);
impl SourceFile {
    fn new(source: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "scarp-binding-syntax-{}-{}.srp",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        file.write_all(source.as_bytes()).unwrap();
        Self(path)
    }
    fn command(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_scarp"))
            .args(args)
            .arg(&self.0)
            .output()
            .unwrap()
    }
}
impl Drop for SourceFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[test]
fn canonical_cli_exec_verify_and_inspection_share_the_new_frontend() {
    let file = SourceFile::new("bind base = 6; var total = base; total *= 7; println(total);");
    let executed = file.command(&["exec"]);
    assert!(executed.status.success(), "{executed:?}");
    assert_eq!(executed.stdout, b"42\n");
    let verified = file.command(&["verify"]);
    assert!(verified.status.success(), "{verified:?}");
    assert!(verified.stdout.is_empty());
    let tokens = file.command(&["inspect", "tokens"]);
    assert!(tokens.status.success(), "{tokens:?}");
    let tokens = String::from_utf8(tokens.stdout).unwrap();
    assert!(tokens.contains("bind"));
    assert!(tokens.contains("var"));
    let ast = file.command(&["inspect", "ast"]);
    assert!(ast.status.success(), "{ast:?}");
    let ast = String::from_utf8(ast.stdout).unwrap();
    assert!(ast.contains("Declaration"));
    assert!(ast.contains("mutable: false"));
    assert!(ast.contains("mutable: true"));
    assert!(!ast.contains("Let {"));
    let arithmetic = SourceFile::new("var x = 1 / 0;");
    assert!(arithmetic.command(&["verify"]).status.success());
    assert_eq!(arithmetic.command(&["exec"]).status.code(), Some(1));
}

#[test]
fn legacy_cli_errors_are_consistent_and_c_emission_preserves_outputs() {
    for (source, replacement) in [
        ("println(1); let x = 1;", "bind"),
        ("let mut x = 1;", "var"),
    ] {
        let file = SourceFile::new(source);
        for args in [vec!["exec"], vec!["verify"], vec!["inspect", "ast"]] {
            let output = file.command(&args);
            assert_eq!(output.status.code(), Some(1), "{output:?}");
            assert!(output.stdout.is_empty());
            let stderr = String::from_utf8(output.stderr).unwrap();
            assert!(stderr.contains("E0005"), "{stderr}");
            assert!(stderr.contains(replacement), "{stderr}");
        }
        // Token inspection intentionally accepts lexically valid legacy words.
        assert!(file.command(&["inspect", "tokens"]).status.success());
        let destination = SourceFile::new("preserve existing output");
        let output = Command::new(env!("CARGO_BIN_EXE_scarp"))
            .args(["compile", "--emit", "c"])
            .arg(&file.0)
            .arg("-o")
            .arg(&destination.0)
            .env("SCARP_CC", "compiler-must-not-run")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert_eq!(
            fs::read(&destination.0).unwrap(),
            b"preserve existing output"
        );
    }
}
