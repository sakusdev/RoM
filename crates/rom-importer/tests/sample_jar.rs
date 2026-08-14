use rom_importer::{ImportOptions, inspect_jar};

#[test]
fn imports_the_bundled_sample_jar() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/sample.jar");
    let report = inspect_jar(path, &ImportOptions::default()).expect("sample JAR should parse");

    assert_eq!(report.summary.classes_parsed, 2);
    assert!(
        report.errors.is_empty(),
        "unexpected errors: {:?}",
        report.errors
    );
    assert!(
        report
            .classes
            .iter()
            .any(|class| class.internal_name == "example/Sample")
    );
    assert!(
        report
            .classes
            .iter()
            .any(|class| class.internal_name == "example/Sample$Mode")
    );
}
