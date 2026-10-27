use scarp::{
    ast::Statement,
    diagnostics::{Diagnostic, DiagnosticKind, Span},
    interpreter, RunError,
};

fn output(source: &str) -> String {
    let mut bytes = Vec::new();
    scarp::run(source, &mut bytes).unwrap();
    String::from_utf8(bytes).unwrap()
}

fn error(source: &str) -> Diagnostic {
    let expected = scarp::check(source).unwrap_err();
    let mut bytes = Vec::new();
    let RunError::Diagnostic(actual) = scarp::run(source, &mut bytes).unwrap_err() else {
        panic!("expected source diagnostic");
    };
    assert!(bytes.is_empty());
    assert_eq!(actual, expected);
    actual
}

#[test]
fn if_and_else_execute_exactly_one_body() {
    assert_eq!(output("if true { println(1); } else { println(2); } if false { println(3); } else { println(4); } println(5);"), "1\n4\n5\n");
    assert_eq!(
        output("if false {} if true {} else {} println(\"done\");"),
        "done\n"
    );
}

#[test]
fn elif_conditions_stop_after_the_first_selected_body() {
    assert_eq!(output("bind score = 73; if score >= 90 { println(\"A\"); } elif score >= 60 { println(\"B\"); } elif 1 / 0 == 0 { println(\"C\"); } else { println(\"D\"); }"), "B\n");
    assert_eq!(
        output("if false { println(1); } elif false { println(2); } else { println(3); }"),
        "3\n"
    );
    assert_eq!(
        output("if false { println(1); } elif false { println(2); } println(3);"),
        "3\n"
    );
}

#[test]
fn each_body_has_a_separate_lexical_scope() {
    assert_eq!(output("bind x = 1; if true { bind x = x + 1; println(x); } elif false { bind x = \"other\"; println(x); } else { bind x = false; println(x); } println(x);"), "2\n1\n");
    for source in [
        "if true { bind local = 1; } println(local);",
        "if true { bind local = 1; } else { println(local); }",
        "if false { bind local = 1; } elif local == 1 {}",
    ] {
        assert_eq!(error(source).kind, DiagnosticKind::UnknownName);
    }
}

#[test]
fn selected_outer_assignments_survive_scope_exit() {
    assert_eq!(output("var value = 1; var text = \"before\"; if true { value += 2; text = \"after\"; } else { value = 99; } println(value); println(text);"), "3\nafter\n");
    assert_eq!(
        output("var x = 1; if true { var x = 10; x += 1; println(x); } println(x);"),
        "11\n1\n"
    );
}

#[test]
fn conditions_observe_current_state_in_source_order() {
    assert_eq!(output("var ready = false; if ready { println(1); } ready = true; if ready { println(2); ready = false; } if ready { println(3); } else { println(4); }"), "2\n4\n");
}

#[test]
fn conditions_require_explicit_booleans_at_the_condition_span() {
    for condition in ["1", "\"ready\"", "1 + 2"] {
        let source = format!("if {condition} {{}}");
        let diagnostic = error(&source);
        assert_eq!(diagnostic.kind, DiagnosticKind::TypeMismatch);
        assert_eq!(
            &source[diagnostic.span.start..diagnostic.span.end],
            condition
        );
    }
    assert_eq!(
        error("if true {} elif 1 {}").kind,
        DiagnosticKind::TypeMismatch
    );
}

#[test]
fn unreachable_bodies_and_guards_still_check_names_types_and_mutability() {
    for (source, kind) in [
        (
            "if false { println(missing); }",
            DiagnosticKind::UnknownName,
        ),
        ("if true {} elif missing {}", DiagnosticKind::UnknownName),
        (
            "if false { bind x = true + 1; }",
            DiagnosticKind::TypeMismatch,
        ),
        (
            "bind x = 1; if false { x = 2; }",
            DiagnosticKind::ImmutableAssignment,
        ),
        (
            "var x = 1; if false { x = true; }",
            DiagnosticKind::TypeMismatch,
        ),
        (
            "if false { bind x = 1; bind x = 2; }",
            DiagnosticKind::DuplicateBinding,
        ),
    ] {
        assert_eq!(error(source).kind, kind, "{source}");
    }
}

#[test]
fn unselected_arithmetic_is_skipped_but_selected_errors_remain_atomic() {
    assert_eq!(output("println(\"before\"); if false { bind x = 1 / 0; } else { println(\"chosen\"); } if true {} else { println(9223372036854775807 + 1); }"), "before\nchosen\n");
    assert_eq!(
        error("println(\"before\"); if true { println(1 / 0); }").kind,
        DiagnosticKind::DivisionByZero
    );
    assert_eq!(
        error("println(\"before\"); if false {} elif 1 / 0 == 1 {}").kind,
        DiagnosticKind::DivisionByZero
    );
    assert_eq!(
        error("if 1 / 0 == 0 {} else { println(missing); }").kind,
        DiagnosticKind::UnknownName
    );
}

