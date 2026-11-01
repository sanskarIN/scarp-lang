use scarp::{
    diagnostics::{DiagnosticKind as Kind, Span},
    modules::{self, MemorySourceLoader, ModuleLimits, SourceLoadError, SourceLoader},
};
use std::cell::RefCell;

fn loader(files: &[(&str, &str)]) -> MemorySourceLoader {
    let mut loader = MemorySourceLoader::new();
    for (path, source) in files {
        loader.insert(path, *source).unwrap();
    }
    loader
}

fn compile(files: &[(&str, &str)]) -> modules::ModuleProgram {
    modules::compile_with_loader("main.srp", &loader(files), ModuleLimits::default()).unwrap()
}

#[test]
fn import_metadata_has_exact_owning_file_and_utf8_crlf_locations() {
    let source = "// नमस्ते 🌍\r\n import \"lib/café.srp\" as math;\r\nprint(math.value);";
    let program = compile(&[("main.srp", source), ("lib/café.srp", "pub bind value=3;")]);
    let import = &program.imports()[0];
    assert_eq!((import.source, import.target), (0, 1));
    assert_eq!((&*import.alias, &*import.path), ("math", "lib/café.srp"));
    for (span, expected) in [
        (import.span, "import \"lib/café.srp\" as math;"),
        (import.path_span, "\"lib/café.srp\""),
        (import.alias_span, "math"),
    ] {
        let at = program.sources().locate_span(span).unwrap();
        assert_eq!(at.source, 0);
        assert_eq!(&source[at.span.start..at.span.end], expected);
    }
    let at = program.sources().locate_span(import.path_span).unwrap();
    assert_eq!((at.start.line, at.start.column), (2, 9));
}

#[test]
fn source_location_queries_validate_membership_boundaries_and_eof() {
    let program = compile(&[
        ("main.srp", "// café\nimport \"empty.srp\" as empty;"),
        ("empty.srp", ""),
    ]);
    let sources = program.sources();
    for (id, file) in sources.files().iter().enumerate() {
        let eof = file.range().end;
        let at = sources.locate_span(Span::new(eof, eof)).unwrap();
        assert_eq!(at.source, id);
        assert_eq!(at.span.start, sources.source(id).unwrap().len());
    }
    let first = sources.files()[0].range();
    for span in [
        Span::new(7, 8),
        Span::new(2, 1),
        Span::new(first.end, first.end + 1),
        Span::new(0, sources.files()[1].range().end),
        Span::new(usize::MAX, usize::MAX),
    ] {
        assert!(sources.locate_span(span).is_none(), "{span:?}");
    }
    assert!(sources.source(usize::MAX).is_none());
}

#[test]
fn borrowed_function_details_keep_original_names_parameters_defaults_and_contracts() {
    let program = compile(&[("main.srp", "import \"lib.srp\" as lib; funcra local()->unit {}"),
        ("lib.srp", "pub bind value=7; pub pure funcra adjust(value: integer, step: integer=value)->integer needs step>=0; ensures result>=value; {return helper(value)+step;} pure funcra helper(n: integer)->integer {return n;}")]);
    let functions: Vec<_> = program.functions_in(1).unwrap().collect();
    assert_eq!(functions.len(), 2);
    let (symbol, info) = functions[0];
    assert_eq!(symbol.name, "adjust");
    assert!(symbol.public && info.pure);
    assert_eq!(
        info.parameters
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>(),
        ["value", "step"]
    );
    assert!(info.parameters[0].default.is_none());
    assert_eq!(
        program
            .sources()
            .locate_span(info.parameters[1].default.unwrap())
            .unwrap()
            .source,
        1
    );
    assert_eq!(info.contracts.len(), 2);
    assert_eq!(functions[1].0.name, "helper");
    assert!(!functions[1].0.public);
    assert_eq!(
        program.functions_in(0).unwrap().next().unwrap().0.name,
        "local"
    );
    assert!(program.functions_in(2).is_none());
}

#[test]
fn diamond_dependency_queries_deduplicate_edges_and_preserve_defined_order() {
    let program = compile(&[("main.srp", "import \"a.srp\" as a; import \"b.srp\" as b; import \"common.srp\" as c; import \"./a.srp\" as again;"),
        ("a.srp", "import \"common.srp\" as c;"), ("b.srp", "import \"common.srp\" as c;"), ("common.srp", "pub bind value=1;")]);
    assert_eq!(program.imports().len(), 6);
    assert_eq!(program.imports_in(0).unwrap().len(), 4);
    assert_eq!(program.dependencies(0).unwrap(), &[1, 3, 2]);
    assert_eq!(program.dependents(2).unwrap(), &[0, 1, 3]);
    assert_eq!(program.transitive_dependencies(0).unwrap(), &[1, 2, 3]);
    assert_eq!(program.transitive_dependencies(1).unwrap(), &[2]);
    assert_eq!(program.transitive_dependents(2).unwrap(), &[0, 1, 3]);
    assert!(program.dependencies(2).unwrap().is_empty());
    assert!(program.functions_in(2).unwrap().next().is_none());
    assert!(program.dependencies(usize::MAX).is_none());
    assert!(program.dependents(usize::MAX).is_none());
    assert!(program.transitive_dependencies(usize::MAX).is_none());
    assert!(program.transitive_dependents(usize::MAX).is_none());
    assert!(program.imports_in(usize::MAX).is_none());
}

