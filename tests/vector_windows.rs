use scarp::{
    diagnostics::{Diagnostic, DiagnosticKind as Kind},
    numeric::{self, Builtin},
    value::{NumericVector, Value, ValueType, MAX_VECTOR_LENGTH},
    ExecutionLimits, ExecutionOptions, RunError,
};

fn output(source: &str) -> String {
    let mut bytes = Vec::new();
    scarp::run(source, &mut bytes).unwrap();
    scarp::check(source).unwrap();
    String::from_utf8(bytes).unwrap()
}

fn error(source: &str, kind: Kind) -> Diagnostic {
    let expected = scarp::check(source).unwrap_err();
    assert_eq!(expected.kind, kind, "{source}: {expected:?}");
    let mut bytes = Vec::new();
    let RunError::Diagnostic(actual) = scarp::run(source, &mut bytes).unwrap_err() else {
        panic!("expected a language diagnostic");
    };
    assert!(bytes.is_empty(), "failed execution must not leak output");
    assert_eq!(actual, expected);
    actual
}

fn vector(values: &[f64]) -> Value {
    Value::Vector(NumericVector::new(values).unwrap())
}

#[test]
fn windows_and_stitches_have_first_class_vector_semantics() {
    assert_eq!(
        output(
            "bind x = [1.0, 2.0, 3.0, 4.0]; bind mid = window(x, 1, 2); print(mid, mid[0], len(mid)); print(window(mid, 1, 1)); print(stitch(mid, [9.0])); print(x); print(mid == [2.0, 3.0]);"
        ),
        "[2.0, 3.0] 2.0 2\n[3.0]\n[2.0, 3.0, 9.0]\n[1.0, 2.0, 3.0, 4.0]\ntrue\n"
    );
    assert_eq!(
        output(
            "print(window([], 0, 0)); print(window([1.0], 1, 0)); print(stitch([], [])); print(stitch([1.0], []));"
        ),
        "[]\n[]\n[]\n[1.0]\n"
    );
    assert_eq!(
        scarp::compile("bind x = window([1.0, 2.0], 0, 1); bind y = stitch(x, []);")
            .unwrap()
            .binding_types(),
        &[ValueType::Vector, ValueType::Vector]
    );
}

#[test]
fn visible_slices_determine_equality_and_numeric_operations() {
    let source = "bind a = window([100.0, 1.0, 2.0, 3.0], 1, 3); bind b = window([9.0, 1.0, 2.0, 3.0, 9.0], 1, 3); print(a == b, a != [1.0, 2.0]); print(sum(a), mean(a), dot(a, b)); print(add(a, b)); print(stitch(a, b));";
    assert_eq!(
        output(source),
        "true true\n6.0 2.0 14.0\n[2.0, 4.0, 6.0]\n[1.0, 2.0, 3.0, 1.0, 2.0, 3.0]\n"
    );
    assert_eq!(
        output("bind x = window([-0.0, 2.0, -0.0], 2, 1); print(x); print(x == [0.0]);"),
        "[-0.0]\ntrue\n"
    );
}

#[test]
fn original_and_nested_windows_share_storage_without_exposing_mutation() {
    let original = NumericVector::new(&[10.0, 20.0, 30.0, 40.0, 50.0]).unwrap();
    let middle = original.window(1, 3).unwrap();
    let nested = middle.window(1, 1).unwrap();
    assert!(std::ptr::eq(
        middle.as_slice().as_ptr(),
        original.as_slice()[1..].as_ptr()
    ));
    assert!(std::ptr::eq(
        nested.as_slice().as_ptr(),
        original.as_slice()[2..].as_ptr()
    ));
    assert_eq!(nested, NumericVector::new(&[30.0]).unwrap());
    assert_eq!(middle.len(), 3);
    assert_eq!(nested.as_slice()[0].get(), 30.0);
    assert!(middle.window(4, 0).is_none());
    assert!(middle.window(3, 0).is_some());
    assert!(middle.window(2, 2).is_none());
    assert!(middle.window(usize::MAX, 2).is_none());

    let combined = numeric::evaluate(
        Builtin::Stitch,
        &[Value::Vector(middle.clone()), Value::Vector(nested)],
        4,
    )
    .unwrap();
    let combined = combined.as_vector().unwrap();
    assert_eq!(
        combined,
        &NumericVector::new(&[20.0, 30.0, 40.0, 30.0]).unwrap()
    );
    assert!(!std::ptr::eq(
        combined.as_slice().as_ptr(),
        middle.as_slice().as_ptr()
    ));
    assert_eq!(original.as_slice()[0].get(), 10.0);
}

