use scarp::{
    diagnostics::{location, DiagnosticKind as Kind},
    RunError,
};
use std::{fs, path::Path, process::Command};

#[test]
fn numerical_diagnostics_match_reviewed_fixtures_in_library_and_cli() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/numeric");
    for (name, kind) in [
        ("dimension", Kind::VectorLength),
        ("index", Kind::VectorIndex),
        ("shape", Kind::VectorLength),
        ("type", Kind::TypeMismatch),
        ("nested", Kind::VectorIndex),
        ("literal", Kind::InvalidFloat),
    ] {
        let source = fs::read_to_string(root.join(format!("{name}.srp"))).unwrap();
        let error = scarp::check(&source).unwrap_err();
        assert_eq!(error.kind, kind);
        let expected = fs::read_to_string(root.join(format!("{name}.stderr"))).unwrap();
        assert_eq!(
            format!("{}\n", error.render("fixture.srp", &source)),
            expected,
            "{name}"
        );
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let RunError::Diagnostic(actual) =
            scarp::run_with_io(&source, &mut out, &mut err).unwrap_err()
        else {
            panic!("diagnostic expected")
        };
        assert_eq!(actual, error);
        assert!(out.is_empty() && err.is_empty());
        for command in ["run", "check"] {
            let process = Command::new(env!("CARGO_BIN_EXE_scarp"))
                .current_dir(&root)
                .args([command, &format!("{name}.srp")])
                .output()
                .unwrap();
            assert_eq!(process.status.code(), Some(1));
            assert!(process.stdout.is_empty());
            assert_eq!(
                String::from_utf8(process.stderr).unwrap(),
                expected.replace("fixture.srp", &format!("{name}.srp"))
            );
        }
    }
}

#[test]
fn tile_argument_spans_keep_utf8_and_crlf_locations() {
    let source = "// café 🌍\r\nbind t = tile([1.0], 1, 1); print(cell(t, 0, -1));\r\n";
    let error = scarp::check(source).unwrap_err();
    assert_eq!(error.kind, Kind::VectorIndex);
    assert_eq!(&source[error.span.start..error.span.end], "-1");
    assert_eq!(location(source, error.span.start).line, 2);
    assert!(error
        .render("unicode.srp", source)
        .contains("tile column index -1"));
    for (call, invalid) in [
        ("tile([], 0, 65537)", "65537"),
        ("tile([], -1, 0)", "-1"),
        ("lane(tile([], 2, 0), 2)", "2"),
    ] {
        let source = format!("// café 🌍\r\nprint({call});");
        let error = scarp::check(&source).unwrap_err();
        assert_eq!(&source[error.span.start..error.span.end], invalid);
        assert_eq!(location(&source, error.span.start).line, 2);
    }
}
