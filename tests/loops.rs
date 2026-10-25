use scarp::{
    diagnostics::{Diagnostic, DiagnosticKind},
    interpreter, ExecutionLimits, RunError,
};

fn output(source: &str) -> String {
    let mut bytes = Vec::new();
    scarp::run(source, &mut bytes).unwrap();
    String::from_utf8(bytes).unwrap()
}

fn error(source: &str, limits: ExecutionLimits) -> Diagnostic {
    let expected = scarp::check_with_limits(source, limits).unwrap_err();
    let mut bytes = Vec::new();
    let RunError::Diagnostic(actual) =
        scarp::run_with_limits(source, &mut bytes, limits).unwrap_err()
    else {
        panic!("expected diagnostic");
    };
    assert!(bytes.is_empty());
    assert_eq!(actual, expected);
    actual
}

#[test]
fn while_rechecks_current_state_and_skips_false_bodies() {
    assert_eq!(output("let mut x = 0; while x < 4 { println(x); x += 1; } println(x); while false { println(1 / 0); }"), "0\n1\n2\n3\n4\n");
}

#[test]
fn break_and_continue_control_the_nearest_loop() {
    let source = "let mut outer = 0; let mut total = 0; while outer < 3 { outer += 1; let mut inner = 0; while true { inner += 1; if inner == 2 { continue; } if inner > 3 { break; } total += outer * 10 + inner; } } println(total);";
    assert_eq!(output(source), "132\n");
    assert_eq!(
        output("while true { { if true { break; } } println(1 / 0); } println(7);"),
        "7\n"
    );
}

#[test]
fn iteration_declarations_reinitialize_without_escaping_or_aliasing() {
    assert_eq!(output("let local = 99; let mut x = 0; while x < 3 { let mut local = x; local += 10; println(local); x += 1; } println(local);"), "10\n11\n12\n99\n");
    assert_eq!(output("let mut x = 0; let mut text = \"first\"; while x < 2 { let copy = text; println(copy); text = \"later\"; x += 1; }"), "first\nlater\n");
    assert_eq!(
        error(
            "while false { let hidden = 1; } println(hidden);",
            ExecutionLimits::default()
        )
        .kind,
        DiagnosticKind::UnknownName
    );
}

#[test]
fn unreachable_loop_bodies_are_statically_checked() {
    for (source, kind) in [
        (
            "while false { println(missing); }",
            DiagnosticKind::UnknownName,
        ),
        (
            "while false { let x = 1; x = 2; }",
            DiagnosticKind::ImmutableAssignment,
        ),
        ("while 1 {}", DiagnosticKind::TypeMismatch),
        (
            "while false { println(true + 1); }",
            DiagnosticKind::TypeMismatch,
        ),
    ] {
        assert_eq!(error(source, ExecutionLimits::default()).kind, kind);
    }
}

#[test]
fn loop_control_outside_a_loop_is_rejected_even_in_skipped_paths() {
    for source in [
        "break;",
        "continue;",
        "if false { break; }",
        "{ continue; }",
        "while false {} break;",
    ] {
        assert_eq!(
            error(source, ExecutionLimits::default()).kind,
            DiagnosticKind::LoopControl
        );
    }
    for name in ["while", "break", "continue"] {
        assert_eq!(
            error(&format!("let {name} = 1;"), ExecutionLimits::default()).kind,
            DiagnosticKind::ReservedName
        );
    }
}

#[test]
fn nonterminating_loops_stop_deterministically_without_output() {
    let limits = ExecutionLimits {
        max_steps: 100,
        ..ExecutionLimits::default()
    };
    for source in [
        "println(\"before\"); while true {}",
        "while true { continue; }",
        "let mut x = 1; while x > 0 { x = 1; }",
    ] {
        let diagnostic = error(source, limits);
        assert_eq!(diagnostic.kind, DiagnosticKind::ExecutionLimit);
        assert!(diagnostic.span.end <= source.len());
    }
    assert_eq!(
        scarp::check("while true {}").unwrap_err().kind,
        DiagnosticKind::ExecutionLimit
    );
}

