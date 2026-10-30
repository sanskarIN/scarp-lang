use scarp::{
    diagnostics::{Diagnostic, DiagnosticKind},
    value::{NumericVector, Value, ValueType, MAX_VECTOR_LENGTH},
    ExecutionLimits, RunError,
};

fn output(source: &str) -> String {
    let mut bytes = Vec::new();
    scarp::run(source, &mut bytes).unwrap();
    String::from_utf8(bytes).unwrap()
}
fn error(source: &str) -> Diagnostic {
    let expected = scarp::check(source).unwrap_err();
    let mut bytes = Vec::new();
    let RunError::Diagnostic(actual) = scarp::run(source, &mut bytes).unwrap_err() else {
        panic!("expected diagnostic")
    };
    assert!(bytes.is_empty());
    assert_eq!(actual, expected);
    actual
}

#[test]
fn literals_indexing_and_length_have_explicit_types() {
    assert_eq!(output("bind values = [1.0, -2.5, 3e0,]; print(values); print(values[1]); print(len(values)); print([]); print(len([])); print(([4.0])[0]);"),"[1.0, -2.5, 3.0]\n-2.5\n3\n[]\n0\n4.0\n");
    assert_eq!(
        scarp::compile("bind v = []; bind x = v[0];")
            .unwrap()
            .binding_types(),
        &[ValueType::Vector, ValueType::Float]
    );
}

#[test]
fn vectors_are_immutable_values_with_shared_copy_semantics() {
    assert_eq!(output("var v = [1.0, 2.0]; bind copy = v; print(v); v = scale(v, 2.0); print(copy); print(v); print(copy == [1.0, 2.0]); print(copy != v);"),"[1.0, 2.0]\n[1.0, 2.0]\n[2.0, 4.0]\ntrue\ntrue\n");
    assert_eq!(
        error("var v = []; v = 1.0;").kind,
        DiagnosticKind::TypeMismatch
    );
    assert_eq!(
        error("var v = []; v += [];").kind,
        DiagnosticKind::TypeMismatch
    );
}

#[test]
fn native_kernels_cover_vector_and_scalar_results() {
    assert_eq!(output("print(sum([1.0, 2.0, 3.0])); print(mean([1.0, 2.0, 3.0])); print(dot([1.0, 2.0], [3.0, 4.0])); print(add([1.0, 2.0], [3.0, 4.0])); print(scale([1.0, -2.0], 3.0)); print(clamp([-2.0, 0.5, 3.0], 0.0, 1.0)); print(fill(1.5, 3));"),"6.0\n2.0\n11.0\n[4.0, 6.0]\n[3.0, -6.0]\n[0.0, 0.5, 1.0]\n[1.5, 1.5, 1.5]\n");
    assert_eq!(output("print(sum([])); print(dot([], [])); print(add([], [])); print(fill(1.0, 0)); print(scale([], 1.0));"),"0.0\n0.0\n[]\n[]\n[]\n");
}

#[test]
fn compensated_reductions_preserve_small_cancellation_terms() {
    assert_eq!(output("print(sum([1e16, 1.0, -1e16])); print(dot([1e16, 1.0, -1e16], [1.0, 1.0, 1.0])); print(mean([1e16, 3.0, -1e16]));"),"1.0\n1.0\n1.0\n");
}

#[test]
fn softmax_handles_large_logits_extreme_shifts_and_equal_values() {
    assert_eq!(output("print(softmax([1000.0, 1000.0])); print(softmax([-1e308, 1e308])); print(softmax([2.0])); bind p = softmax([1000.0, 1001.0, 1002.0]); print(sum(p) > 0.999999 and sum(p) < 1.000001); print(p[0] < p[1] and p[1] < p[2]);"),"[0.5, 0.5]\n[0.0, 1.0]\n[1.0]\ntrue\ntrue\n");
}

#[test]
fn mean_squared_error_is_a_checked_native_reduction() {
    assert_eq!(
        output("print(mse([1.0, 2.0], [1.0, 4.0])); print(mse([1.0, 2.0], [1.0, 2.0]));"),
        "2.0\n0.0\n"
    );
    assert_eq!(
        error("print(mse([1e308], [-1e308]));").kind,
        DiagnosticKind::NonFiniteNumber
    );
}

#[test]
fn vector_sizes_shapes_and_indices_report_distinct_failures() {
    for source in [
        "print(fill(0.0, -1));",
        "print(fill(0.0, 65537));",
        "print(dot([1.0], []));",
        "print(add([], [1.0]));",
        "print(mse([], [1.0]));",
    ] {
        assert_eq!(error(source).kind, DiagnosticKind::VectorLength, "{source}");
    }
    for source in [
        "print([][0]);",
        "print([1.0][-1]);",
        "print([1.0][1]);",
        "print([1.0][9223372036854775807]);",
    ] {
        let diagnostic = error(source);
        assert_eq!(diagnostic.kind, DiagnosticKind::VectorIndex);
        assert!(source[diagnostic.span.start..diagnostic.span.end].starts_with('['));
    }
}

#[test]
fn kernel_domain_errors_and_nonfinite_results_remain_atomic() {
    for expression in [
        "mean([])",
        "softmax([])",
        "mse([], [])",
        "clamp([1.0], 2.0, 1.0)",
    ] {
        assert_eq!(
            error(&format!("print(\"before\"); print({expression});")).kind,
            DiagnosticKind::NumericDomain
        );
    }
    for expression in [
        "sum([1e308, 1e308])",
        "dot([1e308], [2.0])",
        "scale([1e308], 2.0)",
        "add([1e308], [1e308])",
    ] {
        assert_eq!(
            error(&format!("print(\"before\"); print({expression});")).kind,
            DiagnosticKind::NonFiniteNumber
        );
    }
}

