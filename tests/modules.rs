use scarp::{
    diagnostics::DiagnosticKind as Kind,
    modules::{self, MemorySourceLoader, ModuleLimits, ModuleProgram, ModuleRunError},
    ExecutionOptions,
};
fn compile(files: &[(&str, &str)]) -> Result<ModuleProgram, modules::ModuleDiagnostic> {
    let mut loader = MemorySourceLoader::new();
    for (name, source) in files {
        loader.insert(name, *source).unwrap();
    }
    modules::compile_with_loader("main.srp", &loader, ModuleLimits::default())
}
fn output(files: &[(&str, &str)]) -> Vec<u8> {
    let program = compile(files).unwrap_or_else(|e| panic!("{}", e.render()));
    program.check(ExecutionOptions::default()).unwrap();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    program
        .execute(&mut out, &mut err, ExecutionOptions::default())
        .unwrap();
    assert!(err.is_empty());
    out
}
#[test]
fn public_functions_constants_and_private_helpers_are_linked() {
    assert_eq!(output(&[("main.srp","import \"math.srp\" as math; print(math.twice(math.factor));"),
        ("math.srp","pub bind factor = 2.0; pub pure funcra twice(x: float) -> float { return helper(x); } pure funcra helper(x: float)->float {return x*factor;}")]),b"4.0\n");
}
#[test]
fn module_namespaces_defaults_contracts_and_local_shadowing_work() {
    assert_eq!(output(&[("main.srp", "import \"a.srp\" as a; import \"b.srp\" as b; print(a.f(), b.f());"),
        ("a.srp", "bind constant=7; pub pure funcra f(x: integer=constant)->integer ensures result==x; { {bind constant=3; } return x; }"),
        ("b.srp", "pub pure funcra f()->integer {return 9;}")]),b"7 9\n");
}
#[test]
fn multi_level_imports_and_repeated_aliases_share_one_source() {
    let files=[("main.srp","import \"lib/a.srp\" as a; import \"./lib/../lib/a.srp\" as again; print(a.f(),again.f());"),
        ("lib/a.srp","import \"../shared.srp\" as s; pub pure funcra f()->integer{return s.value;}"),
        ("shared.srp","pub bind value=42;")];
    assert_eq!(output(&files), b"42 42\n");
    let program = compile(&files).unwrap();
    assert_eq!(program.sources().files().len(), 3);
    assert_eq!(program.program().functions().len(), 1);
}
#[test]
fn visibility_and_unknown_members_have_source_aware_errors() {
    for (member, kind) in [
        ("hidden", Kind::ModuleVisibility),
        ("absent", Kind::ModuleSymbol),
    ] {
        let source = format!("import \"lib.srp\" as lib; print(lib.{member});");
        let error = compile(&[("main.srp", &source), ("lib.srp", "bind hidden=3;")]).unwrap_err();
        assert_eq!(error.diagnostic.kind, kind);
        assert!(error.render().contains("main.srp:1:"));
        if kind == Kind::ModuleVisibility {
            assert!(error.render().contains("lib.srp:1:"));
        }
    }
}
#[test]
fn cycles_missing_sources_and_library_side_effects_are_rejected() {
    for (files, kind) in [
        (
            vec![("main.srp", "import \"absent.srp\" as missing;")],
            Kind::ModuleLoad,
        ),
        (
            vec![
                ("main.srp", "import \"b.srp\" as b;"),
                ("b.srp", "import \"main.srp\" as root;"),
            ],
            Kind::ModuleCycle,
        ),
        (
            vec![
                ("main.srp", "import \"b.srp\" as b;"),
                ("b.srp", "print(\"unexpected\");"),
            ],
            Kind::ModuleSyntax,
        ),
        (
            vec![
                ("main.srp", "import \"b.srp\" as b;"),
                ("b.srp", "pub bind value=fill(1.0,2);"),
            ],
            Kind::ModuleSyntax,
        ),
    ] {
        assert_eq!(compile(&files).unwrap_err().diagnostic.kind, kind);
    }
}
#[test]
fn cross_file_runtime_traces_and_atomic_streams_are_preserved() {
    let program = compile(&[
        (
            "main.srp",
            "import \"bad.srp\" as bad; print(\"hidden\"); eprint(\"hidden\"); print(bad.f());",
        ),
        ("bad.srp", "pub pure funcra f()->integer {return 1/0;}"),
    ])
    .unwrap();
    let expected = program.check(ExecutionOptions::default()).unwrap_err();
    assert_eq!(expected.diagnostic.kind, Kind::DivisionByZero);
    let rendered = expected.render();
    assert!(rendered.contains("bad.srp:1:"), "{rendered}");
    assert!(rendered.contains("main.srp:1:"), "{rendered}");
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let ModuleRunError::Diagnostic(actual) = program
        .execute(&mut out, &mut err, ExecutionOptions::default())
        .unwrap_err()
    else {
        panic!()
    };
    assert_eq!(actual.render(), rendered);
    assert!(out.is_empty() && err.is_empty());
}
#[test]
fn contextual_module_words_preserve_old_single_file_identifiers() {
    assert_eq!(output(&[("main.srp","bind import=2; bind pub=3; pure funcra as(x: integer)->integer{return x;} print(as(import+pub));")]),b"5\n");
    assert_eq!(
        scarp::compile("import \"math.srp\" as math;")
            .unwrap_err()
            .kind,
        Kind::ModuleSyntax
    );
}

