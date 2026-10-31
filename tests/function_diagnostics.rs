use scarp::{
    diagnostics::{Diagnostic, DiagnosticKind},
    ExecutionOptions, RunError,
};

fn failure(source: &str, options: ExecutionOptions) -> Diagnostic {
    let expected = scarp::check_with_options(source, options).unwrap_err();
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    let RunError::Diagnostic(actual) =
        scarp::run_with_io_and_options(source, &mut stdout, &mut stderr, options).unwrap_err()
    else {
        panic!("expected diagnostic");
    };
    assert!(stdout.is_empty() && stderr.is_empty());
    assert_eq!(actual, expected);
    actual
}

#[test]
fn default_type_errors_highlight_the_initializer_and_relate_the_parameter() {
    for (kind, expression) in [
        ("integer", "true"),
        ("boolean", "1"),
        ("string", "false"),
        ("integer", "(1 < 2)"),
    ] {
        let source =
            format!("// café 🌍\r\nfuncra unused(value: {kind} = {expression}) -> unit {{}}");
        let error = scarp::compile(&source).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::TypeMismatch);
        assert_eq!(&source[error.span.start..error.span.end], expression);
        assert_eq!(error.related.len(), 1);
        let declaration = error.related[0].span;
        assert_eq!(&source[declaration.start..declaration.end], "value");
        assert!(error
            .render("default.srp", &source)
            .contains("default.srp:2:"));
    }
}

#[test]
fn depth_and_total_call_limits_name_the_distinct_exhausted_option() {
    for (source, options, expected, field) in [
        ("print(\"hidden\"); funcra f(n: integer) -> integer { if n==0 {return 0;} return f(n-1)+0; } print(f(1));", ExecutionOptions { max_call_depth: 1, ..ExecutionOptions::default() }, "function call depth limit of 1", "max_call_depth"),
        ("eprint(\"hidden\"); funcra f() -> integer {return 1;} print(f(), f());", ExecutionOptions { max_calls: 1, ..ExecutionOptions::default() }, "total function call limit of 1", "max_calls"),
    ] {
        let error = failure(source, options);
        assert_eq!(error.kind, DiagnosticKind::CallLimit);
        assert!(error.message.contains(expected));
        assert!(error.help.unwrap().contains(field));
    }
}

#[test]
fn file_and_function_slot_failures_identify_the_configured_slot_limit() {
    for source in [
        "bind value=1;",
        "funcra f(value: integer) -> integer {return value;} print(f(1));",
    ] {
        let error = failure(
            source,
            ExecutionOptions {
                max_live_slots: 0,
                ..ExecutionOptions::default()
            },
        );
        assert_eq!(error.kind, DiagnosticKind::CallLimit);
        assert!(error.message.contains("live binding slot limit of 0"));
        assert!(error.help.unwrap().contains("max_live_slots"));
    }
}

#[test]
fn exact_call_and_frame_boundaries_keep_mutable_locals_isolated() {
    let source = "funcra copy(value: integer) -> integer { var local=value; local+=1; return local; } print(copy(4));";
    let program = scarp::compile(source).unwrap();
    let options = ExecutionOptions {
        max_call_depth: 1,
        max_calls: 1,
        max_live_slots: 3,
        ..ExecutionOptions::default()
    };
    for _ in 0..3 {
        let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
        scarp::interpreter::execute_with_io_and_options(
            &program,
            &mut stdout,
            &mut stderr,
            options,
        )
        .unwrap();
        assert_eq!(stdout, b"5\n");
        assert!(stderr.is_empty());
    }
    let error = failure(
        source,
        ExecutionOptions {
            max_live_slots: 2,
            ..options
        },
    );
    assert_eq!(error.kind, DiagnosticKind::CallLimit);
}
