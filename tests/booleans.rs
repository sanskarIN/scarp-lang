use scarp::{
    ast::{AssignmentOperator, BinaryOperator, PrintArgument, Statement},
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
    let expected = scarp::check(source).unwrap_err();
    let mut bytes = Vec::new();
    let RunError::Diagnostic(actual) = scarp::run(source, &mut bytes).unwrap_err() else {
        panic!("expected a source diagnostic");
    };
    assert!(bytes.is_empty());
    assert_eq!(actual, expected);
    actual
}

#[test]
fn boolean_bindings_capture_values_and_preserve_types() {
    let source = "let mut ready = false; let saved = ready; println(ready); ready = true; println(saved); println(ready);";
    assert_eq!(output(source), "false\nfalse\ntrue\n");
    assert_eq!(
        scarp::compile(source).unwrap().binding_types(),
        &[ValueType::Boolean, ValueType::Boolean]
    );
    for source in [
        "let mut x = true; x = 1;",
        "let mut x = 1; x = false;",
        "let mut x = true; x += false;",
    ] {
        let diagnostic = error(source);
        assert_eq!(diagnostic.kind, DiagnosticKind::TypeMismatch);
        assert_eq!(diagnostic.related.len(), 1);
    }
}

#[test]
fn comparisons_cover_ordering_equality_and_integer_extremes() {
    assert_eq!(output("println(1 < 2); println(2 <= 2); println(3 > 2); println(3 >= 4); println(4 == 4); println(4 != 4);"), "true\ntrue\ntrue\nfalse\ntrue\nfalse\n");
    assert_eq!(output("println(-9223372036854775808 < 9223372036854775807); println(-9223372036854775808 == -9223372036854775808);"), "true\ntrue\n");
}

#[test]
fn equality_compares_string_contents_and_booleans_without_coercion() {
    assert_eq!(output("let a = \"नमस्ते 🌍\"; let b = \"नमस्ते 🌍\"; println(a == b); println(\"\" != \"x\"); println(true == (not false));"), "true\ntrue\ntrue\n");
    assert_eq!(
        output("println(\"é\" == \"é\"); println(false != true);"),
        "false\ntrue\n"
    );
    for expression in [
        "true == 1",
        "1 != \"1\"",
        "\"a\" < \"b\"",
        "true >= false",
        "not 1",
        "true + 1",
        "-false",
        "true and 1",
        "0 or true",
    ] {
        assert_eq!(
            error(&format!("println({expression});")).kind,
            DiagnosticKind::TypeMismatch,
            "{expression}"
        );
    }
}

#[test]
fn not_binds_after_comparison_and_and_binds_before_or() {
    assert_eq!(output("println(not 1 + 2 * 3 < 8); println(true or false and false); println((true or false) and false); println(not not true); println(not false == true);"), "false\ntrue\nfalse\ntrue\ntrue\n");
    assert_eq!(output("println((1 < 2) == true);"), "true\n");
}

#[test]
fn skipped_right_operands_do_not_evaluate_arithmetic() {
    assert_eq!(output("println(false and 1 / 0 == 0); println(true or 9223372036854775807 + 1 == 0); println(true and (false or true)); println(false or (true and false));"), "false\ntrue\ntrue\nfalse\n");
    for expression in ["true and 1 / 0 == 0", "false or 1 / 0 == 0"] {
        assert_eq!(
            error(&format!("println({expression});")).kind,
            DiagnosticKind::DivisionByZero
        );
    }
    assert_eq!(
        error("println(1 / 0 == 0 and false);").kind,
        DiagnosticKind::DivisionByZero
    );
}

#[test]
fn skipped_operands_still_resolve_names_and_check_types() {
    assert_eq!(
        error("println(false and missing);").kind,
        DiagnosticKind::UnknownName
    );
    assert_eq!(
        error("println(true or 1);").kind,
        DiagnosticKind::TypeMismatch
    );
    assert_eq!(
        error("println(false and (\"text\" < 1));").kind,
        DiagnosticKind::TypeMismatch
    );
    assert_eq!(
        error("println(1 / 0); println(true or missing);").kind,
        DiagnosticKind::UnknownName
    );
}

#[test]
fn boolean_words_are_reserved_but_longer_names_are_allowed() {
    for name in ["true", "false", "and", "or", "not"] {
        assert_eq!(
            error(&format!("let {name} = 1;")).kind,
            DiagnosticKind::ReservedName
        );
    }
    assert_eq!(output("let true_value = true; let notable = false; let orange = 7; println(true_value and not notable); println(orange);"), "true\n7\n");
}

#[test]
fn chained_comparisons_require_explicit_boolean_composition() {
    for expression in ["1 < 2 < 3", "1 == 1 != false", "1 < 2 == true"] {
        let diagnostic = error(&format!("println({expression});"));
        assert_eq!(diagnostic.kind, DiagnosticKind::UnexpectedToken);
        assert!(diagnostic.help.unwrap().contains("and"));
    }
    assert_eq!(output("let x = 2; println(1 < x and x < 3);"), "true\n");
}

