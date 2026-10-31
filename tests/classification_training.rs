use scarp::{
    diagnostics::{Diagnostic, DiagnosticKind as Kind},
    numeric::{self, Builtin},
    value::{NumericVector, Value, MAX_VECTOR_LENGTH},
    ExecutionOptions, RunError,
};

fn vector(values: &[f64]) -> Value {
    Value::Vector(NumericVector::new(values).unwrap())
}

fn index(value: i64) -> Value {
    Value::Integer(value)
}

fn verify(source: &str, options: ExecutionOptions) -> String {
    scarp::check_with_options(source, options).unwrap();
    let mut stdout = Vec::new();
    scarp::run_with_options(source, &mut stdout, options).unwrap();
    String::from_utf8(stdout).unwrap()
}

fn failure(source: &str, options: ExecutionOptions, kind: Kind) -> Diagnostic {
    let diagnostic = scarp::check_with_options(source, options).unwrap_err();
    assert_eq!(diagnostic.kind, kind, "{source}: {diagnostic:?}");
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    let RunError::Diagnostic(actual) =
        scarp::run_with_io_and_options(source, &mut stdout, &mut stderr, options).unwrap_err()
    else {
        panic!("expected diagnostic");
    };
    assert_eq!(diagnostic, actual);
    assert!(stdout.is_empty() && stderr.is_empty());
    diagnostic
}

#[test]
fn onehot_materializes_a_single_bounded_target_vector() {
    assert_eq!(
        verify(
            "print(onehot(3, 1)); print(onehot(1, 0));",
            ExecutionOptions::default()
        ),
        "[0.0, 1.0, 0.0]\n[1.0]\n"
    );
    assert_eq!(
        verify(
            "pure funcra target(size: integer, selected: integer) -> vector ensures len(result) == size; { return onehot(size, selected); } print(target(2, 0));",
            ExecutionOptions::default()
        ),
        "[1.0, 0.0]\n"
    );
    assert_eq!(
        verify(
            "pure funcra onehot(size: integer, idx: integer) -> integer { return idx; } print(onehot(3, 2));",
            ExecutionOptions::default()
        ),
        "2\n"
    );
}

#[test]
fn lossgrad_is_stable_for_equal_logits_and_conserves_total_gradient() {
    let gradients = numeric::evaluate(Builtin::LossGrad, &[vector(&[0.0, 0.0]), index(0)], 12)
        .unwrap();
    let result = gradients.as_vector().unwrap().as_slice();
    assert_eq!(result[0].get(), -0.5);
    assert_eq!(result[1].get(), 0.5);
    assert_eq!(
        verify(
            "print(lossgrad([0.0, 0.0], 1)); print(lossgrad([1000.0], 0));",
            ExecutionOptions::default()
        ),
        "[0.5, -0.5]\n[0.0]\n"
    );
    let inputs = [1.5, 1.25, -4.0, 0.0];
    let v = vector(&inputs);
    let before = v.clone();
    let result = numeric::evaluate(Builtin::LossGrad, &[v.clone(), index(1)], 24).unwrap();
    let values = result.as_vector().unwrap().as_slice();
    let sum: f64 = values.iter().map(|x| x.get()).sum();
    assert!(sum.abs() < 1e-13);
    assert_eq!(v, before);
}

#[test]
fn lossgrad_handles_extreme_finite_logit_differences() {
    let extreme = vector(&[-f64::MAX, f64::MAX]);
    let result = numeric::evaluate(Builtin::LossGrad, &[extreme.clone(), index(0)], 12)
        .unwrap();
    let values = result.as_vector().unwrap().as_slice();
    assert_eq!(values[0].get(), -1.0);
    assert_eq!(values[1].get(), 1.0);
    let result = numeric::evaluate(Builtin::LossGrad, &[extreme, index(1)], 12).unwrap();
    let values = result.as_vector().unwrap().as_slice();
    assert_eq!(values[0].get(), 0.0);
    assert_eq!(values[1].get(), 0.0);

    let huge_equal = vector(&[f64::MAX, f64::MAX]);
    let values = numeric::evaluate(Builtin::LossGrad, &[huge_equal, index(0)], 12).unwrap();
    assert_eq!(values.as_vector().unwrap().as_slice()[0].get(), -0.5);
}

