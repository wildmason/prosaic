//! Integration tests for the graph-based REG backend (Krahmer 2003 greedy).
//!
//! These tests exercise the full `Engine` + `pipe_refer` path: entities are
//! registered with relations, a template containing `{name|refer}` is
//! rendered, and the output is inspected for the expected relation clause.
//!
//! All tests in this file require the `reg` feature.

#![cfg(feature = "reg")]

use nlg_core::{
    Context, Engine, EntityDescriptor, RegAlgorithm, Session, Strictness, Value, Variation,
};
use nlg_grammar_en::English;

fn engine_graph() -> Engine {
    Engine::new(English::new())
        .strictness(Strictness::Strict)
        .variation(Variation::Fixed)
        .reg_algorithm(RegAlgorithm::GraphBased)
}

// ── Core graph-based path ─────────────────────────────────────────────────

#[test]
fn graph_based_relation_in_referring_expression() {
    let mut engine = engine_graph();
    engine.register_entity(
        EntityDescriptor::new("LoginHandler", "function")
            .with_attribute("layer", "api")
            .with_relation("that calls", "AuthService"),
    );
    engine.register_entity(
        EntityDescriptor::new("LogoutHandler", "function")
            .with_attribute("layer", "api")
            .with_relation("that calls", "SessionService"),
    );
    engine.register_template("t", "{name|refer} was modified").unwrap();

    let mut session = Session::new();
    let mut ctx = Context::new();
    ctx.insert("entity_type", Value::String("function".into()));
    ctx.insert("name", Value::String("LoginHandler".into()));

    let out = engine.render(&mut session, "t", &ctx).unwrap();
    assert!(
        out.contains("that calls AuthService"),
        "expected relation clause, got: {out}"
    );
}

#[test]
fn graph_based_falls_back_to_plain_when_attributes_suffice() {
    let mut engine = engine_graph();
    engine.register_entity(
        EntityDescriptor::new("UserService", "class")
            .with_attribute("layer", "domain"),
    );
    engine.register_entity(
        EntityDescriptor::new("AuthService", "class")
            .with_attribute("layer", "infra"),
    );
    engine.register_template("t", "{name|refer} was modified").unwrap();

    let mut session = Session::new();
    let mut ctx = Context::new();
    ctx.insert("entity_type", Value::String("class".into()));
    ctx.insert("name", Value::String("UserService".into()));

    let out = engine.render(&mut session, "t", &ctx).unwrap();
    // Attributes disambiguate; no relation clause should appear.
    assert!(
        out.contains("The domain class UserService"),
        "expected premodified name, got: {out}"
    );
    assert!(
        !out.contains("that"),
        "no relation clause expected, got: {out}"
    );
}

#[test]
fn graph_based_default_dale_reiter_does_not_emit_relations() {
    // Same registry as the first test but with the default (D&R) algorithm —
    // D&R cannot use relations, so no relation clause must appear.
    let mut engine = Engine::new(English::new())
        .strictness(Strictness::Strict)
        .variation(Variation::Fixed);
    engine.register_entity(
        EntityDescriptor::new("LoginHandler", "function")
            .with_attribute("layer", "api")
            .with_relation("that calls", "AuthService"),
    );
    engine.register_entity(
        EntityDescriptor::new("LogoutHandler", "function")
            .with_attribute("layer", "api")
            .with_relation("that calls", "SessionService"),
    );
    engine.register_template("t", "{name|refer} was modified").unwrap();

    let mut session = Session::new();
    let mut ctx = Context::new();
    ctx.insert("entity_type", Value::String("function".into()));
    ctx.insert("name", Value::String("LoginHandler".into()));

    let out = engine.render(&mut session, "t", &ctx).unwrap();
    // D&R produces "the api function LoginHandler" but cannot use the
    // relation to further distinguish — no "calls" in the output.
    assert!(
        !out.contains("calls"),
        "D&R must not emit relation clause: {out}"
    );
}

// ── Edge cases ────────────────────────────────────────────────────────────

#[test]
fn graph_based_single_entity_no_distractors() {
    // Single entity in registry — no distractors, neither attributes nor
    // relations should be added.
    let mut engine = engine_graph();
    engine.register_entity(
        EntityDescriptor::new("UserService", "class")
            .with_attribute("layer", "domain")
            .with_relation("that handles", "Logins"),
    );
    engine.register_template("t", "{name|refer} was modified").unwrap();

    let mut session = Session::new();
    let mut ctx = Context::new();
    ctx.insert("entity_type", Value::String("class".into()));
    ctx.insert("name", Value::String("UserService".into()));

    let out = engine.render(&mut session, "t", &ctx).unwrap();
    assert_eq!(out, "The class UserService was modified.");
}

#[test]
fn graph_based_full_surface_form_with_relation() {
    // Verify the exact rendered surface form including attributes + relation.
    let mut engine = engine_graph();
    engine.register_entity(
        EntityDescriptor::new("LoginHandler", "function")
            .with_attribute("layer", "api")
            .with_relation("that calls", "AuthService"),
    );
    engine.register_entity(
        EntityDescriptor::new("LogoutHandler", "function")
            .with_attribute("layer", "api")
            .with_relation("that calls", "SessionService"),
    );
    engine.register_template("t", "{name|refer} was modified").unwrap();

    let mut session = Session::new();
    let mut ctx = Context::new();
    ctx.insert("entity_type", Value::String("function".into()));
    ctx.insert("name", Value::String("LoginHandler".into()));

    let out = engine.render(&mut session, "t", &ctx).unwrap();
    // Both handlers have layer=api — the shared attribute does not
    // distinguish, so D&R contributes no premodifiers. The graph-based
    // algorithm then appends the distinguishing relation.
    // Full form: "The function LoginHandler that calls AuthService <rest>"
    assert!(
        out.starts_with("The function LoginHandler that calls AuthService"),
        "unexpected surface form: {out}"
    );
}
