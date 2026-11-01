use scarp::{diagnostics::DiagnosticKind, ExecutionLimits, RunError};

fn output(source: &str) -> String {
    let mut out = Vec::new();
    scarp::run(source, &mut out).unwrap();
    scarp::check(source).unwrap();
    String::from_utf8(out).unwrap()
}

#[test]
fn snapshots_survive_retiring_their_original_scope() {
    let source = r#"
        bind base = [1.0, 2.0];
        {
            var values = [3.0, 4.0];
            bind alias = values;
            print(alias);
            values = scale(values, 2.0);
            print(alias, values);
        }
        print(base);
    "#;
    assert_eq!(
        output(source),
        "[3.0, 4.0]\n[3.0, 4.0] [6.0, 8.0]\n[1.0, 2.0]\n"
    );
}

#[test]
fn short_circuits_and_conditional_scopes_do_not_escape() {
    let source = r#"
        var outer = "original";
        if false { bind ignored = [1.0]; }
        elif true {
            bind kept = outer;
            print(kept);
            outer = "updated";
        } else { bind unused = [1.0]; }
        print(outer);
        { bind outer = [1.0]; print(len(outer)); }
        print(outer);
    "#;
    assert_eq!(output(source), "original\nupdated\n1\nupdated\n");
    assert_eq!(
        scarp::check("{ bind private = [1.0]; } print(private);")
            .unwrap_err()
            .kind,
        DiagnosticKind::UnknownName
    );
}

#[test]
fn nested_continue_and_break_retire_only_exited_loop_scopes() {
    let source = r#"
        var count = 0;
        repeat 3 as i {
            bind outer = [1.0, 2.0];
            repeat 4 as j {
                bind inner = [3.0, 4.0];
                if j == 1 { bind temporary = [9.0]; continue; }
                if j == 3 { break; }
                count += len(outer) + len(inner);
            }
            print(i, count);
        }
    "#;
    assert_eq!(output(source), "0 8\n1 16\n2 24\n");
    let source = r#"
        var i = 0;
        var total = 0;
        while i < 3 {
            i += 1;
            bind outer = [1.0];
            while true {
                bind inner = [2.0];
                if i == 2 { break; }
                total += len(outer) + len(inner);
                break;
            }
        }
        print(total);
    "#;
    assert_eq!(output(source), "4\n");
}

#[test]
fn early_returns_keep_parameter_and_result_for_postconditions() {
    let source = r#"
        pure funcra duplicate(v: vector) -> vector
            ensures len(result) == len(v);
        {
            bind transient = scale(v, 4.0);
            if len(v) > 0 {
                bind local = scale(transient, 0.5);
                return local;
            }
            return v;
        }
        print(duplicate([1.0, 2.0]));
        print(duplicate([]));
    "#;
    assert_eq!(output(source), "[2.0, 4.0]\n[]\n");
}

#[test]
fn nested_output_and_diagnostics_remain_atomic() {
    let source = r#"
        print("before");
        repeat 2 as i {
            bind values = [1.0, 2.0];
            print(values);
            if i == 1 { print(1 / 0); }
        }
    "#;
    let expected = scarp::check(source).unwrap_err();
    assert_eq!(expected.kind, DiagnosticKind::DivisionByZero);
    let mut out = Vec::new();
    let RunError::Diagnostic(actual) = scarp::run(source, &mut out).unwrap_err() else {
        panic!("expected runtime diagnostic");
    };
    assert_eq!(actual, expected);
    assert!(out.is_empty());
}

#[test]
fn retirement_is_invisible_to_existing_step_accounting() {
    // Empty source containers and exiting scopes have no additional
    // user-visible work charge compared with the pre-retirement interpreter.
    scarp::check_with_limits(
        "{ }",
        ExecutionLimits {
            max_steps: 0,
            max_prints: 0,
        },
    )
    .unwrap();
    scarp::check_with_limits(
        "{ bind value = 1; }",
        ExecutionLimits {
            max_steps: 2,
            max_prints: 0,
        },
    )
    .unwrap();
    assert_eq!(
        scarp::check_with_limits(
            "{ bind value = 1; }",
            ExecutionLimits {
                max_steps: 1,
                max_prints: 0,
            },
        )
        .unwrap_err()
        .kind,
        DiagnosticKind::ExecutionLimit
    );
}

#[test]
fn windows_tiles_defaults_recursion_and_terminal_transfers_keep_their_payloads() {
    let source = r#"
        pure funcra recurse(n: integer, t: tile, v: vector = unroll(t)) -> tile
            ensures result == t;
        {
            { bind transient = turn(t); }
            if n == 0 { return tile(v, height(t), width(t)); }
            return recurse(n - 1, t);
        }
        pure funcra terminal(n: integer, t: tile, v: vector = unroll(t)) -> tile {
            if n == 0 { return tile(v, height(t), width(t)); }
            bind scratch = turn(t);
            return terminal(n - 1, turn(scratch));
        }
        { bind values = [99.0, 1.0, 2.0, 88.0];
          bind t = tile(window(values, 1, 2), 1, 2);
          print(recurse(3, t), terminal(300, t)); }
    "#;
    assert_eq!(
        output(source),
        "tile(1x2)[[1.0, 2.0]] tile(1x2)[[1.0, 2.0]]\n"
    );
}
