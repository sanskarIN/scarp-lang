use scarp::{
    compile,
    diagnostics::{location, DiagnosticKind, LocationTracker},
    interpreter,
};

#[test]
fn check_validates_names_mutation_and_arithmetic_without_an_output_sink() {
    scarp::check("var x = 1; { bind x = x + 1; println(x); } x += 3; println(x);").unwrap();
    assert_eq!(
        scarp::check("bind x = missing;").unwrap_err().kind,
        DiagnosticKind::UnknownName
    );
    assert_eq!(
        scarp::check("bind x = 1; x = 2;").unwrap_err().kind,
        DiagnosticKind::ImmutableAssignment
    );
    assert_eq!(
        scarp::check("var x = 1; x /= 0;").unwrap_err().kind,
        DiagnosticKind::DivisionByZero
    );
}

#[test]
fn syntax_compilation_and_validation_are_separate_phases() {
    let source = "bind value = 1 / 0;";
    assert!(scarp::parse(source).is_ok());
    assert!(compile(source).is_ok());
    assert_eq!(
        scarp::check(source).unwrap_err().kind,
        DiagnosticKind::DivisionByZero
    );
    assert!(scarp::parse("println(unknown);").is_ok());
    assert!(compile("println(unknown);").is_err());
}

#[test]
fn validation_does_not_change_a_compiled_programs_state() {
    let program = compile("var x = 1; x += 1; println(x);").unwrap();
    for _ in 0..20 {
        interpreter::validate(&program).unwrap();
        let mut output = Vec::new();
        interpreter::execute(&program, &mut output).unwrap();
        assert_eq!(output, b"2\n");
    }
}

#[test]
fn location_tracker_agrees_with_random_access_for_all_byte_offsets() {
    let source = "\u{feff}é🌍\r\n\tनमस्ते\nlast";
    let mut tracker = LocationTracker::new(source);
    for offset in 0..=source.len() + 2 {
        assert_eq!(tracker.at(offset), location(source, offset), "{offset}");
    }
    for offset in (0..=source.len() + 2).rev() {
        assert_eq!(tracker.at(offset), location(source, offset), "{offset}");
    }
    for offset in [usize::MAX, 0, 3, 4, 0, source.len(), 2] {
        assert_eq!(tracker.at(offset), location(source, offset));
    }
}

#[test]
fn sequential_locations_handle_large_single_line_inputs() {
    let source = "x".repeat(100_000);
    let mut tracker = LocationTracker::new(&source);
    for offset in 0..=source.len() {
        let position = tracker.at(offset);
        assert_eq!(position.line, 1);
        assert_eq!(position.column, offset + 1);
    }
    let mut empty = LocationTracker::new("");
    assert_eq!(empty.at(usize::MAX), location("", 0));
}

#[test]
fn check_and_run_report_the_same_first_diagnostic() {
    for source in [
        "bind x = 1; bind x = 2;",
        "{ bind x = 1; } println(x);",
        "var x = 1; x /= 0;",
    ] {
        let expected = scarp::check(source).unwrap_err();
        let mut output = Vec::new();
        let scarp::RunError::Diagnostic(actual) = scarp::run(source, &mut output).unwrap_err()
        else {
            panic!("expected diagnostic");
        };
        assert_eq!(actual, expected);
        assert!(output.is_empty());
    }
}

#[test]
fn alternating_expression_shapes_preserve_state_and_captured_values() {
    let mut source = "var total = 0; var label = \"before\";".to_string();
    let mut expected = String::new();
    for index in 1..=512 {
        source.push_str("total += (1 + 2) * (3 + 4); println(total); println(total > 0 and not false); println(label); label = \"after\";");
        expected.push_str(&format!(
            "{}\ntrue\n{}\n",
            index * 21,
            if index == 1 { "before" } else { "after" }
        ));
        source
            .push_str("if false { println(1 / 0); } else { println((\"selected\")); } println(1);");
        expected.push_str("selected\n1\n");
    }
    let program = compile(&source).unwrap();
    for _ in 0..3 {
        interpreter::validate(&program).unwrap();
        let mut output = Vec::new();
        interpreter::execute(&program, &mut output).unwrap();
        assert_eq!(output, expected.as_bytes());
    }
}

#[test]
fn validation_evaluates_print_arguments_after_many_discarded_prints() {
    let source = format!(
        "{} if true {{ println(1 / 0); }}",
        "println(1 + 2); println(\"text\");".repeat(4096)
    );
    let program = compile(&source).unwrap();
    let expected = interpreter::validate(&program).unwrap_err();
    assert_eq!(expected.kind, DiagnosticKind::DivisionByZero);
    let mut output = Vec::new();
    let scarp::RunError::Diagnostic(actual) =
        interpreter::execute(&program, &mut output).unwrap_err()
    else {
        panic!("expected a diagnostic");
    };
    assert_eq!(actual, expected);
    assert!(output.is_empty());
}
