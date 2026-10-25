use scarp::{
    compile,
    diagnostics::{location, DiagnosticKind, LocationTracker},
    interpreter,
};

#[test]
fn check_validates_names_mutation_and_arithmetic_without_an_output_sink() {
    scarp::check("let mut x = 1; { let x = x + 1; println(x); } x += 3; println(x);").unwrap();
    assert_eq!(
        scarp::check("let x = missing;").unwrap_err().kind,
        DiagnosticKind::UnknownName
    );
    assert_eq!(
        scarp::check("let x = 1; x = 2;").unwrap_err().kind,
        DiagnosticKind::ImmutableAssignment
    );
    assert_eq!(
        scarp::check("let mut x = 1; x /= 0;").unwrap_err().kind,
        DiagnosticKind::DivisionByZero
    );
}

#[test]
fn syntax_compilation_and_validation_are_separate_phases() {
    let source = "let value = 1 / 0;";
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
    let program = compile("let mut x = 1; x += 1; println(x);").unwrap();
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
        "let x = 1; let x = 2;",
        "{ let x = 1; } println(x);",
        "let mut x = 1; x /= 0;",
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
