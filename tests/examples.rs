use std::{fs, path::Path};

#[test]
fn every_shipped_example_matches_its_expected_output() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
    let mut paths: Vec<_> = fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "srp"))
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "the example corpus must not be empty");
    for path in &paths {
        let source = fs::read_to_string(path).unwrap();
        let expected_path = path.with_extension("stdout");
        let expected = fs::read(&expected_path)
            .unwrap_or_else(|error| panic!("{}: {error}", expected_path.display()));
        let mut actual = Vec::new();
        scarp::run(&source, &mut actual)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert_eq!(actual, expected, "{}", path.display());
    }
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path
            .extension()
            .is_some_and(|extension| extension == "stdout")
        {
            assert!(
                paths.contains(&path.with_extension("srp")),
                "orphaned expected-output file: {}",
                path.display()
            );
        }
    }
}
