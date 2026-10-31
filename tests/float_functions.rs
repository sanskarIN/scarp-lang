use scarp::{
    diagnostics::{Diagnostic, DiagnosticKind as Kind},
    value::FiniteFloat,
    ExecutionLimits, ExecutionOptions, RunError,
};

fn output(source: &str) -> (Vec<u8>, Vec<u8>) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    scarp::run_with_io(source, &mut out, &mut err).unwrap();
    scarp::check(source).unwrap();
    (out, err)
}

fn failure(source: &str, options: ExecutionOptions, kind: Kind) -> Diagnostic {
    let expected = scarp::check_with_options(source, options).unwrap_err();
    assert_eq!(expected.kind, kind, "{source}: {expected:?}");
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let RunError::Diagnostic(actual) =
        scarp::run_with_io_and_options(source, &mut out, &mut err, options).unwrap_err()
    else {
        panic!("expected diagnostic")
    };
    assert!(out.is_empty() && err.is_empty());
    assert_eq!(expected, actual);
    actual
}

#[test]
fn float_defaults_named_calls_and_recursive_returns_keep_frame_values() {
    let source = r#"
        pure funcra advance(value: float, step: float = 0.25) -> float {
            return value + step;
        }
        pure funcra recurse(n: integer, value: float) -> float {
            if n == 0 { return value; }
            return recurse(n - 1, advance(value));
        }
        print(advance(1.0), advance(step: 0.5, value: 2.0), recurse(8, -0.0));
    "#;
    assert_eq!(output(source).0, b"1.25 2.5 2.0\n");
    let program = scarp::compile(source).unwrap();
    assert_eq!(
        program.functions()[0].parameters[0].kind,
        scarp::value::ValueType::Float
    );
    assert_eq!(
        program.functions()[0].result,
        scarp::value::ValueType::Float
    );
    assert!(program.functions()[1].recursive);
}

#[test]
fn explicit_argument_effects_precede_outer_float_output_in_source_order() {
    let source = r#"
        funcra mark(value: float) -> float { eprint(value); return value; }
        funcra sum(a: float, b: float) -> float { return a + b; }
        print("sum", sum(b: mark(0.5), a: mark(0.25)));
    "#;
    assert_eq!(
        output(source),
        (b"sum 0.75\n".to_vec(), b"0.5\n0.25\n".to_vec())
    );
}

#[test]
fn float_contracts_defaults_and_body_mutation_are_typed_and_pure() {
    let source = r#"
        pure funcra half(value: float) -> float { return value / 2.0; }
        pure funcra f(value: float, step: float = half(value)) -> float
            needs value > 0.0;
            ensures result >= value;
        { var local = value; local += step; return local; }
        print(f(2.0));
    "#;
    assert_eq!(output(source).0, b"3.0\n");
    for source in [
        "pure funcra f() -> float { eprint(1.0); return 1.0; }",
        "funcra f() -> float { return 1.0; } funcra g(n: float=f()) -> unit {}",
    ] {
        failure(source, ExecutionOptions::default(), Kind::ImpureFunction);
    }
    for clause in ["needs value > 0.0;", "ensures result > 0.0;"] {
        failure(&format!("funcra f(value: float) -> float {clause} {{ eprint(1.0); return value; }} print(2.0); print(f(-1.0));"), ExecutionOptions::default(), Kind::ContractViolation);
    }
}

