use scarp::{
    diagnostics::{Diagnostic, DiagnosticKind as Kind},
    numeric::{self, Builtin},
    value::{NumericVector, Value},
    ExecutionOptions, RunError,
};

fn vector(values: &[f64]) -> Value {
    Value::Vector(NumericVector::new(values).unwrap())
}

fn label(index: i64) -> Value {
    Value::Integer(index)
}

fn output(source: &str) -> String {
    scarp::check(source).unwrap();
    let mut stdout = Vec::new();
    scarp::run(source, &mut stdout).unwrap();
    String::from_utf8(stdout).unwrap()
}

fn failed(source: &str, options: ExecutionOptions, kind: Kind) -> Diagnostic {
    let diagnostic = scarp::check_with_options(source, options).unwrap_err();
    assert_eq!(diagnostic.kind, kind, "{source}: {diagnostic:?}");
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    let RunError::Diagnostic(run) =
        scarp::run_with_io_and_options(source, &mut stdout, &mut stderr, options).unwrap_err()
    else {
        panic!("expected a language error");
    };
    assert_eq!(diagnostic, run);
    assert!(stdout.is_empty() && stderr.is_empty(), "failure leaked output");
    diagnostic
}

#[test]
fn source_calls_are_typed_pure_and_work_inside_function_contracts() {
    assert_eq!(
        output("pure funcra loss(logits: vector, target: integer) -> float ensures result >= 0.0; { return crossentropy(logits, target); } print(loss([0.0], 0));"),
        "0.0\n"
    );
    assert_eq!(
        output("pure funcra logprob(v: vector, index: integer) -> integer { return index; } print(logprob([3.0, 4.0], 1));"),
        "1\n"
    );
    assert_eq!(
        output("bind logprob = 4; bind crossentropy = 5; print(logprob, crossentropy);"),
        "4 5\n"
    );
    for expr in [
        "crossentropy(1.0, 0)",
        "logprob([1], 0)",
        "logprob([1.0], 0.0)",
    ] {
        failed(&format!("print({expr});"), ExecutionOptions::default(), Kind::TypeMismatch);
    }
    for expr in [
        "logprob()",
        "crossentropy([1.0])",
        "crossentropy([1.0], 0, 1)",
        "logprob(logits: [1.0], index: 0)",
    ] {
        failed(&format!("print({expr});"), ExecutionOptions::default(), Kind::CallArguments);
    }
}

#[test]
fn basic_likelihood_and_loss_are_consistent_with_softmax() {
    let v = vector(&[3.0, 4.0, 2.0]);
    let reference = (1.0 + (-1.0_f64).exp() + (-2.0_f64).exp()).ln();
    let ce = numeric::evaluate(Builtin::CrossEntropy, &[v.clone(), label(1)], 12)
        .unwrap()
        .as_float()
        .unwrap();
    let lp = numeric::evaluate(Builtin::LogProb, &[v.clone(), label(1)], 12)
        .unwrap()
        .as_float()
        .unwrap();
    assert!((ce - reference).abs() < 1e-14);
    assert_eq!(lp, -ce);
    let softmax = numeric::evaluate(Builtin::LogSoftmax, &[v], 12).unwrap();
    assert!((softmax.as_vector().unwrap().as_slice()[1].get() - lp).abs() < 1e-14);
}

#[test]
fn huge_equal_logits_keep_small_nonzero_loss_instead_of_cancelling() {
    for magnitude in [1e100, 1e308, f64::MAX] {
        let logits = vector(&[magnitude, magnitude]);
        let ce = numeric::evaluate(Builtin::CrossEntropy, &[logits.clone(), label(0)], 8)
            .unwrap()
            .as_float()
            .unwrap();
        assert!((ce - std::f64::consts::LN_2).abs() < 1e-15);
        let lp = numeric::evaluate(Builtin::LogProb, &[logits, label(1)], 8)
            .unwrap()
            .as_float()
            .unwrap();
        assert_eq!(lp, -ce);
    }

    let singleton = vector(&[-1000.0]);
    assert_eq!(
        numeric::evaluate(Builtin::CrossEntropy, &[singleton, label(0)], 4)
            .unwrap()
            .as_float(),
        Some(0.0)
    );
}