#[test]
fn nested_decisions_and_short_circuit_guards_compose() {
    assert_eq!(output("if true or 1 / 0 == 0 { if false { println(1); } elif not false { println(2); } else { println(3); } println(4); } else { println(5); }"), "2\n4\n");
}

#[test]
fn malformed_branch_syntax_returns_diagnostics_without_panics() {
    for source in [
        "if",
        "if true",
        "if {}",
        "if true {",
        "if true {};",
        "else {}",
        "elif true {}",
        "if true {} else",
        "if true {} else if false {}",
        "if true {} elif {}",
        "if true {} else {} else {}",
        "bind x = if true {};",
    ] {
        let diagnostic = error(source);
        assert!(
            matches!(
                diagnostic.kind,
                DiagnosticKind::UnexpectedToken | DiagnosticKind::UnknownStatement
            ),
            "{source}: {diagnostic}"
        );
    }
    for name in ["if", "elif", "else"] {
        assert_eq!(
            error(&format!("bind {name} = true;")).kind,
            DiagnosticKind::ReservedName
        );
    }
}

#[test]
fn conditional_and_branch_spans_include_the_original_braces() {
    let source = "if true { println(1); } elif false {} else { println(2); }";
    let program = scarp::parse(source).unwrap();
    let Statement::If { branches, span, .. } = &program.statements[0] else {
        panic!("expected conditional");
    };
    assert_eq!(&source[span.start..span.end], source);
    assert_eq!(
        &source[branches[0].span.start..branches[0].span.end],
        "{ println(1); }"
    );
    assert_eq!(&source[branches[1].span.start..branches[1].span.end], "{}");
}

#[test]
fn twenty_thousand_elif_clauses_are_flat_during_parsing_and_execution() {
    let source = format!(
        "if false {{}}{} else {{ println(7); }}",
        " elif false {}".repeat(20_000)
    );
    assert_eq!(output(&source), "7\n");
}

#[test]
fn conditional_braces_share_the_existing_block_depth_limit() {
    let source = format!("{}println(true);{}", "if true {".repeat(64), "}".repeat(64));
    assert_eq!(output(&source), "true\n");
    let source = format!("{}println(true);{}", "if true {".repeat(65), "}".repeat(65));
    assert_eq!(error(&source).kind, DiagnosticKind::BlockNesting);
    let source = format!("{}if true {{}}{}", "{".repeat(64), "}".repeat(64));
    assert_eq!(error(&source).kind, DiagnosticKind::BlockNesting);
}

#[test]
fn resolver_checks_public_conditional_structure_and_depth() {
    let invalid = scarp::ast::Program {
        statements: vec![Statement::If {
            branches: Vec::new(),
            otherwise: Vec::new(),
            span: Span::default(),
        }],
    };
    assert_eq!(
        scarp::resolver::resolve(invalid).unwrap_err().kind,
        DiagnosticKind::UnexpectedToken
    );
    let mut statement = scarp::parse("if true {}").unwrap().statements.remove(0);
    for _ in 0..64 {
        let mut wrapper = scarp::parse("if true {}").unwrap().statements.remove(0);
        if let Statement::If { branches, .. } = &mut wrapper {
            branches[0].statements.push(statement);
        }
        statement = wrapper;
    }
    assert_eq!(
        scarp::resolver::resolve(scarp::ast::Program {
            statements: vec![statement]
        })
        .unwrap_err()
        .kind,
        DiagnosticKind::BlockNesting
    );
}

#[test]
fn compiled_conditional_metadata_and_state_are_stable() {
    let program = scarp::compile("var x = 1; if true { bind local = 2; x += local; } else { bind local = 4; x += local; } println(x);").unwrap();
    assert_eq!(program.binding_count(), 3);
    assert_eq!(program.statement_count(), 7);
    std::thread::scope(|scope| {
        for _ in 0..8 {
            let program = &program;
            scope.spawn(move || {
                for _ in 0..5 {
                    interpreter::validate(program).unwrap();
                    let mut bytes = Vec::new();
                    interpreter::execute(program, &mut bytes).unwrap();
                    assert_eq!(bytes, b"3\n");
                }
            });
        }
    });
}

#[test]
fn generated_branch_selection_matches_an_independent_model() {
    let mut source = String::new();
    let mut expected = String::new();
    for value in -100..=100 {
        source.push_str(&format!("{{ bind value = {value}; if value < -10 {{ println(1); }} elif value <= 10 {{ println(2); }} else {{ println(3); }} }}"));
        expected.push_str(if value < -10 {
            "1\n"
        } else if value <= 10 {
            "2\n"
        } else {
            "3\n"
        });
    }
    assert_eq!(output(&source), expected);
}
