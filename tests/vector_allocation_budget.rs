use scarp::{
    diagnostics::{Diagnostic, DiagnosticKind as Kind},
    numeric::{self, Builtin},
    value::{NumericVector, Value},
    ExecutionOptions, RunError,
};

fn checked(source: &str, max_vector_bytes: usize) -> Vec<u8> {
    let options = ExecutionOptions {
        max_vector_bytes,
        ..ExecutionOptions::default()
    };
    scarp::check_with_options(source, options).unwrap();
    let mut stdout = Vec::new();
    scarp::run_with_options(source, &mut stdout, options).unwrap();
    stdout
}

fn failure(source: &str, options: ExecutionOptions, kind: Kind) -> Diagnostic {
    let check = scarp::check_with_options(source, options).unwrap_err();
    assert_eq!(check.kind, kind, "{source}: {check:?}");
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    let RunError::Diagnostic(run) =
        scarp::run_with_io_and_options(source, &mut stdout, &mut stderr, options).unwrap_err()
    else {
        panic!("expected diagnostic");
    };
    assert!(stdout.is_empty() && stderr.is_empty());
    assert_eq!(run, check);
    run
}

fn limit(bytes: usize) -> ExecutionOptions {
    ExecutionOptions {
        max_vector_bytes: bytes,
        ..ExecutionOptions::default()
    }
}

#[test]
fn vector_literals_charge_exact_element_bytes() {
    let source = "bind x = [1.0, 2.0, 3.0]; print(x);";
    assert_eq!(checked(source, 24), b"[1.0, 2.0, 3.0]\n");
    let error = failure(source, limit(23), Kind::VectorBudget);
    assert_eq!(error.kind.code(), "E0036");
    assert!(error.message.contains("23 bytes"));
    assert!(error.help.unwrap().contains("--max-vector-bytes"));
    assert_eq!(checked("print([]);", 0), b"[]\n");
}

#[test]
fn shares_and_reads_do_not_spend_the_allocation_quota_again() {
    assert_eq!(
        checked(
            "bind original = [1.0, 2.0]; bind copy = original; print(copy, original[1]); print(len(copy));",
            16
        ),
        b"[1.0, 2.0] 2.0\n2\n"
    );
    assert_eq!(
        checked(
            "pure funcra identity(value: vector) -> vector { return value; } bind x = [1.0]; print(identity(x), identity(x));",
            8
        ),
        b"[1.0] [1.0]\n"
    );
}

#[test]
fn new_materialization_is_cumulative_even_after_previous_values_retire() {
    let source = "var i = 0; while i < 3 { bind temp = fill(2.0, 2); i += 1; } print(i);";
    assert_eq!(checked(source, 48), b"3\n");
    failure(source, limit(47), Kind::VectorBudget);
    let function = "pure funcra build() -> vector { return fill(3.0, 2); } print(build(), build());";
    assert_eq!(checked(function, 32), b"[3.0, 3.0] [3.0, 3.0]\n");
    failure(function, limit(31), Kind::VectorBudget);
}

#[test]
fn each_native_buffer_allocates_under_the_same_quota() {
    let source = "bind a = [1.0, 2.0]; bind b = fill(3.0, 2); bind c = add(a, b); bind d = scale(c, 2.0); bind e = clamp(d, 0.0, 10.0); print(softmax(e));";
    // a, b, c, d, e and softmax(e) each allocate 2 x 8 bytes.
    assert!(checked(source, 96).starts_with(b"["));
    failure(source, limit(95), Kind::VectorBudget);
    for function in [Builtin::Fill, Builtin::Softmax, Builtin::Scale] {
        let vector = Value::Vector(NumericVector::new(&[1.0, 2.0]).unwrap());
        let args = match function {
            Builtin::Fill => vec![Value::Float(scarp::value::FiniteFloat::new(1.0).unwrap()), Value::Integer(2)],
            Builtin::Softmax => vec![vector],
            Builtin::Scale => vec![vector, Value::Float(scarp::value::FiniteFloat::new(2.0).unwrap())],
            _ => unreachable!(),
        };
        numeric::evaluate_with_vector_limit(function, &args, 1_000, 16).unwrap();
        let failed = numeric::evaluate_with_vector_limit(function, &args, 1_000, 15).unwrap_err();
        assert_eq!(failed.kind, Kind::VectorBudget);
    }
}

#[test]
fn scalar_kernels_and_empty_vector_kernels_do_not_require_payload_bytes() {
    let vector = Value::Vector(NumericVector::new(&[1.0, 2.0]).unwrap());
    let result =
        numeric::evaluate_with_vector_limit(Builtin::Sum, &[vector.clone()], 100, 0).unwrap();
    assert_eq!(result.as_float(), Some(3.0));
    let result = numeric::evaluate_with_vector_limit(
        Builtin::Add,
        &[Value::Vector(NumericVector::new(&[]).unwrap()), Value::Vector(NumericVector::new(&[]).unwrap())],
        100,
        0,
    )
    .unwrap();
    assert!(result.as_vector().unwrap().is_empty());
    let failure = numeric::evaluate_with_vector_limit(
        Builtin::Add,
        &[vector.clone(), vector],
        100,
        0,
    )
    .unwrap_err();
    assert_eq!(failure.kind, Kind::VectorBudget);
}

#[test]
fn source_errors_and_output_are_atomic_under_all_budget_failure_modes() {
    let source = "eprint(\"stderr-before\"); print(\"stdout-before\"); print([1.0, 2.0]);";
    let error = failure(source, limit(8), Kind::VectorBudget);
    assert!(error.render("memory.srp", source).contains("E0036"));
    let exhausted = failure(
        "print(fill(0.5, 100));",
        ExecutionOptions {
            max_vector_bytes: 0,
            limits: scarp::ExecutionLimits {
                max_steps: 1_000_000,
                max_prints: 1,
            },
            ..ExecutionOptions::default()
        },
        Kind::VectorBudget,
    );
    assert!(exhausted.span.end <= "print(fill(0.5, 100));".len());
}

#[test]
fn vector_limit_is_independent_from_existing_work_and_capture_limits() {
    let source = "print([1.0, 2.0]);";
    let limited_work = ExecutionOptions {
        max_vector_bytes: 16,
        limits: scarp::ExecutionLimits { max_steps: 0, max_prints: 10 },
        ..ExecutionOptions::default()
    };
    failure(source, limited_work, Kind::ExecutionLimit);
    let limited_capture = ExecutionOptions {
        max_vector_bytes: 16,
        max_capture_bytes: 79,
        ..ExecutionOptions::default()
    };
    failure(source, limited_capture, Kind::CaptureLimit);
    assert_eq!(checked(source, 16), b"[1.0, 2.0]\n");
}

#[test]
fn compiled_program_can_be_reused_with_fresh_memory_budgets() {
    let source = "print(fill(0.25, 4));";
    let program = scarp::compile(source).unwrap();
    for _ in 0..4 {
        let mut output = Vec::new();
        scarp::interpreter::execute_with_options(&program, &mut output, limit(32)).unwrap();
        assert_eq!(output, b"[0.25, 0.25, 0.25, 0.25]\n");
    }
    let error = scarp::interpreter::validate_with_options(&program, limit(31)).unwrap_err();
    assert_eq!(error.kind, Kind::VectorBudget);
}
