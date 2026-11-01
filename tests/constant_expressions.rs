use scarp::{
    diagnostics::{DiagnosticKind as Kind, Span},
    modules::{self, ConstantLimits, MemorySourceLoader, ModuleLimits},
    ExecutionOptions,
};

fn compile(files: &[(&str, &str)]) -> Result<modules::ModuleProgram, modules::ModuleDiagnostic> {
    let mut loader = MemorySourceLoader::new();
    for (name, source) in files {
        loader.insert(*name, *source).unwrap();
    }
    modules::compile_with_loader("main.srp", &loader, ModuleLimits::default())
}

fn output(files: &[(&str, &str)]) -> Vec<u8> {
    let program = compile(files).unwrap();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    program
        .execute(&mut out, &mut err, ExecutionOptions::default())
        .unwrap();
    assert!(err.is_empty());
    out
}

fn limited(
    source: &str,
    limits: ConstantLimits,
    bytes: usize,
) -> Result<modules::ModuleProgram, modules::ModuleDiagnostic> {
    let mut loader = MemorySourceLoader::new();
    loader.insert("main.srp", source).unwrap();
    modules::compile_with_loader_and_constant_limits(
        "main.srp",
        &loader,
        ModuleLimits {
            max_expansion_bytes: bytes,
            ..ModuleLimits::default()
        },
        limits,
    )
}

#[test]
fn scalar_operators_types_signed_zero_and_string_equality_work() {
    assert_eq!(
        output(&[(
            "main.srp",
            r#"
        pub bind base=10; pub bind multiplier=4; pub bind result=base*multiplier;
        pub bind negative=-(result/2-1); pub bind fraction=1.5*2.0/4.0;
        pub bind zero=-0.0; pub bind label="café 🌍"; pub bind copy=label;
        pub bind ready=not false and (result==40 or false);
        print(result,negative,fraction,zero,ready,copy=="café 🌍",result>=40,result!=0);
    "#
        )]),
        "40 -19 0.75 -0.0 true true true true\n".as_bytes()
    );
}

#[test]
fn checked_integer_and_finite_float_errors_keep_operator_spans() {
    for (expression, kind, operator) in [
        ("9223372036854775807+1", Kind::ArithmeticOverflow, "+"),
        ("-9223372036854775808-1", Kind::ArithmeticOverflow, "-1"),
        ("3037000500*3037000500", Kind::ArithmeticOverflow, "*"),
        ("-9223372036854775808/(-1)", Kind::ArithmeticOverflow, "/"),
        ("-(-9223372036854775808)", Kind::ArithmeticOverflow, "-("),
        ("1/0", Kind::DivisionByZero, "/"),
        ("1.0/(-0.0)", Kind::DivisionByZero, "/"),
        ("1e308*10.0", Kind::NonFiniteNumber, "*"),
    ] {
        let source = format!("pub bind bad={expression};");
        let error = compile(&[("main.srp", &source)]).unwrap_err();
        assert_eq!(error.diagnostic.kind, kind, "{expression}");
        let start = source.find(expression).unwrap() + expression.find(operator).unwrap();
        assert_eq!(
            error.diagnostic.span,
            Span::new(start, start + 1),
            "{expression}"
        );
        assert!(error
            .diagnostic
            .related
            .iter()
            .any(|r| r.message == "constant initialized here"));
    }
    assert_eq!(output(&[("main.srp", "pub bind min=-9223372036854775808; pub bind max=9223372036854775807; pub bind tiny=1e-300*1e-300; print(min,max,tiny);")]),
        b"-9223372036854775808 9223372036854775807 0.0\n");
}

#[test]
fn static_typing_checks_nested_and_short_circuited_operands() {
    for expression in [
        "1+2.0",
        "true+1",
        "not 1",
        "true and 1",
        "1==1.0",
        "\"a\"+\"b\"",
        "true or (1+(2.0*3.0)>0)",
    ] {
        let source = format!("pub bind bad={expression};");
        assert_eq!(
            compile(&[("main.srp", &source)])
                .unwrap_err()
                .diagnostic
                .kind,
            Kind::TypeMismatch,
            "{expression}"
        );
    }
    let source = "pub bind bad=1+(2.0*3.0);";
    let error = compile(&[("main.srp", source)]).unwrap_err();
    assert_eq!(
        &source[error.diagnostic.span.start..error.diagnostic.span.end],
        "+"
    );
    assert!(error
        .diagnostic
        .related
        .iter()
        .any(|r| r.message == "right operand has type float"));
}

