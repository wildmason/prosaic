use prosaic_core::{Context, Session, Value};
use prosaic_project::Project;
use std::path::Path;

#[test]
fn into_engine_renders_template() {
    let p = Project::load_from_dir(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/multi-variant"),
    )
    .unwrap();

    let engine = p.into_engine().unwrap();
    let mut ctx = Context::new();
    ctx.insert("name", Value::String("Foo".into()));
    ctx.insert("consumer_count", Value::Number(3));

    let mut session = Session::new();
    let out = engine.render(&mut session, "code.modified", &ctx).unwrap();
    assert!(out.contains("Foo"));
    assert!(out.contains("3"));
}

#[test]
fn into_engine_blank_project_succeeds() {
    let p = Project::load_from_dir(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/blank-project"),
    )
    .unwrap();
    let engine = p.into_engine().unwrap();
    drop(engine);
}

#[test]
fn into_engine_applies_style_preference() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir(tmp.path().join("templates")).unwrap();
    std::fs::write(
        tmp.path().join("prosaic.toml"),
        r#"
            name = "styled"
            version = "0.1.0"
            language = "en"

            [engine]
            style = "executive"
        "#,
    )
    .unwrap();
    std::fs::write(
        tmp.path().join("templates").join("event.toml"),
        r#"
            key = "event"

            [[variants]]
            body = "technical {name}"

            [[variants]]
            style = "executive"
            body = "executive {name}"
        "#,
    )
    .unwrap();

    let p = Project::load_from_dir(tmp.path()).unwrap();
    let engine = p.into_engine().unwrap();
    let mut ctx = Context::new();
    ctx.insert("name", Value::String("summary".into()));

    let out = engine.render(&mut Session::new(), "event", &ctx).unwrap();
    assert_eq!(out, "executive summary");
}