#[test]
fn integer_float_mixing_stays_a_static_error_in_unused_function_paths() {
    for source in [
        "funcra f(value: float) -> float { return value; } print(f(1));",
        "funcra f(value: float=1) -> unit {}",
        "funcra f(value: float) -> unit { value += 1.0; }",
        "funcra unused() -> unit { var value=0.0; value+=1; }",
        "funcra unused() -> unit { if false { print(1.0+1); } }",
        "repeat 1.0 as index {}",
        "while 1.0 {}",
    ] {
        let kind = if source.contains("value += 1.0") {
            Kind::ImmutableAssignment
        } else {
            Kind::TypeMismatch
        };
        failure(source, ExecutionOptions::default(), kind);
    }
    failure(
        "funcra f() -> float { return 1; }",
        ExecutionOptions::default(),
        Kind::InvalidReturn,
    );
    let error = failure(
        "// café 🌍\r\nfuncra unused(value: float = 1) -> unit {}",
        ExecutionOptions::default(),
        Kind::TypeMismatch,
    );
    assert_eq!(error.related.len(), 1);
}

#[test]
fn loop_assignments_shadowing_and_short_circuiting_preserve_float_snapshots() {
    let source = r#"
        pure funcra total(n: integer) -> float {
            var value = 0.0;
            repeat n as i { value += 0.25; }
            { bind value = value + 0.5; if value < 0.0 { return value; } }
            return value;
        }
        var value = total(4); print(value); value *= 2.0; print(value);
        print(false and 1.0 / -0.0 > 0.0, true or 1e308 * 2.0 == 0.0);
    "#;
    assert_eq!(output(source).0, b"1.0\n2.0\nfalse true\n");
}

#[test]
fn float_failures_keep_operator_spans_bounded_traces_and_atomic_streams() {
    for (expression, kind, operator) in [
        ("1e308 * 2.0", Kind::NonFiniteNumber, "*"),
        ("1.0 / -0.0", Kind::DivisionByZero, "/"),
    ] {
        let source = format!("// café 🌍\r\nfuncra f() -> float {{ return {expression}; }} print(1.0); eprint(2.0); print(f());");
        let error = failure(&source, ExecutionOptions::default(), kind);
        assert_eq!(&source[error.span.start..error.span.end], operator);
        assert_eq!(error.related.len(), 1);
        assert!(error.render("float.srp", &source).contains("float.srp:2:"));
    }
    for literal in [
        ".5",
        "1.",
        "1e",
        "1e+",
        "1.2.3",
        "1e2e3",
        "1.0_suffix",
        "1e309",
    ] {
        failure(
            &format!("print(1.0); eprint(2.0); print({literal});"),
            ExecutionOptions::default(),
            Kind::InvalidFloat,
        );
    }
    assert_eq!(Kind::InvalidFloat.code(), "E0030");
    assert_eq!(Kind::NonFiniteNumber.code(), "E0031");
    assert_eq!(Kind::CaptureLimit.code(), "E0029");
}

#[test]
fn all_six_resource_boundaries_apply_to_float_default_calls() {
    let source = "pure funcra twice(n: float=0.5) -> float { return n*2.0; } print(twice());";
    let options = ExecutionOptions {
        limits: ExecutionLimits {
            max_steps: 13,
            max_prints: 1,
        },
        max_call_depth: 1,
        max_calls: 1,
        max_live_slots: 2,
        max_capture_bytes: 72,
        max_vector_bytes: ExecutionOptions::default().max_vector_bytes,
    };
    scarp::check_with_options(source, options).unwrap();
    for (options, kind) in [
        (
            ExecutionOptions {
                limits: ExecutionLimits {
                    max_steps: 12,
                    ..options.limits
                },
                ..options
            },
            Kind::ExecutionLimit,
        ),
        (
            ExecutionOptions {
                limits: ExecutionLimits {
                    max_prints: 0,
                    ..options.limits
                },
                ..options
            },
            Kind::OutputLimit,
        ),
        (
            ExecutionOptions {
                max_call_depth: 0,
                ..options
            },
            Kind::CallLimit,
        ),
        (
            ExecutionOptions {
                max_calls: 0,
                ..options
            },
            Kind::CallLimit,
        ),
        (
            ExecutionOptions {
                max_live_slots: 1,
                ..options
            },
            Kind::CallLimit,
        ),
        (
            ExecutionOptions {
                max_capture_bytes: 71,
                ..options
            },
            Kind::CaptureLimit,
        ),
    ] {
        failure(source, options, kind);
    }
}

