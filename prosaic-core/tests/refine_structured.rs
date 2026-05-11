//! Integration tests for the structured render path.
//!
//! `DocumentPlan::render_structured` produces a `RenderedDocument` with
//! per-paragraph and per-sentence metadata that diagnosers and the
//! retrospective-pass scorer consume. These tests verify the structured
//! path is internally consistent and aligns with the flat-text path
//! produced by `DocumentPlan::render` for non-gapping inputs.

use prosaic_core::{
    Context, DocumentPlan, Engine, Salience, Session, Strictness, Value, Variation,
};
use prosaic_grammar_en::English;

fn engine() -> Engine {
    let mut engine = Engine::new(English::new())
        .strictness(Strictness::Strict)
        .variation(Variation::Fixed);
    engine
        .register_template_at("code.modified", "{name} was modified", Salience::Medium)
        .unwrap();
    engine
        .register_template_at(
            "code.renamed",
            "{name} was renamed to {new_name}",
            Salience::Medium,
        )
        .unwrap();
    engine
}

fn ctx_named(name: &str) -> Context {
    let mut c = Context::new();
    c.insert("name", Value::String(name.into()));
    c.insert("entity_type", Value::String("class".into()));
    c
}

fn ctx_renamed(old: &str, new: &str) -> Context {
    let mut c = Context::new();
    c.insert("name", Value::String(old.into()));
    c.insert("new_name", Value::String(new.into()));
    c.insert("entity_type", Value::String("class".into()));
    c
}

fn build_plan() -> DocumentPlan {
    let events: Vec<(&str, Context)> = vec![
        ("code.modified", ctx_named("UserService")),
        ("code.renamed", ctx_renamed("UserService", "AccountService")),
        ("code.modified", ctx_named("AuthService")),
    ];
    let engine = engine();
    DocumentPlan::from_events(&events, &engine)
}

#[test]
fn render_structured_renders_sentence_by_sentence_without_gapping() {
    // The flat `render` path goes through `render_batch`, which applies
    // forward-conjunction reduction (gapping) and same-entity aggregation
    // — collapsing multiple events into one sentence when the surrounding
    // context permits. `render_structured` keeps one sentence per event
    // because diagnosers and the scorer reason at sentence granularity.
    // This test pins that documented v1 difference: the structured path
    // emits at least as many sentences as there are events.
    let plan = build_plan();
    let engine = engine();
    let event_count: usize = plan.paragraphs.iter().map(|p| p.events.len()).sum();
    let structured = plan
        .render_structured(&engine, &mut Session::new())
        .unwrap();
    assert_eq!(
        structured.sentences.len(),
        event_count,
        "structured render must emit one sentence per event (no gapping). \
         Events: {event_count}, sentences: {}, text: {}",
        structured.sentences.len(),
        structured.text
    );
}

#[test]
fn render_structured_populates_paragraph_count() {
    let plan = build_plan();
    let engine = engine();
    let doc = plan
        .render_structured(&engine, &mut Session::new())
        .unwrap();
    // The plan groups by entity; UserService has two consecutive events,
    // AuthService has one — so two paragraphs.
    assert_eq!(doc.paragraphs.len(), 2);
}

#[test]
fn render_structured_populates_per_sentence_word_counts() {
    let plan = build_plan();
    let engine = engine();
    let doc = plan
        .render_structured(&engine, &mut Session::new())
        .unwrap();
    for sentence in &doc.sentences {
        assert!(
            sentence.word_count > 0,
            "sentence `{}` should have non-zero word count",
            sentence.text
        );
    }
}

#[test]
fn render_structured_records_connectives_used() {
    let plan = build_plan();
    let engine = engine();
    let doc = plan
        .render_structured(&engine, &mut Session::new())
        .unwrap();
    // The second event in the UserService paragraph should pick up an
    // automatic connective (SameEntityDifferentAction relation).
    assert!(
        !doc.connectives_used.is_empty(),
        "expected auto-connective in same-entity paragraph: {:?}",
        doc.text
    );
    // Each connective entry's paragraph + sentence indices must point at
    // a real sentence in the document.
    for c in &doc.connectives_used {
        let paragraph = doc
            .paragraphs
            .get(c.paragraph_index)
            .expect("connective references paragraph that exists");
        assert!(
            c.sentence_index_in_paragraph < paragraph.sentences.len(),
            "connective sentence index out of range: {} >= {}",
            c.sentence_index_in_paragraph,
            paragraph.sentences.len()
        );
    }
}

#[test]
fn render_structured_flattened_sentences_align_with_paragraph_sentences() {
    let plan = build_plan();
    let engine = engine();
    let doc = plan
        .render_structured(&engine, &mut Session::new())
        .unwrap();
    let total: usize = doc.paragraphs.iter().map(|p| p.sentences.len()).sum();
    assert_eq!(total, doc.sentences.len());
}