#[test]
fn short_circuiting_skips_arithmetic_but_not_reference_validation() {
    assert_eq!(output(&[("main.srp", "pub bind good=false and (1/0>0); pub bind other=true or (9223372036854775807+1>0); print(good,other);")]), b"false true\n");
    for source in [
        "pub bind bad=true or missing;",
        "pub bind bad=false and missing;",
    ] {
        assert_eq!(
            compile(&[("main.srp", source)])
                .unwrap_err()
                .diagnostic
                .kind,
            Kind::UnknownName
        );
    }
}

#[test]
fn functions_builtins_vectors_and_indexing_are_never_constant_expressions() {
    for expression in [
        "f()",
        "sqrt(4.0)",
        "[1.0]",
        "[1.0][0]",
        "true or f()",
        "false and (sqrt(4.0)>0.0)",
    ] {
        let source =
            format!("pure funcra f()->boolean {{return true;}} pub bind bad={expression};");
        assert_eq!(
            compile(&[("main.srp", &source)])
                .unwrap_err()
                .diagnostic
                .kind,
            Kind::ModuleSyntax,
            "{expression}"
        );
    }
    let source = "pure funcra f()->integer {return 1;} pub bind bad=f;";
    assert_eq!(
        compile(&[("main.srp", source)])
            .unwrap_err()
            .diagnostic
            .kind,
        Kind::ModuleSymbol
    );
}

#[test]
fn declaration_order_reports_the_later_constant_and_cycles_use_actual_references() {
    let source = "pub bind first=second+1; pub bind second=2;";
    let error = compile(&[("main.srp", source)]).unwrap_err();
    assert_eq!(error.diagnostic.kind, Kind::ModuleSymbol);
    assert_eq!(
        &source[error.diagnostic.span.start..error.diagnostic.span.end],
        "second"
    );
    assert_eq!(
        error.diagnostic.related[0].span.start,
        source.rfind("second").unwrap()
    );
    for source in [
        "pub bind a=a+1;",
        "pub bind prefix=a; pub bind a=b; pub bind b=c; pub bind c=a;",
    ] {
        let error = compile(&[("main.srp", source)]).unwrap_err();
        assert_eq!(error.diagnostic.kind, Kind::ModuleCycle);
        assert!(error
            .diagnostic
            .message
            .contains("constant dependency cycle"));
        assert!(error
            .diagnostic
            .related
            .iter()
            .all(|r| !source[r.span.start..r.span.end].contains("prefix")));
        assert!(error
            .diagnostic
            .related
            .iter()
            .all(|r| r.span.start >= source.find("a=").unwrap()));
    }
}

#[test]
fn entry_script_bindings_stay_runtime_values_and_functions_can_use_later_constants() {
    assert!(scarp::compile("bind runtime=1/0;").is_ok());
    assert!(compile(&[("main.srp", "bind runtime=1/0;")]).is_ok());
    assert_eq!(
        compile(&[("main.srp", "bind runtime=2; pub bind bad=runtime;")])
            .unwrap_err()
            .diagnostic
            .kind,
        Kind::UnknownName
    );
    assert_eq!(
        output(&[(
            "main.srp",
            "pure funcra f()->integer {return later;} pub bind later=3*7; print(f());"
        )]),
        b"21\n"
    );
    assert_eq!(
        output(&[(
            "main.srp",
            "pub bind value=2*3; {bind value=value+1; print(value);} print(value);"
        )]),
        b"7\n6\n"
    );
}

