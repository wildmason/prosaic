//! Integration tests for the faithfulness scorer.
//!
//! These live in tests/ (not src/) to avoid the two-crate identity issue
//! that arises when nlg-grammar-en (a dev-dep) is used inside nlg-core
//! unit tests — Rust would see two copies of nlg-core and the Language
//! trait bound would fail.

use nlg_core::{ctx, score_faithfulness, FaithfulnessScore, PolarityDrift};
use nlg_grammar_en::English;

fn lang() -> English {
    English::new()
}

// ── Scoring: basic entailment ─────────────────────────────────────────────

#[test]
fn faithful_simple_rename() {
    let ctx = ctx! {
        old_name: "UserService",
        new_name: "AccountService",
        entity_type: "class",
    };
    let lits: &[&str] = &["The ", " was renamed to ", "."];
    let output = "The class UserService was renamed to AccountService.";
    let score = score_faithfulness(output, &ctx, lits, &lang());
    assert_eq!(score.precision, 1.0, "all content tokens should be entailed");
    assert!(score.polarity_match);
    assert!(score.unentailed.is_empty());
}

#[test]
fn unentailed_word_lowers_precision() {
    let ctx = ctx! { name: "UserService", action: "modified" };
    let lits: &[&str] = &[" was "];
    // Output says "removed" but source only has "modified"
    let output = "UserService was removed.";
    let score = score_faithfulness(output, &ctx, lits, &lang());
    assert!(
        score.precision < 1.0,
        "precision should be below 1.0 with an unentailed word"
    );
    assert!(
        score.unentailed.contains(&"removed".to_string()),
        "unentailed should list 'removed'; got {:?}",
        score.unentailed
    );
}

// ── Scoring: polarity ─────────────────────────────────────────────────────

#[test]
fn polarity_drift_detected() {
    // Source has zero "not" tokens; hypothesis introduces one.
    let ctx = ctx! { name: "UserService" };
    let lits: &[&str] = &["was modified"];
    let output = "UserService was not modified.";
    let score = score_faithfulness(output, &ctx, lits, &lang());
    assert!(!score.polarity_match, "polarity drift should be detected");
    let drift = score
        .polarity_drift
        .iter()
        .find(|d| d.token == "not")
        .expect("PolarityDrift for 'not' should be present");
    assert_eq!(drift.in_source, 0);
    assert_eq!(drift.in_hypothesis, 1);
}

#[test]
fn polarity_preservation_detected() {
    // Source has "not" once; hypothesis has "not" once — match.
    let ctx = ctx! { status: "not available" };
    let lits: &[&str] = &["The service is "];
    let output = "The service is not available.";
    let score = score_faithfulness(output, &ctx, lits, &lang());
    assert!(score.polarity_match, "polarity counts match — should be ok");
    assert!(score.polarity_drift.is_empty());
}

#[test]
fn multiple_polarity_tokens_tracked_independently() {
    // Source has "not" once and "never" zero times; hypothesis flips both.
    let ctx = ctx! { status: "not ready" };
    let lits: &[&str] = &[];
    let output = "never ready.";
    let score = score_faithfulness(output, &ctx, lits, &lang());
    assert!(!score.polarity_match);
    assert!(
        score
            .polarity_drift
            .iter()
            .any(|d| d.token == "not" && d.in_source == 1 && d.in_hypothesis == 0),
        "expected 'not' drift; got {:?}",
        score.polarity_drift
    );
    assert!(
        score
            .polarity_drift
            .iter()
            .any(|d| d.token == "never" && d.in_source == 0 && d.in_hypothesis == 1),
        "expected 'never' drift; got {:?}",
        score.polarity_drift
    );
}

// ── Scoring: morphological tolerance ─────────────────────────────────────

#[test]
fn singular_plural_tolerance() {
    // Source has "service" (singular); output has "services" (plural).
    let ctx = ctx! { kind: "service" };
    let lits: &[&str] = &["multiple "];
    let output = "multiple services";
    let score = score_faithfulness(output, &ctx, lits, &lang());
    assert_eq!(
        score.precision, 1.0,
        "plural form should be tolerated via singularize; unentailed={:?}",
        score.unentailed
    );
}

