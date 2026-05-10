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
fn into_engine_applies_inline_style_profile() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir(tmp.path().join("templates")).unwrap();
    std::fs::write(
        tmp.path().join("prosaic.toml"),
        r#"
            name = "profile-demo"
            version = "0.1.0"
            language = "en"

            [style_profile]
            name = "verbose"
            verbosity = "verbose"
        "#,
    )
    .unwrap();
    std::fs::write(
        tmp.path().join("templates").join("event.toml"),
        r#"
            key = "event"

            [[variants]]
            salience = "low"
            body = "TERSE: {name}"

            [[variants]]
            salience = "medium"
            body = "MEDIUM: {name} was modified"

            [[variants]]
            salience = "high"
            body = "VERBOSE: {name} was modified across consumers"
        "#,
    )
    .unwrap();

    let p = Project::load_from_dir(tmp.path()).unwrap();
    let engine = p.into_engine().unwrap();
    assert_eq!(engine.current_style_profile().name, "verbose");

    let mut ctx = Context::new();
    ctx.insert("name", Value::String("UserService".into()));
    ctx.insert("entity_type", Value::String("class".into()));
    // consumer_count = 5 lands in Medium; verbosity=verbose shifts to High.
    ctx.insert("consumer_count", Value::Number(5));
    let out = engine.render(&mut Session::new(), "event", &ctx).unwrap();
    assert!(
        out.starts_with("VERBOSE:"),
        "verbose-profile project should pick the High-tier variant; got {out:?}"
    );
}

#[test]
fn into_engine_applies_style_profile_from_extends() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir(tmp.path().join("templates")).unwrap();
    std::fs::create_dir(tmp.path().join("profiles")).unwrap();
    std::fs::write(
        tmp.path().join("profiles").join("terse.toml"),
        r#"
            name = "terse-base"
            verbosity = "terse"
        "#,
    )
    .unwrap();
    std::fs::write(
        tmp.path().join("prosaic.toml"),
        r#"
            name = "extends-demo"
            version = "0.1.0"
            language = "en"

            [style_profile]
            extends = "profiles/terse.toml"
        "#,
    )
    .unwrap();
    std::fs::write(
        tmp.path().join("templates").join("event.toml"),
        r#"
            key = "event"

            [[variants]]
            salience = "low"
            body = "TERSE: {name}"

            [[variants]]
            salience = "medium"
            body = "MEDIUM: {name} was modified"
        "#,
    )
    .unwrap();

    let p = Project::load_from_dir(tmp.path()).unwrap();
    let engine = p.into_engine().unwrap();
    assert_eq!(engine.current_style_profile().name, "terse-base");

    let mut ctx = Context::new();
    ctx.insert("name", Value::String("UserService".into()));
    ctx.insert("entity_type", Value::String("class".into()));
    ctx.insert("consumer_count", Value::Number(5)); // Medium tier baseline
    let out = engine.render(&mut Session::new(), "event", &ctx).unwrap();
    assert!(
        out.starts_with("TERSE:"),
        "extends should pull terse profile from disk; got {out:?}"
    );
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
