use scarp::{diagnostics::DiagnosticKind as Kind, ExecutionOptions, RunError};

fn output(source: &str) -> (Vec<u8>, Vec<u8>) {
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    scarp::run_with_io(source, &mut stdout, &mut stderr).unwrap();
    scarp::check(source).unwrap();
    (stdout, stderr)
}

fn failure(source: &str, options: ExecutionOptions, kind: Kind) {
    let expected = scarp::check_with_options(source, options).unwrap_err();
    assert_eq!(expected.kind, kind);
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    let RunError::Diagnostic(actual) =
        scarp::run_with_io_and_options(source, &mut stdout, &mut stderr, options).unwrap_err()
    else {
        panic!("expected source diagnostic");
    };
    assert_eq!(actual, expected);
    assert!(stdout.is_empty() && stderr.is_empty());
}

#[test]
fn count_calls_suspend_once_and_keep_the_enclosing_snapshot() {
    let source = r#"
        funcra count(n: integer) -> integer { eprint("count", n); return n; }
        var n = 3;
        repeat count(n) as i { n = 0; print(i); }
        print(n);
        repeat count(0) as i { print("unreachable"); }
    "#;
    assert_eq!(
        output(source),
        (b"0\n1\n2\n0\n".to_vec(), b"count 3\ncount 0\n".to_vec())
    );
}

#[test]
fn recursive_calls_in_repeat_counts_and_nested_returns_keep_frames_isolated() {
    let source = r#"
        pure funcra count(n: integer) -> integer {
            if n == 0 { return 0; }
            return count(n - 1) + 1;
        }
        funcra search(n: integer) -> integer ensures result >= -1; {
            repeat count(n) as i {
                if i == 0 { continue; }
                while true { if i == 2 { return i; } break; }
            }
            return -1;
        }
        repeat 3 as outer { print(search(outer + 1), outer); }
    "#;
    assert_eq!(output(source).0, b"-1 0\n-1 1\n2 2\n");
}

#[test]
fn repeat_counts_preserve_static_checks_negative_errors_and_atomic_output() {
    for (source, kind) in [
        (
            "funcra n() -> integer { eprint(1); return -1; } print(2); repeat n() as i {}",
            Kind::RepeatCount,
        ),
        (
            "funcra n() -> boolean { return true; } repeat n() as i {}",
            Kind::TypeMismatch,
        ),
        (
            "funcra n() -> unit {} repeat n() as i {}",
            Kind::TypeMismatch,
        ),
        (
            "funcra f() -> unit { repeat 0 as i { i = 1; } }",
            Kind::ImmutableAssignment,
        ),
        (
            "funcra f() -> unit { repeat 0 as i { print(missing); } }",
            Kind::UnknownName,
        ),
        (
            "funcra f() -> integer { repeat 1 as i { return i; } }",
            Kind::MissingReturn,
        ),
        (
            "funcra f() -> unit { repeat 1 as i {} print(i); }",
            Kind::UnknownName,
        ),
        (
            "funcra f() -> unit {} repeat 0 as f {}",
            Kind::FunctionDefinition,
        ),
    ] {
        failure(source, ExecutionOptions::default(), kind);
    }
    assert_eq!(Kind::RepeatCount.code(), "E0019");
}

#[test]
fn counts_inside_functions_share_call_work_and_output_limits() {
    let source = "pure funcra n() -> integer { return 2; } funcra f() -> unit { repeat n() as i { print(i); } } f();";
    scarp::check_with_options(
        source,
        ExecutionOptions {
            max_calls: 2,
            ..ExecutionOptions::default()
        },
    )
    .unwrap();
    failure(
        source,
        ExecutionOptions {
            max_calls: 1,
            ..ExecutionOptions::default()
        },
        Kind::CallLimit,
    );
    failure(
        source,
        ExecutionOptions {
            max_call_depth: 1,
            ..ExecutionOptions::default()
        },
        Kind::CallLimit,
    );
    failure(
        source,
        ExecutionOptions {
            limits: scarp::ExecutionLimits {
                max_steps: 1000,
                max_prints: 1,
            },
            ..ExecutionOptions::default()
        },
        Kind::OutputLimit,
    );
}

#[test]
fn call_graph_includes_repeat_counts_and_checks_their_purity() {
    let source =
        "pure funcra n() -> integer { return 0; } pure funcra f() -> unit { repeat n() as i {} }";
    let program = scarp::compile(source).unwrap();
    assert_eq!(program.functions()[1].callees, ["n"]);
    failure(
        &source.replacen("pure funcra n", "funcra n", 1),
        ExecutionOptions::default(),
        Kind::ImpureFunction,
    );
}

#[test]
fn contextual_loop_binding_names_remain_usable() {
    assert_eq!(
        output("var repeat = 2; bind as = 1; repeat += as; repeat repeat as i { print(i); }").0,
        b"0\n1\n2\n"
    );
    failure(
        "funcra repeat() -> integer { return 1; }",
        ExecutionOptions::default(),
        Kind::FunctionDefinition,
    );
    assert_eq!(
        output("funcra as() -> integer { return 2; } repeat as() as i { print(i); }").0,
        b"0\n1\n"
    );
}

#[test]
fn parameter_mutation_help_describes_valid_local_copy_syntax() {
    let error = scarp::compile("funcra f(n: integer) -> unit { n += 1; }").unwrap_err();
    assert_eq!(error.kind, Kind::ImmutableAssignment);
    assert!(error
        .help
        .unwrap()
        .contains("copy the parameter into a local var"));
}

#[test]
fn nested_output_calls_preserve_event_order_and_output_sink_failures() {
    use std::{cell::RefCell, io, rc::Rc};
    struct Sink(Rc<RefCell<Vec<u8>>>, bool);
    impl io::Write for Sink {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.1 {
                return Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed"));
            }
            self.0.borrow_mut().extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let source = "funcra mark(n: integer) -> integer { ewrite(n); return n; } print(\"answer\", mark(1), mark(2));";
    let bytes = Rc::new(RefCell::new(Vec::new()));
    scarp::run_with_io(
        source,
        &mut Sink(bytes.clone(), false),
        &mut Sink(bytes.clone(), false),
    )
    .unwrap();
    assert_eq!(&*bytes.borrow(), b"12answer 1 2\n");
    let bytes = Rc::new(RefCell::new(Vec::new()));
    let result = scarp::run_with_io(
        source,
        &mut Sink(bytes.clone(), true),
        &mut Sink(bytes.clone(), false),
    );
    assert!(matches!(result, Err(RunError::Output(_))));
    assert_eq!(&*bytes.borrow(), b"12");
}

#[test]
fn immutable_function_programs_can_be_reused_concurrently() {
    let program = std::sync::Arc::new(scarp::compile("pure funcra f(n: integer) -> integer { var total = n; repeat n as i { total += i; } return total; } print(f(4));").unwrap());
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let program = program.clone();
            std::thread::spawn(move || {
                let mut stdout = Vec::new();
                scarp::interpreter::execute(&program, &mut stdout).unwrap();
                assert_eq!(stdout, b"10\n");
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }
}
