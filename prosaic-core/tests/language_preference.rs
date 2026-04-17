//! Tests for `Engine::language_preference` + `register_template_with_language`.

use prosaic_core::{Context, Engine, Salience, Session, Strictness, Value, Variation};
use prosaic_grammar_en::English;

#[test]
fn language_preference_picks_matching_variant() {
    let mut engine = Engine::new(English::new())
        .strictness(Strictness::Strict)
        .variation(Variation::Fixed)
        .language_preference("es");

    engine
        .register_template_with_language("greet", "Hello {name}", Some("en"))
        .unwrap();
    engine
        .register_template_with_language("greet", "Hola {name}", Some("es"))
        .unwrap();

    let mut ctx = Context::new();
    ctx.insert("name", Value::String("world".to_string()));

    let mut session = Session::new();
    let out = engine.render(&mut session, "greet", &ctx).unwrap();
    assert!(
        out.starts_with("Hola"),
        "expected Spanish variant; got: {out}"
    );
}

#[test]
fn language_preference_falls_back_to_unspecified() {
    let mut engine = Engine::new(English::new())
        .strictness(Strictness::Strict)
        .variation(Variation::Fixed)
        .language_preference("es");

    // Only an English variant registered.
    engine
        .register_template_with_language("greet", "Hello {name}", Some("en"))
        .unwrap();

    let mut ctx = Context::new();
    ctx.insert("name", Value::String("world".to_string()));

    let mut session = Session::new();
    let out = engine.render(&mut session, "greet", &ctx).unwrap();
    assert!(
        out.starts_with("Hello"),
        "expected fallback to English; got: {out}"
    );
}

#[test]
fn language_preference_falls_back_to_untagged() {
    let mut engine = Engine::new(English::new())
        .strictness(Strictness::Strict)
        .variation(Variation::Fixed)
        .language_preference("es");

    // No language-tagged variant; one untagged. Should pick the untagged.
    engine.register_template("greet", "Hello {name}").unwrap();

    let mut ctx = Context::new();
    ctx.insert("name", Value::String("world".to_string()));

    let mut session = Session::new();
    let out = engine.render(&mut session, "greet", &ctx).unwrap();
    assert!(out.starts_with("Hello"));
}

#[test]
fn no_preference_picks_among_all() {
    let mut engine = Engine::new(English::new())
        .strictness(Strictness::Strict)
        .variation(Variation::Fixed);
    // No language preference set.

    engine
        .register_template_with_language("greet", "Hello {name}", Some("en"))
        .unwrap();

    let mut ctx = Context::new();
    ctx.insert("name", Value::String("world".to_string()));

    let mut session = Session::new();
    let out = engine.render(&mut session, "greet", &ctx).unwrap();
    assert!(out.starts_with("Hello"));
}

#[test]
fn language_tagged_variant_with_salience() {
    let mut engine = Engine::new(English::new())
        .strictness(Strictness::Strict)
        .variation(Variation::Fixed)
        .language_preference("es");

    engine
        .register_template_with_language_at(
            "alert",
            "Brief alert for {name}",
            Salience::Low,
            Some("en"),
        )
        .unwrap();
    engine
        .register_template_with_language_at(
            "alert",
            "Alerta breve para {name}",
            Salience::Low,
            Some("es"),
        )
        .unwrap();

    let mut ctx = Context::new();
    ctx.insert("name", Value::String("Foo".to_string()));
    ctx.insert("salience", Value::String("low".to_string()));

    let mut session = Session::new();
    let out = engine.render(&mut session, "alert", &ctx).unwrap();
    assert!(out.starts_with("Alerta"), "got: {out}");
}
