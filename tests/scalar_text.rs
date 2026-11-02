#[test]
fn conversions_are_explicit_and_lossless() {
    let mut bytes = Vec::new();
    scarp::run("print(to_float(42),to_integer(42.0));", &mut bytes).unwrap();
    assert_eq!(bytes, b"42.0 42\n");
}

#[test]
fn unicode_queries_count_scalars_and_search_without_normalization() {
    let mut bytes = Vec::new();
    scarp::run("bind text=\"aé🌍\"; print(text_len(text),text_bytes(text),text_codepoint(text,2),text_find(text,\"🌍\"),text_contains(text,\"é\"),text_starts_with(text,\"a\"),text_ends_with(text,\"🌍\"));",&mut bytes).unwrap();
    assert_eq!(bytes, b"3 7 127757 2 true true true\n");
}
use scarp::{
    diagnostics::{DiagnosticKind as Kind, Span},
    numeric::{self, Builtin},
    value::{FiniteFloat, Value},
    ExecutionOptions,
};

fn float(value: f64) -> Value {
    Value::Float(FiniteFloat::new(value).unwrap())
}
fn host(function: Builtin, args: &[Value]) -> Value {
    numeric::evaluate(function, args, u64::MAX).unwrap()
}

#[test]
fn integer_to_float_checks_actual_representability_including_both_i64_edges() {
    for value in [
        0,
        1,
        -1,
        1_i64 << 53,
        (1_i64 << 53) + 2,
        i64::MIN,
        i64::MAX - 1023,
    ] {
        assert_eq!(
            host(Builtin::ToFloat, &[Value::Integer(value)]).as_float(),
            Some(value as f64)
        );
    }
    for value in [
        (1_i64 << 53) + 1,
        -((1_i64 << 53) + 1),
        i64::MAX,
        i64::MIN + 1,
    ] {
        let error = numeric::evaluate(Builtin::ToFloat, &[Value::Integer(value)], 1).unwrap_err();
        assert_eq!(error.kind, Kind::NumericConversion);
        assert_eq!(error.span, Span::default());
    }
}

#[test]
fn float_to_integer_rejects_fraction_and_positive_saturating_boundary() {
    for (value, expected) in [
        (42.0, 42),
        (-42.0, -42),
        (0.0, 0),
        (-0.0, 0),
        (i64::MIN as f64, i64::MIN),
        ((i64::MAX - 1023) as f64, i64::MAX - 1023),
    ] {
        assert_eq!(
            host(Builtin::ToInteger, &[float(value)]),
            Value::Integer(expected)
        );
    }
    for value in [
        0.5,
        -1.5,
        -(i64::MIN as f64),
        (i64::MIN as f64) - 2048.0,
        1e100,
    ] {
        assert_eq!(
            numeric::evaluate(Builtin::ToInteger, &[float(value)], 1)
                .unwrap_err()
                .kind,
            Kind::NumericConversion
        );
    }
}

#[test]
fn text_queries_distinguish_scalars_bytes_combining_marks_and_graphemes() {
    let combined = Value::from("e\u{301}");
    assert_eq!(
        host(Builtin::TextLength, std::slice::from_ref(&combined)),
        Value::Integer(2)
    );
    assert_eq!(
        host(Builtin::TextBytes, std::slice::from_ref(&combined)),
        Value::Integer(3)
    );
    assert_eq!(
        host(
            Builtin::TextCodepoint,
            &[combined.clone(), Value::Integer(1)]
        ),
        Value::Integer(769)
    );
    assert_eq!(
        host(Builtin::TextContains, &[combined, Value::from("é")]),
        Value::Boolean(false)
    );
    assert_eq!(
        host(Builtin::TextLength, &[Value::from("👨‍👩‍👧")]),
        Value::Integer(5)
    );
    assert_eq!(
        host(
            Builtin::TextCodepoint,
            &[Value::from("a\0b"), Value::Integer(1)]
        ),
        Value::Integer(0)
    );
}

#[test]
fn empty_missing_and_prefix_suffix_results_are_defined() {
    for text in ["", "aé🌍"] {
        assert_eq!(
            host(Builtin::TextFind, &[Value::from(text), Value::from("")]),
            Value::Integer(0)
        );
        for function in [
            Builtin::TextContains,
            Builtin::TextStartsWith,
            Builtin::TextEndsWith,
        ] {
            assert_eq!(
                host(function, &[Value::from(text), Value::from("")]),
                Value::Boolean(true)
            );
            assert_eq!(
                host(function, &[Value::from(text), Value::from("absent")]),
                Value::Boolean(false)
            );
        }
        assert_eq!(
            host(
                Builtin::TextFind,
                &[Value::from(text), Value::from("absent")]
            ),
            Value::Integer(-1)
        );
    }
}