#[test]
fn windows_keep_snapshots_through_reassignment_and_functions() {
    assert_eq!(
        output(
            "pure funcra tail(values: vector, from: integer) -> vector ensures len(result) <= len(values); { return window(values, from, len(values) - from); } var v = [1.0, 2.0, 3.0]; bind selected = tail(v, 1); v = stitch(v, [4.0]); print(selected, v);"
        ),
        "[2.0, 3.0] [1.0, 2.0, 3.0, 4.0]\n"
    );
    assert_eq!(
        output(
            "funcra make() -> vector { bind data = [4.0, 5.0, 6.0]; return window(data, 1, 1); } print(make());"
        ),
        "[5.0]\n"
    );
}

#[test]
fn wrong_types_arity_and_ranges_have_precise_diagnostics() {
    for source in [
        "print(window([1.0], -1, 0));",
        "print(window([1.0], 2, 0));",
        "print(window([1.0], 0, 2));",
        "print(window([], 1, 0));",
        "print(window([1.0], 9223372036854775807, 0));",
    ] {
        error(source, Kind::VectorIndex);
    }
    for source in [
        "print(window([1.0], 0, -1));",
        "print(window([1.0], 0, 65537));",
        "print(stitch(fill(1.0, 65536), [2.0]));",
    ] {
        error(source, Kind::VectorLength);
    }
    for source in [
        "print(window([1.0], 0.0, 1));",
        "print(window(1.0, 0, 0));",
        "print(stitch([1.0], 2.0));",
    ] {
        error(source, Kind::TypeMismatch);
    }
    for source in [
        "print(window([1.0], 0));",
        "print(stitch([1.0]));",
        "print(window(values: [1.0], 0, 1));",
    ] {
        error(source, Kind::CallArguments);
    }
    error(
        "print(\"before\"); print(window([1.0], 1, 1));",
        Kind::VectorIndex,
    );
}

#[test]
fn user_functions_take_precedence_and_bindings_remain_separate() {
    assert_eq!(
        output("pure funcra window(x: integer) -> integer { return x + 1; } print(window(4));"),
        "5\n"
    );
    assert_eq!(
        output(
            "bind window = 7; bind stitch = 8; print(window, stitch); print(len(stitch([1.0], [2.0])));"
        ),
        "7 8\n2\n"
    );
}

#[test]
fn each_native_kernel_prepays_actual_work() {
    let values = vector(&[1.0, 2.0, 3.0]);
    let window_args = [values.clone(), Value::Integer(1), Value::Integer(2)];
    numeric::evaluate(Builtin::Window, &window_args, 1).unwrap();
    assert_eq!(
        numeric::evaluate(Builtin::Window, &window_args, 0)
            .unwrap_err()
            .kind,
        Kind::ExecutionLimit
    );
    let stitch_args = [values.clone(), vector(&[4.0, 5.0])];
    numeric::evaluate(Builtin::Stitch, &stitch_args, 5).unwrap();
    assert_eq!(
        numeric::evaluate(Builtin::Stitch, &stitch_args, 4)
            .unwrap_err()
            .kind,
        Kind::ExecutionLimit
    );
    numeric::evaluate(Builtin::Stitch, &[vector(&[]), vector(&[])], 0).unwrap();
    assert_eq!(
        numeric::evaluate(
            Builtin::Window,
            &[values, Value::Integer(3), Value::Integer(0)],
            1
        )
        .unwrap()
        .as_vector()
        .unwrap()
        .len(),
        0
    );
}

#[test]
fn large_bounded_windows_and_stitches_preserve_the_vector_cap() {
    let full = NumericVector::new(&vec![2.0; MAX_VECTOR_LENGTH]).unwrap();
    let end = full.window(MAX_VECTOR_LENGTH, 0).unwrap();
    assert!(end.is_empty());
    assert_eq!(full.window(0, MAX_VECTOR_LENGTH).unwrap(), full);
    assert!(full.window(MAX_VECTOR_LENGTH, 1).is_none());

    let full_value = Value::Vector(full);
    let selected = numeric::evaluate(
        Builtin::Window,
        &[
            full_value.clone(),
            Value::Integer(0),
            Value::Integer(65_536),
        ],
        1,
    )
    .unwrap();
    assert_eq!(selected.as_vector().unwrap().len(), MAX_VECTOR_LENGTH);
    assert_eq!(
        numeric::evaluate(Builtin::Stitch, &[selected, vector(&[3.0])], 1_000_000)
            .unwrap_err()
            .kind,
        Kind::VectorLength
    );
    let picked = numeric::evaluate(
        Builtin::Window,
        &[full_value, Value::Integer(65_535), Value::Integer(1)],
        1,
    )
    .unwrap();
    assert_eq!(picked.as_vector().unwrap().as_slice()[0].get(), 2.0);
}

