use scarp::diagnostics::DiagnosticKind as Kind;
use scarp::{
    ast::Statement,
    modules::{self, MemorySourceLoader, ModuleLimits},
    value::ValueType,
};

fn run(source: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    scarp::run(source, &mut bytes).unwrap();
    bytes
}

#[test]
fn all_primitive_value_kinds_can_be_annotated() {
    assert_eq!(run("bind n: integer=2; bind f: float=1.5; bind yes: boolean=n<3; bind text: string=\"café\"; bind data: vector=[f]; bind grid: tile=tile(data,1,1); print(n,f,yes,text,len(data),cell(grid,0,0));"),"2 1.5 true café 1 1.5\n".as_bytes());
}

#[test]
fn annotation_nodes_keep_exact_unicode_crlf_type_spans_and_old_nodes_have_none() {
    let source = "// नमस्ते 🌍\r\nbind text: string=\"café\"; var count=2;";
    let parsed = scarp::parse(source).unwrap();
    let Statement::Declaration {
        annotation: Some(annotation),
        ..
    } = parsed.statements[0]
    else {
        panic!("annotation")
    };
    assert_eq!(annotation.kind, ValueType::String);
    assert_eq!(
        &source[annotation.span.start..annotation.span.end],
        "string"
    );
    assert_eq!(
        scarp::diagnostics::location(source, annotation.span.start),
        scarp::diagnostics::Location {
            line: 2,
            column: 12
        }
    );
    assert!(matches!(
        parsed.statements[1],
        Statement::Declaration {
            annotation: None,
            ..
        }
    ));
}

#[test]
fn unknown_unit_and_malformed_binding_types_have_actionable_errors() {
    for source in [
        "bind n: unit=1;",
        "var n: number=1;",
        "bind n:=1;",
        "bind n: integer;",
        "bind n: integer=;",
        "bind n::integer=1;",
    ] {
        let error = scarp::parse(source).unwrap_err();
        assert_eq!(error.kind, Kind::UnexpectedToken, "{source}");
        assert!(error.help.is_some(), "{source}");
    }
    assert_eq!(
        scarp::parse("bind f: funcra=1;").unwrap_err().kind,
        Kind::FunctionUnsupported
    );
}

#[test]
fn inferred_and_annotated_types_do_not_coerce_or_change_assignment_rules() {
    for source in [
        "bind n: float=1;",
        "bind n: integer=1.0;",
        "bind n: boolean=0;",
        "bind n: vector=1.0;",
        "var n: integer=1; n=2.0;",
    ] {
        assert_eq!(
            scarp::compile(source).unwrap_err().kind,
            Kind::TypeMismatch,
            "{source}"
        );
    }
    assert_eq!(
        scarp::compile("bind n: integer=1; n=2;").unwrap_err().kind,
        Kind::ImmutableAssignment
    );
    let error = scarp::compile("bind n: float=(1+2);").unwrap_err();
    assert_eq!(error.related.len(), 1);
    assert_eq!(
        error.related[0].message,
        "the binding's type is declared here"
    );
}

#[test]
fn typed_shadowing_initializes_from_the_outer_binding_and_self_use_is_rejected() {
    assert_eq!(
        run("bind n: integer=4; {bind n: integer=n+1; print(n);} print(n);"),
        b"5\n4\n"
    );
    assert_eq!(
        scarp::compile("bind n: integer=n;").unwrap_err().kind,
        Kind::UnknownName
    );
    assert_eq!(
        scarp::compile("bind n: integer=1; var n: integer=2;")
            .unwrap_err()
            .kind,
        Kind::DuplicateBinding
    );
}

#[test]
fn typed_locals_work_in_functions_loops_defaults_contracts_and_early_returns() {
    let source="pure funcra total(n: integer=3)->integer ensures result>=0; {var sum: integer=0; repeat n as index {bind step: integer=index+1; sum+=step;} return sum;} bind result: integer=total(); print(result);";
    assert_eq!(run(source), b"6\n");
    assert_eq!(
        scarp::compile("funcra f()->integer {bind value: boolean=true; return value;}")
            .unwrap_err()
            .kind,
        Kind::InvalidReturn
    );
}