#[test]
fn negative_empty_oversized_and_end_scalar_indices_are_errors() {
    for (text, index) in [("", 0), ("aé🌍", -1), ("aé🌍", 3), ("aé🌍", i64::MAX)] {
        let error = numeric::evaluate(
            Builtin::TextCodepoint,
            &[Value::from(text), Value::Integer(index)],
            100,
        )
        .unwrap_err();
        assert_eq!(error.kind, Kind::TextIndex);
        assert!(error.help.unwrap().contains("Unicode scalar"));
    }
}

#[test]
fn exact_kernel_budgets_charge_before_text_scans_and_do_not_allocate_vectors() {
    for (function, args, cost) in [
        (Builtin::ToFloat, vec![Value::Integer(1)], 1),
        (Builtin::ToInteger, vec![float(1.0)], 1),
        (Builtin::TextBytes, vec![Value::from("aé🌍")], 1),
        (Builtin::TextLength, vec![Value::from("aé🌍")], 7),
        (
            Builtin::TextCodepoint,
            vec![Value::from("aé🌍"), Value::Integer(2)],
            7,
        ),
        (
            Builtin::TextStartsWith,
            vec![Value::from("aé🌍"), Value::from("aé")],
            3,
        ),
        (
            Builtin::TextEndsWith,
            vec![Value::from("aé🌍"), Value::from("🌍")],
            4,
        ),
        (
            Builtin::TextFind,
            vec![Value::from("aé🌍"), Value::from("🌍")],
            39,
        ),
        (
            Builtin::TextContains,
            vec![Value::from("aé🌍"), Value::from("🌍")],
            39,
        ),
        (
            Builtin::TextFind,
            vec![Value::from("aé🌍"), Value::from("")],
            1,
        ),
        (Builtin::TextLength, vec![Value::from("")], 1),
    ] {
        let before = args.clone();
        numeric::evaluate_with_vector_limit(function, &args, cost, 0).unwrap();
        assert_eq!(
            numeric::evaluate(function, &args, cost - 1)
                .unwrap_err()
                .kind,
            Kind::ExecutionLimit,
            "{function:?}"
        );
        assert_eq!(args, before);
    }
}

#[test]
fn signatures_arity_named_arguments_and_argument_types_are_checked_consistently() {
    for source in [
        "print(to_float(1.0));",
        "print(to_integer(1));",
        "print(text_len(1));",
        "print(text_codepoint(\"a\",1.0));",
        "print(text_find(\"a\",1));",
    ] {
        assert_eq!(
            scarp::compile(source).unwrap_err().kind,
            Kind::TypeMismatch,
            "{source}"
        );
    }
    for source in [
        "print(text_len());",
        "print(to_float(1,2));",
        "print(text_len(text: \"a\"));",
    ] {
        assert_eq!(
            scarp::compile(source).unwrap_err().kind,
            Kind::CallArguments
        );
    }
    assert_eq!(
        numeric::evaluate(Builtin::TextLength, &[], 0)
            .unwrap_err()
            .kind,
        Kind::CallArguments
    );
    assert_eq!(
        numeric::evaluate(Builtin::TextLength, &[Value::Integer(1)], 0)
            .unwrap_err()
            .kind,
        Kind::TypeMismatch
    );
    assert_eq!(Builtin::from_name("text_find"), Some(Builtin::TextFind));
}

#[test]
fn pure_defaults_contracts_and_function_shadowing_use_the_existing_call_rules() {
    let mut out = Vec::new();
    scarp::run("pure funcra count(text: string=\"café\")->integer ensures result>=0; {return text_len(text);} bind value: float=to_float(count()); print(value);",&mut out).unwrap();
    assert_eq!(out, b"4.0\n");
    let mut out = Vec::new();
    scarp::run("pure funcra to_float(n: integer)->integer{return n+1;} bind text_len=7; print(to_float(4),text_len);",&mut out).unwrap();
    assert_eq!(out, b"5 7\n");
}

