//! Backwards-compatibility gate for the `StyleProfile` design.
//!
//! `StyleProfile::neutral()` is contractually byte-for-byte equivalent to
//! never calling `.style_profile()` at all. This test exercises every
//! decision site a profile is allowed to bias — variant selection across
//! multiple registered templates, connective selection across multiple
//! renders inside a document plan, list-style rotation, hedge mapping,
//! and referring-expression generation — and asserts the two render paths
//! produce identical output character-for-character.
//!
//! If this test ever fails, a dial wiring is reading from the engine
//! profile when it should be honoring `is_neutral()` short-circuits, OR a
//! decision site that is supposed to defer to the existing engine logic
//! is consulting the profile instead of the legacy code path. Either way,
//! the regression invalidates the no-breaking-change guarantee in the
//! StyleProfile design spec.

use prosaic_core::{
    Context, DocumentPlan, Engine, EntityDescriptor, Session, Strictness, StyleProfile, Value,
    Variation,
};
use prosaic_grammar_en::English;

/// Build an engine with the workspace-default configuration, optionally
/// applying a style profile. Templates and entities are registered through
/// the closure so callers can vary the corpus independently.
fn build_engine(profile: Option<StyleProfile>) -> Engine {
    let mut engine = Engine::new(English::new())
        .strictness(Strictness::Strict)
        .variation(Variation::Seeded(42));
    if let Some(p) = profile {
        engine = engine.style_profile(p);
    }
    register_corpus(&mut engine);
    engine
}

fn register_corpus(engine: &mut Engine) {
    // Multiple variants for the same key — exercises select_variant +
    // choose-best scoring with the rhythm penalty under Variation::Seeded.
    engine
        .register_template_at(
            "code.modified",
            "{name|refer} was modified",
            prosaic_core::Salience::Medium,
        )
        .unwrap();
    engine
        .register_template_at(
            "code.modified",
            "Modifications landed on {name|refer}",
            prosaic_core::Salience::Medium,
        )
        .unwrap();
    engine
        .register_template_at(
            "code.modified",
            "Changes were merged into {name|refer}",
            prosaic_core::Salience::Medium,
        )
        .unwrap();

    engine
        .register_template_at(
            "code.renamed",
            "{old_name|refer} was renamed to {new_name}",
            prosaic_core::Salience::Medium,
        )
        .unwrap();

    // High-salience variant exercises the salience tier filter.
    engine
        .register_template_at(
            "code.renamed",
            "{old_name|refer} was renamed to {new_name} \
             which impacts {count} direct {count|pluralize:consumer} \
             {consumers|join}",
            prosaic_core::Salience::High,
        )
        .unwrap();

    // Low-salience variant — terse alternative.
    engine
        .register_template_at(
            "code.renamed",
            "{old_name|refer} → {new_name}",
            prosaic_core::Salience::Low,
        )
        .unwrap();

    // Hedge pipe usage.
    engine
        .register_template(
            "code.unstable",
            "{name|refer} {confidence|hedge} broke the build",
        )
        .unwrap();

    // Several entities so REG fires distinguishing-attribute walks.
    engine.register_entity(
        EntityDescriptor::new("UserService", "class").with_attribute("layer", "domain"),
    );
    engine.register_entity(
        EntityDescriptor::new("AuthService", "class").with_attribute("layer", "infra"),
    );
    engine.register_entity(EntityDescriptor::new("OrderService", "class"));
}

fn ctx_renamed(old: &str, new: &str, count: i64, consumers: &[&str]) -> Context {
    let mut c = Context::new();
    c.insert("entity_type", Value::String("class".into()));
    c.insert("old_name", Value::String(old.into()));
    c.insert("new_name", Value::String(new.into()));
    c.insert("count", Value::Number(count));
    c.insert(
        "consumers",
        Value::List(consumers.iter().map(|s| s.to_string()).collect()),
    );
    c
}

fn ctx_modified(name: &str) -> Context {
    let mut c = Context::new();
    c.insert("entity_type", Value::String("class".into()));
    c.insert("name", Value::String(name.into()));
    c
}

fn ctx_unstable(name: &str, confidence: i64) -> Context {
    let mut c = Context::new();
    c.insert("entity_type", Value::String("class".into()));
    c.insert("name", Value::String(name.into()));
    c.insert("confidence", Value::Number(confidence));
    c
}