#[test]
fn diamond_imports_evaluate_dependencies_once_and_preserve_public_order() {
    let files = [("main.srp", "pub bind result=b.value+a.value; import \"a.srp\" as a; import \"b.srp\" as b; print(result);"),
        ("a.srp", "import \"common.srp\" as c; pub bind value=c.base*2;"),
        ("b.srp", "import \"common.srp\" as c; pub bind value=c.base+1;"),
        ("common.srp", "pub bind base=10;")];
    let program = compile(&files).unwrap();
    assert_eq!(output(&files), b"31\n");
    assert_eq!(
        program
            .modules()
            .iter()
            .map(|m| m.source)
            .collect::<Vec<_>>(),
        [0, 1, 2, 3]
    );
    let dependencies = program.constant_dependencies();
    assert_eq!(
        dependencies
            .iter()
            .map(|d| (d.source, d.target))
            .collect::<Vec<_>>(),
        [(0, 3), (0, 1), (1, 2), (3, 2)]
    );
    for edge in dependencies {
        for (span, source) in [
            (edge.declaration, edge.source),
            (edge.target_declaration, edge.target),
            (edge.span, edge.source),
        ] {
            assert_eq!(program.sources().locate_span(span).unwrap().source, source);
        }
    }
    let mut inventory = Vec::new();
    program.write_inspection(&mut inventory).unwrap();
    assert!(String::from_utf8(inventory)
        .unwrap()
        .contains("pub bind result: integer"));
}

#[test]
fn constant_visibility_uses_the_same_lookup_and_never_reexports_aliases() {
    for (expression, kind) in [
        ("lib.hidden", Kind::ModuleVisibility),
        ("lib.missing", Kind::ModuleSymbol),
        ("lib.f", Kind::ModuleSymbol),
        ("other.value", Kind::ModuleSymbol),
        ("lib.inner", Kind::ModuleSymbol),
    ] {
        let source = format!("import \"lib.srp\" as lib; pub bind bad={expression};");
        let error = compile(&[("main.srp", &source),("lib.srp","import \"inner.srp\" as inner; bind hidden=2*3; pub bind value=hidden+1; pub pure funcra f()->integer {return value;}"),("inner.srp", "pub bind value=1;")]).unwrap_err();
        assert_eq!(error.diagnostic.kind, kind, "{expression}");
        if kind == Kind::ModuleVisibility {
            assert_eq!(
                error
                    .sources
                    .locate_span(error.diagnostic.related[0].span)
                    .unwrap()
                    .source,
                1
            );
        }
    }
}

#[test]
fn dependency_metadata_includes_skipped_references_and_repeated_uses() {
    let source = "pub bind yes=true; pub bind result=yes or (yes and false);";
    let program = compile(&[("main.srp", source)]).unwrap();
    assert_eq!(program.constant_dependencies().len(), 2);
    assert!(program
        .constant_dependencies()
        .iter()
        .all(|edge| &source[edge.span.start..edge.span.end] == "yes"));
    let again = compile(&[("main.srp", source)]).unwrap();
    assert_eq!(
        program.constant_dependencies(),
        again.constant_dependencies()
    );
}

#[test]
fn node_and_task_limits_have_exact_cumulative_boundaries() {
    let source = "pub bind a=1+2; pub bind b=a*3;";
    let exact = ConstantLimits {
        max_nodes: 6,
        max_steps: 8,
    };
    limited(source, exact, 0).unwrap();
    for limits in [
        ConstantLimits {
            max_nodes: 5,
            ..exact
        },
        ConstantLimits {
            max_steps: 7,
            ..exact
        },
        ConstantLimits {
            max_steps: 0,
            ..exact
        },
    ] {
        let error = limited(source, limits, 0).unwrap_err();
        assert_eq!(error.diagnostic.kind, Kind::ModuleLimit);
        assert!(error.diagnostic.help.unwrap().contains("ConstantLimits"));
    }
    limited(
        "print(1+2);",
        ConstantLimits {
            max_nodes: 0,
            max_steps: 0,
        },
        0,
    )
    .unwrap();
    let skipped = "pub bind ok=true or (1/0>0);";
    limited(
        skipped,
        ConstantLimits {
            max_nodes: 8,
            max_steps: 3,
        },
        0,
    )
    .unwrap();
    assert_eq!(
        limited(
            skipped,
            ConstantLimits {
                max_nodes: 7,
                max_steps: 3
            },
            0
        )
        .unwrap_err()
        .diagnostic
        .kind,
        Kind::ModuleLimit
    );
}