#[test]
fn comparison_tokens_use_longest_match_and_exact_byte_spans() {
    use scarp::lexer::{lex, TokenKind};
    let tokens = lex("== != < <= > >= =").unwrap();
    let expected = [
        (TokenKind::EqualEqual, 0, 2),
        (TokenKind::NotEqual, 3, 5),
        (TokenKind::Less, 6, 7),
        (TokenKind::LessEqual, 8, 10),
        (TokenKind::Greater, 11, 12),
        (TokenKind::GreaterEqual, 13, 15),
        (TokenKind::Equal, 16, 17),
    ];
    for (token, (kind, start, end)) in tokens.iter().zip(expected) {
        assert_eq!(token.kind, kind);
        assert_eq!((token.span.start, token.span.end), (start, end));
    }
    assert_eq!(tokens.len(), 8);
    for source in ["!", "&&", "||"] {
        assert_eq!(
            lex(source).unwrap_err().kind,
            DiagnosticKind::InvalidCharacter
        );
    }
}

#[test]
fn type_errors_highlight_the_responsible_operator() {
    for (source, operator) in [
        ("println(true and 1);", "and"),
        ("println(1 == false);", "=="),
        ("println(not 1);", "not"),
        ("println(\"a\" >= 1);", ">="),
    ] {
        let diagnostic = error(source);
        assert_eq!(
            &source[diagnostic.span.start..diagnostic.span.end],
            operator
        );
    }
}

#[test]
fn standalone_evaluation_checks_the_entire_expression() {
    fn expression(source: &str) -> scarp::ast::Expression {
        let program = scarp::parse(&format!("println({source});")).unwrap();
        let Statement::Println {
            argument: PrintArgument::Expression(expression),
            ..
        } = program.statements.into_iter().next().unwrap()
        else {
            panic!("expected expression");
        };
        expression
    }
    let boolean = expression("false or not false");
    assert_eq!(
        evaluator::evaluate_value(&boolean).unwrap(),
        Value::Boolean(true)
    );
    assert_eq!(
        evaluator::evaluate(&boolean).unwrap_err().kind,
        DiagnosticKind::TypeMismatch
    );
    assert_eq!(
        evaluator::evaluate_value(&expression("false and missing"))
            .unwrap_err()
            .kind,
        DiagnosticKind::UnknownName
    );
    assert_eq!(Value::Boolean(true).as_boolean(), Some(true));
    assert_eq!(Value::Integer(1).as_boolean(), None);
    assert_eq!(Value::Boolean(false).as_integer(), None);
    assert_eq!(Value::Boolean(false).as_str(), None);
    assert_eq!(Value::Boolean(false).to_string(), "false");
}

#[test]
fn long_logical_and_not_chains_remain_iterative() {
    let source = format!("println({}true);", "not ".repeat(20_000));
    assert_eq!(output(&source), "true\n");
    for (operand, operator, expected) in [("true", "and", "true\n"), ("false", "or", "false\n")] {
        let source = format!(
            "println({operand}{});",
            format!(" {operator} {operand}").repeat(20_000)
        );
        assert_eq!(output(&source), expected);
    }
    let source = format!("println({}true{});", "(".repeat(64), ")".repeat(64));
    assert_eq!(output(&source), "true\n");
    assert_eq!(
        error(&format!(
            "println({}true{});",
            "(".repeat(65),
            ")".repeat(65)
        ))
        .kind,
        DiagnosticKind::ExpressionNesting
    );
}

#[test]
fn generated_comparison_results_agree_with_an_independent_model() {
    let mut source = String::new();
    let mut expected = String::new();
    for left in -12..=12 {
        for right in -12..=12 {
            for (operator, result) in [
                ("<", left < right),
                ("<=", left <= right),
                (">", left > right),
                (">=", left >= right),
                ("==", left == right),
                ("!=", left != right),
            ] {
                source.push_str(&format!("println({left} {operator} {right});"));
                expected.push_str(if result { "true\n" } else { "false\n" });
            }
        }
    }
    assert_eq!(output(&source), expected);
}

#[test]
fn public_syntax_cannot_use_comparison_as_compound_assignment() {
    let mut program = scarp::parse("let mut x = 1; x += 1;").unwrap();
    let Statement::Assign { operator, .. } = &mut program.statements[1] else {
        panic!("expected assignment");
    };
    *operator = AssignmentOperator::Compound(BinaryOperator::Equal);
    assert_eq!(
        scarp::resolver::resolve(program).unwrap_err().kind,
        DiagnosticKind::UnexpectedToken
    );
}

#[test]
fn compiled_boolean_programs_are_reusable_and_thread_safe() {
    let program =
        scarp::compile("let mut ready = false; println(ready); ready = not ready; println(ready);")
            .unwrap();
    std::thread::scope(|scope| {
        for _ in 0..8 {
            let program = &program;
            scope.spawn(move || {
                interpreter::validate(program).unwrap();
                let mut bytes = Vec::new();
                interpreter::execute(program, &mut bytes).unwrap();
                assert_eq!(bytes, b"false\ntrue\n");
            });
        }
    });
}