#[test]
fn windows_and_stitches_preserve_execution_atomicity_under_quota_failure() {
    let source = "print(\"start\"); repeat 10 as i { bind x = stitch([1.0, 2.0], [3.0, 4.0]); print(window(x, 1, 2)); }";
    let limit = ExecutionLimits {
        max_steps: 30,
        max_prints: 50,
    };
    let expected = scarp::check_with_limits(source, limit).unwrap_err();
    assert_eq!(expected.kind, Kind::ExecutionLimit);
    let mut bytes = Vec::new();
    let RunError::Diagnostic(actual) =
        scarp::run_with_limits(source, &mut bytes, limit).unwrap_err()
    else {
        panic!("expected bounded failure");
    };
    assert_eq!(actual, expected);
    assert!(bytes.is_empty());
}

fn bounded_error(source: &str, options: ExecutionOptions, kind: Kind) -> Diagnostic {
    let expected = scarp::check_with_options(source, options).unwrap_err();
    assert_eq!(expected.kind, kind, "{source}");
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    let RunError::Diagnostic(actual) =
        scarp::run_with_io_and_options(source, &mut stdout, &mut stderr, options).unwrap_err()
    else {
        panic!("expected a language diagnostic");
    };
    assert_eq!(actual, expected);
    assert!(stdout.is_empty() && stderr.is_empty(), "failed program leaked output");
    actual
}

#[test]
fn cumulative_payload_counts_only_new_stitch_elements_and_literals() {
    let source = "bind base = [1.0, 2.0, 3.0]; bind part = window(base, 1, 2); bind chosen = window(part, 0, 1); print(stitch(chosen, part));";
    let options = ExecutionOptions {
        max_vector_bytes: 48,
        ..ExecutionOptions::default()
    };
    scarp::check_with_options(source, options).unwrap();
    let mut stdout = Vec::new();
    scarp::run_with_options(source, &mut stdout, options).unwrap();
    assert_eq!(stdout, b"[2.0, 2.0, 3.0]\n");
    bounded_error(
        source,
        ExecutionOptions { max_vector_bytes: 47, ..options },
        Kind::VectorBudget,
    );
    // The two windows and the scalar indexing allocate no element payload.
    let source = "bind base = [1.0, 2.0, 3.0]; bind part = window(base, 1, 2); print(window(part, 2, 0), part[1]);";
    scarp::check_with_options(
        source,
        ExecutionOptions { max_vector_bytes: 24, ..options },
    )
    .unwrap();
    bounded_error(
        source,
        ExecutionOptions { max_vector_bytes: 23, ..options },
        Kind::VectorBudget,
    );
}

#[test]
fn standalone_host_windows_are_allocation_free_but_stitches_are_charged() {
    let original = vector(&[1.0, 2.0, 3.0]);
    let window = numeric::evaluate_with_vector_limit(
        Builtin::Window,
        &[original.clone(), Value::Integer(1), Value::Integer(2)],
        1,
        0,
    )
    .unwrap();
    assert_eq!(window.as_vector().unwrap().as_slice()[0].get(), 2.0);
    assert_eq!(
        numeric::evaluate_with_vector_limit(
            Builtin::Window,
            &[original, Value::Integer(1), Value::Integer(1)],
            0,
            0,
        )
        .unwrap_err()
        .kind,
        Kind::ExecutionLimit
    );
    let args = [window, vector(&[4.0])];
    numeric::evaluate_with_vector_limit(Builtin::Stitch, &args, 3, 24).unwrap();
    assert_eq!(
        numeric::evaluate_with_vector_limit(Builtin::Stitch, &args, 3, 23)
            .unwrap_err()
            .kind,
        Kind::VectorBudget
    );
    assert_eq!(
        numeric::evaluate_with_vector_limit(Builtin::Stitch, &args, 2, 24)
            .unwrap_err()
            .kind,
        Kind::ExecutionLimit
    );
}

#[test]
fn windows_feed_lossgrad_and_logspace_kernels_without_full_buffer_exposure() {
    assert_eq!(
        output(
            "bind logits = [1000.0, 0.0, 0.0, -1000.0]; bind sample = window(logits, 1, 2); print(argmax(sample)); print(crossentropy(sample, 0)); print(lossgrad(sample, 1));"
        ),
        "0\n0.6931471805599453\n[0.5, -0.5]\n"
    );
    let original = NumericVector::new(&[1.0, 2.0, 3.0, 4.0]).unwrap();
    let selected = original.window(1, 2).unwrap();
    let softmax = numeric::evaluate(
        Builtin::Softmax,
        &[Value::Vector(selected.clone())],
        8,
    )
    .unwrap();
    assert_eq!(softmax.as_vector().unwrap().len(), 2);
    assert_eq!(original.as_slice().len(), 4);
    assert_eq!(selected.as_slice().len(), 2);
}

#[test]
fn exact_example_file_output_is_checked() {
    let source = include_str!("../examples/vector_windows.srp");
    let expected = include_str!("../examples/vector_windows.stdout");
    assert_eq!(output(source), expected);
}
