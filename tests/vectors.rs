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
    assert_eq!(output("bind values = [1.0, -2.5, 3e0,]; println(values); println(values[1]); println(len(values)); println([]); println(len([])); println(([4.0])[0]);"),"[1.0, -2.5, 3.0]\n-2.5\n3\n[]\n0\n4.0\n");
    assert_eq!(
        scarp::compile("bind v = []; bind x = v[0];")
            .unwrap()
            .binding_types(),
        &[ValueType::Vector, ValueType::Float]
    );
}

#[test]
fn vectors_are_immutable_values_with_shared_copy_semantics() {
    assert_eq!(output("var v = [1.0, 2.0]; bind copy = v; println(v); v = scale(v, 2.0); println(copy); println(v); println(copy == [1.0, 2.0]); println(copy != v);"),"[1.0, 2.0]\n[1.0, 2.0]\n[2.0, 4.0]\ntrue\ntrue\n");
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
    assert_eq!(output("println(sum([1.0, 2.0, 3.0])); println(mean([1.0, 2.0, 3.0])); println(dot([1.0, 2.0], [3.0, 4.0])); println(add([1.0, 2.0], [3.0, 4.0])); println(scale([1.0, -2.0], 3.0)); println(clamp([-2.0, 0.5, 3.0], 0.0, 1.0)); println(fill(3, 1.5));"),"6.0\n2.0\n11.0\n[4.0, 6.0]\n[3.0, -6.0]\n[0.0, 0.5, 1.0]\n[1.5, 1.5, 1.5]\n");
    assert_eq!(output("println(sum([])); println(dot([], [])); println(add([], [])); println(fill(0, 1.0)); println(scale([], 1.0));"),"0.0\n0.0\n[]\n[]\n[]\n");
}

#[test]
fn compensated_reductions_preserve_small_cancellation_terms() {
    assert_eq!(output("println(sum([1e16, 1.0, -1e16])); println(dot([1e16, 1.0, -1e16], [1.0, 1.0, 1.0])); println(mean([1e16, 3.0, -1e16]));"),"1.0\n1.0\n1.0\n");
}

#[test]
fn softmax_handles_large_logits_extreme_shifts_and_equal_values() {
    assert_eq!(output("println(softmax([1000.0, 1000.0])); println(softmax([-1e308, 1e308])); println(softmax([2.0])); bind p = softmax([1000.0, 1001.0, 1002.0]); println(sum(p) > 0.999999 and sum(p) < 1.000001); println(p[0] < p[1] and p[1] < p[2]);"),"[0.5, 0.5]\n[0.0, 1.0]\n[1.0]\ntrue\ntrue\n");
}

#[test]
fn mean_squared_error_is_a_checked_native_reduction() {
    assert_eq!(
        output("println(mse([1.0, 2.0], [1.0, 4.0])); println(mse([1.0, 2.0], [1.0, 2.0]));"),
        "2.0\n0.0\n"
    );
    assert_eq!(
        error("println(mse([1e308], [-1e308]));").kind,
        DiagnosticKind::NonFiniteNumber
    );
}

#[test]
fn vector_sizes_shapes_and_indices_report_distinct_failures() {
    for source in [
        "println(fill(-1, 0.0));",
        "println(fill(65537, 0.0));",
        "println(dot([1.0], []));",
        "println(add([], [1.0]));",
        "println(mse([], [1.0]));",
    ] {
        assert_eq!(error(source).kind, DiagnosticKind::VectorLength, "{source}");
    }
    for source in [
        "println([][0]);",
        "println([1.0][-1]);",
        "println([1.0][1]);",
        "println([1.0][9223372036854775807]);",
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
            error(&format!("println(\"before\"); println({expression});")).kind,
            DiagnosticKind::InvalidArgument
        );
    }
    for expression in [
        "sum([1e308, 1e308])",
        "dot([1e308], [2.0])",
        "scale([1e308], 2.0)",
        "add([1e308], [1e308])",
    ] {
        assert_eq!(
            error(&format!("println(\"before\"); println({expression});")).kind,
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
            error(&format!("println({expression});")).kind,
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
            error(&format!("println({expression});")).kind,
            DiagnosticKind::ArgumentCount
        );
    }
    assert_eq!(
        error("println(unknown_kernel([]));").kind,
        DiagnosticKind::UnknownFunction
    );
    for name in [
        "len", "sum", "mean", "dot", "scale", "add", "clamp", "fill", "softmax", "mse",
    ] {
        assert_eq!(output(&format!("bind {name} = 1; println({name});")), "1\n");
    }
    assert_eq!(
        output("bind dot = 4; println(dot); println(dot([1.0], [2.0]));"),
        "4\n2.0\n"
    );
}

#[test]
fn unreachable_runtime_failures_are_skipped_but_type_errors_are_not() {
    assert_eq!(output("if false { println(dot([], [1.0])); println([][-1]); println(fill(-1, 0.0)); } println(true or mean([]) == 0.0);"),"true\n");
    assert_eq!(
        error("if false { println(sum([1])); }").kind,
        DiagnosticKind::TypeMismatch
    );
    assert_eq!(
        error("if false { println(dot([missing], [])); }").kind,
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
        "println(sum(fill(1000, 1.0)));",
        "println(softmax(fill(40, 1.0)));",
        "bind v = fill(40, 1.0); while true { println(v); }",
        "bind v = fill(40, 1.0); while v == v {}",
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
    assert_eq!(output("println(len(fill(65536, 1.0)));"), "65536\n");
    assert!(NumericVector::new(&[f64::NAN]).is_none());
    assert!(NumericVector::new(&[f64::INFINITY]).is_none());
    assert!(NumericVector::new(&vec![0.0; MAX_VECTOR_LENGTH + 1]).is_none());
    let vector = NumericVector::new(&[1.0, 2.0]).unwrap();
    assert_eq!(vector.as_slice()[1].get(), 2.0);
    assert_eq!(vector.len(), 2);
    assert!(!vector.is_empty());
    assert_eq!(Value::Vector(vector).to_string(), "[1.0, 2.0]");
    let oversized = format!("println([{}]);", "1.0,".repeat(MAX_VECTOR_LENGTH + 1));
    let at_limit = format!("println(len([{}]));", "1.0,".repeat(MAX_VECTOR_LENGTH));
    assert_eq!(output(&at_limit), "65536\n");
    assert_eq!(error(&oversized).kind, DiagnosticKind::VectorLength);
}

#[test]
fn structured_expression_nesting_and_trailing_commas_are_bounded() {
    assert_eq!(output("println(sum([1.0, 2.0,],));"), "3.0\n");
    for source in [
        "println([);",
        "println([1.0,,2.0]);",
        "println(sum([1.0]);",
        "println([1.0][]);",
    ] {
        assert!(scarp::check(source).is_err());
    }
    let too_deep = format!("println({}1.0{});", "[".repeat(65), "]".repeat(65));
    assert_eq!(error(&too_deep).kind, DiagnosticKind::ExpressionNesting);
}

#[test]
fn loops_can_compare_native_and_indexed_dot_products() {
    let source="bind a = [1.0, 2.0, 3.0, 4.0]; bind b = [2.0, -1.0, 0.5, 2.0]; var index = 0; var total = 0.0; while index < len(a) { total += a[index] * b[index]; index += 1; } println(total == dot(a, b));";
    assert_eq!(output(source), "true\n");
}

#[test]
fn compiled_vectors_and_kernel_results_are_reusable_across_threads() {
    let program=scarp::compile("var v = fill(3, 1.0); bind copy = v; println(v); v = add(v, copy); println(dot(v, copy)); println(copy);").unwrap();
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
