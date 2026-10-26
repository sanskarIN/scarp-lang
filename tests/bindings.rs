use scarp::{
    compile,
    diagnostics::{Diagnostic, DiagnosticKind},
    interpreter, run, RunError,
};

fn output(source: &str) -> String {
    let mut output = Vec::new();
    run(source, &mut output).unwrap();
    String::from_utf8(output).unwrap()
}

fn error(source: &str) -> Diagnostic {
    let mut output = Vec::new();
    let error = run(source, &mut output).unwrap_err();
    assert!(output.is_empty());
    match error {
        RunError::Diagnostic(error) => error,
        error => panic!("expected a diagnostic, got {error}"),
    }
}

#[test]
fn declarations_can_feed_later_bindings_and_print_arguments() {
    assert_eq!(
        output("let width = 6; let area = width * 7; println(area); println(-area + 1);"),
        "42\n-41\n"
    );
}

#[test]
fn binding_only_programs_do_not_write_anything() {
    assert_eq!(output("let value = 40 + 2;"), "");
    let program = compile("let a = 1; let b = a + 1;").unwrap();
    assert_eq!(program.binding_count(), 2);
    assert_eq!(program.statement_count(), 2);
}

#[test]
fn initializers_keep_integer_boundaries_and_arithmetic_rules() {
    assert_eq!(output("let low = -9223372036854775808; let high = 9223372036854775807; println(low); println(high);"), format!("{}\n{}\n", i64::MIN, i64::MAX));
    for (source, expected) in [
        ("let x = 1 / 0;", DiagnosticKind::DivisionByZero),
        (
            "let x = 9223372036854775807 + 1;",
            DiagnosticKind::ArithmeticOverflow,
        ),
        (
            "let x = 9223372036854775808;",
            DiagnosticKind::InvalidInteger,
        ),
    ] {
        assert_eq!(error(source).kind, expected);
    }
}

#[test]
fn unknown_names_point_to_the_reference() {
    let source = "// café 🌍\r\nlet value = missing + 1;";
    let diagnostic = error(source);
    assert_eq!(diagnostic.kind, DiagnosticKind::UnknownName);
    assert_eq!(
        &source[diagnostic.span.start..diagnostic.span.end],
        "missing"
    );
    let rendered = diagnostic.render("names.srp", source);
    assert!(rendered.contains("names.srp:2:13"), "{rendered}");
}

#[test]
fn forward_and_self_references_are_not_visible() {
    for source in [
        "println(later); let later = 1;",
        "let x = x + 1;",
        "let a = b; let b = a;",
    ] {
        assert_eq!(error(source).kind, DiagnosticKind::UnknownName);
    }
}

#[test]
fn duplicate_declarations_include_the_original_location() {
    let source = "let value = 1;\nlet value = 2;";
    let diagnostic = error(source);
    assert_eq!(diagnostic.kind, DiagnosticKind::DuplicateBinding);
    assert_eq!(&source[diagnostic.span.start..diagnostic.span.end], "value");
    assert_eq!(diagnostic.related.len(), 1);
    assert_eq!(diagnostic.related[0].span.start, 4);
    let rendered = diagnostic.render("duplicate.srp", source);
    assert!(rendered.contains("duplicate.srp:2:5"));
    assert!(rendered.contains("duplicate.srp:1:5"));
    assert!(rendered.contains("previous declaration"));
}

#[test]
fn reserved_names_are_rejected_as_declarations_and_references() {
    for name in ["let", "mut", "println"] {
        let declaration = if name == "mut" {
            "let mut mut = 1;".to_string()
        } else {
            format!("let {name} = 1;")
        };
        for source in [declaration, format!("println({name});")] {
            assert_eq!(
                error(&source).kind,
                DiagnosticKind::ReservedName,
                "{source}"
            );
        }
    }
}

#[test]
fn names_are_case_sensitive_and_underscore_is_an_ordinary_binding() {
    assert_eq!(
        output("let Value = 1; let value = 2; let _ = 3; let x2 = Value + value + _; println(x2);"),
        "6\n"
    );
    assert_eq!(
        error("let value = 1; println(Value);").kind,
        DiagnosticKind::UnknownName
    );
}

#[test]
fn malformed_declarations_are_diagnosed_without_panics() {
    for source in [
        "let",
        "let x",
        "let = 1;",
        "let 1 = 2;",
        "let x 1;",
        "let x =;",
        "let x = 1",
        "let x = \"text\";",
    ] {
        assert_eq!(
            error(source).kind,
            DiagnosticKind::UnexpectedToken,
            "{source}"
        );
    }
    assert_eq!(
        error("let x = 1, y = 2;").kind,
        DiagnosticKind::InvalidCharacter
    );
}

#[test]
fn each_execution_of_a_compiled_program_has_fresh_storage() {
    let compiled = compile("let a = 7; let b = a * a; println(b);").unwrap();
    for _ in 0..20 {
        let mut output = Vec::new();
        interpreter::execute(&compiled, &mut output).unwrap();
        assert_eq!(output, b"49\n");
    }
    assert_eq!(error("println(a);").kind, DiagnosticKind::UnknownName);
}

#[test]
fn name_resolution_precedes_all_arithmetic_evaluation() {
    let source = "println(\"first\"); let bad = 1 / 0; println(unknown);";
    assert_eq!(error(source).kind, DiagnosticKind::UnknownName);
    assert_eq!(
        error("println(\"first\"); let bad = 1 / 0;").kind,
        DiagnosticKind::DivisionByZero
    );
}

#[test]
fn comments_and_unicode_trivia_can_separate_declaration_tokens() {
    assert_eq!(
        output("let // name\n width\u{2003}= // initializer\n 6; println(width);"),
        "6\n"
    );
    assert_eq!(
        output("let slash = 8 / // divisor\r\n 2; println(slash);"),
        "4\n"
    );
}

#[test]
fn parses_bindings_without_resolving_and_evaluator_rejects_unbound_names() {
    let ast = scarp::parse("let x = missing; println(x);").unwrap();
    assert_eq!(ast.statements.len(), 2);
    assert!(scarp::resolver::resolve(ast).is_err());
    let ast = scarp::parse("println(missing);").unwrap();
    let scarp::ast::Statement::Println {
        argument: scarp::ast::PrintArgument::Integer(expression),
        ..
    } = &ast.statements[0]
    else {
        panic!("expected expression");
    };
    assert_eq!(
        scarp::evaluator::evaluate(expression).unwrap_err().kind,
        DiagnosticKind::UnknownName
    );
}

#[test]
fn resolves_long_dependency_chains_without_recursive_execution() {
    let mut source = String::from("let v0 = 0;\n");
    for index in 1..10_000 {
        source.push_str(&format!("let v{index} = v{} + 1;\n", index - 1));
    }
    source.push_str("println(v9999);");
    assert_eq!(output(&source), "9999\n");
}

#[test]
fn every_truncation_returns_valid_source_and_related_spans() {
    let source = "// नमस्ते\nlet seed = 7; let result = seed * (seed + 1); println(result);";
    for end in (0..=source.len()).filter(|offset| source.is_char_boundary(*offset)) {
        let prefix = &source[..end];
        if let Err(diagnostic) = compile(prefix) {
            for span in std::iter::once(diagnostic.span)
                .chain(diagnostic.related.iter().map(|note| note.span))
            {
                assert!(span.start <= span.end && span.end <= prefix.len());
                assert!(prefix.is_char_boundary(span.start) && prefix.is_char_boundary(span.end));
            }
            assert!(diagnostic.render("prefix.srp", prefix).contains('^'));
        }
    }
}
