use scarp::{
    diagnostics::DiagnosticKind,
    ExecutionOptions, RunError,
};

fn evaluate(source: &str, options: ExecutionOptions) -> Vec<u8> {
    scarp::check_with_options(source, options).unwrap();
    let mut output = Vec::new();
    scarp::run_with_options(source, &mut output, options).unwrap();
    output
}

fn failure(source: &str, options: ExecutionOptions, kind: DiagnosticKind) -> scarp::diagnostics::Diagnostic {
    let expected = scarp::check_with_options(source, options).unwrap_err();
    assert_eq!(expected.kind, kind);
    let mut output = Vec::new();
    let RunError::Diagnostic(actual) =
        scarp::run_with_options(source, &mut output, options).unwrap_err()
    else {
        panic!("expected diagnostic, not a writer error");
    };
    assert!(output.is_empty(), "pending output must remain atomic");
    assert_eq!(actual, expected);
    actual
}

#[test]
fn terminal_self_transfer_uses_one_frame_at_long_depth() {
    let source = "pure funcra descend(n: integer, acc: integer = 0) -> integer needs n >= 0; { if n == 0 { return acc; } return descend(n - 1, acc + 1); } print(descend(5000));";
    let options = ExecutionOptions {
        max_call_depth: 1,
        max_calls: 5001,
        max_live_slots: 3,
        ..ExecutionOptions::default()
    };
    assert_eq!(evaluate(source, options), b"5000\n");
    assert_eq!(evaluate(source, options), b"5000\n");
}

#[test]
fn terminal_transfer_from_a_nested_caller_preserves_the_final_result() {
    let source = "pure funcra countdown(n: integer) -> integer { if n == 0 { return 7; } return countdown(n - 1); } funcra wrapper(n: integer) -> integer { return countdown(n); } print(wrapper(1000));";
    let options = ExecutionOptions {
        max_call_depth: 2,
        max_calls: 1002,
        max_live_slots: 5,
        ..ExecutionOptions::default()
    };
    assert_eq!(evaluate(source, options), b"7\n");
}

#[test]
fn terminal_calls_still_count_against_total_call_budget() {
    let source = "pure funcra down(n: integer) -> integer { if n == 0 { return 0; } return down(n - 1); } print(down(5));";
    let exhausted = ExecutionOptions {
        max_call_depth: 1,
        max_calls: 5,
        ..ExecutionOptions::default()
    };
    let diagnostic = failure(source, exhausted, DiagnosticKind::CallLimit);
    assert!(diagnostic.message.contains("total function call limit of 5"));
    assert_eq!(
        evaluate(
            source,
            ExecutionOptions {
                max_calls: 6,
                ..exhausted
            }
        ),
        b"0\n"
    );
}

#[test]
fn postconditions_keep_independent_frames_and_run_on_each_return() {
    let source = "pure funcra checked(n: integer) -> integer ensures result >= 0; { if n == 0 { return 0; } return checked(n - 1); } print(checked(2));";
    let options = ExecutionOptions {
        max_call_depth: 1,
        ..ExecutionOptions::default()
    };
    let diagnostic = failure(source, options, DiagnosticKind::CallLimit);
    assert!(diagnostic.message.contains("function call depth"));
    assert_eq!(
        evaluate(
            source,
            ExecutionOptions {
                max_call_depth: 3,
                ..options
            }
        ),
        b"0\n"
    );
}

#[test]
fn expressions_with_work_after_self_call_are_not_terminal() {
    for recursive_return in ["return 1 + walk(n - 1);", "return (walk(n - 1));"] {
        let source = format!("pure funcra walk(n: integer) -> integer {{ if n == 0 {{ return 0; }} {recursive_return} }} print(walk(2));");
        failure(
            &source,
            ExecutionOptions {
                max_call_depth: 1,
                ..ExecutionOptions::default()
            },
            DiagnosticKind::CallLimit,
        );
    }
}

#[test]
fn terminal_calls_evaluate_argument_defaults_and_needs_on_every_iteration() {
    let source = "pure funcra next(n: integer, accumulated: integer = 2) -> integer needs accumulated >= 0; { if n == 0 { return accumulated; } return next(n - 1); } print(next(100));";
    assert_eq!(
        evaluate(
            source,
            ExecutionOptions {
                max_call_depth: 1,
                max_calls: 101,
                max_live_slots: 3,
                ..ExecutionOptions::default()
            }
        ),
        b"2\n"
    );
}

#[test]
fn terminal_failure_keeps_bounded_nested_source_trace_and_atomic_output() {
    let source = "pure funcra f(n: integer) -> integer needs n > 0; { return f(n - 1); } print(\"hidden\"); print(f(50));";
    let options = ExecutionOptions {
        max_call_depth: 1,
        max_calls: 60,
        max_live_slots: 2,
        ..ExecutionOptions::default()
    };
    let diagnostic = failure(source, options, DiagnosticKind::ContractViolation);
    assert_eq!(diagnostic.related.len(), 16);
    assert!(diagnostic
        .related
        .iter()
        .all(|location| location.message.contains("called funcra f")));
    assert!(diagnostic
        .related
        .iter()
        .all(|location| location.span.end <= source.len()));
}

#[test]
fn a_selected_work_quota_is_still_enforced_during_terminal_transfer() {
    let source = "pure funcra recurse(n: integer) -> integer { if n == 0 { return 0; } return recurse(n - 1); } print(\"uncommitted\"); print(recurse(100));";
    let diagnostic = failure(
        source,
        ExecutionOptions {
            max_call_depth: 1,
            max_calls: 101,
            limits: scarp::ExecutionLimits {
                max_steps: 50,
                max_prints: 10,
            },
            ..ExecutionOptions::default()
        },
        DiagnosticKind::ExecutionLimit,
    );
    assert!(diagnostic.message.contains("step limit"));
}
