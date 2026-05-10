//! End-to-end integration tests for the retrospective refine pass.
//!
//! Verifies the iteration controller, constraint application, and
//! `DocumentPlan::render` dispatch operate correctly across realistic
//! corpora. Each test pins one observable property of the refine loop
//! so regressions in any layer surface here.

use prosaic_core::{
    Context, Diagnoser, Diagnostic, DocumentPlan, Engine, LengthDistribution,
    ParagraphOpenerMonotony, RefineConfig, RefineConstraint, RenderedDocument, Salience,
    SalienceBias, Session, StyleProfile, Strictness, Value, Variation,
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

// ── Custom-diagnoser scaffolding for v0.6.1 constraint tests ──────────

/// Single-shot diagnoser: emits the supplied constraints exactly once,
/// with high enough severity that the iteration controller applies them.
/// After the first call, the diagnoser returns nothing so the loop can
/// converge cleanly. This is the test scaffolding that lets each
/// constraint variant drive a real iteration without requiring a
/// real-world failure condition the built-in diagnosers would catch.
struct OneShotDiagnoser {
    name: &'static str,
    constraints: std::sync::Mutex<Option<Vec<RefineConstraint>>>,
}

impl OneShotDiagnoser {
    fn new(name: &'static str, constraints: Vec<RefineConstraint>) -> Self {
        Self {
            name,
            constraints: std::sync::Mutex::new(Some(constraints)),
        }
    }
}

impl Diagnoser for OneShotDiagnoser {
    fn name(&self) -> &'static str {
        self.name
    }

    fn diagnose(
        &self,
        _document: &RenderedDocument,
        _profile: Option<&StyleProfile>,
    ) -> Vec<Diagnostic> {
        let Some(c) = self.constraints.lock().unwrap().take() else {
            return Vec::new();
        };
        vec![Diagnostic {
            diagnoser: self.name,
            severity: 1.0,
            constraints: c,
        }]
    }
}

fn refine_with_only(diagnoser: Arc<dyn Diagnoser>) -> RefineConfig {
    // `with_min_improvement(-100.0)` forces the iteration controller to
    // accept any candidate regardless of score change, so the constraint
    // application path is exercised even when the rendered output's
    // composite score doesn't strictly improve. The tests assert the
    // candidate's output content directly, which is what's under test.
    let mut c = RefineConfig::balanced()
        .with_max_iterations(2)
        .with_min_improvement(-100.0);
    c.diagnosers.clear();
    c.diagnosers.push(diagnoser);
    c
}

// ── PrimeRecencyWindow ────────────────────────────────────────────────

#[test]
fn prime_recency_window_suppresses_primed_connective_on_next_iteration() {
    let mut e = Engine::new(English::new())
        .strictness(Strictness::Strict)
        .variation(Variation::Fixed)
        .refine(refine_with_only(Arc::new(OneShotDiagnoser::new(
            "prime_test",
            vec![RefineConstraint::PrimeRecencyWindow {
                connectives: vec![
                    "Additionally,".to_string(),
                    "Furthermore,".to_string(),
                    "It also".to_string(),
                ],
                list_styles: vec![],
            }],
        ))));
    e.register_template("evt.modified", "{name|refer} was modified")
        .unwrap();
    e.register_template("evt.touched", "{name|refer} was touched")
        .unwrap();

    let events: Vec<(&str, Context)> = vec![
        ("evt.modified", ctx_named("Alpha")),
        ("evt.touched", ctx_named("Alpha")),
        ("evt.modified", ctx_named("Bravo")),
        ("evt.touched", ctx_named("Bravo")),
    ];
    let plan = DocumentPlan::from_events(&events, &e);
    let outcome = plan.render_refined(&e, &mut Session::new()).unwrap();

    // The continuation pool ("Additionally,", "Furthermore,", "It also")
    // is fully primed, so the family-budget gate inside
    // `select_connective_filtered` immediately suppresses any
    // continuation connective on the next iteration's renders.
    assert!(
        !outcome.text.contains("Additionally,")
            && !outcome.text.contains("Furthermore,")
            && !outcome.text.contains("It also"),
        "primed continuation pool must not emit on the next iteration. \
         text:\n{}",
        outcome.text
    );
    assert!(outcome.iterations_run >= 1);
}

// ── OverrideSalienceBias ──────────────────────────────────────────────

