use scarp::{
    diagnostics::{Diagnostic, DiagnosticKind as Kind},
    numeric::{self, Builtin},
    value::{NumericVector, Value},
    RunError,
};

fn vector(values: &[f64]) -> Value {
    Value::Vector(NumericVector::new(values).unwrap())
}

fn output(source: &str) -> String {
    let mut bytes = Vec::new();
    scarp::run(source, &mut bytes).unwrap();
    scarp::check(source).unwrap();
    String::from_utf8(bytes).unwrap()
}

fn checked_failure(source: &str, kind: Kind) -> Diagnostic {
    let expected = scarp::check(source).unwrap_err();
    assert_eq!(expected.kind, kind, "{source}");
    let mut bytes = Vec::new();
    let RunError::Diagnostic(actual) = scarp::run(source, &mut bytes).unwrap_err() else {
        panic!("expected a language diagnostic");
    };
    assert!(bytes.is_empty(), "failed programs must not leak output");
    assert_eq!(actual, expected);
    actual
}

#[test]
fn l2_norm_and_normalize_are_typed_pure_kernels() {
    assert_eq!(
        output(
            "print(norm([3.0, 4.0])); print(normalize([3.0, 4.0])); print(norm([]));"
        ),
        "5.0\n[0.6, 0.8]\n0.0\n"
    );
    assert_eq!(
        output(
            "pure funcra unit(v: vector) -> vector ensures norm(result) > 0.0; { return normalize(v); } print(unit([3.0, 4.0]));"
        ),
        "[0.6, 0.8]\n"
    );
    // User functions shadow builtins only in call position.
    assert_eq!(
        output(
            "pure funcra norm(v: vector) -> integer { return len(v); } print(norm([1.0, 2.0]));"
        ),
        "2\n"
    );
    assert_eq!(
        output("bind norm = 7; bind normalize = 8; print(norm, normalize);"),
        "7 8\n"
    );
}

#[test]
fn normalized_vectors_survive_intermediate_and_final_norm_overflow() {
    let values = vector(&[1e308, -1e308]);
    let norm = numeric::evaluate(Builtin::Norm, std::slice::from_ref(&values), 8)
        .unwrap()
        .as_float()
        .unwrap();
    assert!((norm / 1e308 - std::f64::consts::SQRT_2).abs() < 1e-14);

    let huge = vector(&[f64::MAX, f64::MAX]);
    assert_eq!(
        numeric::evaluate(Builtin::Norm, std::slice::from_ref(&huge), 8)
            .unwrap_err()
            .kind,
        Kind::NonFiniteNumber
    );
    let unit = numeric::evaluate(Builtin::Normalize, &[huge], 8).unwrap();
    let elements = unit.as_vector().unwrap().as_slice();
    for element in elements {
        assert!((element.get() - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-14);
    }
}

#[test]
fn subnormals_and_signed_zeros_are_handled_without_squaring_underflow() {
    let tiny = f64::from_bits(1);
    let unit = numeric::evaluate(Builtin::Normalize, &[vector(&[tiny, tiny])], 8).unwrap();
    for element in unit.as_vector().unwrap().as_slice() {
        assert!((element.get() - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-14);
    }
    let unit = numeric::evaluate(Builtin::Normalize, &[vector(&[-0.0, 1.0])], 8).unwrap();
    assert_eq!(unit.as_vector().unwrap().as_slice()[0].get().to_bits(), (-0.0_f64).to_bits());
    assert_eq!(
        numeric::evaluate(Builtin::Norm, &[vector(&[-0.0, 0.0])], 8)
            .unwrap()
            .as_float()
            .unwrap()
            .to_bits(),
        0.0_f64.to_bits()
    );
}

#[test]
fn invalid_normalization_is_atomic_and_provides_domain_diagnostics() {
    for expression in ["normalize([])", "normalize([0.0, -0.0])"] {
        checked_failure(
            &format!("print(\"before\"); eprint(\"also before\"); print({expression});"),
            Kind::NumericDomain,
        );
    }
    for expression in ["norm(1.0)", "normalize([1])"] {
        checked_failure(&format!("print({expression});"), Kind::TypeMismatch);
    }
    for expression in ["norm()", "normalize([1.0], [2.0])", "norm(v: [1.0])"] {
        checked_failure(&format!("print({expression});"), Kind::CallArguments);
    }
}

#[test]
fn bounded_native_l2_work_is_prepaid_before_allocation() {
    let values = vector(&[3.0, 4.0]);
    for kernel in [Builtin::Norm, Builtin::Normalize] {
        numeric::evaluate(kernel, std::slice::from_ref(&values), 8).unwrap();
        assert_eq!(
            numeric::evaluate(kernel, std::slice::from_ref(&values), 7)
                .unwrap_err()
                .kind,
            Kind::ExecutionLimit
        );
    }
    assert_eq!(
        numeric::evaluate(Builtin::Norm, &[vector(&[])], 0)
            .unwrap()
            .as_float(),
        Some(0.0)
    );
    assert_eq!(
        numeric::evaluate(Builtin::Normalize, &[vector(&[])], 0)
            .unwrap_err()
            .kind,
        Kind::NumericDomain
    );
}