/// Render a representative narrative through `engine` against a fresh
/// session, returning the concatenated outputs for byte comparison.
fn render_corpus(engine: &Engine) -> String {
    let mut session = Session::new();
    let mut out = String::new();

    // Sequence 1 — same entity, multiple actions (centering theory hot path
    // for full → short → pronoun reference transitions).
    out.push_str(
        &engine
            .render(&mut session, "code.modified", ctx_modified("UserService"))
            .unwrap(),
    );
    out.push('\n');
    out.push_str(
        &engine
            .render(&mut session, "code.modified", ctx_modified("UserService"))
            .unwrap(),
    );
    out.push('\n');
    out.push_str(
        &engine
            .render(
                &mut session,
                "code.renamed",
                ctx_renamed(
                    "UserService",
                    "AccountService",
                    25, // High salience tier
                    &["A", "B", "C", "D", "E", "F"],
                ),
            )
            .unwrap(),
    );
    out.push('\n');

    // Sequence 2 — different entity, same action (DifferentEntitySameAction
    // discourse relation; exercises connective selection).
    out.push_str(
        &engine
            .render(&mut session, "code.modified", ctx_modified("AuthService"))
            .unwrap(),
    );
    out.push('\n');
    out.push_str(
        &engine
            .render(&mut session, "code.modified", ctx_modified("OrderService"))
            .unwrap(),
    );
    out.push('\n');

    // Sequence 3 — hedge pipe across the confidence range.
    for conf in [10, 35, 55, 80, 95] {
        out.push_str(
            &engine
                .render(
                    &mut session,
                    "code.unstable",
                    ctx_unstable("UserService", conf),
                )
                .unwrap(),
        );
        out.push('\n');
    }

    // Sequence 4 — low salience tier (terse variant).
    out.push_str(
        &engine
            .render(
                &mut session,
                "code.renamed",
                ctx_renamed("OrderService", "BillingService", 0, &[]),
            )
            .unwrap(),
    );
    out.push('\n');

    out
}

/// Render a multi-paragraph DocumentPlan to exercise the document-scope
/// connective rotation across paragraph boundaries — the path the spec
/// most cares about for the StyleProfile + Self-Refine integration.
fn render_document(engine: &Engine) -> String {
    let mut session = Session::new();
    let renamed_ctx = ctx_renamed("UserService", "AccountService", 12, &["X", "Y", "Z"]);
    let mod_user = ctx_modified("UserService");
    let mod_user_2 = ctx_modified("UserService");
    let mod_auth = ctx_modified("AuthService");
    let mod_order = ctx_modified("OrderService");
    let events: Vec<(&str, Context)> = vec![
        ("code.modified", mod_user),
        ("code.modified", mod_user_2),
        ("code.renamed", renamed_ctx),
        ("code.modified", mod_auth),
        ("code.modified", mod_order),
    ];
    let plan = DocumentPlan::from_events(&events, engine);
    plan.render(engine, &mut session).unwrap()
}

#[test]
fn neutral_profile_byte_equals_no_profile_for_corpus() {
    let no_profile = build_engine(None);
    let neutral = build_engine(Some(StyleProfile::neutral()));

    let a = render_corpus(&no_profile);
    let b = render_corpus(&neutral);

    assert_eq!(
        a, b,
        "StyleProfile::neutral() must be byte-for-byte equivalent to no profile.\n\
         no_profile:\n{a}\n---\nneutral:\n{b}"
    );
}

#[test]
fn neutral_profile_byte_equals_no_profile_for_document_plan() {
    let no_profile = build_engine(None);
    let neutral = build_engine(Some(StyleProfile::neutral()));

    let a = render_document(&no_profile);
    let b = render_document(&neutral);

    assert_eq!(
        a, b,
        "StyleProfile::neutral() must be byte-for-byte equivalent to no profile across documents.\n\
         no_profile:\n{a}\n---\nneutral:\n{b}"
    );
}

#[test]
fn neutral_profile_round_trips_across_repeated_renders() {
    // Determinism: same profile + same engine + fresh sessions produces
    // the same output across N runs. Catches accidental hidden state
    // introduced by the profile plumbing.
    let engine = build_engine(Some(StyleProfile::neutral()));
    let first = render_corpus(&engine);
    for _ in 0..50 {
        assert_eq!(render_corpus(&engine), first);
    }
}

#[test]
fn current_style_profile_returns_neutral_by_default() {
    let engine = build_engine(None);
    assert!(engine.current_style_profile().is_neutral());
}

#[test]
fn explicitly_applied_neutral_profile_reads_back_neutral() {
    let engine = build_engine(Some(StyleProfile::neutral()));
    assert!(engine.current_style_profile().is_neutral());
    assert_eq!(engine.current_style_profile().name, "neutral");
}