#[test]
fn ordinary_single_file_semantic_errors_keep_their_established_diagnostics() {
    for source in [
        "funcra f()->unit {} funcra f()->unit {}",
        "funcra repeat()->unit {}",
        "bind value=1; bind value=2;",
        "pure funcra f(x: integer)->integer{return x;} print(f(true));",
        "bind global=1; funcra f()->integer{return global;}",
    ] {
        let expected = scarp::compile(source).unwrap_err();
        assert_eq!(
            compile(&[("main.srp", source)]).unwrap_err().diagnostic,
            expected,
            "{source}"
        );
    }
}

#[test]
fn forward_mutual_recursion_and_builtin_precedence_are_module_local() {
    assert_eq!(output(&[("main.srp","import \"local.srp\" as lib; print(lib.even(8), sum([1.0,2.0]), lib.sum(3));"),
        ("local.srp", "pub pure funcra even(n: integer)->boolean {if n==0{return true;} return odd(n-1);} pure funcra odd(n: integer)->boolean {if n==0{return false;} return even(n-1);} pub pure funcra sum(n: integer)->integer{return n+10;}")]),b"true 3.0 13\n");
}

#[test]
fn entry_functions_do_not_capture_library_builtins_or_binding_names() {
    assert_eq!(output(&[("main.srp", "import \"lib.srp\" as lib; pure funcra sum(n: integer)->integer {return n+10;} pure funcra local()->integer {return 99;} print(sum(3), lib.f());"),
        ("lib.srp", "pub pure funcra f()->float {bind local=2.0; return sum([local,3.0]);}")]), b"13 5.0\n");
}

#[test]
fn library_bindings_still_cannot_collide_with_their_own_functions() {
    for body in [
        "pub pure funcra f()->integer {bind f=1; return f;}",
        "pub pure funcra f(f: integer)->integer {return f;}",
        "pub pure funcra f()->integer {repeat 1 as f {} return 0;}",
    ] {
        assert_eq!(
            compile(&[
                ("main.srp", "import \"lib.srp\" as lib;"),
                ("lib.srp", body)
            ])
            .unwrap_err()
            .diagnostic
            .kind,
            Kind::FunctionDefinition
        );
    }
}

