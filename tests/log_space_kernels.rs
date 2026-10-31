use scarp::{
    diagnostics::DiagnosticKind as Kind,
    numeric::{self, Builtin},
    value::{NumericVector, Value},
    ExecutionOptions, RunError,
};

fn vector(values: &[f64]) -> Value {
    Value::Vector(NumericVector::new(values).unwrap())
}

fn run(source: &str) -> String {
    let mut output = Vec::new();
    scarp::run(source, &mut output).unwrap();
    scarp::check(source).unwrap();
    String::from_utf8(output).unwrap()
}

fn failed(source: &str, options: ExecutionOptions, kind: Kind) {
    let check = scarp::check_with_options(source, options).unwrap_err();
    assert_eq!(check.kind, kind, "{check:?}");
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let RunError::Diagnostic(runtime) =
        scarp::run_with_io_and_options(source, &mut out, &mut err, options).unwrap_err()
    else {
        panic!("expected a language diagnostic");
    };
    assert_eq!(runtime, check);
    assert!(out.is_empty() && err.is_empty(), "failed run leaked output");
    assert!(runtime.span.end <= source.len());
}

#[test]
fn log_space_kernels_are_typed_pure_and_have_no_implicit_coercions() {
    assert_eq!(
        run("print(logsumexp([0.0])); print(logsoftmax([0.0]));"),
        "0.0\n[0.0]\n"
    );
    assert_eq!(
        run("pure funcra score(v: vector) -> float ensures result >= 0.0; { return logsumexp(v); } print(score([0.0, 0.0]));")
            .parse::<f64>()
            .unwrap(),
        2.0_f64.ln()
    );
    assert_eq!(
        run("pure funcra logsumexp(v: vector) -> integer { return len(v); } print(logsumexp([1.0, 2.0]));"),
        "2\n"
    );
    assert_eq!(
        run("bind logsumexp = 4; bind logsoftmax = 5; print(logsumexp, logsoftmax);"),
        "4 5\n"
    );
    for invalid in ["logsumexp(3.0)", "logsoftmax([1])"] {
        failed(
            &format!("print({invalid});"),
            ExecutionOptions::default(),
            Kind::TypeMismatch,
        );
    }
    for invalid in [
        "logsumexp()",
        "logsoftmax([1.0], [2.0])",
        "logsumexp(v: [1.0])",
    ] {
        failed(
            &format!("print({invalid});"),
            ExecutionOptions::default(),
            Kind::CallArguments,
        );
    }
}

#[test]
fn large_positive_and_negative_logits_remain_stable() {
    let input = vector(&[1000.0, 1001.0, 999.0]);
    let expected = 1001.0_f64 + (1.0 + (-1.0_f64).exp() + (-2.0_f64).exp()).ln();
    let lse = numeric::evaluate(Builtin::LogSumExp, std::slice::from_ref(&input), 12)
        .unwrap()
        .as_float()
        .unwrap();
    assert!((lse - expected).abs() < 1e-12);
    let result = numeric::evaluate(Builtin::LogSoftmax, &[input], 12).unwrap();
    let values = result.as_vector().unwrap().as_slice();
    let probability_sum: f64 = values.iter().map(|x| x.get().exp()).sum();
    assert!((probability_sum - 1.0).abs() < 1e-13);
    assert!((values[0].get() - values[1].get() + 1.0).abs() < 1e-13);
    assert!((values[2].get() - values[1].get() + 2.0).abs() < 1e-13);

    let sparse = vector(&[-1000.0, 0.0]);
    let score = numeric::evaluate(Builtin::LogSumExp, std::slice::from_ref(&sparse), 8)
        .unwrap()
        .as_float()
        .unwrap();
    assert_eq!(score, 0.0);
    let logs = numeric::evaluate(Builtin::LogSoftmax, &[sparse], 8).unwrap();
    assert_eq!(logs.as_vector().unwrap().as_slice()[0].get(), -1000.0);
}

#[test]
fn empty_inputs_and_unrepresentable_log_probabilities_fail_atomically() {
    for call in ["logsumexp([])", "logsoftmax([])"] {
        failed(
            &format!("print(\"queued\"); eprint(\"also queued\"); print({call});"),
            ExecutionOptions::default(),
            Kind::NumericDomain,
        );
    }
    let extreme = "print(\"queued\"); print(logsoftmax([-1.7976931348623157e308, 1.7976931348623157e308]));";
    failed(extreme, ExecutionOptions::default(), Kind::NonFiniteNumber);
    let huge = vector(&[f64::MAX, f64::MAX]);
    let result = numeric::evaluate(Builtin::LogSoftmax, &[huge], 8).unwrap();
    let values = result.as_vector().unwrap().as_slice();
    assert!((values[0].get() + std::f64::consts::LN_2).abs() < 1e-15);
    assert!((values[1].get() + std::f64::consts::LN_2).abs() < 1e-15);
}

#[test]
fn native_work_budget_is_prepaid_and_scalar_kernel_never_allocates() {
    let input = vector(&[1.0, 2.0]);
    for kernel in [Builtin::LogSumExp, Builtin::LogSoftmax] {
        numeric::evaluate(kernel, std::slice::from_ref(&input), 8).unwrap();
        assert_eq!(
            numeric::evaluate(kernel, std::slice::from_ref(&input), 7)
                .unwrap_err()
                .kind,
            Kind::ExecutionLimit
        );
    }
    assert_eq!(
        numeric::evaluate(Builtin::LogSumExp, &[vector(&[])], 0)
            .unwrap_err()
            .kind,
        Kind::NumericDomain
    );
    numeric::evaluate_with_vector_limit(
        Builtin::LogSumExp,
        std::slice::from_ref(&input),
        8,
        0,
    )
    .unwrap();
    let err =
        numeric::evaluate_with_vector_limit(Builtin::LogSoftmax, &[input], 8, 15).unwrap_err();
    assert_eq!(err.kind, Kind::VectorBudget);
}

#[test]
fn program_allocation_limit_covers_logsoftmax_but_not_logsumexp() {
    let source = "bind x = [1.0, 2.0]; print(logsumexp(x));";
    let options = ExecutionOptions {
        max_vector_bytes: 16,
        ..ExecutionOptions::default()
    };
    scarp::check_with_options(source, options).unwrap();
    let source = "bind x = [1.0, 2.0]; print(logsoftmax(x));";
    scarp::check_with_options(
        source,
        ExecutionOptions {
            max_vector_bytes: 32,
            ..options
        },
    )
    .unwrap();
    failed(
        source,
        ExecutionOptions {
            max_vector_bytes: 31,
            ..options
        },
        Kind::VectorBudget,
    );
}
