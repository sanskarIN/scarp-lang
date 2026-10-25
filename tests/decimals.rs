use scarp::{
    ast::{PrintArgument, Statement},
    diagnostics::{Diagnostic, DiagnosticKind},
    value::{FiniteFloat, Value, ValueType},
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
        panic!("expected diagnostic")
    };
    assert!(bytes.is_empty());
    assert_eq!(actual, expected);
    actual
}

#[test]
fn decimal_and_exponent_literals_keep_a_distinct_numeric_type() {
    assert_eq!(output("println(1.25); println(1e2); println(2.5E-1); println(0001.5); println(-0.0); println(1 / 2); println(1.0 / 2.0);"),"1.25\n100.0\n0.25\n1.5\n-0.0\n0\n0.5\n");
    assert_eq!(
        scarp::compile("let x = 1.0;").unwrap().binding_types(),
        &[ValueType::Float]
    );
}

#[test]
fn float_arithmetic_precedence_and_all_comparisons_work() {
    assert_eq!(output("println(1.5 + 2.0 * 3.0); println((1.5 + 2.0) * 3.0); println(-7.0 / 2.0); println(1.0 < 2.0); println(1.0 <= 1.0); println(2.0 > 1.0); println(2.0 >= 3.0); println(1.0 == 1.0); println(1.0 != 2.0);"),"7.5\n10.5\n-3.5\ntrue\ntrue\ntrue\nfalse\ntrue\ntrue\n");
    assert_eq!(
        output("println(0.1 + 0.2); println(0.0 == -0.0);"),
        "0.30000000000000004\ntrue\n"
    );
}

#[test]
fn mutable_float_assignments_preserve_snapshots_and_use_all_operators() {
    assert_eq!(output("let mut x = 1.5; let copy = x; println(x); x += 0.5; x *= 3.0; x -= 1.0; x /= 2.0; println(copy); println(x);"),"1.5\n1.5\n2.5\n");
}

#[test]
fn integer_float_and_boolean_coercions_are_rejected_statically() {
    for expression in ["1 + 1.0", "1.0 + 1", "1.0 == 1", "true < 1.0", "not 1.0"] {
        assert_eq!(
            error(&format!("println({expression});")).kind,
            DiagnosticKind::TypeMismatch
        );
    }
    for source in [
        "let mut x = 1.0; x = 1;",
        "let mut x = 1; x = 1.0;",
        "let mut x = 1.0; x += 1;",
    ] {
        let diagnostic = error(source);
        assert_eq!(diagnostic.kind, DiagnosticKind::TypeMismatch);
        assert_eq!(diagnostic.related.len(), 1);
    }
}

#[test]
fn nonfinite_literals_and_results_are_distinct_diagnostics() {
    assert_eq!(error("println(1e309);").kind, DiagnosticKind::InvalidFloat);
    for expression in [
        "1e308 * 2.0",
        "1e308 + 1e308",
        "-1e308 - 1e308",
        "1e308 / 1e-308",
    ] {
        assert_eq!(
            error(&format!("println(\"before\"); println({expression});")).kind,
            DiagnosticKind::NonFiniteNumber
        );
    }
    for divisor in ["0.0", "-0.0"] {
        assert_eq!(
            error(&format!("println(1.0 / {divisor});")).kind,
            DiagnosticKind::DivisionByZero
        );
    }
}

#[test]
fn underflow_is_finite_and_signed_zero_is_preserved() {
    assert_eq!(
        output("println(1e-400); println(-1e-400); println(1e-300 * 1e-300);"),
        "0.0\n-0.0\n0.0\n"
    );
    assert!(FiniteFloat::new(-0.0).unwrap().get().is_sign_negative());
}

#[test]
fn unselected_decimal_arithmetic_is_skipped_but_literals_are_validated() {
    assert_eq!(output("println(true or 1e308 * 2.0 == 0.0); if false { println(1.0 / 0.0); } else { println(2.5); }"),"true\n2.5\n");
    assert_eq!(
        error("if false { println(1e309); }").kind,
        DiagnosticKind::InvalidFloat
    );
    assert_eq!(
        error("println(1.0 / 0.0); println(missing);").kind,
        DiagnosticKind::UnknownName
    );
}

#[test]
fn malformed_decimal_tokens_return_bounded_spans() {
    for literal in ["1.", "1e", "1E+", "1e-", "1.e2"] {
        let source = format!("// 🌍\nprintln({literal});");
        let diagnostic = error(&source);
        assert_eq!(diagnostic.kind, DiagnosticKind::InvalidFloat);
        assert!(source.is_char_boundary(diagnostic.span.start));
        assert!(diagnostic.span.end <= source.len());
    }
    use scarp::lexer::{lex, TokenKind};
    let tokens = lex("1.5e-2 + 4").unwrap();
    assert_eq!(tokens[0].kind, TokenKind::Float("1.5e-2".into()));
    assert_eq!((tokens[0].span.start, tokens[0].span.end), (0, 6));
    assert!(matches!(lex("1e999").unwrap()[0].kind, TokenKind::Float(_)));
}

#[test]
fn finite_host_values_and_standalone_results_keep_invariants() {
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(FiniteFloat::new(invalid).is_none());
    }
    for valid in [f64::MAX, f64::MIN, f64::MIN_POSITIVE, 0.0, -0.0] {
        assert_eq!(FiniteFloat::new(valid).unwrap().get(), valid);
    }
    let value = Value::Float(FiniteFloat::new(1.25).unwrap());
    assert_eq!(value.as_float(), Some(1.25));
    assert_eq!(value.as_integer(), None);
    assert_eq!(value.as_boolean(), None);
    assert_eq!(value.as_str(), None);
    assert_eq!(value.to_string(), "1.25");
    let program = scarp::parse("println(1.25 * 2.0);").unwrap();
    let Statement::Println {
        argument: PrintArgument::Expression(expression),
        ..
    } = &program.statements[0]
    else {
        panic!("expected expression")
    };
    assert_eq!(
        scarp::evaluator::evaluate_value(expression)
            .unwrap()
            .as_float(),
        Some(2.5)
    );
    assert_eq!(
        scarp::evaluator::evaluate(expression).unwrap_err().kind,
        DiagnosticKind::TypeMismatch
    );
}

#[test]
fn decimal_iterations_can_refine_a_numerical_estimate() {
    let source="let mut estimate = 1.0; let mut iteration = 0; while iteration < 5 { estimate = (estimate + 2.0 / estimate) / 2.0; iteration += 1; } println(estimate > 1.414 and estimate < 1.415);";
    assert_eq!(output(source), "true\n");
    let program = scarp::compile(source).unwrap();
    for _ in 0..3 {
        let mut bytes = Vec::new();
        scarp::interpreter::execute(&program, &mut bytes).unwrap();
        assert_eq!(bytes, b"true\n");
    }
}

#[test]
fn float_error_spans_identify_the_responsible_operator() {
    let source = "println(1e308 * 2.0);";
    let diagnostic = error(source);
    assert_eq!(&source[diagnostic.span.start..diagnostic.span.end], "*");
    let source = "let mut x = 1e308; x *= 2.0;";
    let diagnostic = error(source);
    assert_eq!(&source[diagnostic.span.start..diagnostic.span.end], "*=");
}
