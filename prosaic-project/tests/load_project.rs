use prosaic_project::Project;
use std::path::Path;

#[test]
fn load_blank_project() {
    let p = Project::load_from_dir(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/blank-project"),
    )
    .unwrap();
    assert_eq!(p.manifest.name, "blank");
    assert_eq!(p.templates.len(), 0);
    assert_eq!(p.partials.len(), 0);
    assert_eq!(p.fixtures.len(), 0);
    assert_eq!(p.scenarios.len(), 0);
}

#[test]
fn load_multi_variant_project() {
    let p = Project::load_from_dir(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/multi-variant"),
    )
    .unwrap();
    assert_eq!(p.templates.len(), 1);
    let t = p.templates.get("code.modified").unwrap();
    assert_eq!(t.variants.len(), 3);

    assert_eq!(p.partials.len(), 1);
    assert!(p.partials.contains_key("impact_tail"));

    assert_eq!(p.fixtures.len(), 1);
    assert!(p.fixtures.contains_key("userservice"));

    assert_eq!(p.scenarios.len(), 1);
    assert_eq!(p.scenarios.get("smoke").unwrap().events.len(), 1);
}

#[test]
fn missing_manifest_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let res = Project::load_from_dir(tmp.path());
    assert!(res.is_err());
}