#[test]
fn annotations_are_statically_checked_before_selected_arithmetic() {
    assert_eq!(
        scarp::compile("bind value: float=1/0;").unwrap_err().kind,
        Kind::TypeMismatch
    );
    let mut loader = MemorySourceLoader::new();
    loader
        .insert("main.srp", "pub bind value: float=1/0;")
        .unwrap();
    let error =
        modules::compile_with_loader("main.srp", &loader, ModuleLimits::default()).unwrap_err();
    assert_eq!(error.diagnostic.kind, Kind::TypeMismatch);
    assert_eq!(
        error.diagnostic.related[0].span,
        scarp::diagnostics::Span::new(16, 21)
    );
}

#[test]
fn module_constants_and_imported_functions_keep_annotation_source_ownership() {
    let mut loader = MemorySourceLoader::new();
    loader
        .insert(
            "main.srp",
            "import \"lib.srp\" as lib; bind value: integer=lib.answer; print(value,lib.get());",
        )
        .unwrap();
    loader.insert("lib.srp","bind base: integer=6; pub bind answer: integer=base*7; pub pure funcra get()->integer {bind value: integer=answer; return value;}").unwrap();
    let program =
        modules::compile_with_loader("main.srp", &loader, ModuleLimits::default()).unwrap();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    program
        .execute(&mut out, &mut err, scarp::ExecutionOptions::default())
        .unwrap();
    assert_eq!(out, b"42 42\n");
    assert!(err.is_empty());
    let mut loader = MemorySourceLoader::new();
    loader
        .insert("main.srp", "import \"lib.srp\" as lib;")
        .unwrap();
    loader
        .insert("lib.srp", "// café 🌍\r\npub bind answer: float=6*7;")
        .unwrap();
    let error =
        modules::compile_with_loader("main.srp", &loader, ModuleLimits::default()).unwrap_err();
    assert_eq!(error.diagnostic.kind, Kind::TypeMismatch);
    let annotation = error
        .sources
        .locate_span(error.diagnostic.related[0].span)
        .unwrap();
    assert_eq!(
        (
            annotation.source,
            annotation.start.line,
            annotation.start.column
        ),
        (1, 2, 18)
    );
}

#[test]
fn exact_annotation_fixtures_match_library_and_all_compiling_cli_modes() {
    let directory =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/annotations");
    for name in ["mismatch", "unknown"] {
        let filename = format!("{name}.srp");
        let source = std::fs::read_to_string(directory.join(&filename)).unwrap();
        let mut loader = MemorySourceLoader::new();
        loader.insert(&filename, &source).unwrap();
        let error =
            modules::compile_with_loader(&filename, &loader, ModuleLimits::default()).unwrap_err();
        let expected = std::fs::read_to_string(directory.join(format!("{name}.stderr"))).unwrap();
        assert_eq!(format!("{}\n", error.render()), expected);
        for mode in ["run", "check", "modules"] {
            let result = std::process::Command::new(env!("CARGO_BIN_EXE_scarp"))
                .current_dir(&directory)
                .args([mode, &filename])
                .output()
                .unwrap();
            assert_eq!(result.status.code(), Some(1));
            assert!(result.stdout.is_empty());
            assert_eq!(String::from_utf8(result.stderr).unwrap(), expected);
        }
    }
}

#[test]
fn explicit_annotations_preserve_inference_and_mutation() {
    let mut output = Vec::new();
    scarp::run(
        "bind base: integer=6; var value: integer=base*7; value+=1; print(value);",
        &mut output,
    )
    .unwrap();
    assert_eq!(output, b"43\n");
}

#[test]
fn incompatible_annotations_are_type_errors_before_output() {
    let mut output = Vec::new();
    let scarp::RunError::Diagnostic(error) =
        scarp::run("print(99); bind value: float=1;", &mut output).unwrap_err()
    else {
        panic!("expected diagnostic")
    };
    assert_eq!(error.kind, Kind::TypeMismatch);
    assert!(output.is_empty());
}
