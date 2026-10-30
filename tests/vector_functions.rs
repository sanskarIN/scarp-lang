use scarp::{
    diagnostics::{Diagnostic, DiagnosticKind as Kind},
    numeric::{self, Builtin},
    value::{FiniteFloat, NumericVector, Value, ValueType},
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

fn vector(values: &[f64]) -> Value {
    Value::Vector(NumericVector::new(values).unwrap())
}
fn scalar(kernel: Builtin, args: &[Value]) -> f64 {
    numeric::evaluate(kernel, args, 1_000_000)
        .unwrap()
        .as_float()
        .unwrap()
}

#[test]
fn vectors_cross_defaults_named_calls_recursion_and_postconditions() {
    let source = r#"
        pure funcra amplify(v: vector = [1.0, 2.0], factor: float = 2.0) -> vector
            needs len(v) > 0;
            ensures len(result) == len(v);
        { return scale(v, factor); }
        pure funcra recurse(n: integer, v: vector) -> vector {
            if n == 0 { return v; }
            return recurse(n - 1, amplify(factor: 0.5, v: v));
        }
        var v = amplify(); bind snapshot = v; print(v);
        v = recurse(v: v, n: 2); eprint(v); print(snapshot, amplify(factor: 3.0));
    "#;
    assert_eq!(
        output(source),
        (
            b"[2.0, 4.0]\n[2.0, 4.0] [3.0, 6.0]\n".to_vec(),
            b"[0.5, 1.0]\n".to_vec()
        )
    );
    let program = scarp::compile(source).unwrap();
    assert_eq!(program.functions()[0].result, ValueType::Vector);
    assert!(program.functions()[0].callees.is_empty());
    assert_eq!(program.functions()[1].callees, ["amplify", "recurse"]);
    assert!(program.functions()[1].recursive);
}

#[test]
fn argument_effects_are_ordered_inside_vector_literals_and_kernel_calls() {
    let source = r#"
        funcra mark(v: float) -> float { eprint(v); return v; }
        funcra make(v: float) -> vector { return [mark(v)]; }
        print(add(make(1.0), [mark(2.0)]), [mark(3.0), mark(4.0)]);
    "#;
    assert_eq!(
        output(source),
        (
            b"[3.0] [3.0, 4.0]\n".to_vec(),
            b"1.0\n2.0\n3.0\n4.0\n".to_vec()
        )
    );
}

#[test]
fn old_user_function_names_take_precedence_over_new_kernels() {
    assert_eq!(output("pure funcra sum(a: float, b: float=1.0) -> float { return a+b; } pure funcra len(value: integer) -> integer { return value+1; } print(sum(b: 2.0, a: 3.0), len(4));").0, b"5.0 5\n");
    assert_eq!(
        output("bind sum = sum([1.0, 2.0]); bind vector = []; print(sum, len(vector));").0,
        b"3.0 0\n"
    );
    failure(
        "print(sum(v: []));",
        ExecutionOptions::default(),
        Kind::CallArguments,
    );
    // Native calls do not consume user-call counters or depth.
    scarp::check_with_options(
        "print(sum(fill(0.5, 2)));",
        ExecutionOptions {
            max_calls: 0,
            max_call_depth: 0,
            ..ExecutionOptions::default()
        },
    )
    .unwrap();
}

#[test]
fn pure_contracts_and_unselected_paths_still_check_vector_types() {
    for (source, kind) in [
        ("pure funcra f(v: vector) -> vector { return add(v, [1]); }", Kind::TypeMismatch),
        ("funcra f(v: vector = [1]) -> unit {}", Kind::TypeMismatch),
        ("funcra f() -> vector { return 1.0; }", Kind::InvalidReturn),
        ("funcra f(v: vector) -> unit { v = []; }", Kind::ImmutableAssignment),
        ("pure funcra f() -> vector { eprint([]); return []; }", Kind::ImpureFunction),
        ("funcra g() -> vector { return []; } pure funcra f() -> vector { return g(); }", Kind::ImpureFunction),
        ("funcra f(v: vector) -> vector ensures len(result)>0; { print(1); eprint(2); return v; } print(f([]));", Kind::ContractViolation),
    ] { failure(source, ExecutionOptions::default(), kind); }
    assert_eq!(
        output("print(true or mean([])>0.0, false and [1.0][-1]>0.0);").0,
        b"true false\n"
    );
}

#[test]
fn every_existing_budget_has_an_exact_vector_call_boundary() {
    let source =
        "pure funcra f(v: vector=[1.0,2.0]) -> vector { return scale(v,2.0); } print(f());";
    let options = ExecutionOptions {
        limits: ExecutionLimits {
            max_steps: 22,
            max_prints: 1,
        },
        max_call_depth: 1,
        max_calls: 1,
        max_live_slots: 2,
        max_capture_bytes: 80,
    };
    scarp::check_with_options(source, options).unwrap();
    for (options, kind) in [
        (
            ExecutionOptions {
                limits: ExecutionLimits {
                    max_steps: 21,
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
                max_capture_bytes: 79,
                ..options
            },
            Kind::CaptureLimit,
        ),
    ] {
        failure(source, options, kind);
    }
}

#[test]
fn discarded_snapshots_still_charge_logical_vector_bytes() {
    let source = "var v=fill(1.0, 100); repeat 3 as i { print(v); v=scale(v,2.0); }";
    let options = ExecutionOptions {
        max_capture_bytes: 3 * (64 + 800),
        ..ExecutionOptions::default()
    };
    scarp::check_with_options(source, options).unwrap();
    failure(
        source,
        ExecutionOptions {
            max_capture_bytes: options.max_capture_bytes - 1,
            ..options
        },
        Kind::CaptureLimit,
    );
    let source = "print(1); eprint(2); bind v=fill(1.0,65536);";
    failure(
        source,
        ExecutionOptions {
            limits: ExecutionLimits {
                max_steps: 100,
                max_prints: 10,
            },
            ..options
        },
        Kind::ExecutionLimit,
    );
}

#[test]
fn native_host_api_checks_types_arity_caps_and_immutable_sharing() {
    assert_eq!(
        numeric::evaluate(Builtin::Fill, &[], 0).unwrap_err().kind,
        Kind::CallArguments
    );
    assert_eq!(
        numeric::evaluate(Builtin::Sum, &[Value::Integer(1)], 100)
            .unwrap_err()
            .kind,
        Kind::TypeMismatch
    );
    let value = vector(&[1.0, -0.0]);
    let copy = value.clone();
    assert!(std::ptr::eq(
        value.as_vector().unwrap().as_slice().as_ptr(),
        copy.as_vector().unwrap().as_slice().as_ptr()
    ));
    assert_eq!(
        value.as_vector().unwrap().as_slice()[1].get().to_bits(),
        (-0.0_f64).to_bits()
    );
    assert!(NumericVector::new(&[f64::NEG_INFINITY]).is_none());
    assert!(NumericVector::from_finite(vec![FiniteFloat::new(0.0).unwrap(); 65_537]).is_none());
    for (kernel, args, cost) in [
        (Builtin::Sum, vec![value.clone()], 2),
        (Builtin::Mean, vec![value.clone()], 8),
        (Builtin::Mse, vec![value.clone(), copy], 8),
        (Builtin::Softmax, vec![value.clone()], 8),
        (Builtin::Dot, vec![value.clone(), value], 4),
    ] {
        numeric::evaluate(kernel, &args, cost).unwrap();
        assert_eq!(
            numeric::evaluate(kernel, &args, cost - 1).unwrap_err().kind,
            Kind::ExecutionLimit
        );
    }
}

#[test]
fn stable_mean_and_mse_avoid_unnecessary_intermediate_overflow() {
    assert_eq!(scalar(Builtin::Mean, &[vector(&[1e308, 1e308])]), 1e308);
    assert_eq!(
        scalar(Builtin::Mean, &[vector(&[f64::MAX, f64::MAX])]),
        f64::MAX
    );
    assert_eq!(scalar(Builtin::Mean, &[vector(&[1e308, 3.0, -1e308])]), 1.0);
    let subnormal = f64::from_bits(1);
    assert_eq!(
        scalar(Builtin::Mean, &[vector(&[subnormal, subnormal])]),
        subnormal
    );
    let actual = scalar(
        Builtin::Mse,
        &[vector(&[1e154, 1e154]), vector(&[0.0, 0.0])],
    );
    assert!((actual / 1e308 - 1.0).abs() < 1e-14);
    let mut values = vec![0.0; 1000];
    values[0] = 1e155;
    let actual = scalar(Builtin::Mse, &[vector(&values), vector(&vec![0.0; 1000])]);
    assert!((actual / 1e307 - 1.0).abs() < 1e-14);
    assert_eq!(
        scalar(Builtin::Mse, &[vector(&[1e-160]), vector(&[0.0])]).to_bits(),
        (1e-160_f64 * 1e-160).to_bits()
    );
    assert_eq!(
        numeric::evaluate(Builtin::Mse, &[vector(&[f64::MAX]), vector(&[0.0])], 100)
            .unwrap_err()
            .kind,
        Kind::NonFiniteNumber
    );
}

#[test]
fn generated_reductions_match_independent_exact_fraction_models() {
    let mut state = 0x9e37_u64;
    for count in 1..=64 {
        let mut a = Vec::new();
        let mut b = Vec::new();
        for _ in 0..count {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            a.push(((state >> 32) % 2049) as i64 - 1024);
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            b.push(((state >> 32) % 2049) as i64 - 1024);
        }
        let av = vector(&a.iter().map(|&x| x as f64 / 8.0).collect::<Vec<_>>());
        let bv = vector(&b.iter().map(|&x| x as f64 / 8.0).collect::<Vec<_>>());
        let sum = a.iter().sum::<i64>() as f64 / 8.0;
        let dot = a.iter().zip(&b).map(|(x, y)| x * y).sum::<i64>() as f64 / 64.0;
        let mse = a
            .iter()
            .zip(&b)
            .map(|(x, y)| (x - y) * (x - y))
            .sum::<i64>() as f64
            / (64 * count) as f64;
        assert_eq!(scalar(Builtin::Sum, std::slice::from_ref(&av)), sum);
        assert_eq!(scalar(Builtin::Dot, &[av.clone(), bv.clone()]), dot);
        assert_eq!(
            scalar(Builtin::Mean, std::slice::from_ref(&av)),
            sum / count as f64
        );
        assert!(
            (scalar(Builtin::Mse, &[av, bv]) - mse).abs() <= 16.0 * f64::EPSILON * mse.max(1.0)
        );
    }
}

#[test]
fn signed_zero_subnormal_and_underflow_elements_remain_finite() {
    assert_eq!(output("print([-0.0][0], scale([-0.0,5e-324],0.5), [-0.0]==[0.0]); print(sum([-0.0]), clamp([-0.0],0.0,1.0));").0,b"-0.0 [-0.0, 0.0] true\n0.0 [-0.0]\n");
    failure(
        "print(1); eprint(2); print(scale([1e308],2.0));",
        ExecutionOptions::default(),
        Kind::NonFiniteNumber,
    );
}

#[test]
fn unicode_diagnostics_and_recursive_kernel_errors_keep_precise_spans() {
    let source="// café 🌍\r\npure funcra f(v: vector) -> float { return v[-1]; } print(1); eprint(2); print(f([1.0]));";
    let error = failure(source, ExecutionOptions::default(), Kind::VectorIndex);
    assert_eq!(&source[error.span.start..error.span.end], "[-1]");
    assert_eq!(error.related.len(), 1);
    assert!(error.render("vector.srp", source).contains("vector.srp:2:"));
    for (kind, code) in [
        (Kind::VectorLength, "E0032"),
        (Kind::VectorIndex, "E0033"),
        (Kind::NumericDomain, "E0034"),
        (Kind::VectorAllocation, "E0035"),
    ] {
        assert_eq!(kind.code(), code);
    }
}

#[test]
fn vector_recursion_and_output_are_reusable_without_shared_mutable_state() {
    let source="pure funcra f(n: integer,v: vector) -> vector { if n==0 {return v;} return f(n-1,add(v,[1.0])); } print(f(200,[0.0]));";
    let program = scarp::compile(source).unwrap();
    std::thread::scope(|scope| {
        for _ in 0..8 {
            let program = &program;
            scope.spawn(move || {
                for _ in 0..4 {
                    let mut bytes = Vec::new();
                    scarp::interpreter::execute(program, &mut bytes).unwrap();
                    assert_eq!(bytes, b"[200.0]\n");
                }
            });
        }
    });
}

#[test]
fn softmax_matches_sixty_digit_decimal_reference_probabilities() {
    // Fixture computed independently with Decimal.exp at precision 60.
    for row in include_str!("fixtures/softmax-reference.csv")
        .lines()
        .skip(1)
    {
        let fields = row
            .split(',')
            .map(|v| v.parse::<f64>().unwrap())
            .collect::<Vec<_>>();
        let value = numeric::evaluate(Builtin::Softmax, &[vector(&fields[..3])], 100).unwrap();
        let probabilities = value.as_vector().unwrap().as_slice();
        for (actual, &expected) in probabilities.iter().zip(&fields[3..]) {
            assert!(
                (actual.get() - expected).abs() <= 16.0 * f64::EPSILON * expected.max(1e-300),
                "{row}: {actual}"
            );
            assert!((0.0..=1.0).contains(&actual.get()));
        }
        assert!((scalar(Builtin::Sum, &[value]) - 1.0).abs() <= 4.0 * f64::EPSILON);
    }
}

#[test]
fn repeated_postfix_nodes_and_utf8_truncations_do_not_recurse_or_panic() {
    let source = format!("print([1.0]{});", "[0]".repeat(20_000));
    assert!(scarp::parse(&source).is_ok());
    failure(&source, ExecutionOptions::default(), Kind::TypeMismatch);
    let source =
        "// नमस्ते 🌍\nfuncra f(v: vector=[1.0,2.0]) -> vector { return softmax(v); } print(f()[0]);";
    for offset in source
        .char_indices()
        .map(|(i, _)| i)
        .chain(std::iter::once(source.len()))
    {
        if let Err(error) = scarp::check(&source[..offset]) {
            assert!(error.span.start <= error.span.end && error.span.end <= offset);
            assert!(
                source.is_char_boundary(error.span.start)
                    && source.is_char_boundary(error.span.end)
            );
        }
    }
}
