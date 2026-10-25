use scarp::{
    compile,
    diagnostics::{Diagnostic, DiagnosticKind},
    evaluator, interpreter,
    value::{Value, ValueType},
    RunError,
};

fn output(source: &str) -> String {
    let mut bytes = Vec::new();
    scarp::run(source, &mut bytes).unwrap();
    String::from_utf8(bytes).unwrap()
}

fn error(source: &str) -> Diagnostic {
    let mut bytes = Vec::new();
    let error = scarp::run(source, &mut bytes).unwrap_err();
    assert!(bytes.is_empty());
    match error {
        RunError::Diagnostic(error) => error,
        other => panic!("{other}"),
    }
}

#[test]
fn string_bindings_preserve_unicode_escapes_and_empty_values() {
    assert_eq!(output("let greeting = \"नमस्ते 🌍\"; let lines = \"one\\ntwo\"; let empty = \"\"; println(greeting); println(lines); println(empty);"), "नमस्ते 🌍\none\ntwo\n\n");
    assert_eq!(
        output(r#"let text = "a\\b\"c"; println(text);"#),
        "a\\b\"c\n"
    );
}

#[test]
fn string_assignment_does_not_change_copies_or_earlier_output() {
    assert_eq!(output("let mut word = \"first\"; let copy = word; println(word); word = \"second\"; println(copy); println(word);"), "first\nfirst\nsecond\n");
}

#[test]
fn parenthesized_string_values_are_valid() {
    assert_eq!(
        output("let value = ((\"text\")); println((value)); println((\"literal\"));"),
        "text\nliteral\n"
    );
}

#[test]
fn different_scopes_can_shadow_with_different_primitive_types() {
    assert_eq!(output("let value = \"outer\"; { let mut value = 1; value += 1; println(value); } println(value);"), "2\nouter\n");
    assert_eq!(output("let mut value = 1; { let mut value = \"inner\"; value = \"changed\"; println(value); } value += 1; println(value);"), "changed\n2\n");
}

#[test]
fn shadowing_initializers_keep_the_outer_type_until_insertion() {
    assert_eq!(
        output("let value = \"outer\"; { let value = value; println(value); }"),
        "outer\n"
    );
    assert_eq!(
        error("let value = \"outer\"; { let value = value + 1; }").kind,
        DiagnosticKind::TypeMismatch
    );
}

#[test]
fn mutable_bindings_cannot_change_their_inferred_type() {
    for source in [
        "let mut value = 1; value = \"text\";",
        "let mut value = \"text\"; value = 1;",
        "let mut value = \"text\"; let number = 1; value = number;",
    ] {
        let diagnostic = error(source);
        assert_eq!(diagnostic.kind, DiagnosticKind::TypeMismatch);
        assert_eq!(diagnostic.related.len(), 1);
        assert!(diagnostic
            .render("types.srp", source)
            .contains("type is established here"));
    }
}

#[test]
fn arithmetic_does_not_concatenate_or_coerce_string_values() {
    for expression in [
        r#""a" + "b""#,
        r#""a" - 1"#,
        r#"1 * "a""#,
        r#""a" / 1"#,
        r#"-"a""#,
    ] {
        let source = format!("println({expression});");
        assert_eq!(
            error(&source).kind,
            DiagnosticKind::TypeMismatch,
            "{source}"
        );
    }
    assert_eq!(
        error("let text = \"1\"; println(text + 1);").kind,
        DiagnosticKind::TypeMismatch
    );
}

#[test]
fn compound_assignment_is_integer_only() {
    for operator in ["+=", "-=", "*=", "/="] {
        let source = format!("let mut value = \"text\"; value {operator} \"other\";");
        let diagnostic = error(&source);
        assert_eq!(diagnostic.kind, DiagnosticKind::TypeMismatch);
        assert_eq!(
            &source[diagnostic.span.start..diagnostic.span.end],
            operator
        );
        assert_eq!(diagnostic.related.len(), 1);
        assert_eq!(
            error(&format!("let mut value = 1; value {operator} \"other\";")).kind,
            DiagnosticKind::TypeMismatch
        );
    }
}

#[test]
fn type_errors_precede_arithmetic_and_leave_stdout_empty() {
    assert_eq!(
        error("println(\"first\"); let unused = 1 / 0; let mut text = \"hello\"; text = 1;").kind,
        DiagnosticKind::TypeMismatch
    );
    assert_eq!(
        scarp::check("let text = \"hello\"; println(text * 2);")
            .unwrap_err()
            .kind,
        DiagnosticKind::TypeMismatch
    );
}

#[test]
fn name_and_mutability_checks_still_apply_to_strings() {
    assert_eq!(error("let text = text;").kind, DiagnosticKind::UnknownName);
    assert_eq!(
        error("let text = \"x\"; text = \"y\";").kind,
        DiagnosticKind::ImmutableAssignment
    );
    assert_eq!(
        error("let text = \"x\"; let text = \"y\";").kind,
        DiagnosticKind::DuplicateBinding
    );
    assert_eq!(
        error("{ let text = \"x\"; } println(text);").kind,
        DiagnosticKind::UnknownName
    );
}

#[test]
fn resolved_slot_types_are_immutable_metadata_in_declaration_order() {
    let program = compile("let text = \"x\"; { let text = 1; } let copy = text;").unwrap();
    assert_eq!(
        program.binding_types(),
        &[ValueType::String, ValueType::Integer, ValueType::String]
    );
    assert_eq!(program.binding_count(), 3);
}

#[test]
fn standalone_value_evaluation_and_integer_compatibility_are_explicit() {
    let program = scarp::parse("let text = (\"hello\");").unwrap();
    let scarp::ast::Statement::Let { initializer, .. } = &program.statements[0] else {
        panic!("expected declaration");
    };
    let value = evaluator::evaluate_value(initializer).unwrap();
    assert_eq!(value, Value::from("hello"));
    assert_eq!(value.kind(), ValueType::String);
    assert_eq!(value.as_str(), Some("hello"));
    assert_eq!(value.as_integer(), None);
    assert_eq!(value.to_string(), "hello");
    assert_eq!(
        evaluator::evaluate(initializer).unwrap_err().kind,
        DiagnosticKind::TypeMismatch
    );
    let program = scarp::parse("let number = 1 + 2;").unwrap();
    let scarp::ast::Statement::Let { initializer, .. } = &program.statements[0] else {
        panic!("expected declaration");
    };
    assert_eq!(evaluator::evaluate(initializer).unwrap(), 3);
    assert_eq!(
        evaluator::evaluate_value(initializer).unwrap(),
        Value::Integer(3)
    );
}

#[test]
fn standalone_arithmetic_also_rejects_wrong_primitive_types() {
    let program = scarp::parse("let value = \"x\" + 1;").unwrap();
    let scarp::ast::Statement::Let { initializer, .. } = &program.statements[0] else {
        panic!("expected declaration");
    };
    assert_eq!(
        evaluator::evaluate_value(initializer).unwrap_err().kind,
        DiagnosticKind::TypeMismatch
    );
}

#[test]
fn repeated_and_concurrent_string_executions_do_not_share_mutations() {
    let program = std::sync::Arc::new(
        compile("let mut text = \"start\"; println(text); text = \"end\"; println(text);").unwrap(),
    );
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let program = program.clone();
            std::thread::spawn(move || {
                for _ in 0..20 {
                    interpreter::validate(&program).unwrap();
                    let mut bytes = Vec::new();
                    interpreter::execute(&program, &mut bytes).unwrap();
                    assert_eq!(bytes, b"start\nend\n");
                }
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn long_string_alias_chains_preserve_value_semantics() {
    let text = "Scarp🌍".repeat(100);
    let mut source = format!("let s0 = \"{text}\";\n");
    for index in 1..10_000 {
        source.push_str(&format!("let s{index} = s{};\n", index - 1));
    }
    source.push_str("println(s9999);");
    assert_eq!(output(&source), format!("{text}\n"));
}

#[test]
fn truncated_string_programs_report_valid_source_and_related_spans() {
    let source =
        "let mut text = \"नमस्ते 🌍\\n\"; { let text = text; println(text); } text = \"done\";";
    for end in (0..=source.len()).filter(|offset| source.is_char_boundary(*offset)) {
        let prefix = &source[..end];
        if let Err(diagnostic) = compile(prefix) {
            for span in std::iter::once(diagnostic.span)
                .chain(diagnostic.related.iter().map(|note| note.span))
            {
                assert!(span.start <= span.end && span.end <= prefix.len());
                assert!(prefix.is_char_boundary(span.start) && prefix.is_char_boundary(span.end));
            }
            assert!(diagnostic.render("strings.srp", prefix).contains('^'));
        }
    }
}

#[test]
fn generated_string_updates_preserve_all_prior_copies_and_prints() {
    let words = [
        "hello",
        "नमस्ते",
        "🌍",
        "café",
        "line\nbreak",
        "a\\b",
        "quoted \"text\"",
        "",
    ];
    let quote = |text: &str| {
        format!(
            "\"{}\"",
            text.replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', "\\n")
        )
    };
    let mut source = format!("let mut text = {};\n", quote(words[0]));
    let mut expected = String::new();
    let mut previous = words[0];
    for index in 0..160 {
        let next = words[(index + 1) % words.len()];
        source.push_str(&format!(
            "println(text); let copy{index} = text; text = {}; println(copy{index});\n",
            quote(next)
        ));
        expected.push_str(previous);
        expected.push('\n');
        expected.push_str(previous);
        expected.push('\n');
        previous = next;
    }
    source.push_str("println(text);");
    expected.push_str(previous);
    expected.push('\n');
    assert_eq!(output(&source), expected);
}
