use scarp::{diagnostics::DiagnosticKind, RunError};

#[test]
fn incomplete_declarations_explain_the_form_being_written() {
    for keyword in ["bind", "var"] {
        for (tail, expected, highlighted) in [
            (";", format!("use {keyword} name = expression;"), ";"),
            ("name = ;", "provide an initializer".into(), ";"),
            ("name = 1 + ;", "provide an initializer".into(), ";"),
            ("name = 1", format!("end the {keyword} declaration"), ""),
        ] {
            let source = format!("// café 🌍\r\n{keyword} {tail}");
            let error = scarp::compile(&source).unwrap_err();
            assert_eq!(error.kind, DiagnosticKind::UnexpectedToken);
            assert!(error.help.as_deref().unwrap().contains(&expected));
            assert_eq!(&source[error.span.start..error.span.end], highlighted);
            assert!(error
                .render("variable.srp", &source)
                .contains("variable.srp:2:"));
        }
    }
}

#[test]
fn incomplete_assignments_explain_values_and_statement_termination() {
    for operator in ["=", "+=", "-=", "*=", "/="] {
        for (tail, expected) in [(";", "value expression"), ("1", "end the assignment")] {
            let source = format!("var count=0; count {operator} {tail}");
            let error = scarp::compile(&source).unwrap_err();
            assert_eq!(error.kind, DiagnosticKind::UnexpectedToken);
            assert!(error.help.as_deref().unwrap().contains(expected));
        }
    }
}

#[test]
fn declaration_context_preserves_specific_expression_failures() {
    for keyword in ["bind", "var"] {
        let source = format!("{keyword} value = 9223372036854775808;");
        assert_eq!(
            scarp::compile(&source).unwrap_err().kind,
            DiagnosticKind::InvalidInteger
        );
        let source = format!("{keyword} value = {}1{};", "(".repeat(65), ")".repeat(65));
        let error = scarp::compile(&source).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::ExpressionNesting);
        assert!(!error.help.unwrap_or_default().contains("initializer"));
    }
}

#[test]
fn invalid_variable_statements_discard_both_pending_streams() {
    for tail in [
        "bind missing = ;",
        "var missing = 1",
        "var count=0; count += ;",
    ] {
        let source = format!("print(\"hidden\"); eprint(\"hidden\"); {tail}");
        let expected = scarp::check(&source).unwrap_err();
        let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
        let RunError::Diagnostic(actual) =
            scarp::run_with_io(&source, &mut stdout, &mut stderr).unwrap_err()
        else {
            panic!("expected diagnostic");
        };
        assert_eq!(actual, expected);
        assert!(stdout.is_empty() && stderr.is_empty());
    }
}

#[test]
fn variable_forms_keep_combined_block_and_expression_nesting_boundaries() {
    let grouped = format!("{}1{}", "(".repeat(64), ")".repeat(64));
    for statement in [
        format!("bind value={grouped}; print(value);"),
        format!("var value=0; value+={grouped}; print(value);"),
    ] {
        let source = format!("{}{statement}{}", "{".repeat(64), "}".repeat(64));
        let mut output = Vec::new();
        scarp::run(&source, &mut output).unwrap();
        assert_eq!(output, b"1\n");
        assert_eq!(
            scarp::compile(&format!("{{{source}}}")).unwrap_err().kind,
            DiagnosticKind::BlockNesting
        );
    }
}
