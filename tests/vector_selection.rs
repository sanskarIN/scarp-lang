use scarp::{
    diagnostics::DiagnosticKind as Kind,
    numeric::{self, Builtin},
    value::{NumericVector, Value},
    ExecutionOptions, RunError,
};

fn values(items: &[f64]) -> Value {
    Value::Vector(NumericVector::new(items).unwrap())
}

fn output(source: &str) -> String {
    scarp::check(source).unwrap();
    let mut out = Vec::new();
    scarp::run(source, &mut out).unwrap();
    String::from_utf8(out).unwrap()
}

fn failure(source: &str, kind: Kind) {
    let options = ExecutionOptions::default();
    let expected = scarp::check_with_options(source, options).unwrap_err();
    assert_eq!(expected.kind, kind);
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let RunError::Diagnostic(actual) =
        scarp::run_with_io_and_options(source, &mut out, &mut err, options).unwrap_err()
    else {
        panic!("expected diagnostic");
    };
    assert_eq!(actual, expected);
    assert!(
        out.is_empty() && err.is_empty(),
        "errors cannot leak output"
    );
}

#[test]
fn four_vector_selectors_have_explicit_types_and_first_match_ties() {
    assert_eq!(
        output("print(argmax([1.0, 3.0, 3.0])); print(argmin([2.0, 1.0, 1.0])); print(maxval([1.0, 3.0])); print(minval([2.0, 1.0]));"),
        "1\n1\n3.0\n1.0\n"
    );
    assert_eq!(
        output("pure funcra choose(v: vector) -> integer needs len(v) > 0; ensures result >= 0; { return argmax(v); } print(choose([4.0, 5.0]));"),
        "1\n"
    );
    assert_eq!(
        output("pure funcra maxval(v: vector) -> integer { return len(v); } print(maxval([1.0, 2.0]));"),
        "2\n"
    );
    assert_eq!(
        output("bind argmax = 3; bind argmin = 4; print(argmax, argmin);"),
        "3 4\n"
    );
}

#[test]
fn selection_returns_first_signed_zero_and_handles_subnormals() {
    let signed = values(&[-0.0, 0.0]);
    for kernel in [Builtin::MaxValue, Builtin::MinValue] {
        let result = numeric::evaluate(kernel, std::slice::from_ref(&signed), 2)
            .unwrap()
            .as_float()
            .unwrap();
        assert_eq!(result.to_bits(), (-0.0_f64).to_bits());
    }
    let swapped = values(&[0.0, -0.0]);
    let result = numeric::evaluate(Builtin::MinValue, &[swapped], 2)
        .unwrap()
        .as_float()
        .unwrap();
    assert_eq!(result.to_bits(), 0.0_f64.to_bits());

    let tiny = f64::from_bits(1);
    let vector = values(&[-tiny, tiny, 0.0]);
    assert_eq!(
        numeric::evaluate(Builtin::ArgMin, std::slice::from_ref(&vector), 3)
            .unwrap()
            .as_integer(),
        Some(0)
    );
    assert_eq!(
        numeric::evaluate(Builtin::ArgMax, &[vector], 3)
            .unwrap()
            .as_integer(),
        Some(1)
    );
}

#[test]
fn empty_and_type_errors_are_atomic_and_have_stable_codes() {
    for function in ["argmax", "argmin", "maxval", "minval"] {
        failure(
            &format!("print(\"pending\"); eprint(\"pending\"); print({function}([]));"),
            Kind::NumericDomain,
        );
        failure(&format!("print({function}(1.0));"), Kind::TypeMismatch);
        failure(&format!("print({function}([1]));"), Kind::TypeMismatch);
        failure(&format!("print({function}());"), Kind::CallArguments);
        failure(
            &format!("print({function}([1.0], [2.0]));"),
            Kind::CallArguments,
        );
        failure(
            &format!("print({function}(v: [1.0]));"),
            Kind::CallArguments,
        );
    }
}

#[test]
fn selectors_precharge_work_but_do_not_allocate_new_vector_payload() {
    let vector = values(&[1.0, 2.0, 3.0]);
    for function in [
        Builtin::ArgMax,
        Builtin::ArgMin,
        Builtin::MaxValue,
        Builtin::MinValue,
    ] {
        numeric::evaluate_with_vector_limit(function, std::slice::from_ref(&vector), 3, 0).unwrap();
        assert_eq!(
            numeric::evaluate_with_vector_limit(function, std::slice::from_ref(&vector), 2, 0,)
                .unwrap_err()
                .kind,
            Kind::ExecutionLimit
        );
        assert_eq!(
            numeric::evaluate_with_vector_limit(function, &[values(&[])], 0, 0)
                .unwrap_err()
                .kind,
            Kind::NumericDomain
        );
    }
}

#[test]
fn selection_is_deterministic_on_long_equal_vectors() {
    let vector = values(&vec![5.0; scarp::value::MAX_VECTOR_LENGTH]);
    assert_eq!(
        numeric::evaluate(Builtin::ArgMax, std::slice::from_ref(&vector), 65_536)
            .unwrap()
            .as_integer(),
        Some(0)
    );
    assert_eq!(
        numeric::evaluate(Builtin::ArgMin, &[vector], 65_536)
            .unwrap()
            .as_integer(),
        Some(0)
    );
}
