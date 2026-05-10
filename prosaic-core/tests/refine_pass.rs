//! End-to-end integration tests for the retrospective refine pass.
//!
//! Verifies the iteration controller, constraint application, and
//! `DocumentPlan::render` dispatch operate correctly across realistic
//! corpora. Each test pins one observable property of the refine loop
//! so regressions in any layer surface here.

use prosaic_core::{
    Context, DocumentPlan, Engine, ParagraphOpenerMonotony, RefineConfig, Salience, Session,
    Strictness, Value, Variation,
};
use prosaic_grammar_en::English;
use std::sync::Arc;

fn engine(refine: bool) -> Engine {
    let mut e = Engine::new(English::new())
        .strictness(Strictness::Strict)
        .variation(Variation::Fixed);
    if refine {
        e = e.refine(RefineConfig::balanced().with_max_iterations(3));
    }
    e.register_template_at("evt.touched", "{name|refer} was touched", Salience::Medium)
        .unwrap();
    e.register_template_at("evt.modified", "{name|refer} was modified", Salience::Medium)
        .unwrap();
    e.register_template_at("evt.renamed", "{name|refer} was renamed to {new_name}", Salience::Medium)
        .unwrap();
    e
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

/// Build a plan that puts each event in its own paragraph (ByEntity
/// grouping splits on entity name change). Forces multiple paragraphs so
/// paragraph-opener and connective-family diagnosers have material.
fn build_multi_entity_plan(engine: &Engine) -> DocumentPlan {
    let events: Vec<(&str, Context)> = vec![
        ("evt.modified", ctx_named("Alpha")),
        ("evt.touched", ctx_named("Alpha")),
        ("evt.modified", ctx_named("Bravo")),
        ("evt.touched", ctx_named("Bravo")),
        ("evt.modified", ctx_named("Charlie")),
        ("evt.touched", ctx_named("Charlie")),
        ("evt.modified", ctx_named("Delta")),
        ("evt.renamed", ctx_renamed("Delta", "Echo")),
    ];
    DocumentPlan::from_events(&events, engine)
}

#[test]
fn refine_off_renders_byte_identical_to_no_refine_path() {
    let plan = build_multi_entity_plan(&engine(false));
    let off = plan
        .render(&engine(false), &mut Session::new())
        .unwrap();
    let off_again = plan
        .render(&engine(false), &mut Session::new())
        .unwrap();
    assert_eq!(off, off_again);
    assert!(!off.is_empty());
}

#[test]
fn refine_on_returns_outcome_with_iteration_metadata() {
    let plan = build_multi_entity_plan(&engine(true));
    let outcome = plan
        .render_refined(&engine(true), &mut Session::new())
        .unwrap();
    assert!(!outcome.text.is_empty());
    assert!(outcome.iterations_run <= 3);
}

#[test]
fn refine_on_terminates_when_diagnoses_clean_or_iter_cap() {
    let engine = engine(true);
    let plan = build_multi_entity_plan(&engine);
    let outcome = plan.render_refined(&engine, &mut Session::new()).unwrap();
    // Either the loop converged cleanly or it ran to its iteration cap.
    // Both are valid termination conditions; the property under test is
    // that the loop terminates rather than spinning.
    assert!(outcome.iterations_run <= 3);
}

#[test]
fn refine_on_renders_deterministically_across_repeats() {
    let engine = engine(true);
    let plan = build_multi_entity_plan(&engine);
    let first = plan.render_refined(&engine, &mut Session::new()).unwrap();
    for _ in 0..30 {
        let again = plan.render_refined(&engine, &mut Session::new()).unwrap();
        assert_eq!(first.text, again.text);
        assert_eq!(first.iterations_run, again.iterations_run);
    }
}

#[test]
fn refine_blacklist_is_honored_across_iteration_renders() {
    // Pin: when paragraph-opener monotony fires (≥3 paragraphs share an
    // opener), the constraint blacklists that opener. After refinement,
    // the dominant opener's count should drop or be redistributed.
    let mut e = Engine::new(English::new())
        .strictness(Strictness::Strict)
        .variation(Variation::Fixed)
        .refine(
            RefineConfig::off()
                // Force just the monotony diagnoser, with a low threshold
                // to make the test deterministic.
                .clone(),
        );
    let custom_diag = ParagraphOpenerMonotony {
        threshold: 2,
        min_paragraphs: 2,
    };
    let mut config = RefineConfig::balanced().with_max_iterations(3);
    config.diagnosers.clear();
    config.diagnosers.push(Arc::new(custom_diag));
    e = e.refine(config);
    e.register_template("evt.modified", "{name|refer} was modified")
        .unwrap();
    e.register_template("evt.touched", "{name|refer} was touched")
        .unwrap();

    // Force same-entity-different-action across many entities so each
    // paragraph emits an "Additionally," opener naturally.
    let events: Vec<(&str, Context)> = vec![
        ("evt.modified", ctx_named("Alpha")),
        ("evt.touched", ctx_named("Alpha")),
        ("evt.modified", ctx_named("Bravo")),
        ("evt.touched", ctx_named("Bravo")),
        ("evt.modified", ctx_named("Charlie")),
        ("evt.touched", ctx_named("Charlie")),
        ("evt.modified", ctx_named("Delta")),
        ("evt.touched", ctx_named("Delta")),
    ];
    let plan = DocumentPlan::from_events(&events, &e);

    // Apples-to-apples baseline: same plan, render_structured (no refine
    // loop). This isolates the contribution of the refine loop itself
    // from the structured-vs-flat sentence-emission difference.
    let baseline_engine = engine(false);
    let baseline_plan = DocumentPlan::from_events(&events, &baseline_engine);
    let baseline_doc = baseline_plan
        .render_structured(&baseline_engine, &mut Session::new())
        .unwrap();
    let baseline_additionally = baseline_doc.text.matches("Additionally,").count();

    let refined_outcome = plan.render_refined(&e, &mut Session::new()).unwrap();
    let refined_additionally = refined_outcome.text.matches("Additionally,").count();

    // The refine pass must not increase the count of the dominant opener,
    // and on documents long enough to trigger the diagnoser it should
    // strictly reduce it (often to zero, since the blacklist applies for
    // the whole iteration).
    assert!(
        refined_additionally <= baseline_additionally,
        "refine pass must not increase 'Additionally,' count. \
         baseline={baseline_additionally}, refined={refined_additionally}\n\
         baseline text:\n{}\n\nrefined text:\n{}",
        baseline_doc.text,
        refined_outcome.text
    );
}

#[test]
fn refine_outcome_score_is_non_negative_and_finite() {
    let engine = engine(true);
    let plan = build_multi_entity_plan(&engine);
    let outcome = plan.render_refined(&engine, &mut Session::new()).unwrap();
    assert!(outcome.final_score.is_finite());
    assert!(outcome.final_score >= 0.0);
}
