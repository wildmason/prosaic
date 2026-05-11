use prosaic_core::{Context, Session, Value};
use prosaic_project::{BuildTarget, Project, Starter, build_bundle, scaffold_project};
use tempfile::tempdir;

#[test]
fn project_schema_bundle_v1_and_runner_contract_are_stable() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("contract");
    scaffold_project("contract", &root, Starter::VocabPack).unwrap();

    let project = Project::load_from_dir(&root).unwrap();
    assert_eq!(project.manifest.name, "contract");
    assert_eq!(project.manifest.language, "en");
    assert!(project.validate().is_empty());

    let engine = project.into_engine().unwrap();
    let mut ctx = Context::new();
    ctx.insert("entity_type", Value::String("class".into()));
    ctx.insert("name", Value::String("UserService".into()));
    ctx.insert("consumer_count", Value::Number(6));

    let rendered = engine
        .render(&mut Session::new(), "code.modified", &ctx)
        .unwrap();
    assert_eq!(
        rendered,
        "The class UserService was modified, affecting 6 consumers."
    );

    let bundle = build_bundle(&project, BuildTarget::Both).unwrap();
    let json = bundle.json.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["schema_version"], 1);
    assert_eq!(parsed["name"], "contract");
    assert_eq!(parsed["language"], "en");
    assert_eq!(parsed["templates"][0]["key"], "code.modified");
    assert!(
        parsed["partials"][0]["body"]
            .as_str()
            .unwrap()
            .contains("affecting")
    );

    let rust = bundle.rust.unwrap();
    assert!(rust.contains("// schema_version = 1"));
    assert!(rust.contains("pub fn register(engine: &mut Engine)"));
    assert!(rust.contains("engine.register_partial(\"impact_tail\""));
}

#[test]
fn validator_uses_the_core_pipe_registry() {
    let temp = tempdir().unwrap();
    let root = temp.path();
    std::fs::create_dir(root.join("templates")).unwrap();
    std::fs::write(
        root.join("prosaic.toml"),
        "name = \"pipe-contract\"\nversion = \"0.1.0\"\nlanguage = \"en\"\n",
    )
    .unwrap();
    std::fs::write(
        root.join("templates").join("owner.toml"),
        r#"
key = "owner"

[[variants]]
body = "{name|possessive} consumers need review"
"#,
    )
    .unwrap();

    let project = Project::load_from_dir(root).unwrap();
    assert!(
        project.validate().is_empty(),
        "project validation must accept every pipe registered by prosaic-core"
    );
}