// ── Scoring: template literals ────────────────────────────────────────────

#[test]
fn template_literals_contribute_to_source() {
    // "renamed" is in template literals but not in context.
    let ctx = ctx! { name: "UserService" };
    let lits: &[&str] = &[" was renamed to "];
    let output = "UserService was renamed to AccountService.";
    let score = score_faithfulness(output, &ctx, lits, &lang());
    assert!(
        !score.unentailed.contains(&"renamed".to_string()),
        "'renamed' appears in template literal and should be entailed; unentailed={:?}",
        score.unentailed
    );
}

// ── Scoring: vacuous / edge cases ─────────────────────────────────────────

#[test]
fn empty_output_is_vacuously_faithful() {
    let ctx = ctx! { name: "UserService" };
    let score = score_faithfulness("", &ctx, &[], &lang());
    assert_eq!(score.precision, 1.0);
    assert!(score.polarity_match);
    assert!(score.unentailed.is_empty());
}

#[test]
fn numeric_tokens_excluded_from_content() {
    // A number in output that isn't in source shouldn't affect precision.
    let ctx = ctx! { name: "UserService" };
    let lits: &[&str] = &["has "];
    let output = "UserService has 42 consumers.";
    let score = score_faithfulness(output, &ctx, lits, &lang());
    assert!(
        !score.unentailed.contains(&"42".to_string()),
        "pure numeric tokens should not appear in unentailed"
    );
}

#[test]
fn list_values_contribute_tokens_to_source() {
    let ctx = ctx! {
        consumers: ["ProfileComponent", "SettingsComponent"],
    };
    let lits: &[&str] = &["impacts "];
    let output = "impacts ProfileComponent and SettingsComponent.";
    let score = score_faithfulness(output, &ctx, lits, &lang());
    assert!(
        !score.unentailed.contains(&"profilecomponent".to_string())
            && !score.unentailed.contains(&"settingscomponent".to_string()),
        "list items should contribute to source; unentailed={:?}",
        score.unentailed
    );
}

#[test]
fn number_context_value_does_not_panic() {
    // Number values contribute their digit string to source.
    // Numbers are excluded from content scoring (pure numeric), so this just
    // verifies no panic and sane output.
    let ctx = ctx! { count: 5 };
    let lits: &[&str] = &["affecting "];
    let _score = score_faithfulness("affecting 5 consumers.", &ctx, lits, &lang());
    // no panic — test passes
}

// ── assert_faithful! macro ────────────────────────────────────────────────

#[test]
fn assert_faithful_passes_on_faithful_output() {
    let ctx = ctx! { name: "UserService", action: "renamed" };
    let lits: &[&str] = &["The class ", " was ", "."];
    // Should not panic
    nlg_core::assert_faithful!(
        "The class UserService was renamed.",
        ctx,
        lits,
        &lang()
    );
}

#[test]
#[should_panic(expected = "faithfulness violation")]
fn assert_faithful_panics_on_unfaithful_output() {
    let ctx = ctx! { name: "UserService" };
    let lits: &[&str] = &[];
    // "deleted" is unentailed
    nlg_core::assert_faithful!("UserService was deleted.", ctx, lits, &lang());
}

// ── Serde round-trip (feature = "serde") ─────────────────────────────────

#[test]
#[cfg(feature = "serde")]
fn faithfulness_score_serde_round_trip() {
    let score = FaithfulnessScore {
        precision: 0.75,
        polarity_match: true,
        unentailed: vec!["removed".to_string()],
        polarity_drift: vec![],
    };
    let json = serde_json::to_string(&score).expect("serialize");
    let back: FaithfulnessScore = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(score, back);
}

#[test]
#[cfg(feature = "serde")]
fn polarity_drift_serde_round_trip() {
    let drift = PolarityDrift {
        token: "not".to_string(),
        in_source: 0,
        in_hypothesis: 1,
    };
    let json = serde_json::to_string(&drift).expect("serialize");
    let back: PolarityDrift = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(drift, back);
}