#[test]
fn native_call_signatures_and_separate_call_namespace_are_checked_statically() {
    for expression in [
        "[1]",
        "[true]",
        "[[1.0]]",
        "[1.0][true]",
        "1.0[0]",
        "sum(1.0)",
        "fill(2.0, 1.0)",
        "scale([1.0], 2)",
        "dot([1.0], 1.0)",
        "[] + []",
    ] {
        assert_eq!(
            error(&format!("print({expression});")).kind,
            DiagnosticKind::TypeMismatch,
            "{expression}"
        );
    }
    for expression in [
        "len()",
        "sum([], [])",
        "dot([])",
        "fill(1)",
        "clamp([], 0.0)",
    ] {
        assert_eq!(
            error(&format!("print({expression});")).kind,
            DiagnosticKind::CallArguments
        );
    }
    assert_eq!(
        error("print(unknown_kernel([]));").kind,
        DiagnosticKind::UnknownFunction
    );
    for name in [
        "len", "sum", "mean", "dot", "scale", "add", "clamp", "fill", "softmax", "mse",
    ] {
        assert_eq!(output(&format!("bind {name} = 1; print({name});")), "1\n");
    }
    assert_eq!(
        output("bind dot = 4; print(dot); print(dot([1.0], [2.0]));"),
        "4\n2.0\n"
    );
}

#[test]
fn unreachable_runtime_failures_are_skipped_but_type_errors_are_not() {
    assert_eq!(output("if false { print(dot([], [1.0])); print([][-1]); print(fill(0.0, -1)); } print(true or mean([]) == 0.0);"),"true\n");
    assert_eq!(
        error("if false { print(sum([1])); }").kind,
        DiagnosticKind::TypeMismatch
    );
    assert_eq!(
        error("if false { print(dot([missing], [])); }").kind,
        DiagnosticKind::UnknownName
    );
}

#[test]
fn native_element_work_and_vector_output_consume_execution_budget() {
    let limits = ExecutionLimits {
        max_steps: 100,
        max_prints: 10,
    };
    for source in [
        "print(sum(fill(1.0, 1000)));",
        "print(softmax(fill(1.0, 40)));",
        "bind v = fill(1.0, 40); while true { print(v); }",
        "bind v = fill(1.0, 40); while v == v {}",
    ] {
        let expected = scarp::check_with_limits(source, limits).unwrap_err();
        assert_eq!(expected.kind, DiagnosticKind::ExecutionLimit);
        let mut bytes = Vec::new();
        let RunError::Diagnostic(actual) =
            scarp::run_with_limits(source, &mut bytes, limits).unwrap_err()
        else {
            panic!("expected diagnostic")
        };
        assert_eq!(actual, expected);
        assert!(bytes.is_empty());
    }
}

#[test]
fn maximum_vector_size_and_host_construction_enforce_finite_storage() {
    assert_eq!(output("print(len(fill(1.0, 65536)));"), "65536\n");
    assert!(NumericVector::new(&[f64::NAN]).is_none());
    assert!(NumericVector::new(&[f64::INFINITY]).is_none());
    assert!(NumericVector::new(&vec![0.0; MAX_VECTOR_LENGTH + 1]).is_none());
    let vector = NumericVector::new(&[1.0, 2.0]).unwrap();
    assert_eq!(vector.as_slice()[1].get(), 2.0);
    assert_eq!(vector.len(), 2);
    assert!(!vector.is_empty());
    assert_eq!(Value::Vector(vector).to_string(), "[1.0, 2.0]");
    let oversized = format!("print([{}]);", "1.0,".repeat(MAX_VECTOR_LENGTH + 1));
    let at_limit = format!("print(len([{}]));", "1.0,".repeat(MAX_VECTOR_LENGTH));
    assert_eq!(output(&at_limit), "65536\n");
    assert_eq!(error(&oversized).kind, DiagnosticKind::VectorLength);
}

#[test]
fn structured_expression_nesting_and_trailing_commas_are_bounded() {
    assert_eq!(output("print(sum([1.0, 2.0,],));"), "3.0\n");
    for source in [
        "print([);",
        "print([1.0,,2.0]);",
        "print(sum([1.0]);",
        "print([1.0][]);",
    ] {
        assert!(scarp::check(source).is_err());
    }
    let too_deep = format!("print({}1.0{});", "[".repeat(65), "]".repeat(65));
    assert_eq!(error(&too_deep).kind, DiagnosticKind::ExpressionNesting);
}

#[test]
fn loops_can_compare_native_and_indexed_dot_products() {
    let source="bind a = [1.0, 2.0, 3.0, 4.0]; bind b = [2.0, -1.0, 0.5, 2.0]; var index = 0; var total = 0.0; while index < len(a) { total += a[index] * b[index]; index += 1; } print(total == dot(a, b));";
    assert_eq!(output(source), "true\n");
}

#[test]
fn compiled_vectors_and_kernel_results_are_reusable_across_threads() {
    let program=scarp::compile("var v = fill(1.0, 3); bind copy = v; print(v); v = add(v, copy); print(dot(v, copy)); print(copy);").unwrap();
    std::thread::scope(|scope| {
        for _ in 0..8 {
            let program = &program;
            scope.spawn(move || {
                for _ in 0..5 {
                    scarp::interpreter::validate(program).unwrap();
                    let mut bytes = Vec::new();
                    scarp::interpreter::execute(program, &mut bytes).unwrap();
                    assert_eq!(bytes, b"[1.0, 1.0, 1.0]\n6.0\n[1.0, 1.0, 1.0]\n");
                }
            });
        }
    });
}