#[test]
fn imports_do_not_leak_function_shadowing_and_constants_do_not_run_intrinsics() {
    use scarp::modules::{self, MemorySourceLoader, ModuleLimits};
    let mut loader = MemorySourceLoader::new();
    for (name, source) in [
        (
            "main.srp",
            "import \"a.srp\" as a; import \"b.srp\" as b; print(a.to_float(4),b.get());",
        ),
        (
            "a.srp",
            "pub pure funcra to_float(n: integer)->integer{return n+1;}",
        ),
        ("b.srp", "pub pure funcra get()->float{return to_float(4);}"),
    ] {
        loader.insert(name, source).unwrap();
    }
    let program =
        modules::compile_with_loader("main.srp", &loader, ModuleLimits::default()).unwrap();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    program
        .execute(&mut out, &mut err, ExecutionOptions::default())
        .unwrap();
    assert_eq!(out, b"5 4.0\n");
    assert!(err.is_empty());
    let mut loader = MemorySourceLoader::new();
    loader
        .insert("main.srp", "pub bind value=to_float(1);")
        .unwrap();
    assert_eq!(
        modules::compile_with_loader("main.srp", &loader, ModuleLimits::default())
            .unwrap_err()
            .diagnostic
            .kind,
        Kind::ModuleSyntax
    );
}

#[test]
fn short_circuiting_skips_invalid_selected_work_but_not_argument_types() {
    let mut out = Vec::new();
    scarp::run(
        "print(true or text_codepoint(\"\",0)>0, false and to_integer(0.5)>0);",
        &mut out,
    )
    .unwrap();
    assert_eq!(out, b"true false\n");
    assert_eq!(
        scarp::compile("print(true or text_len(1)>0);")
            .unwrap_err()
            .kind,
        Kind::TypeMismatch
    );
}

#[test]
fn generated_unicode_search_matches_an_independent_scalar_window_model() {
    let alphabet = ['a', 'é', '🌍', '\u{301}', '\0'];
    let mut state = 7u64;
    for length in 0..24 {
        for needle_length in 0..7 {
            let mut make = |count| {
                (0..count)
                    .map(|_| {
                        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                        alphabet[(state >> 32) as usize % alphabet.len()]
                    })
                    .collect::<Vec<_>>()
            };
            let chars = make(length);
            let needle = make(needle_length);
            let expected = if needle.is_empty() {
                Some(0)
            } else {
                chars
                    .windows(needle.len())
                    .position(|window| window == needle)
            };
            let text = chars.iter().collect::<String>();
            let pattern = needle.iter().collect::<String>();
            let args = [Value::from(text.as_str()), Value::from(pattern.as_str())];
            assert_eq!(
                host(Builtin::TextFind, &args),
                Value::Integer(expected.map_or(-1, |index| index as i64))
            );
            assert_eq!(
                host(Builtin::TextContains, &args),
                Value::Boolean(expected.is_some())
            );
        }
    }
}

#[test]
fn text_queries_handle_large_shared_inputs_with_exact_work_allowances() {
    let text = "🌍".repeat(100_000);
    let value = Value::from(text.as_str());
    assert_eq!(
        numeric::evaluate(Builtin::TextLength, std::slice::from_ref(&value), 400_000).unwrap(),
        Value::Integer(100_000)
    );
    assert_eq!(
        numeric::evaluate(Builtin::TextLength, &[value], 399_999)
            .unwrap_err()
            .kind,
        Kind::ExecutionLimit
    );
}

#[test]
fn exact_intrinsic_diagnostics_match_library_run_and_check_with_atomic_output() {
    let folder =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/scalar_text");
    for name in ["conversion", "fraction", "index"] {
        let filename = format!("{name}.srp");
        let source = std::fs::read_to_string(folder.join(&filename)).unwrap();
        let expected = std::fs::read_to_string(folder.join(format!("{name}.stderr"))).unwrap();
        let error = scarp::check(&source).unwrap_err();
        assert_eq!(format!("{}\n", error.render(&filename, &source)), expected);
        for mode in ["run", "check"] {
            let result = std::process::Command::new(env!("CARGO_BIN_EXE_scarp"))
                .current_dir(&folder)
                .args([mode, &filename])
                .output()
                .unwrap();
            assert_eq!(result.status.code(), Some(1));
            assert!(result.stdout.is_empty());
            assert_eq!(String::from_utf8(result.stderr).unwrap(), expected);
        }
        let result = std::process::Command::new(env!("CARGO_BIN_EXE_scarp"))
            .current_dir(&folder)
            .args(["modules", &filename])
            .output()
            .unwrap();
        assert!(result.status.success() && result.stderr.is_empty());
    }
}