#[test]
fn negative_extreme_gap_reports_nonfinite_instead_of_returning_infinity() {
    let extreme = vector(&[-f64::MAX, f64::MAX]);
    for kernel in [Builtin::CrossEntropy, Builtin::LogProb] {
        let err = numeric::evaluate(kernel, &[extreme.clone(), label(0)], 8).unwrap_err();
        assert_eq!(err.kind, Kind::NonFiniteNumber);
        let safe = numeric::evaluate(kernel, &[extreme.clone(), label(1)], 8).unwrap();
        assert!(safe.as_float().unwrap().abs() < 1e-15);
    }
    let source = "eprint(\"earlier\"); print(\"queued\"); print(crossentropy([-1.7976931348623157e308, 1.7976931348623157e308], 0));";
    failed(source, ExecutionOptions::default(), Kind::NonFiniteNumber);
}

#[test]
fn invalid_target_index_is_diagnosed_before_value_access() {
    for kernel in ["crossentropy", "logprob"] {
        for target in ["-1", "2", "9223372036854775807"] {
            let source = format!(
                "print(\"queued\"); eprint(\"queued\"); print({kernel}([1.0, 2.0], {target}));"
            );
            let err = failed(&source, ExecutionOptions::default(), Kind::VectorIndex);
            assert_eq!(err.kind.code(), "E0033");
            assert!(err.help.unwrap().contains("zero-based"));
        }
        failed(
            &format!("print({kernel}([], 0));"),
            ExecutionOptions::default(),
            Kind::NumericDomain,
        );
    }
}

#[test]
fn both_scalar_kernels_have_zero_new_payload_and_prepaid_work() {
    let logits = vector(&[1.0, 2.0, 3.0]);
    for kernel in [Builtin::CrossEntropy, Builtin::LogProb] {
        let args = [logits.clone(), label(2)];
        numeric::evaluate_with_vector_limit(kernel, &args, 12, 0).unwrap();
        assert_eq!(
            numeric::evaluate_with_vector_limit(kernel, &args, 11, 0)
                .unwrap_err()
                .kind,
            Kind::ExecutionLimit
        );
    }
    let source = "bind scores = [2.0, 3.0]; print(crossentropy(scores, 0), logprob(scores, 1));";
    let options = ExecutionOptions {
        max_vector_bytes: 16,
        ..ExecutionOptions::default()
    };
    scarp::check_with_options(source, options).unwrap();
    failed(
        source,
        ExecutionOptions {
            max_vector_bytes: 15,
            ..options
        },
        Kind::VectorBudget,
    );
}

#[test]
fn score_losses_handle_subnormal_ties_and_preserve_input_data() {
    let tiny = f64::from_bits(1);
    let v = vector(&[tiny, 0.0, -tiny]);
    let before = v.clone();
    let ce = numeric::evaluate(Builtin::CrossEntropy, &[v.clone(), label(1)], 12)
        .unwrap()
        .as_float()
        .unwrap();
    assert!((ce - 3.0_f64.ln()).abs() < 1e-14);
    assert_eq!(v, before);

    // Two scored entries differ by one representable unit at this magnitude.
    let v = vector(&[1.0, f64::from_bits(1.0_f64.to_bits() + 1)]);
    let ce = numeric::evaluate(Builtin::CrossEntropy, &[v, label(1)], 8)
        .unwrap()
        .as_float()
        .unwrap();
    assert!((ce - std::f64::consts::LN_2).abs() < 1e-15);
}

#[test]
fn example_source_has_identical_check_and_run_semantics() {
    let source = include_str!("../examples/logit_loss.srp");
    let expected = include_str!("../examples/logit_loss.stdout");
    assert_eq!(output(source), expected);
}