#[test]
fn invalid_onehot_counts_and_class_indices_are_atomic() {
    for size in ["-1", "65537", "9223372036854775807"] {
        failure(
            &format!("print(\"queued\"); eprint(\"queued\"); print(onehot({size}, 0));"),
            ExecutionOptions::default(),
            Kind::VectorLength,
        );
    }
    failure("print(onehot(0, 0));", ExecutionOptions::default(), Kind::NumericDomain);
    for statement in [
        "onehot(3, -1)",
        "onehot(3, 3)",
        "lossgrad([0.0, 1.0], -1)",
        "lossgrad([0.0, 1.0], 2)",
    ] {
        let source = format!("print(\"queued\"); eprint(\"queued\"); print({statement});");
        let err = failure(&source, ExecutionOptions::default(), Kind::VectorIndex);
        assert_eq!(err.kind.code(), "E0033");
    }
    failure(
        "print(lossgrad([], 0));",
        ExecutionOptions::default(),
        Kind::NumericDomain,
    );
}

#[test]
fn invalid_types_arity_and_function_shadowing_match_other_numerics() {
    for expression in [
        "onehot(1.0, 0)",
        "onehot(2, 0.0)",
        "lossgrad(1.0, 0)",
        "lossgrad([1], 0)",
        "lossgrad([1.0], 0.0)",
    ] {
        failure(
            &format!("print({expression});"),
            ExecutionOptions::default(),
            Kind::TypeMismatch,
        );
    }
    for expression in [
        "onehot(1)",
        "onehot(1, 0, 0)",
        "lossgrad()",
        "lossgrad([1.0])",
        "lossgrad(v: [1.0], target: 0)",
    ] {
        failure(
            &format!("print({expression});"),
            ExecutionOptions::default(),
            Kind::CallArguments,
        );
    }
    assert_eq!(
        verify(
            "pure funcra lossgrad(v: vector, target: integer) -> integer { return target; } print(lossgrad([1.0, 2.0], 1));",
            ExecutionOptions::default()
        ),
        "1\n"
    );
    assert_eq!(
        verify(
            "bind lossgrad = 4; bind onehot = 5; print(lossgrad, onehot);",
            ExecutionOptions::default()
        ),
        "4 5\n"
    );
}

#[test]
fn both_kernels_charge_steps_and_bytes_before_new_buffers() {
    let inputs = [vector(&[1.0, 2.0, 3.0]), index(1)];
    numeric::evaluate_with_vector_limit(Builtin::LossGrad, &inputs, 18, 24).unwrap();
    assert_eq!(
        numeric::evaluate_with_vector_limit(Builtin::LossGrad, &inputs, 17, 24)
            .unwrap_err()
            .kind,
        Kind::ExecutionLimit
    );
    assert_eq!(
        numeric::evaluate_with_vector_limit(Builtin::LossGrad, &inputs, 18, 23)
            .unwrap_err()
            .kind,
        Kind::VectorBudget
    );
    let args = [index(3), index(2)];
    numeric::evaluate_with_vector_limit(Builtin::OneHot, &args, 3, 24).unwrap();
    assert_eq!(
        numeric::evaluate_with_vector_limit(Builtin::OneHot, &args, 2, 24)
            .unwrap_err()
            .kind,
        Kind::ExecutionLimit
    );
    assert_eq!(
        numeric::evaluate_with_vector_limit(Builtin::OneHot, &args, 3, 23)
            .unwrap_err()
            .kind,
        Kind::VectorBudget
    );

    let source = "bind v = [1.0, 2.0]; print(lossgrad(v, 0));";
    let options = ExecutionOptions {
        max_vector_bytes: 32,
        ..ExecutionOptions::default()
    };
    assert_eq!(verify(source, options).lines().count(), 1);
    failure(source, ExecutionOptions { max_vector_bytes: 31, ..options }, Kind::VectorBudget);
}

#[test]
fn max_size_onehot_writes_only_one_selected_element() {
    let args = [index(MAX_VECTOR_LENGTH as i64), index(MAX_VECTOR_LENGTH as i64 - 1)];
    let required_bytes = MAX_VECTOR_LENGTH * std::mem::size_of::<scarp::value::FiniteFloat>();
    let value = numeric::evaluate_with_vector_limit(
        Builtin::OneHot,
        &args,
        MAX_VECTOR_LENGTH as u64,
        required_bytes,
    )
    .unwrap();
    let values = value.as_vector().unwrap().as_slice();
    assert_eq!(values.len(), MAX_VECTOR_LENGTH);
    assert_eq!(values[0].get(), 0.0);
    assert_eq!(values[MAX_VECTOR_LENGTH - 1].get(), 1.0);
}

#[test]
fn sample_is_checked_and_matches_its_exact_output_fixture() {
    assert_eq!(
        verify(include_str!("../examples/classification_training.srp"), ExecutionOptions::default()),
        include_str!("../examples/classification_training.stdout")
    );
}
