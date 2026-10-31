use scarp::{
    diagnostics::{Diagnostic, DiagnosticKind as Kind},
    ExecutionLimits, ExecutionOptions, RunError,
};

fn output(source: &str) -> (String, String) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    scarp::run_with_io(source, &mut out, &mut err).unwrap();
    scarp::check(source).unwrap();
    (
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

fn invalid(source: &str, kind: Kind) -> Diagnostic {
    let error = scarp::compile(source).unwrap_err();
    assert_eq!(error.kind, kind, "{source}: {error:?}");
    error
}

fn runtime_error(source: &str, options: ExecutionOptions, kind: Kind) -> Diagnostic {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let error = scarp::run_with_io_and_options(source, &mut out, &mut err, options).unwrap_err();
    assert!(out.is_empty() && err.is_empty());
    let RunError::Diagnostic(error) = error else {
        panic!("expected runtime diagnostic")
    };
    assert_eq!(error.kind, kind, "{source}: {error:?}");
    assert_eq!(
        scarp::check_with_options(source, options).unwrap_err(),
        error
    );
    error
}

#[test]
fn required_examples_execute_actual_results() {
    for (source, expected) in [
        ("funcra sum(a: integer, b: integer) -> integer { return a + b; } print(sum(2, 3));", "5\n"),
        ("funcra identity(x: string) -> string { return x; } print(identity(\"Scarp\"));", "Scarp\n"),
        ("funcra fib(n: integer) -> integer { if n <= 1 { return n; } return fib(n-1) + fib(n-2); } print(fib(10));", "55\n"),
        ("pure funcra clipped(x: integer, lo: integer = 0, hi: integer = 10) -> integer needs lo <= hi; ensures result >= lo and result <= hi; { if x < lo { return lo; } if x > hi { return hi; } return x; } print(clipped(x: 15));", "10\n"),
        ("funcra add(a: integer, b: integer) -> integer { return a + b; } funcra square(value: integer) -> integer { return value*value; } bind result = add(10,square(5)); print(result);", "35\n"),
    ] { assert_eq!(output(source), (expected.into(), String::new())); }
}

#[test]
fn recursion_forward_calls_and_mutual_recursion_use_fresh_frames() {
    let source = "print(even(21), odd(21));
        funcra even(n: integer) -> boolean { if n == 0 { return true; } return odd(n-1); }
        funcra odd(n: integer) -> boolean { if n == 0 { return false; } return even(n-1); }
        funcra factorial(n: integer) -> integer { if n <= 1 { return 1; } return n*factorial(n-1); }
        print(factorial(5), factorial(3));";
    let program = scarp::compile(source).unwrap();
    for _ in 0..5 {
        let mut out = Vec::new();
        scarp::interpreter::execute(&program, &mut out).unwrap();
        assert_eq!(out, b"false true\n120 6\n");
    }
    assert!(program.functions().iter().all(|f| f.recursive));
}

#[test]
fn all_primitive_types_pass_by_value_and_parameters_are_immutable() {
    assert_eq!(output("funcra transform(n: integer, s: string, b: boolean) -> integer { print(s,b); var copy=n; copy+=2; return copy; } var n=3; print(transform(n,\"café\",true)); print(n);"), ("café true\n5\n3\n".into(), "".into()));
    invalid(
        "funcra f(n: integer) -> unit { n = 3; }",
        Kind::ImmutableAssignment,
    );
    invalid(
        "funcra f(n: boolean) -> boolean { return n; } print(f(1));",
        Kind::TypeMismatch,
    );
}

#[test]
fn unit_calls_fallthrough_and_explicit_return_are_statement_only() {
    assert_eq!(output("funcra greet(s: string) -> unit { print(\"Hello\",s); return; print(\"unreachable\"); }
        funcra empty() -> unit {} greet(\"Scarp\"); empty();
        funcra value() -> integer { return 4; } value();"), ("Hello Scarp\n".into(), "".into()));
    for tail in [
        "bind x = f();",
        "print(f());",
        "print(f() == f());",
        "var x = (f());",
    ] {
        invalid(
            &format!("funcra f() -> unit {{}} {tail}"),
            Kind::InvalidReturn,
        );
    }
    invalid("funcra f() -> unit {} print(f()+1);", Kind::TypeMismatch);
    invalid(
        "funcra f() -> unit {} funcra g(x: integer) -> unit {} g(f());",
        Kind::TypeMismatch,
    );
}

#[test]
fn returns_unwind_only_the_current_frame_and_nested_loop() {
    assert_eq!(
        output(
            "funcra search(n: integer) -> integer { var i = 0;
        while i < n { i += 1; if i < 2 { continue; } { if i == 3 { return i; } } }
        return -1; }
        var j = 0; while j < 3 { print(search(j+2)); j += 1; }"
        ),
        ("-1\n3\n3\n".into(), "".into())
    );
    invalid(
        "while true { funcra f() -> unit { break; } }",
        Kind::FunctionUnsupported,
    );
    invalid("funcra f() -> unit { break; }", Kind::LoopControl);
    invalid("funcra f() -> unit { continue; }", Kind::LoopControl);
}

#[test]
fn return_flow_and_unused_bodies_are_checked() {
    for source in [
        "return;",
        "{ return 1; }",
        "funcra f() -> integer { return; }",
        "funcra f() -> integer { return true; }",
        "funcra f() -> unit { return 1; }",
    ] {
        invalid(source, Kind::InvalidReturn);
    }
    for body in ["", "if true { return 1; }", "while true { return 1; }"] {
        invalid(
            &format!("funcra f() -> integer {{ {body} }}"),
            Kind::MissingReturn,
        );
    }
    scarp::compile("funcra f(n: integer) -> integer { if n==0 { return 0; } elif n==1 { return 1; } else { return 2; } }").unwrap();
    invalid(
        "funcra f() -> integer { return 1; print(missing); }",
        Kind::UnknownName,
    );
    invalid(
        "funcra unused() -> unit { print(1+true); }",
        Kind::TypeMismatch,
    );
}

#[test]
fn lexical_identity_does_not_capture_file_bindings() {
    invalid(
        "bind outer = 9; funcra f() -> integer { return outer; }",
        Kind::UnknownName,
    );
    for source in [
        "funcra f() -> unit {} bind f = 1;",
        "bind f = 1; funcra f() -> unit {}",
        "funcra f() -> unit {} funcra g(f: integer) -> unit {}",
        "funcra f() -> unit {} { bind f = 3; }",
    ] {
        invalid(source, Kind::FunctionDefinition);
    }
    let duplicate = invalid(
        "funcra f() -> unit {} funcra f() -> unit {}",
        Kind::FunctionDefinition,
    );
    assert_eq!(duplicate.related.len(), 1);
    assert!(duplicate.span.start > duplicate.related[0].span.start);
    assert_eq!(output("funcra f(n: integer) -> integer { bind n = n+1; { bind n = n+1; print(n); } return n; } print(f(1));"),
        ("3\n2\n".into(), "".into()));
}

#[test]
fn function_names_are_collected_file_wide_and_bindings_are_not_callable() {
    assert_eq!(
        output("print(sum(4,5)); funcra sum(a: integer,b: integer) -> integer { return a+b; }"),
        ("9\n".into(), "".into())
    );
    invalid("funcra print() -> unit {}", Kind::ReservedName);
    invalid("bind f = 3; f();", Kind::UnknownFunction);
    invalid("missing();", Kind::UnknownFunction);
}

#[test]
fn named_arguments_evaluate_once_in_source_order_before_defaults() {
    assert_eq!(
        output(
            "funcra mark(n: integer) -> integer { print(n); return n; }
        pure funcra next(n: integer) -> integer { return n+1; }
        funcra f(a: integer, b: integer, c: integer = next(a)) -> integer { return a*100+b*10+c; }
        print(\"answer\",f(b:mark(2),a:mark(1)));"
        ),
        ("2\n1\nanswer 122\n".into(), "".into())
    );
    assert_eq!(
        output(
            "funcra f(a: integer=2,b: integer=a+1,c: integer=b+1) -> integer { return a+b+c; }
        print(f(),f(c:9,a:5),f(1,2,3,),f(b:10));"
        ),
        ("9 20 6 23\n".into(), "".into())
    );
    assert_eq!(
        output("funcra f(a: integer=1/0) -> integer { return a; } print(f(7));"),
        ("7\n".into(), "".into())
    );
}

#[test]
fn argument_mapping_and_default_rules_produce_static_diagnostics() {
    let prefix = "funcra f(a: integer,b: integer=2) -> integer { return a+b; } ";
    for call in [
        "f();",
        "f(1,2,3);",
        "f(a:1,2);",
        "f(1,a:2);",
        "f(a:1,a:2);",
        "f(z:1);",
    ] {
        invalid(&format!("{prefix}{call}"), Kind::CallArguments);
    }
    invalid(&format!("{prefix}f(a:true);"), Kind::TypeMismatch);
    invalid(
        "funcra f(a: integer=1,b: integer) -> unit {}",
        Kind::FunctionDefinition,
    );
    invalid(
        "funcra f(a: integer,b: integer,a: integer) -> unit {}",
        Kind::FunctionDefinition,
    );
    invalid(
        "funcra f(a: integer=b,b: integer=1) -> unit {}",
        Kind::UnknownName,
    );
    invalid("funcra f(a: integer=a) -> unit {}", Kind::UnknownName);
    invalid(
        "bind x=1; funcra f(a: integer=x) -> unit {}",
        Kind::UnknownName,
    );
    invalid("funcra f(a: integer=true) -> unit {}", Kind::TypeMismatch);
    invalid(
        "funcra effect() -> integer { return 1; } funcra f(n: integer=effect()) -> unit {}",
        Kind::ImpureFunction,
    );
}

#[test]
fn purity_is_declared_checked_and_transitive_with_local_mutation_allowed() {
    assert_eq!(
        output(
            "pure funcra total(n: integer) -> integer { var sum=0; var i=0;
        while i<n { sum+=i; i+=1; } return sum; } print(total(4));"
        ),
        ("6\n".into(), "".into())
    );
    for operation in [
        "print();",
        "println(1);",
        "write();",
        "eprint();",
        "ewrite();",
    ] {
        invalid(
            &format!("pure funcra f() -> unit {{ {operation} }}"),
            Kind::ImpureFunction,
        );
    }
    invalid(
        "pure funcra f() -> unit { g(); } funcra g() -> unit {}",
        Kind::ImpureFunction,
    );
    invalid(
        "pure funcra f() -> unit { if false { g(); } } funcra g() -> unit {}",
        Kind::ImpureFunction,
    );
    assert_eq!(output("pure funcra f(n: integer) -> integer { if n==0 { return 0; } return f(n-1)+1; } print(f(9));"), ("9\n".into(), "".into()));
}

#[test]
fn contracts_see_parameters_and_result_but_never_body_locals() {
    assert_eq!(output("pure funcra f(n: integer) -> integer needs n>0; needs n<10; ensures result==n+1; ensures result>0; { return n+1; } print(f(2));
        funcra unit(n: integer) -> unit needs n==1; ensures n==1; {} unit(1);"), ("3\n".into(), "".into()));
    for source in [
        "funcra f() -> unit needs result==0; {}",
        "funcra f() -> unit ensures result==0; {}",
        "funcra f() -> integer ensures x==0; { bind x=0; return x; }",
    ] {
        invalid(source, Kind::UnknownName);
    }
    invalid("funcra f() -> unit needs 1; {}", Kind::TypeMismatch);
    invalid(
        "funcra g() -> boolean { return true; } funcra f() -> unit ensures g(); {}",
        Kind::ImpureFunction,
    );
}

#[test]
fn failed_contracts_discard_both_streams_and_keep_source_trace() {
    for clause in ["needs n>0;", "ensures result>0;"] {
        let source = format!("funcra f(n: integer) -> integer {clause} {{ eprint(\"hidden\"); return n; }} print(\"hidden\"); print(f(-1));");
        let error = runtime_error(
            &source,
            ExecutionOptions::default(),
            Kind::ContractViolation,
        );
        assert!(source[error.span.start..error.span.end].contains(clause));
        assert_eq!(error.related.len(), 1);
        assert!(error.message.contains("funcra f"));
    }
    runtime_error(
        "funcra f() -> integer needs false; needs 1/0>0; { return 1; } print(f());",
        ExecutionOptions::default(),
        Kind::ContractViolation,
    );
}

#[test]
fn arithmetic_errors_and_left_to_right_short_circuit_keep_existing_kinds() {
    for (expression, kind) in [
        ("9223372036854775807+1", Kind::ArithmeticOverflow),
        ("1/0", Kind::DivisionByZero),
    ] {
        let ty = "integer";
        runtime_error(
            &format!(
                "funcra f() -> {ty} {{ return {expression}; }} print(\"hidden\"); eprint(f());"
            ),
            ExecutionOptions::default(),
            kind,
        );
    }
    assert_eq!(output("funcra bad() -> boolean { print(\"bad\"); return 1/0==0; } print(false and bad(), true or bad());"), ("false true\n".into(), "".into()));
    runtime_error(
        "funcra f(a: integer,b: integer) -> unit {} f(1/0,9223372036854775807+1);",
        ExecutionOptions::default(),
        Kind::DivisionByZero,
    );
}

#[test]
fn depth_calls_slots_and_work_limits_are_deterministic() {
    let source = "pure funcra f(n: integer) -> integer { if n==0 { return 0; } return f(n-1)+0; } print(f(4));";
    let mut options = ExecutionOptions {
        max_call_depth: 5,
        max_calls: 5,
        ..ExecutionOptions::default()
    };
    scarp::check_with_options(source, options).unwrap();
    options.max_call_depth = 4;
    runtime_error(source, options, Kind::CallLimit);
    options.max_call_depth = 5;
    options.max_calls = 4;
    runtime_error(source, options, Kind::CallLimit);
    options.max_calls = 5;
    options.max_live_slots = 9;
    runtime_error(source, options, Kind::CallLimit);
    options.max_live_slots = 10;
    scarp::check_with_options(source, options).unwrap();
    options.limits.max_steps = 1;
    runtime_error(source, options, Kind::ExecutionLimit);
    runtime_error(
        "funcra f() -> unit {} f();",
        ExecutionOptions {
            max_call_depth: 0,
            ..ExecutionOptions::default()
        },
        Kind::CallLimit,
    );
}

#[test]
fn deep_recursion_uses_heap_frames_on_a_small_host_stack() {
    let program = scarp::compile("funcra f(n: integer) -> integer { if n==0 { return 0; } return f(n-1)+1; } print(f(10000));").unwrap();
    std::thread::Builder::new()
        .stack_size(128 * 1024)
        .spawn(move || {
            let mut out = Vec::new();
            scarp::interpreter::execute_with_options(
                &program,
                &mut out,
                ExecutionOptions {
                    max_call_depth: 10001,
                    ..ExecutionOptions::default()
                },
            )
            .unwrap();
            assert_eq!(out, b"10000\n");
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn default_and_contract_recursion_share_budgets_and_bounded_traces() {
    for source in [
        "pure funcra f(n: integer=f()) -> integer { return n; } print(f());",
        "pure funcra f() -> integer needs f()==1; { return 1; } print(f());",
        "pure funcra f() -> integer ensures f()==1; { return 1; } print(f());",
    ] {
        let error = runtime_error(source, ExecutionOptions::default(), Kind::CallLimit);
        assert_eq!(error.related.len(), 16);
        assert!(error.related.iter().all(|r| r.message.len() < 120));
    }
}

#[test]
fn capture_budget_counts_pending_outer_arguments_and_validation_equally() {
    let source =
        "funcra f() -> string { eprint(\"nested\"); return \"value\"; } print(\"prefix\",f());";
    runtime_error(
        source,
        ExecutionOptions {
            max_capture_bytes: 100,
            ..ExecutionOptions::default()
        },
        Kind::CaptureLimit,
    );
    assert_eq!(output(source), ("prefix value\n".into(), "nested\n".into()));
    runtime_error(
        "funcra f() -> unit { print(); } f(); f();",
        ExecutionOptions {
            limits: ExecutionLimits {
                max_steps: 100,
                max_prints: 1,
            },
            ..ExecutionOptions::default()
        },
        Kind::OutputLimit,
    );
}

#[test]
fn old_single_stream_apis_reject_required_stderr_before_stdout() {
    let source = "funcra f() -> unit { eprint(\"error\"); } print(\"hidden\"); f();";
    let mut out = Vec::new();
    let RunError::Output(error) = scarp::run(source, &mut out).unwrap_err() else {
        panic!()
    };
    assert_eq!(error.kind(), std::io::ErrorKind::Unsupported);
    assert!(out.is_empty());
    assert_eq!(
        output("funcra f() -> unit { ewrite(); } f(); print(1);"),
        ("1\n".into(), "".into())
    );
}

#[test]
fn metadata_is_deterministic_complete_and_does_not_execute() {
    let source =
        "pure funcra z(n: integer=1/0) -> integer needs n>0; ensures result>0; { return a(n); }
        pure funcra a(n: integer) -> integer { return z(n); }
        funcra unused() -> unit { print(1/0); } z();";
    let program = scarp::compile(source).unwrap();
    let functions = program.functions();
    assert_eq!(
        functions
            .iter()
            .map(|f| f.name.as_str())
            .collect::<Vec<_>>(),
        ["z", "a", "unused"]
    );
    assert!(functions[0].recursive && functions[1].recursive && !functions[2].recursive);
    assert_eq!(functions[1].callees, ["z"]);
    let mut a = Vec::new();
    let mut b = Vec::new();
    scarp::resolver::write_function_insight(&program, &mut a).unwrap();
    scarp::resolver::write_function_insight(&scarp::compile(source).unwrap(), &mut b).unwrap();
    assert_eq!(a, b);
    let text = String::from_utf8(a).unwrap();
    assert!(
        text.contains("default@") && text.contains("ensures [") && text.contains("recursive: true")
    );
}

#[test]
fn future_function_forms_are_explicitly_unsupported() {
    for source in [
        "bind f = funcra (n: integer) -> integer { return n; };",
        "bind f = pure funcra () -> unit {};",
        "funcra f(g: funcra(integer)->integer)->unit {}",
        "funcra f() -> unit {} bind g = f;",
        "funcra f() -> unit { funcra g() -> unit {} }",
    ] {
        invalid(source, Kind::FunctionUnsupported);
    }
}

#[test]
fn tokens_spans_case_and_adjacent_arrow_follow_source_bytes() {
    let source = "// café 🌍\r\nfuncra f(n: integer,) -> integer { return n; } print(f(n:2,));";
    let tokens = scarp::lexer::lex(source).unwrap();
    let arrow = tokens
        .iter()
        .find(|t| t.kind == scarp::lexer::TokenKind::Arrow)
        .unwrap();
    assert_eq!(&source[arrow.span.start..arrow.span.end], "->");
    assert_eq!(output(source).0, "2\n");
    invalid("funcra f() - > unit {}", Kind::UnexpectedToken);
    invalid("Funcra f() -> unit {}", Kind::UnknownStatement);
    invalid("funcra f(n: unit) -> unit {}", Kind::FunctionDefinition);
    invalid("funcra f(n) -> unit {}", Kind::UnexpectedToken);
    invalid("funcra f() {}", Kind::UnexpectedToken);
}

#[test]
fn adversarial_function_grammar_terminates_with_valid_diagnostic_spans() {
    let source="// café\r\npure funcra f(n: integer=2) -> integer needs n>0; ensures result>0; { if n==1 { return 1; } return f(n:n-1); } print(f());";
    for (end, _) in source.char_indices() {
        if let Err(error) = scarp::compile(&source[..end]) {
            assert!(
                error.span.start <= error.span.end && error.span.end <= end,
                "{error:?}"
            );
            assert!(
                source.is_char_boundary(error.span.start)
                    && source.is_char_boundary(error.span.end)
            );
        }
    }
    for token in [
        "(", ")", "->", ":", ",", "funcra", "return", "needs", "ensures", "pure", ";",
    ] {
        for size in [1, 2, 16, 128, 1024] {
            let _ = scarp::compile(&token.repeat(size));
        }
    }
    let nested = format!(
        "funcra f(n: integer) -> integer {{ return n; }} print({}1{});",
        "f(".repeat(65),
        ")".repeat(65)
    );
    invalid(&nested, Kind::ExpressionNesting);
    let large = format!(
        "funcra f({}) -> unit {{}}",
        (0..1025)
            .map(|i| format!("p{i}: integer"))
            .collect::<Vec<_>>()
            .join(",")
    );
    invalid(&large, Kind::FunctionDefinition);
}

#[test]
fn generated_named_calls_match_an_independent_argument_model() {
    for n in 0..100 {
        let source=format!("funcra combine(a: integer,b: integer=2,c: integer=a+b) -> integer {{ return a*100+b*10+c; }} print(combine(b:{},a:{}));",n%7,n);
        assert_eq!(output(&source).0, format!("{}\n", n * 101 + (n % 7) * 11));
    }
}

#[test]
fn function_diagnostic_fixtures_match_exact_rendered_locations_and_traces() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/functions");
    for name in ["contract", "return_type", "named_argument", "unicode_call"] {
        let filename = format!("{name}.srp");
        let source = std::fs::read_to_string(root.join(&filename)).unwrap();
        let error = scarp::check(&source).unwrap_err();
        let rendered = format!("{}\n", error.render(&filename, &source));
        assert_eq!(
            rendered.as_bytes(),
            std::fs::read(root.join(format!("{name}.stderr"))).unwrap(),
            "{name}"
        );
    }
}

#[test]
fn postcondition_result_is_distinct_from_a_function_with_that_name() {
    assert_eq!(
        output(
            "pure funcra result() -> integer { return 9; }
        pure funcra f() -> integer ensures result==result(); { return 9; } print(f());"
        )
        .0,
        "9\n"
    );
}
