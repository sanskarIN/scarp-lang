use scarp::{diagnostics::DiagnosticKind as Kind, ExecutionLimits, RunError};
use std::path::Path;

#[test]
fn exact_loop_diagnostics_keep_codes_locations_and_actionable_help() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/iteration");
    for (name, kind, steps, prints) in [
        ("outside", Kind::LoopControl, 100, 10),
        ("guard", Kind::TypeMismatch, 100, 10),
        ("count", Kind::TypeMismatch, 100, 10),
        ("negative", Kind::RepeatCount, 100, 10),
        ("index", Kind::ImmutableAssignment, 100, 10),
        ("missing", Kind::UnexpectedToken, 100, 10),
        ("braces", Kind::UnexpectedToken, 100, 10),
        ("steps", Kind::ExecutionLimit, 5, 10),
        ("prints", Kind::OutputLimit, 100, 1),
    ] {
        let filename = format!("{name}.srp");
        let source = std::fs::read_to_string(root.join(&filename)).unwrap();
        let limits = ExecutionLimits {
            max_steps: steps,
            max_prints: prints,
        };
        let error = scarp::check_with_limits(&source, limits).unwrap_err();
        assert_eq!(error.kind, kind);
        assert!(error.help.is_some());
        let rendered = format!("{}\n", error.render(&filename, &source));
        assert_eq!(
            rendered.as_bytes(),
            std::fs::read(root.join(format!("{name}.stderr"))).unwrap(),
            "{name}"
        );
        let mut out = Vec::new();
        let RunError::Diagnostic(actual) =
            scarp::run_with_limits(&source, &mut out, limits).unwrap_err()
        else {
            panic!()
        };
        assert!(out.is_empty());
        assert_eq!(actual, error);
        for command in ["run", "check"] {
            let process = std::process::Command::new(env!("CARGO_BIN_EXE_scarp"))
                .current_dir(&root)
                .args([
                    command,
                    "--max-steps",
                    &steps.to_string(),
                    "--max-prints",
                    &prints.to_string(),
                    &filename,
                ])
                .output()
                .unwrap();
            assert_eq!(process.status.code(), Some(1));
            assert!(process.stdout.is_empty());
            assert_eq!(process.stderr, rendered.as_bytes(), "{name} {command}");
        }
    }
}
