use scarp::{
    ast::{ExpressionKind, PrintArgument, Statement},
    compile,
    diagnostics::{Diagnostic, DiagnosticKind},
    evaluator,
    lexer::{lex, TokenKind},
    parser::MAX_EXPRESSION_DEPTH,
    run, RunError,
};

fn evaluate(expression: &str) -> String {
    let mut output = Vec::new();
    run(&format!("println({expression});"), &mut output).unwrap();
    String::from_utf8(output).unwrap()
}

fn diagnostic(source: &str) -> Diagnostic {
    let mut output = Vec::new();
    let error = run(source, &mut output).unwrap_err();
    assert!(output.is_empty(), "invalid program wrote output");
    match error {
        RunError::Diagnostic(error) => error,
        other => panic!("expected a source diagnostic, got {other}"),
    }
}

#[test]
fn lexes_integer_spelling_operators_and_comments() {
    let source = "001 + 2 - 3 * 4 / 5 // ignored / 9\n";
    let tokens = lex(source).unwrap();
    let kinds: Vec<_> = tokens.iter().map(|token| token.kind.clone()).collect();
    assert_eq!(
        kinds,
        vec![
            TokenKind::Integer("001".into()),
            TokenKind::Plus,
            TokenKind::Integer("2".into()),
            TokenKind::Minus,
            TokenKind::Integer("3".into()),
            TokenKind::Star,
            TokenKind::Integer("4".into()),
            TokenKind::Slash,
            TokenKind::Integer("5".into()),
            TokenKind::Eof,
        ]
    );
    assert_eq!(&source[tokens[0].span.start..tokens[0].span.end], "001");
    assert_eq!(&source[tokens[7].span.start..tokens[7].span.end], "/");
}

#[test]
fn applies_precedence_grouping_and_left_associativity() {
    for (source, expected) in [
        ("1 + 2 * 3", 7),
        ("(1 + 2) * 3", 9),
        ("8 - 3 - 1", 4),
        ("24 / 3 / 2", 4),
        ("24 / (3 / 2)", 24),
        ("2 * 3 + 4 * 5", 26),
        ("1 + 2 - 3 + 4", 4),
        ("-2 * -3 + 4", 10),
        ("-(1 + 2) * 3", -9),
        ("1--2", 3),
        ("00012", 12),
    ] {
        assert_eq!(evaluate(source), format!("{expected}\n"), "{source}");
    }
}

#[test]
fn division_truncates_toward_zero_for_all_signs() {
    for (source, expected) in [
        ("7 / 2", 3),
        ("-7 / 2", -3),
        ("7 / -2", -3),
        ("-7 / -2", 3),
        ("-1 / 2", 0),
    ] {
        assert_eq!(evaluate(source), format!("{expected}\n"));
    }
}

#[test]
fn accepts_integer_boundaries_and_direct_negative_minimum() {
    for (source, expected) in [
        ("9223372036854775807", i64::MAX),
        ("-9223372036854775808", i64::MIN),
        ("- 0009223372036854775808", i64::MIN),
        ("(-9223372036854775808)", i64::MIN),
        ("0", 0),
        ("-0", 0),
        ("-(-9223372036854775807)", i64::MAX),
        ("-9223372036854775808 + 1", i64::MIN + 1),
    ] {
        assert_eq!(evaluate(source), format!("{expected}\n"), "{source}");
    }
    assert_eq!(evaluate(&format!("{}1", "0".repeat(10_000))), "1\n");
}

#[test]
fn rejects_out_of_range_literals_with_their_original_spans() {
    for expression in [
        "9223372036854775808",
        "-9223372036854775809",
        "18446744073709551616",
        "-(9223372036854775808)",
    ] {
        let source = format!("println({expression});");
        let error = diagnostic(&source);
        assert_eq!(error.kind, DiagnosticKind::InvalidInteger, "{expression}");
        assert!(source[error.span.start..error.span.end]
            .chars()
            .all(|ch| ch.is_ascii_digit()));
        assert!(error.render("range.srp", &source).contains("E0006"));
    }
    assert_eq!(
        diagnostic(&format!("println({});", "9".repeat(10_000))).kind,
        DiagnosticKind::InvalidInteger
    );
}

#[test]
fn checks_every_arithmetic_operator_for_overflow() {
    for (expression, operator) in [
        ("9223372036854775807 + 1", "+"),
        ("-9223372036854775808 - 1", "-"),
        ("9223372036854775807 * 2", "*"),
        ("-9223372036854775808 / -1", "/"),
        ("-(-9223372036854775808)", "-"),
        ("3037000500 * 3037000500", "*"),
    ] {
        let source = format!("println({expression});");
        let error = diagnostic(&source);
        assert_eq!(
            error.kind,
            DiagnosticKind::ArithmeticOverflow,
            "{expression}"
        );
        assert_eq!(&source[error.span.start..error.span.end], operator);
    }
}

