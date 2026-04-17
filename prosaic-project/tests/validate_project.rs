use prosaic_project::{Project, ValidationLevel};
use std::path::Path;

#[test]
fn validate_clean_project_has_no_issues() {
    let p = Project::load_from_dir(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/multi-variant"),
    )
    .unwrap();
    let issues = p.validate();
    let errors: Vec<_> = issues
        .iter()
        .filter(|i| i.level == ValidationLevel::Error)
        .collect();
    assert!(
        errors.is_empty(),
        "expected no validation errors, got: {errors:?}"
    );
}

#[test]
fn validate_unknown_pipe_reports_error() {
    let p = Project::load_from_dir(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/invalid-projects/unknown-pipe"),
    )
    .unwrap();
    let issues = p.validate();
    let errors: Vec<_> = issues
        .iter()
        .filter(|i| i.level == ValidationLevel::Error)
        .collect();
    assert!(!errors.is_empty());
    assert!(errors.iter().any(|i| i.message.contains("nonexistent_pipe")));
}

#[test]
fn validate_unknown_partial_reports_error() {
    let p = Project::load_from_dir(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/invalid-projects/unknown-partial"),
    )
    .unwrap();
    let issues = p.validate();
    let errors: Vec<_> = issues
        .iter()
        .filter(|i| i.level == ValidationLevel::Error)
        .collect();
    assert!(
        errors
            .iter()
            .any(|i| i.message.contains("nonexistent_partial"))
    );
}
