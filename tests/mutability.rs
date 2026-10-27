use scarp::{
    compile,
    diagnostics::{Diagnostic, DiagnosticKind},
    interpreter,
    lexer::{lex, TokenKind},
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
fn mutable_assignments_preserve_earlier_print_values() {
    assert_eq!(
        output("var x = 1; println(x); x = x + 1; println(x); x = -x; println(x);"),
        "1\n2\n-2\n"
    );
}

#[test]
fn initializers_copy_values_instead_of_aliasing_storage() {
    assert_eq!(output("var source = 1; bind saved = source; var copy = source; source = 9; copy = 7; println(saved); println(copy); println(source);"), "1\n7\n9\n");
}

#[test]
fn compound_assignments_read_the_previous_value_and_use_precedence() {
    assert_eq!(output("var x = 2; x += x + 1; println(x); x *= 2 + 1; println(x); x -= 8; x /= -2; println(x);"), "5\n15\n-3\n");
}

#[test]
fn writes_to_immutable_bindings_include_the_declaration() {
    for operator in ["=", "+=", "-=", "*=", "/="] {
        let source = format!("bind value = 1;\nvalue {operator} 2;");
        let diagnostic = error(&source);
        assert_eq!(diagnostic.kind, DiagnosticKind::ImmutableAssignment);
        assert_eq!(&source[diagnostic.span.start..diagnostic.span.end], "value");
        assert_eq!(diagnostic.related.len(), 1);
        assert_eq!(diagnostic.related[0].span.start, 5);
        assert!(diagnostic.render("immutable.srp", &source).contains("var"));
    }
}

#[test]
fn assignments_cannot_implicitly_declare_or_forward_reference_names() {
    for source in [
        "x = 1;",
        "x += 1;",
        "x = 1; var x = 0;",
        "var x = 1; x = missing;",
    ] {
        assert_eq!(error(source).kind, DiagnosticKind::UnknownName);
    }
}

#[test]
fn compound_overflow_and_division_errors_point_to_the_whole_operator() {
    for (initial, operator, value, expected) in [
        (
            "9223372036854775807",
            "+=",
            "1",
            DiagnosticKind::ArithmeticOverflow,
        ),
        (
            "-9223372036854775808",
            "-=",
            "1",
            DiagnosticKind::ArithmeticOverflow,
        ),
        (
            "9223372036854775807",
            "*=",
            "2",
            DiagnosticKind::ArithmeticOverflow,
        ),
        (
            "-9223372036854775808",
            "/=",
            "-1",
            DiagnosticKind::ArithmeticOverflow,
        ),
        ("7", "/=", "0", DiagnosticKind::DivisionByZero),
    ] {
        let source = format!("var x = {initial}; println(x); x {operator} {value};");
        let diagnostic = error(&source);
        assert_eq!(diagnostic.kind, expected, "{source}");
        assert_eq!(
            &source[diagnostic.span.start..diagnostic.span.end],
            operator
        );
    }
}

#[test]
fn rhs_arithmetic_errors_point_to_the_rhs_operator() {
    let source = "var x = 1; x += 1 / 0;";
    let diagnostic = error(source);
    assert_eq!(diagnostic.kind, DiagnosticKind::DivisionByZero);
    assert_eq!(&source[diagnostic.span.start..diagnostic.span.end], "/");
}

#[test]
fn incomplete_or_expression_assignments_are_rejected() {
    for source in [
        "var",
        "var = 1;",
        "var x;",
        "var mut x = 1;",
        "var x = 1; x =;",
        "var x = 1; x +=;",
        "var x = 1; x = 2",
        "var x = 1; println(x = 2);",
        "var x = 1; x = y = 2;",
        "var x = 1; x++;",
        "var x = 1; x + = 2;",
        "var x = 1; x = \"text\";",
        "var x = 1; x +== 2;",
    ] {
        error(source);
    }
}

#[test]
fn lexer_uses_longest_match_and_keeps_comment_precedence() {
    let source = "+= -= *= /= = // += /=\n/";
    let tokens = lex(source).unwrap();
    assert_eq!(
        tokens
            .iter()
            .map(|token| token.kind.clone())
            .collect::<Vec<_>>(),
        vec![
            TokenKind::PlusEqual,
            TokenKind::MinusEqual,
            TokenKind::StarEqual,
            TokenKind::SlashEqual,
            TokenKind::Equal,
            TokenKind::Slash,
            TokenKind::Eof,
        ]
    );
    assert_eq!(&source[tokens[3].span.start..tokens[3].span.end], "/=");
    assert_eq!(output("var x = 4; x /= // divisor\n 2; println(x);"), "2\n");
}

#[test]
fn mutability_does_not_allow_duplicate_or_reserved_declarations() {
    assert_eq!(
        error("var x = 1; var x = 2;").kind,
        DiagnosticKind::DuplicateBinding
    );
    assert_eq!(
        error("bind x = 1; var x = 2;").kind,
        DiagnosticKind::DuplicateBinding
    );
    assert_eq!(error("var println = 1;").kind, DiagnosticKind::ReservedName);
}

#[test]
fn repeated_and_concurrent_executions_do_not_share_mutable_state() {
    let program = std::sync::Arc::new(compile("var x = 1; x += 6; println(x);").unwrap());
    let threads: Vec<_> = (0..4)
        .map(|_| {
            let program = program.clone();
            std::thread::spawn(move || {
                for _ in 0..25 {
                    let mut bytes = Vec::new();
                    interpreter::execute(&program, &mut bytes).unwrap();
                    assert_eq!(bytes, b"7\n");
                }
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
}

#[test]
fn deterministic_assignment_sequence_matches_a_reference_model() {
    let mut source = String::from("var value = 0;\n");
    let mut expected = String::new();
    let mut value = 0_i64;
    for index in 0..1_000 {
        let amount = (index % 17) as i64;
        if index % 2 == 0 {
            source.push_str(&format!("value += {amount};\n"));
            value += amount;
        } else {
            source.push_str(&format!("value -= {amount};\n"));
            value -= amount;
        }
        if index % 50 == 0 {
            source.push_str("println(value);\n");
            expected.push_str(&format!("{value}\n"));
        }
    }
    assert_eq!(output(&source), expected);
}

#[test]
fn long_assignment_lists_execute_without_recursive_state_updates() {
    let source = format!("var x = 0; {} println(x);", "x += 1;".repeat(50_000));
    assert_eq!(output(&source), "50000\n");
}

#[test]
fn mutation_corpus_truncations_never_escape_original_source_spans() {
    let source = "// 🌍\nvar x = 2; x += (x * 3); x /= -2; println(x);";
    for end in (0..=source.len()).filter(|offset| source.is_char_boundary(*offset)) {
        let prefix = &source[..end];
        if let Err(diagnostic) = compile(prefix) {
            assert!(
                diagnostic.span.start <= diagnostic.span.end && diagnostic.span.end <= prefix.len()
            );
            assert!(prefix.is_char_boundary(diagnostic.span.start));
            assert!(prefix.is_char_boundary(diagnostic.span.end));
        }
    }
}