#[test]
fn division_by_zero_points_to_the_division_operator() {
    for expression in ["1 / 0", "0 / 0", "1 / (2 - 2)", "-9223372036854775808 / 0"] {
        let source = format!("println({expression});");
        let error = diagnostic(&source);
        assert_eq!(error.kind, DiagnosticKind::DivisionByZero);
        assert_eq!(&source[error.span.start..error.span.end], "/");
    }
}

#[test]
fn arithmetic_errors_in_later_statements_produce_no_output() {
    for tail in [
        "1 / 0",
        "9223372036854775807 + 1",
        "-(-9223372036854775808)",
    ] {
        diagnostic(&format!("println(\"first\"); println(7); println({tail});"));
    }
}

#[test]
fn accepts_the_nesting_limit_and_rejects_the_next_group() {
    let at_limit = format!(
        "{}1{}",
        "(".repeat(MAX_EXPRESSION_DEPTH),
        ")".repeat(MAX_EXPRESSION_DEPTH)
    );
    assert_eq!(evaluate(&at_limit), "1\n");
    let too_deep = format!("println(({}));", at_limit);
    let error = diagnostic(&too_deep);
    assert_eq!(error.kind, DiagnosticKind::ExpressionNesting);
    assert_eq!(&too_deep[error.span.start..error.span.end], "(");
    let adversarial = format!("println({}1{});", "-(".repeat(10_000), ")".repeat(10_000));
    assert_eq!(
        diagnostic(&adversarial).kind,
        DiagnosticKind::ExpressionNesting
    );
}

#[test]
fn handles_long_binary_chains_and_destroys_the_ast_without_recursion() {
    let expression = format!("0{}", "+1".repeat(50_000));
    assert_eq!(evaluate(&expression), "50000\n");
}

#[test]
fn rejects_malformed_expressions_and_deferred_syntax() {
    for argument in [
        "",
        "()",
        "1 +",
        "1 ** 2",
        "--1",
        "+1",
        "(1",
        "1)",
        "1 2",
        "x",
        "1.0",
        "0xff",
        "1_000",
        "1, 2",
        "1 % 2",
        "\"a\" + \"b\"",
        "(\"text\")",
        "1 + \"text\"",
        "1 /* block */ 2",
    ] {
        diagnostic(&format!("println({argument});"));
    }
}

#[test]
fn keeps_strings_and_arithmetic_comments_compatible() {
    let source = "println(\"1 + 2 // text\"); println(8 / // divisor\n 2); println(- // sign\n 3);";
    let mut output = Vec::new();
    run(source, &mut output).unwrap();
    assert_eq!(output, b"1 + 2 // text\n4\n-3\n");
}

#[test]
fn expression_nodes_preserve_spans_and_postorder_invariants() {
    let source = "println(-(1 + 2) * 3);";
    let program = compile(source).unwrap();
    let Statement::Println {
        argument: PrintArgument::Integer(expression),
        ..
    } = &program.statements[0]
    else {
        panic!("expected integer expression");
    };
    assert_eq!(evaluator::evaluate(expression).unwrap(), -9);
    let root = &expression.nodes()[expression.root()];
    assert_eq!(&source[root.span.start..root.span.end], "-(1 + 2) * 3");
    for (id, node) in expression.nodes().iter().enumerate() {
        match node.kind {
            ExpressionKind::Integer(_) => {}
            ExpressionKind::Negate { operand, .. } => assert!(operand < id),
            ExpressionKind::Group { expression } => assert!(expression < id),
            ExpressionKind::Binary {
                left,
                right,
                operator_span,
                ..
            } => {
                assert!(left < id && right < id);
                assert_eq!(operator_span.end - operator_span.start, 1);
            }
        }
    }
}

#[test]
fn truncation_corpus_returns_valid_diagnostic_spans() {
    let program = "// café 🌍\r\nprintln(-(12 + 3) * 4 / 2); println(\"done\");";
    for end in (0..=program.len()).filter(|end| program.is_char_boundary(*end)) {
        let source = &program[..end];
        let mut output = Vec::new();
        if let Err(RunError::Diagnostic(error)) = run(source, &mut output) {
            assert!(error.span.start <= error.span.end);
            assert!(error.span.end <= source.len());
            assert!(source.is_char_boundary(error.span.start));
            assert!(source.is_char_boundary(error.span.end));
            assert!(error.render("corpus.srp", source).contains('^'));
            assert!(output.is_empty());
        }
    }
}

#[test]
fn bounded_arithmetic_agrees_with_an_independent_reference() {
    for left in -12_i64..=12 {
        for right in -8_i64..=8 {
            for (operator, expected) in [
                ("+", left + right),
                ("-", left - right),
                ("*", left * right),
            ] {
                assert_eq!(
                    evaluate(&format!("{left} {operator} {right}")),
                    format!("{expected}\n")
                );
            }
            if right != 0 {
                assert_eq!(
                    evaluate(&format!("{left} / {right}")),
                    format!("{}\n", left / right)
                );
            }
        }
    }
}
