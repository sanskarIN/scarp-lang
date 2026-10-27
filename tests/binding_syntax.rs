use scarp::{ast::Statement, diagnostics::DiagnosticKind, lexer::TokenKind, RunError};

#[test]
fn legacy_declarations_have_precise_migration_help_before_output() {
    for (declaration, replacement, highlighted) in [
        ("let value = 1;", "bind", "let"),
        ("let mut value = 1;", "var", "let mut"),
        (
            "let // old form\r\n mut value = 1;",
            "var",
            "let // old form\r\n mut",
        ),
    ] {
        let source = format!("// café 🌍\r\nprintln(42); {declaration}");
        let mut bytes = Vec::new();
        let RunError::Diagnostic(error) = scarp::run(&source, &mut bytes).unwrap_err() else {
            panic!("expected a migration diagnostic");
        };
        assert!(bytes.is_empty());
        assert_eq!(error.kind, DiagnosticKind::UnknownStatement);
        assert_eq!(&source[error.span.start..error.span.end], highlighted);
        assert!(error.render("legacy.srp", &source).contains(replacement));
    }
}

#[test]
fn declaration_nodes_expose_mutability_and_source_spans() {
    let source = "bind fixed = 1; var changing = fixed;";
    let ast = scarp::parse(source).unwrap();
    for (statement, expected) in ast.statements.iter().zip([false, true]) {
        let Statement::Declaration {
            mutable,
            name_span,
            span,
            ..
        } = statement
        else {
            panic!("expected a declaration");
        };
        assert_eq!(*mutable, expected);
        assert!(source[span.start..span.end].ends_with(';'));
        assert_eq!(
            &source[name_span.start..name_span.end],
            if expected { "changing" } else { "fixed" }
        );
    }
}

#[test]
fn new_words_do_not_rewrite_string_contents_comments_or_longer_identifiers() {
    let source = "// let mut must remain text\nbind letter = \"let mut bind var\"; var variable = letter; println(variable);";
    let mut bytes = Vec::new();
    scarp::run(source, &mut bytes).unwrap();
    assert_eq!(bytes, b"let mut bind var\n");
    assert!(
        matches!(&scarp::lexer::lex("bind var").unwrap()[0].kind, TokenKind::Identifier(word) if word == "bind")
    );
}

#[test]
fn declaration_keywords_require_initializers_and_reject_mut_modifiers() {
    for source in ["bind x;", "var x;", "bind mut x = 1;", "var mut x = 1;"] {
        assert!(scarp::compile(source).is_err(), "{source}");
    }
    for name in ["bind", "var", "let", "mut"] {
        assert_eq!(
            scarp::compile(&format!("bind {name} = 1;"))
                .unwrap_err()
                .kind,
            DiagnosticKind::ReservedName
        );
    }
}

#[test]
fn new_declarations_preserve_shadowing_repeat_snapshots_and_fresh_storage() {
    let program = scarp::compile("var count=3; var total=0; repeat count as i { bind next=i+1; { var next=next; next+=1; total+=next; } count=0; } println(total); println(count);").unwrap();
    for _ in 0..3 {
        let mut bytes = Vec::new();
        scarp::interpreter::execute(&program, &mut bytes).unwrap();
        assert_eq!(bytes, b"9\n0\n");
    }
}

#[test]
fn published_variable_examples_migrate_by_tokens_and_keep_output_bytes() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/releases/v0.0.2/examples");
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|s| s.to_str()) != Some("srp") {
            continue;
        }
        let source = std::fs::read_to_string(&path).unwrap();
        let tokens = scarp::lexer::lex(&source).unwrap();
        let mut migrated = source.clone();
        let mut replacements = Vec::new();
        let mut cursor = 0;
        while cursor < tokens.len() {
            if matches!(&tokens[cursor].kind, TokenKind::Identifier(word) if word == "let") {
                let mutable = tokens.get(cursor + 1).is_some_and(
                    |token| matches!(&token.kind, TokenKind::Identifier(word) if word == "mut"),
                );
                let end = tokens[cursor + usize::from(mutable)].span.end;
                replacements.push((
                    tokens[cursor].span.start..end,
                    if mutable { "var" } else { "bind" },
                ));
                cursor += usize::from(mutable);
            }
            cursor += 1;
        }
        if !replacements.is_empty() {
            assert_eq!(
                scarp::compile(&source).unwrap_err().kind,
                DiagnosticKind::UnknownStatement
            );
        }
        for (span, replacement) in replacements.into_iter().rev() {
            migrated.replace_range(span, replacement);
        }
        let mut bytes = Vec::new();
        scarp::run(&migrated, &mut bytes).unwrap();
        assert_eq!(
            bytes,
            std::fs::read(path.with_extension("stdout")).unwrap(),
            "{}",
            path.display()
        );
    }
}