#[test]
fn limits_are_shared_across_modules_and_diamonds_do_not_double_charge() {
    let mut loader = MemorySourceLoader::new();
    for (name, source) in [
        ("main.srp", "import \"a.srp\" as a; import \"b.srp\" as b;"),
        ("a.srp", "import \"common.srp\" as c; pub bind a=c.value;"),
        ("b.srp", "import \"common.srp\" as c; pub bind b=c.value;"),
        ("common.srp", "pub bind value=1;"),
    ] {
        loader.insert(name, source).unwrap();
    }
    for nodes in [3, 2] {
        let result = modules::compile_with_loader_and_constant_limits(
            "main.srp",
            &loader,
            ModuleLimits::default(),
            ConstantLimits {
                max_nodes: nodes,
                max_steps: 3,
            },
        );
        if nodes == 3 {
            result.unwrap();
        } else {
            assert_eq!(result.unwrap_err().diagnostic.kind, Kind::ModuleLimit);
        }
    }
}

#[test]
fn folded_string_storage_is_charged_before_new_clones_and_literals_keep_old_limits() {
    let source = "pub bind a=\"abcd\"; pub bind b=a; print(b);";
    limited(source, ConstantLimits::default(), 12).unwrap();
    assert_eq!(
        limited(source, ConstantLimits::default(), 11)
            .unwrap_err()
            .diagnostic
            .kind,
        Kind::ModuleLimit
    );
    limited(
        "pub bind text=\"abcd\"; print(text);",
        ConstantLimits::default(),
        4,
    )
    .unwrap();
}

#[test]
fn constant_failures_keep_unicode_crlf_locations_and_loaded_sources() {
    let library = "// नमस्ते 🌍\r\npub bind base=7;\r\npub bind bad=base/0;";
    let error = compile(&[
        ("main.srp", "import \"café.srp\" as lib;"),
        ("café.srp", library),
    ])
    .unwrap_err();
    let at = error.sources.locate_span(error.diagnostic.span).unwrap();
    assert_eq!((at.source, at.start.line, at.start.column), (1, 3, 18));
    assert!(error.render().contains("café.srp:3:18"));
    assert_eq!(error.sources.source(1), Some(library));
}

#[test]
fn long_flat_expressions_dependency_chains_and_cycles_are_iterative() {
    let mut source = String::from("pub bind c0=1;");
    for id in 1..3000 {
        source.push_str(&format!("pub bind c{id}=c{}+1;", id - 1));
    }
    source.push_str("print(c2999);");
    assert_eq!(output(&[("main.srp", &source)]), b"3000\n");
    let expression = std::iter::repeat("1")
        .take(20_000)
        .collect::<Vec<_>>()
        .join("+");
    assert_eq!(
        output(&[(
            "main.srp",
            &format!("pub bind result={expression}; print(result);")
        )]),
        b"20000\n"
    );
    let mut cycle = String::new();
    for id in 0..3000 {
        cycle.push_str(&format!("pub bind c{id}=c{};", (id + 1) % 3000));
    }
    let error = compile(&[("main.srp", &cycle)]).unwrap_err();
    assert_eq!(error.diagnostic.kind, Kind::ModuleCycle);
    assert!(error.diagnostic.related.len() <= 16);
}

#[test]
fn constant_and_runtime_scalar_semantics_match() {
    for expression in [
        "1+2*3",
        "(8-3)/2",
        "1.0/4.0",
        "-0.0",
        "not (1<2)",
        "true or (1/0>0)",
        "\"café\"==\"café\"",
        "9223372036854775807+1",
        "1/0",
        "1+true",
    ] {
        let constant = format!("pub bind result={expression}; print(result);");
        let runtime = format!("bind result={expression}; print(result);");
        match compile(&[("main.srp", &constant)]) {
            Ok(_) => {
                let mut expected = Vec::new();
                scarp::run(&runtime, &mut expected).unwrap();
                assert_eq!(output(&[("main.srp", &constant)]), expected, "{expression}");
            }
            Err(error) => {
                let scarp::RunError::Diagnostic(runtime) =
                    scarp::run(&runtime, &mut Vec::new()).unwrap_err()
                else {
                    panic!("output error")
                };
                assert_eq!(error.diagnostic.kind, runtime.kind, "{expression}");
            }
        }
    }
}