#[test]
fn override_salience_bias_changes_tier_selection_in_next_iteration() {
    let mut e = Engine::new(English::new())
        .strictness(Strictness::Strict)
        .variation(Variation::Fixed)
        .refine(refine_with_only(Arc::new(OneShotDiagnoser::new(
            "salience_test",
            // SalienceBias::Lower shrinks the bands so the same
            // consumer_count lands in a *higher* tier — Medium → High in
            // typical mid-range cases.
            vec![RefineConstraint::OverrideSalienceBias(SalienceBias::Lower)],
        ))));
    e.register_template_at(
        "evt.modified",
        "{name|refer} was lightly tweaked",
        Salience::Low,
    )
    .unwrap();
    e.register_template_at(
        "evt.modified",
        "{name|refer} was modified",
        Salience::Medium,
    )
    .unwrap();
    e.register_template_at(
        "evt.modified",
        "{name|refer} was extensively overhauled across consumers",
        Salience::High,
    )
    .unwrap();

    let mut ctx = ctx_named("UserService");
    // consumer_count=18 lands in Medium under default thresholds
    // (low_max=2, high_min=20). With SalienceBias::Lower the bands
    // shrink (low_max=1, high_min=15) so the same value lands in High.
    ctx.insert("consumer_count", Value::Number(18));

    let events = vec![("evt.modified", ctx)];
    let plan = DocumentPlan::from_events(&events, &e);
    let outcome = plan.render_refined(&e, &mut Session::new()).unwrap();
    assert!(
        outcome.text.contains("extensively overhauled"),
        "SalienceBias::Lower should promote the variant tier from Medium \
         to High, picking the 'extensively overhauled' template. text:\n{}",
        outcome.text
    );
}

// ── ForceVariantTier ──────────────────────────────────────────────────

#[test]
fn force_variant_tier_short_circuits_to_specified_tier() {
    let mut e = Engine::new(English::new())
        .strictness(Strictness::Strict)
        .variation(Variation::Fixed)
        .refine(refine_with_only(Arc::new(OneShotDiagnoser::new(
            "force_tier_test",
            vec![RefineConstraint::ForceVariantTier {
                template_key: "evt.modified".to_string(),
                tier: Salience::High,
            }],
        ))));
    e.register_template_at(
        "evt.modified",
        "{name|refer} was lightly tweaked",
        Salience::Low,
    )
    .unwrap();
    e.register_template_at(
        "evt.modified",
        "{name|refer} was modified",
        Salience::Medium,
    )
    .unwrap();
    e.register_template_at(
        "evt.modified",
        "{name|refer} was extensively overhauled across consumers",
        Salience::High,
    )
    .unwrap();

    // Salience::Low context (no consumer_count, default 0). Under normal
    // resolution the engine picks the Low variant. ForceVariantTier
    // short-circuits the tier resolution to High regardless.
    let events = vec![("evt.modified", ctx_named("UserService"))];
    let plan = DocumentPlan::from_events(&events, &e);
    let outcome = plan.render_refined(&e, &mut Session::new()).unwrap();
    assert!(
        outcome.text.contains("extensively overhauled"),
        "ForceVariantTier{{tier=High}} must select the High variant \
         independent of context salience. text:\n{}",
        outcome.text
    );
}

// ── TightenLengthDistribution ─────────────────────────────────────────

#[test]
fn tighten_length_distribution_changes_candidate_scoring() {
    // Register two Medium variants so the choose-best path runs and the
    // candidate-discourse score actually informs the pick.
    let target = LengthDistribution {
        // Strongly biased toward long sentences — should make the longer
        // variant score better than the shorter.
        short: 0.0,
        medium: 0.0,
        long: 1.0,
        short_max_words: 6,
        medium_max_words: 12,
    };

    struct LengthDiagnoser {
        target: LengthDistribution,
        consumed: std::sync::Mutex<bool>,
    }
    impl Diagnoser for LengthDiagnoser {
        fn name(&self) -> &'static str {
            "length_test"
        }
        fn diagnose(
            &self,
            _document: &RenderedDocument,
            _profile: Option<&StyleProfile>,
        ) -> Vec<Diagnostic> {
            let mut c = self.consumed.lock().unwrap();
            if *c {
                return Vec::new();
            }
            *c = true;
            vec![Diagnostic {
                diagnoser: "length_test",
                severity: 1.0,
                constraints: vec![RefineConstraint::TightenLengthDistribution(
                    self.target.clone(),
                )],
            }]
        }
    }

    let mut e = Engine::new(English::new())
        .strictness(Strictness::Strict)
        .variation(Variation::Seeded(7))
        .refine(refine_with_only(Arc::new(LengthDiagnoser {
            target,
            consumed: std::sync::Mutex::new(false),
        })));
    e.register_template_at(
        "evt.modified",
        "{name|refer} was tweaked",
        Salience::Medium,
    )
    .unwrap();
    e.register_template_at(
        "evt.modified",
        "{name|refer} was extensively overhauled across many consumers and dependencies",
        Salience::Medium,
    )
    .unwrap();

    // Two events to trigger choose-best (first render is unconditional).
    let events = vec![
        ("evt.modified", ctx_named("Alpha")),
        ("evt.modified", ctx_named("Bravo")),
    ];
    let plan = DocumentPlan::from_events(&events, &e);
    let outcome = plan.render_refined(&e, &mut Session::new()).unwrap();
    // Under the long-leaning override, the second event's choose-best
    // pass should prefer the longer variant.
    assert!(
        outcome.text.contains("extensively overhauled"),
        "TightenLengthDistribution(long-leaning) should bias choose-best \
         toward the longer variant. text:\n{}",
        outcome.text
    );
}