#[test]
fn finite_display_round_trips_bits_including_subnormals_and_signed_zero() {
    let mut state = 0x5120_0100_a17e_8723_u64;
    for bits in [0, 1_u64 << 63, 1, (1_u64 << 63) | 1, f64::MAX.to_bits()]
        .into_iter()
        .chain((0..10_000).map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        }))
    {
        if let Some(value) = FiniteFloat::new(f64::from_bits(bits)) {
            let text = value.to_string();
            assert_eq!(text.parse::<f64>().unwrap().to_bits(), bits, "{text}");
        }
    }
    assert_eq!(FiniteFloat::new(0.0), FiniteFloat::new(-0.0));
    assert_eq!(
        output("print(5e-324, 2e-324, -2e-324, 5e-324 / 2.0, -5e-324 / 2.0);").0,
        b"5e-324 0.0 -0.0 0.0 -0.0\n"
    );
}

#[test]
fn float_program_reuse_and_concurrency_do_not_share_frame_state() {
    let program = std::sync::Arc::new(scarp::compile("pure funcra f(n: integer, v: float=0.0) -> float { if n==0 { return v; } return f(n-1,v+0.25); } print(f(32));").unwrap());
    let threads: Vec<_> = (0..8)
        .map(|_| {
            let program = program.clone();
            std::thread::spawn(move || {
                for _ in 0..3 {
                    let mut out = Vec::new();
                    scarp::interpreter::execute(&program, &mut out).unwrap();
                    assert_eq!(out, b"8.0\n");
                }
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
}

#[test]
fn truncated_float_function_sources_keep_valid_utf8_spans() {
    let source = "// café 🌍\r\npure funcra f(value: float=2.5E-4) -> float needs value>0.0; ensures result>=0.0; { return value*0.5; } print(f());";
    for (end, _) in source.char_indices() {
        if let Err(error) = scarp::compile(&source[..end]) {
            assert!(error.span.start <= error.span.end && error.span.end <= end);
            assert!(
                source.is_char_boundary(error.span.start)
                    && source.is_char_boundary(error.span.end)
            );
        }
    }
}

#[test]
fn generated_float_calls_agree_with_an_exact_integer_fraction_model() {
    for n in -100..100 {
        let a = format!("{:?}", f64::from(n) / 8.0);
        let b = format!("{:?}", f64::from(n % 7) / 8.0);
        let source = format!("pure funcra combine(a: float,b: float) -> float {{ return (a+b)*0.5+0.125; }} print(combine(b:{b},a:{a}));");
        // All intermediates are exactly representable multiples of 1/16.
        let expected = f64::from(n + n % 7 + 2) / 16.0;
        assert_eq!(output(&source).0, format!("{expected:?}\n").as_bytes());
    }
}

#[test]
fn long_flat_float_expressions_do_not_recurse_on_the_host_stack() {
    let source = format!("print(0.0{});", "+0.5".repeat(50_000));
    assert_eq!(output(&source).0, b"25000.0\n");
}

#[test]
fn float_type_word_is_contextual_and_does_not_reserve_existing_bindings() {
    assert_eq!(output("bind float=2; funcra identity(float: float) -> float { return float; } print(float, identity(1.5));").0, b"2 1.5\n");
}

#[test]
fn literal_rounding_and_exponent_boundaries_preserve_integer_precision() {
    assert_eq!(output("print(1.7976931348623157e308 > 1e308, -1.7976931348623157e308 < -1e308); print(9007199254740993, 9007199254740993.0 == 9007199254740992.0); print(1e-99999999);").0,
        b"true true\n9007199254740993 true\n0.0\n");
    for literal in ["1.7976931348623159e308", "1e99999999"] {
        failure(
            &format!("print({literal});"),
            ExecutionOptions::default(),
            Kind::InvalidFloat,
        );
    }
}