#[test]
fn public_and_qualified_syntax_preserves_the_small_parser_stack_boundary() {
    use scarp::parser::{MAX_BLOCK_DEPTH, MAX_EXPRESSION_DEPTH};
    let (source, syntax) = std::thread::Builder::new()
        .stack_size(512 * 1024)
        .spawn(|| {
            let source = format!(
                "import \"math.srp\" as math; pub pure funcra f()->integer{{{}return {}math.value{};{}}} print(f());",
                "{".repeat(MAX_BLOCK_DEPTH - 1),
                "(".repeat(MAX_EXPRESSION_DEPTH),
                ")".repeat(MAX_EXPRESSION_DEPTH),
                "}".repeat(MAX_BLOCK_DEPTH - 1)
            );
            let syntax = scarp::parse(&source).unwrap();
            (source, syntax)
        })
        .unwrap()
        .join()
        .unwrap();
    assert_eq!(syntax.statements.len(), 3);
    // Link/resolve/execute at the same boundaries on the ordinary worker stack.
    assert_eq!(
        output(&[("main.srp", &source), ("math.srp", "pub bind value=42;")]),
        b"42\n"
    );
}

#[test]
fn module_writer_failures_preserve_io_kind_and_partial_replay_behavior() {
    use std::io::{self, Write};
    struct Closed;
    impl Write for Closed {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let program = compile(&[
        (
            "main.srp",
            "import \"lib.srp\" as lib; print(lib.text); eprint(lib.text);",
        ),
        ("lib.srp", "pub bind text=\"ready\";"),
    ])
    .unwrap();
    let mut stdout = Vec::new();
    let error = program
        .execute(&mut stdout, &mut Closed, ExecutionOptions::default())
        .unwrap_err();
    assert!(std::error::Error::source(&error).is_some());
    assert!(matches!(error, ModuleRunError::Output(e) if e.kind()==io::ErrorKind::BrokenPipe));
    assert_eq!(stdout, b"ready\n");
    let mut stderr = Vec::new();
    assert!(
        matches!(program.execute(&mut Closed, &mut stderr, ExecutionOptions::default()),
        Err(ModuleRunError::Output(e)) if e.kind()==io::ErrorKind::BrokenPipe)
    );
    assert!(stderr.is_empty());
}

#[test]
fn constant_shadowing_defaults_and_postcondition_result_names_stay_lexical() {
    assert_eq!(output(&[("main.srp","import \"constants.srp\" as c; print(c.f(),c.g(),c.minimum,c.minuszero);"),
        ("constants.srp", "bind value=7; bind result=999; pub bind minimum=-9223372036854775808; pub bind minuszero=-0.0; pub pure funcra f(value: integer=value)->integer ensures result==value; {return value;} pub pure funcra g()->integer {bind value=value+1; {bind value=2;} return value;}")]),b"7 8 -9223372036854775808 -0.0\n");
}

#[test]
fn module_alias_collisions_and_mutable_or_dynamic_library_globals_fail() {
    for source in [
        "import \"lib.srp\" as x; bind x=1;",
        "import \"lib.srp\" as x; import \"lib.srp\" as x;",
        "import \"lib.srp\" as x; funcra x()->unit {}",
    ] {
        assert_eq!(
            compile(&[("main.srp", source), ("lib.srp", "")])
                .unwrap_err()
                .diagnostic
                .kind,
            Kind::DuplicateBinding
        );
    }
    for library in [
        "var value=3;",
        "bind vector=[1.0];",
        "bind value=fill(1.0,3);",
        "pub bind value=[1.0];",
        "pub var value=1;",
    ] {
        assert_eq!(
            compile(&[
                ("main.srp", "import \"lib.srp\" as lib;"),
                ("lib.srp", library)
            ])
            .unwrap_err()
            .diagnostic
            .kind,
            Kind::ModuleSyntax
        );
    }
    assert_eq!(
        compile(&[("main.srp", "pub bind value=1; bind value=2;")])
            .unwrap_err()
            .diagnostic
            .kind,
        Kind::DuplicateBinding
    );
}

#[test]
fn import_paths_are_portable_and_cannot_escape_the_root() {
    for path in [
        "../out.srp",
        "/tmp/file.srp",
        "C:/file.srp",
        "https://host/file.srp",
        "lib//file.srp",
        "lib\\file.srp",
        "a.txt",
        "",
    ] {
        let mut loader = MemorySourceLoader::new();
        loader
            .insert("main.srp", format!("import {path:?} as library;"))
            .unwrap();
        assert_eq!(
            modules::compile_with_loader("main.srp", &loader, ModuleLimits::default())
                .unwrap_err()
                .diagnostic
                .kind,
            Kind::ModulePath,
            "{path}"
        );
    }
}

#[test]
fn module_source_limits_have_exact_boundaries_and_cached_diamonds_obey_depth() {
    let root = "import \"lib.srp\" as lib;";
    let library = "pub bind value=1;";
    let mut loader = MemorySourceLoader::new();
    loader.insert("main.srp", root).unwrap();
    loader.insert("lib.srp", library).unwrap();
    let exact = ModuleLimits {
        max_files: 2,
        max_depth: 1,
        max_file_bytes: root.len(),
        max_total_bytes: root.len() + library.len(),
        max_imports: 1,
        ..ModuleLimits::default()
    };
    modules::compile_with_loader("main.srp", &loader, exact).unwrap();
    for limits in [
        ModuleLimits {
            max_files: 1,
            ..exact
        },
        ModuleLimits {
            max_depth: 0,
            ..exact
        },
        ModuleLimits {
            max_file_bytes: root.len() - 1,
            ..exact
        },
        ModuleLimits {
            max_total_bytes: root.len() + library.len() - 1,
            ..exact
        },
        ModuleLimits {
            max_imports: 0,
            ..exact
        },
    ] {
        assert_eq!(
            modules::compile_with_loader("main.srp", &loader, limits)
                .unwrap_err()
                .diagnostic
                .kind,
            Kind::ModuleLimit
        );
    }
    let mut loader = MemorySourceLoader::new();
    loader
        .insert(
            "main.srp",
            "import \"shared.srp\" as s; import \"a.srp\" as a;",
        )
        .unwrap();
    loader
        .insert("shared.srp", "import \"leaf.srp\" as leaf;")
        .unwrap();
    loader.insert("leaf.srp", "").unwrap();
    loader.insert("a.srp", "import \"b.srp\" as b;").unwrap();
    loader
        .insert("b.srp", "import \"shared.srp\" as s;")
        .unwrap();
    modules::compile_with_loader(
        "main.srp",
        &loader,
        ModuleLimits {
            max_depth: 4,
            ..ModuleLimits::default()
        },
    )
    .unwrap();
    assert_eq!(
        modules::compile_with_loader(
            "main.srp",
            &loader,
            ModuleLimits {
                max_depth: 3,
                ..ModuleLimits::default()
            }
        )
        .unwrap_err()
        .diagnostic
        .kind,
        Kind::ModuleLimit
    );
}

#[test]
fn repeated_imports_read_each_canonical_source_only_once() {
    use modules::{SourceLoadError, SourceLoader};
    use std::cell::RefCell;
    struct Counted {
        inner: MemorySourceLoader,
        reads: RefCell<Vec<String>>,
    }
    impl SourceLoader for Counted {
        fn resolve(&self, importer: Option<&str>, path: &str) -> Result<String, SourceLoadError> {
            self.inner.resolve(importer, path)
        }
        fn load(&self, id: &str, bytes: usize) -> Result<String, SourceLoadError> {
            self.reads.borrow_mut().push(id.into());
            self.inner.load(id, bytes)
        }
    }
    let mut inner = MemorySourceLoader::new();
    inner
        .insert(
            "main.srp",
            "import \"a.srp\" as a; import \"b.srp\" as b; import \"./a.srp\" as again;",
        )
        .unwrap();
    inner
        .insert("a.srp", "import \"common.srp\" as c;")
        .unwrap();
    inner
        .insert("b.srp", "import \"common.srp\" as c;")
        .unwrap();
    inner.insert("common.srp", "").unwrap();
    let loader = Counted {
        inner,
        reads: RefCell::new(Vec::new()),
    };
    let graph = modules::load("main.srp", &loader, ModuleLimits::default()).unwrap();
    assert_eq!(
        &*loader.reads.borrow(),
        &["main.srp", "a.srp", "common.srp", "b.srp"]
    );
    assert_eq!(graph.file_count(), 4);
    modules::resolve(graph).unwrap();
}

#[test]
fn constant_expansion_is_bounded_before_large_literal_cloning() {
    let mut loader = MemorySourceLoader::new();
    loader
        .insert(
            "main.srp",
            "import \"lib.srp\" as lib; print(lib.text,lib.text);",
        )
        .unwrap();
    loader
        .insert("lib.srp", "pub bind text=\"abcdefgh\";")
        .unwrap();
    modules::compile_with_loader(
        "main.srp",
        &loader,
        ModuleLimits {
            max_expansion_bytes: 16,
            ..ModuleLimits::default()
        },
    )
    .unwrap();
    assert_eq!(
        modules::compile_with_loader(
            "main.srp",
            &loader,
            ModuleLimits {
                max_expansion_bytes: 15,
                ..ModuleLimits::default()
            }
        )
        .unwrap_err()
        .diagnostic
        .kind,
        Kind::ModuleLimit
    );
}

#[test]
fn imported_numerical_calls_share_runtime_payload_and_work_budgets() {
    let program=compile(&[("main.srp","import \"linear.srp\" as linear; print(linear.project([1.0,1.0]));"),
        ("linear.srp","pub pure funcra project(v: vector)->vector ensures len(result)==2; { return matvec(tile([1.0,2.0,3.0,4.0],2,2),v);}")]).unwrap();
    let exact = ExecutionOptions {
        max_vector_bytes: 64,
        ..ExecutionOptions::default()
    };
    program.check(exact).unwrap();
    for (options, kind) in [
        (
            ExecutionOptions {
                max_vector_bytes: 63,
                ..exact
            },
            Kind::VectorBudget,
        ),
        (
            ExecutionOptions {
                max_calls: 0,
                ..exact
            },
            Kind::CallLimit,
        ),
        (
            ExecutionOptions {
                limits: scarp::ExecutionLimits {
                    max_steps: 0,
                    ..exact.limits
                },
                ..exact
            },
            Kind::ExecutionLimit,
        ),
    ] {
        assert_eq!(program.check(options).unwrap_err().diagnostic.kind, kind);
        let (mut out, mut err) = (Vec::new(), Vec::new());
        assert!(matches!(
            program.execute(&mut out, &mut err, options),
            Err(ModuleRunError::Diagnostic(_))
        ));
        assert!(out.is_empty() && err.is_empty());
    }
}

#[test]
fn compiled_modules_are_deterministic_and_reusable_concurrently() {
    let files = [
        ("main.srp", "import \"lib.srp\" as lib; print(lib.f(5));"),
        (
            "lib.srp",
            "pub pure funcra f(x: integer)->integer {return x+2;}",
        ),
    ];
    let first = compile(&files).unwrap();
    let second = compile(&files).unwrap();
    assert_eq!(
        format!("{:?}", first.program()),
        format!("{:?}", second.program())
    );
    let program = std::sync::Arc::new(first);
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let program = program.clone();
            std::thread::spawn(move || {
                for _ in 0..3 {
                    let (mut out, mut err) = (Vec::new(), Vec::new());
                    program
                        .execute(&mut out, &mut err, ExecutionOptions::default())
                        .unwrap();
                    assert_eq!(out, b"7\n");
                    assert!(err.is_empty());
                }
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn every_truncated_module_source_is_panic_free_and_errors_keep_owned_sources() {
    let source="// café 🌍\r\nimport \"math.srp\" as math; pub pure funcra f()->integer{return math.value;} print(f());";
    for end in (0..=source.len()).filter(|&n| source.is_char_boundary(n)) {
        let result = std::panic::catch_unwind(|| {
            compile(&[
                ("main.srp", &source[..end]),
                ("math.srp", "pub bind value=4;"),
            ])
        });
        if let Err(error) = result.unwrap() {
            assert!(error.render().contains("error["));
        }
    }
    let error = compile(&[
        ("main.srp", "import \"lib.srp\" as lib; print(lib.hidden);"),
        ("lib.srp", "// नमस्ते 🌍\r\nbind hidden=3;"),
    ])
    .unwrap_err();
    assert!(error.render().contains("lib.srp:2:"));
    assert_eq!(error.to_string(), error.render());
    let run_error = ModuleRunError::Diagnostic(error.clone());
    assert_eq!(run_error.to_string(), error.render());
    assert!(std::error::Error::source(&error).is_some());
}

#[test]
fn default_file_count_and_depth_boundaries_are_enforced_iteratively() {
    let mut loader = MemorySourceLoader::new();
    let root = (1..128)
        .map(|i| format!("import \"m{i}.srp\" as m{i};"))
        .collect::<String>();
    loader.insert("main.srp", &root).unwrap();
    for i in 1..128 {
        loader.insert(format!("m{i}.srp"), "").unwrap();
    }
    let program =
        modules::compile_with_loader("main.srp", &loader, ModuleLimits::default()).unwrap();
    assert_eq!(program.modules().len(), 128);
    program
        .check(ExecutionOptions {
            limits: scarp::ExecutionLimits {
                max_steps: 0,
                max_prints: 0,
            },
            max_call_depth: 0,
            max_calls: 0,
            max_live_slots: 0,
            max_capture_bytes: 0,
            max_vector_bytes: 0,
        })
        .unwrap();
    assert_eq!(
        modules::compile_with_loader(
            "main.srp",
            &loader,
            ModuleLimits {
                max_files: 127,
                ..ModuleLimits::default()
            }
        )
        .unwrap_err()
        .diagnostic
        .kind,
        Kind::ModuleLimit
    );
    let mut loader = MemorySourceLoader::new();
    loader
        .insert("main.srp", "import \"m1.srp\" as next;")
        .unwrap();
    for i in 1..=32 {
        loader
            .insert(
                format!("m{i}.srp"),
                if i == 32 {
                    String::new()
                } else {
                    format!("import \"m{}.srp\" as next;", i + 1)
                },
            )
            .unwrap();
    }
    modules::compile_with_loader("main.srp", &loader, ModuleLimits::default()).unwrap();
    assert_eq!(
        modules::compile_with_loader(
            "main.srp",
            &loader,
            ModuleLimits {
                max_depth: 31,
                ..ModuleLimits::default()
            }
        )
        .unwrap_err()
        .diagnostic
        .kind,
        Kind::ModuleLimit
    );
}

#[test]
fn loader_byte_allowances_are_rechecked_before_parsing() {
    struct Oversized;
    impl modules::SourceLoader for Oversized {
        fn resolve(&self, _: Option<&str>, _: &str) -> Result<String, modules::SourceLoadError> {
            Ok("root".into())
        }
        fn load(&self, _: &str, _: usize) -> Result<String, modules::SourceLoadError> {
            Ok("NOT VALID SOURCE".into())
        }
    }
    assert_eq!(
        modules::compile_with_loader(
            "main.srp",
            &Oversized,
            ModuleLimits {
                max_total_bytes: 1,
                ..ModuleLimits::default()
            }
        )
        .unwrap_err()
        .diagnostic
        .kind,
        Kind::ModuleLimit
    );
}

#[test]
fn loading_is_separate_from_linking_and_typed_metadata_is_inspectable() {
    let mut loader = MemorySourceLoader::new();
    loader
        .insert("main.srp", "import \"lib.srp\" as lib;")
        .unwrap();
    loader
        .insert("lib.srp", "print(\"must not run during discovery\");")
        .unwrap();
    let graph = modules::load("main.srp", &loader, ModuleLimits::default()).unwrap();
    assert_eq!(graph.file_count(), 2);
    assert_eq!(
        modules::resolve(graph).unwrap_err().diagnostic.kind,
        Kind::ModuleSyntax
    );
    let program = compile(&[
        ("main.srp", "import \"lib.srp\" as lib;"),
        (
            "lib.srp",
            "pub bind amount=3; pub pure funcra flat(t: tile)->vector {return unroll(t);}",
        ),
    ])
    .unwrap();
    assert_eq!(
        program.modules()[1].symbols[0].kind,
        modules::SymbolKind::Constant(scarp::value::ValueType::Integer)
    );
    assert!(program.modules()[1].symbols[0].public);
    assert_eq!(
        program.modules()[1].symbols[1].kind,
        modules::SymbolKind::Function {
            parameters: vec![scarp::value::ValueType::Tile],
            result: scarp::value::ValueType::Vector,
            pure: true
        }
    );
}