#[test]
fn exact_step_and_print_budget_boundaries_are_consistent() {
    let limits = ExecutionLimits {
        max_steps: 1,
        max_prints: 1,
    };
    scarp::check_with_limits("println(\"text\");", limits).unwrap();
    assert_eq!(
        error("println(1);", limits).kind,
        DiagnosticKind::ExecutionLimit
    );
    scarp::check_with_limits(
        "",
        ExecutionLimits {
            max_steps: 0,
            max_prints: 0,
        },
    )
    .unwrap();
    let limits = ExecutionLimits {
        max_steps: 10000,
        max_prints: 2,
    };
    scarp::check_with_limits("println(1); println(2);", limits).unwrap();
    assert_eq!(
        error("println(1); println(2); println(3);", limits).kind,
        DiagnosticKind::OutputLimit
    );
    assert_eq!(
        error("while true { println(1); }", limits).kind,
        DiagnosticKind::OutputLimit
    );
}

#[test]
fn expression_work_consumes_the_same_budget_as_program_instructions() {
    let source = format!("println({}1);", "1 + ".repeat(100));
    assert_eq!(
        error(
            &source,
            ExecutionLimits {
                max_steps: 50,
                ..ExecutionLimits::default()
            }
        )
        .kind,
        DiagnosticKind::ExecutionLimit
    );
    scarp::check_with_limits(
        &source,
        ExecutionLimits {
            max_steps: 1000,
            ..ExecutionLimits::default()
        },
    )
    .unwrap();
}

#[test]
fn selected_errors_and_budget_failures_keep_output_atomic() {
    assert_eq!(
        error(
            "println(1); while true { println(1 / 0); }",
            ExecutionLimits::default()
        )
        .kind,
        DiagnosticKind::DivisionByZero
    );
    assert_eq!(
        error(
            "while true {} println(missing);",
            ExecutionLimits::default()
        )
        .kind,
        DiagnosticKind::UnknownName
    );
}

#[test]
fn compiled_loop_programs_have_fresh_state_and_budgets() {
    let program = scarp::compile("let mut x = 0; while x < 20 { x += 1; } println(x);").unwrap();
    std::thread::scope(|scope| {
        for _ in 0..8 {
            let program = &program;
            scope.spawn(move || {
                let low = ExecutionLimits {
                    max_steps: 10,
                    ..ExecutionLimits::default()
                };
                assert_eq!(
                    interpreter::validate_with_limits(program, low)
                        .unwrap_err()
                        .kind,
                    DiagnosticKind::ExecutionLimit
                );
                for _ in 0..3 {
                    interpreter::validate(program).unwrap();
                    let mut bytes = Vec::new();
                    interpreter::execute(program, &mut bytes).unwrap();
                    assert_eq!(bytes, b"20\n");
                }
            });
        }
    });
}

#[test]
fn large_iteration_counts_use_flat_jumps_and_fixed_storage() {
    let program = scarp::compile(
        "let mut x = 0; while x < 100000 { let next = x + 1; x = next; } println(x);",
    )
    .unwrap();
    assert_eq!(program.binding_count(), 2);
    let mut bytes = Vec::new();
    interpreter::execute_with_limits(
        &program,
        &mut bytes,
        ExecutionLimits {
            max_steps: 2000000,
            ..ExecutionLimits::default()
        },
    )
    .unwrap();
    assert_eq!(bytes, b"100000\n");
}

#[test]
fn malformed_and_excessively_nested_loops_report_errors() {
    for source in [
        "while",
        "while true",
        "while {}",
        "while true {",
        "while true {};",
        "while false { break }",
        "while false { continue 1; }",
    ] {
        assert!(scarp::check(source).is_err(), "{source}");
    }
    let valid = format!(
        "{}println(1);{}",
        "while false {".repeat(64),
        "}".repeat(64)
    );
    scarp::check(&valid).unwrap();
    let invalid = format!(
        "{}println(1);{}",
        "while false {".repeat(65),
        "}".repeat(65)
    );
    assert_eq!(
        error(&invalid, ExecutionLimits::default()).kind,
        DiagnosticKind::BlockNesting
    );
}

#[test]
fn generated_loop_results_agree_with_a_reference_sum() {
    for limit in 0..100 {
        let source=format!("let mut x = 0; let mut total = 0; while x < {limit} {{ x += 1; if x == 3 {{ continue; }} if x == 8 {{ break; }} total += x; }} println(total);");
        let expected: i64 = (1..=limit)
            .take_while(|x| *x != 8)
            .filter(|x| *x != 3)
            .sum();
        assert_eq!(output(&source), format!("{expected}\n"));
    }
}