#[test]
fn maximum_depth_queries_and_inspection_are_reusable_across_threads() {
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
    let program = std::sync::Arc::new(
        modules::compile_with_loader("main.srp", &loader, ModuleLimits::default()).unwrap(),
    );
    let mut expected = Vec::new();
    program.write_inspection(&mut expected).unwrap();
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let program = program.clone();
            let expected = expected.clone();
            std::thread::spawn(move || {
                assert_eq!(
                    program.transitive_dependencies(0).unwrap(),
                    (1..=32).collect::<Vec<_>>()
                );
                assert_eq!(
                    program.transitive_dependents(32).unwrap(),
                    (0..32).collect::<Vec<_>>()
                );
                let mut actual = Vec::new();
                program.write_inspection(&mut actual).unwrap();
                assert_eq!(actual, expected);
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn inspection_never_executes_defaults_contracts_or_statements_or_prints_literals() {
    let program = compile(&[("main.srp", "import \"lib.srp\" as lib; print(\"PROGRAM OUTPUT\"); print(lib.f()); while true {continue;}"),
        ("lib.srp", "pub bind secret=\"hidden literal\"; pub pure funcra f(n: integer=1/0)->integer needs false; {return n;}")]);
    let mut report = Vec::new();
    program.write_inspection(&mut report).unwrap();
    let report = String::from_utf8(report).unwrap();
    assert!(report.contains("pub pure funcra f(n: integer = <default>) -> integer"));
    assert!(report.contains("pub bind secret: string"));
    assert!(
        !report.contains("PROGRAM OUTPUT")
            && !report.contains("hidden literal")
            && !report.contains("1/0")
    );
}

#[test]
fn inspection_writer_failures_keep_the_original_io_error() {
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
    assert_eq!(
        compile(&[("main.srp", "")])
            .write_inspection(&mut Closed)
            .unwrap_err()
            .kind(),
        io::ErrorKind::BrokenPipe
    );
}

#[test]
fn import_limit_diagnostics_point_to_the_first_exceeding_path() {
    let source = "import \"a.srp\" as a;\nimport \"b.srp\" as b;";
    let loader = loader(&[("main.srp", source)]);
    let error = modules::load(
        "main.srp",
        &loader,
        ModuleLimits {
            max_imports: 1,
            ..ModuleLimits::default()
        },
    )
    .unwrap_err();
    assert_eq!(error.diagnostic.kind, Kind::ModuleLimit);
    let at = error.sources.locate_span(error.diagnostic.span).unwrap();
    assert_eq!((at.start.line, at.start.column), (2, 8));
    assert_eq!(&source[at.span.start..at.span.end], "\"b.srp\"");
}

#[test]
fn spelling_repairs_are_deterministic_and_never_suggest_private_members() {
    for library in [
        "pub bind cast=1; pub bind cost=2; bind secret=3;",
        "pub bind cost=2; pub bind cast=1; bind secret=3;",
    ] {
        for (name, expected) in [
            ("math.cist", Some("math.cast")),
            ("mat.cast", Some("alias \"math\"")),
            ("math.secrex", None),
        ] {
            let source = format!("import \"lib.srp\" as math; print({name});");
            let error = modules::compile_with_loader(
                "main.srp",
                &loader(&[("main.srp", &source), ("lib.srp", library)]),
                ModuleLimits::default(),
            )
            .unwrap_err();
            assert_eq!(error.diagnostic.kind, Kind::ModuleSymbol);
            let help = error.diagnostic.help.unwrap();
            assert!(!help.contains("secret"));
            if let Some(expected) = expected {
                assert!(help.contains(expected), "{help}");
            } else {
                assert!(!help.contains("did you mean"));
            }
        }
    }
    let long = "x".repeat(65);
    let source = format!("import \"lib.srp\" as lib; print(lib.{long});");
    let error = modules::compile_with_loader(
        "main.srp",
        &loader(&[("main.srp", &source), ("lib.srp", "pub bind x=1;")]),
        ModuleLimits::default(),
    )
    .unwrap_err();
    assert!(!error.diagnostic.help.unwrap().contains("did you mean"));
}

#[test]
fn self_import_cycles_have_only_the_closing_edge() {
    let error = modules::load(
        "main.srp",
        &loader(&[("main.srp", "\nimport \"main.srp\" as me;")]),
        ModuleLimits::default(),
    )
    .unwrap_err();
    assert_eq!(error.diagnostic.kind, Kind::ModuleCycle);
    assert!(error.diagnostic.related.is_empty());
    assert!(error.render().contains("main.srp:2:8"));
}

#[test]
fn shipped_inspection_example_has_reviewed_static_inventory_and_normal_output() {
    let mut loader = MemorySourceLoader::new();
    loader
        .insert(
            "module_inspection.srp",
            include_str!("../examples/module_inspection.srp"),
        )
        .unwrap();
    loader
        .insert(
            "modules/inspection.srp",
            include_str!("../examples/modules/inspection.srp"),
        )
        .unwrap();
    let program =
        modules::compile_with_loader("module_inspection.srp", &loader, ModuleLimits::default())
            .unwrap();
    let mut inventory = Vec::new();
    program.write_inspection(&mut inventory).unwrap();
    assert_eq!(
        inventory,
        include_bytes!("../examples/module_inspection.modules")
    );
    let (mut out, mut err) = (Vec::new(), Vec::new());
    program
        .execute(&mut out, &mut err, scarp::ExecutionOptions::default())
        .unwrap();
    assert_eq!(out, include_bytes!("../examples/module_inspection.stdout"));
    assert!(err.is_empty());
}

#[test]
fn inspection_escapes_custom_loader_labels_instead_of_injecting_terminal_controls() {
    struct Labels(MemorySourceLoader);
    impl SourceLoader for Labels {
        fn resolve(&self, importer: Option<&str>, path: &str) -> Result<String, SourceLoadError> {
            self.0.resolve(importer, path)
        }
        fn load(&self, id: &str, cap: usize) -> Result<String, SourceLoadError> {
            self.0.load(id, cap)
        }
        fn display_name(&self, _: &str) -> String {
            "bad\n\u{1b}label.srp".into()
        }
    }
    let loader = Labels(loader(&[
        ("main.srp", "import \"lib.srp\" as lib;"),
        ("lib.srp", ""),
    ]));
    let program =
        modules::compile_with_loader("main.srp", &loader, ModuleLimits::default()).unwrap();
    let mut text = Vec::new();
    program.write_inspection(&mut text).unwrap();
    let text = String::from_utf8(text).unwrap();
    assert!(
        !text.contains('\u{1b}') && text.contains("bad\\n\\u{1b}label.srp"),
        "{text}"
    );
}

#[test]
fn invalid_import_lists_are_rejected_before_any_dependency_callback() {
    struct Counted {
        inner: MemorySourceLoader,
        calls: RefCell<Vec<String>>,
    }
    impl SourceLoader for Counted {
        fn resolve(&self, importer: Option<&str>, path: &str) -> Result<String, SourceLoadError> {
            self.calls.borrow_mut().push(format!("resolve:{path}"));
            self.inner.resolve(importer, path)
        }
        fn load(&self, id: &str, bytes: usize) -> Result<String, SourceLoadError> {
            self.calls.borrow_mut().push(format!("read:{id}"));
            self.inner.load(id, bytes)
        }
    }
    for (source, kind) in [
        (
            "import \"lib.srp\" as lib; import \"missing.srp\" as lib;",
            Kind::DuplicateBinding,
        ),
        (
            "import \"lib.srp\" as lib; import \"missing.srp\" as print;",
            Kind::ReservedName,
        ),
        (
            "import \"lib.srp\" as lib; import \"/outside.srp\" as outside;",
            Kind::ModulePath,
        ),
        (
            "import \"missing.srp\" as item; bind item=1;",
            Kind::DuplicateBinding,
        ),
    ] {
        let counted = Counted {
            inner: loader(&[("main.srp", source), ("lib.srp", "pub bind value=1;")]),
            calls: RefCell::new(Vec::new()),
        };
        let error = modules::load("main.srp", &counted, ModuleLimits::default()).unwrap_err();
        assert_eq!(error.diagnostic.kind, kind, "{source}");
        assert_eq!(
            &*counted.calls.borrow(),
            &["resolve:main.srp", "read:main.srp"],
            "{source}"
        );
    }
}

#[test]
fn cycle_notes_exclude_acyclic_prefixes_and_point_at_actual_imports() {
    let loader = loader(&[
        ("main.srp", "import \"prefix.srp\" as prefix;"),
        ("prefix.srp", "import \"a.srp\" as a;"),
        ("a.srp", "// first library\n\nimport \"b.srp\" as b;"),
        ("b.srp", "// second library\nimport \"a.srp\" as a;"),
    ]);
    let error = modules::load("main.srp", &loader, ModuleLimits::default()).unwrap_err();
    assert_eq!(error.diagnostic.kind, Kind::ModuleCycle);
    let rendered = error.render();
    assert!(rendered.contains("b.srp:2:8"), "{rendered}");
    assert!(rendered.contains("a.srp:3:8"), "{rendered}");
    assert!(!rendered.contains("prefix.srp:"), "{rendered}");
    assert_eq!(error.diagnostic.related.len(), 1);
}
