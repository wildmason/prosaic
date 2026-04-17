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