#[test]
fn loading_is_nonevaluating_and_additive_limit_apis_preserve_module_limit_literals() {
    let mut loader = MemorySourceLoader::new();
    loader.insert("main.srp", "pub bind bad=1/0;").unwrap();
    let limits = ModuleLimits {
        max_files: 128,
        max_depth: 32,
        max_file_bytes: 1024 * 1024,
        max_total_bytes: 8 * 1024 * 1024,
        max_imports: 1024,
        max_expansion_bytes: 8 * 1024 * 1024,
    };
    let graph = modules::load("main.srp", &loader, limits).unwrap();
    assert_eq!(graph.file_count(), 1);
    assert_eq!(
        modules::resolve_with_constant_limits(graph, ConstantLimits::default())
            .unwrap_err()
            .diagnostic
            .kind,
        Kind::DivisionByZero
    );
}

#[test]
fn folded_types_are_checked_in_cross_file_arguments_defaults_and_returns() {
    let library="pub bind count=2*3; pub bind flag=count>0; pub pure funcra take(value: float)->float {return value;}";
    let error = compile(&[
        (
            "main.srp",
            "import \"lib.srp\" as lib; print(lib.take(lib.count));",
        ),
        ("lib.srp", library),
    ])
    .unwrap_err();
    assert_eq!(error.diagnostic.kind, Kind::TypeMismatch);
    assert_eq!(
        error
            .sources
            .locate_span(error.diagnostic.span)
            .unwrap()
            .source,
        0
    );
    assert_eq!(
        error
            .sources
            .locate_span(error.diagnostic.related[0].span)
            .unwrap()
            .source,
        1
    );
    for (source, kind) in [
        (
            "pub bind flag=2>1; pure funcra bad(n: integer=flag)->integer {return n;}",
            Kind::TypeMismatch,
        ),
        (
            "pub bind flag=2>1; pub pure funcra bad()->integer {return flag;}",
            Kind::InvalidReturn,
        ),
    ] {
        let error = compile(&[
            ("main.srp", "import \"lib.srp\" as lib;"),
            ("lib.srp", source),
        ])
        .unwrap_err();
        assert_eq!(error.diagnostic.kind, kind);
        assert!(error.diagnostic.related.iter().all(|r| error
            .sources
            .locate_span(r.span)
            .unwrap()
            .source
            == 1));
        if kind == Kind::InvalidReturn {
            assert!(error
                .diagnostic
                .related
                .iter()
                .any(|r| r.message == "returned expression has type boolean"));
            assert!(error.diagnostic.help.unwrap().contains("integer"));
        }
    }
}

#[test]
fn argument_type_errors_relate_parameters_consistently_in_single_and_multi_file_compilation() {
    let source = "pure funcra take(value: integer)->integer {return value;} print(take(1<2));";
    let plain = scarp::compile(source).unwrap_err();
    let module = compile(&[("main.srp", source)]).unwrap_err();
    assert_eq!(plain, module.diagnostic);
    assert_eq!(plain.kind, Kind::TypeMismatch);
    assert_eq!(plain.related.len(), 1);
    assert_eq!(
        &source[plain.related[0].span.start..plain.related[0].span.end],
        "value"
    );
}

#[test]
fn arithmetic_constants_are_available_to_importers_and_functions() {
    let program = compile(&[("main.srp", "import \"lib.srp\" as lib; print(lib.answer, lib.get());"),
        ("lib.srp", "bind base=10; pub bind answer=base*4+2; pub pure funcra get()->integer {return answer;}")]).unwrap();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    program
        .execute(&mut out, &mut err, ExecutionOptions::default())
        .unwrap();
    assert_eq!(out, b"42 42\n");
    assert!(err.is_empty());
}

#[test]
fn constant_division_errors_keep_the_operator_source_location() {
    let error = compile(&[("main.srp", "pub bind bad=1/0;")]).unwrap_err();
    assert_eq!(error.diagnostic.kind.code(), "E0008");
    assert_eq!(error.diagnostic.span, scarp::diagnostics::Span::new(14, 15));
}
