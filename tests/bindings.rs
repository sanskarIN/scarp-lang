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
        output("bind width = 6; bind area = width * 7; println(area); println(-area + 1);"),
        "42\n-41\n"
    );
}

#[test]
fn binding_only_programs_do_not_write_anything() {
    assert_eq!(output("bind value = 40 + 2;"), "");
    let program = compile("bind a = 1; bind b = a + 1;").unwrap();
    assert_eq!(program.binding_count(), 2);
    assert_eq!(program.statement_count(), 2);
}

#[test]
fn initializers_keep_integer_boundaries_and_arithmetic_rules() {
    assert_eq!(output("bind low = -9223372036854775808; bind high = 9223372036854775807; println(low); println(high);"), format!("{}\n{}\n", i64::MIN, i64::MAX));
    for (source, expected) in [
        ("bind x = 1 / 0;", DiagnosticKind::DivisionByZero),
        (
            "bind x = 9223372036854775807 + 1;",
            DiagnosticKind::ArithmeticOverflow,
        ),
        (
            "bind x = 9223372036854775808;",
            DiagnosticKind::InvalidInteger,
        ),
    ] {
        assert_eq!(error(source).kind, expected);
    }
}

#[test]
fn unknown_names_point_to_the_reference() {
    let source = "// café 🌍\r\nbind value = missing + 1;";
    let diagnostic = error(source);
    assert_eq!(diagnostic.kind, DiagnosticKind::UnknownName);
    assert_eq!(
        &source[diagnostic.span.start..diagnostic.span.end],
        "missing"
    );
    let rendered = diagnostic.render("names.srp", source);
    assert!(rendered.contains("names.srp:2:14"), "{rendered}");
}

#[test]
fn forward_and_self_references_are_not_visible() {
    for source in [
        "println(later); bind later = 1;",
        "bind x = x + 1;",
        "bind a = b; bind b = a;",
    ] {
        assert_eq!(error(source).kind, DiagnosticKind::UnknownName);
    }
}

#[test]
fn duplicate_declarations_include_the_original_location() {
    let source = "bind value = 1;\nbind value = 2;";
    let diagnostic = error(source);
    assert_eq!(diagnostic.kind, DiagnosticKind::DuplicateBinding);
    assert_eq!(&source[diagnostic.span.start..diagnostic.span.end], "value");
    assert_eq!(diagnostic.related.len(), 1);
    assert_eq!(diagnostic.related[0].span.start, 5);
    let rendered = diagnostic.render("duplicate.srp", source);
    assert!(rendered.contains("duplicate.srp:2:6"));
    assert!(rendered.contains("duplicate.srp:1:6"));
    assert!(rendered.contains("previous declaration"));
}

#[test]
fn reserved_names_are_rejected_as_declarations_and_references() {
    for name in ["bind", "mut", "println"] {
        let declaration = if name == "mut" {
            "var mut = 1;".to_string()
        } else {
            format!("bind {name} = 1;")
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
        output(
            "bind Value = 1; bind value = 2; bind _ = 3; bind x2 = Value + value + _; println(x2);"
        ),
        "6\n"
    );
    assert_eq!(
        error("bind value = 1; println(Value);").kind,
        DiagnosticKind::UnknownName
    );
}

#[test]
fn malformed_declarations_are_diagnosed_without_panics() {
    for source in [
        "bind",
        "bind x",
        "bind = 1;",
        "bind 1 = 2;",
        "bind x 1;",
        "bind x =;",
        "bind x = 1",
        "bind x = \"text\" 1;",
    ] {
        assert_eq!(
            error(source).kind,
            DiagnosticKind::UnexpectedToken,
            "{source}"
        );
    }
    assert_eq!(
        error("bind x = 1, y = 2;").kind,
        DiagnosticKind::UnexpectedToken
    );
}

#[test]
fn each_execution_of_a_compiled_program_has_fresh_storage() {
    let compiled = compile("bind a = 7; bind b = a * a; println(b);").unwrap();
    for _ in 0..20 {
        let mut output = Vec::new();
        interpreter::execute(&compiled, &mut output).unwrap();
        assert_eq!(output, b"49\n");
    }
    assert_eq!(error("println(a);").kind, DiagnosticKind::UnknownName);
}

#[test]
fn name_resolution_precedes_all_arithmetic_evaluation() {
    let source = "println(\"first\"); bind bad = 1 / 0; println(unknown);";
    assert_eq!(error(source).kind, DiagnosticKind::UnknownName);
    assert_eq!(
        error("println(\"first\"); bind bad = 1 / 0;").kind,
        DiagnosticKind::DivisionByZero
    );
}

#[test]
fn comments_and_unicode_trivia_can_separate_declaration_tokens() {
    assert_eq!(
        output("bind // name\n width = // initializer\n 6; println(width);"),
        "6\n"
    );
    assert_eq!(
        output("bind slash = 8 / // divisor\r\n 2; println(slash);"),
        "4\n"
    );
}

#[test]
fn parses_bindings_without_resolving_and_evaluator_rejects_unbound_names() {
    let ast = scarp::parse("bind x = missing; println(x);").unwrap();
    assert_eq!(ast.statements.len(), 2);
    assert!(scarp::resolver::resolve(ast).is_err());
    let ast = scarp::parse("println(missing);").unwrap();
    let scarp::ast::Statement::Println {
        argument: scarp::ast::PrintArgument::Expression(expression),
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
    let mut source = String::from("bind v0 = 0;\n");
    for index in 1..10_000 {
        source.push_str(&format!("bind v{index} = v{} + 1;\n", index - 1));
    }
    source.push_str("println(v9999);");
    assert_eq!(output(&source), "9999\n");
}

#[test]
fn every_truncation_returns_valid_source_and_related_spans() {
    let source = "// नमस्ते\nbind seed = 7; bind result = seed * (seed + 1); println(result);";
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
