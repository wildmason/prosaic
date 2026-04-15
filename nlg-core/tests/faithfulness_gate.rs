//! Integration tests for the Engine faithfulness gate.
//!
//! Tests the `Engine::with_faithfulness_gate(threshold)` builder option.

use nlg_core::{ctx, Engine, NlgError, Session, Variation};
use nlg_grammar_en::English;

fn base_engine() -> Engine {
    Engine::new(English::new()).variation(Variation::Fixed)
}

// ── Gate off by default ───────────────────────────────────────────────────

#[test]
fn gate_off_by_default_allows_unfaithful_render() {
    // The |negated pipe on a value with no registered antonym produces
    // "not <word>", which introduces the polarity token "not" into the output
    // without any "not" in the context or template literals.
    // Without a gate this passes; with a gate it would be rejected.
    let mut engine = base_engine();
    engine
        .register_template("t", "{name} {action|negated}")
        .unwrap();

    let ctx = ctx! { name: "UserService", action: "succeeded" };
    let mut session = Session::new();
    // No gate → renders fine even though output has polarity drift.
    let result = engine.render(&mut session, "t", &ctx);
    assert!(result.is_ok(), "no gate → render should succeed; got {:?}", result);
    let output = result.unwrap();
    // Output should include "not" from the negated pipe
    assert!(output.contains("not"), "negated pipe should produce 'not'; got: {}", output);
}

// ── Gate on: rejection ────────────────────────────────────────────────────

#[test]
fn gate_on_rejects_polarity_drift() {
    // The |negated pipe introduces "not" into the output.
    // Context has no "not", template literal has no "not".
    // → polarity_match = false → FaithfulnessRejection.
    let mut engine = base_engine().with_faithfulness_gate(1.0);
    engine
        .register_template("t", "{name} {action|negated}")
        .unwrap();

    let ctx = ctx! { name: "UserService", action: "succeeded" };
    let mut session = Session::new();
    let result = engine.render(&mut session, "t", &ctx);
    assert!(
        matches!(result, Err(NlgError::FaithfulnessRejection { polarity_match: false, .. })),
        "polarity drift should cause FaithfulnessRejection; got {:?}",
        result
    );
}

#[test]
fn gate_on_rejects_unentailed_content_tokens() {
    // The |verb:past pipe conjugates context value "rename" → "renamed".
    // "renamed" is not in source ("rename" is). singularize("renamed") = "renamed"
    // which is still not in source → content token is unentailed → precision < 1.0.
    let mut engine = base_engine().with_faithfulness_gate(1.0);
    engine
        .register_template("t", "{name} {action|verb:past}")
        .unwrap();

    let ctx = ctx! { name: "UserService", action: "rename" };
    let mut session = Session::new();
    let result = engine.render(&mut session, "t", &ctx);
    assert!(
        matches!(result, Err(NlgError::FaithfulnessRejection { .. })),
        "verb conjugation producing unentailed token should be rejected; got {:?}",
        result
    );
}

// ── Gate on: acceptance ───────────────────────────────────────────────────

#[test]
fn gate_on_permits_fully_faithful_output() {
    // Simple template: all content tokens come from context or template literals.
    let mut engine = base_engine().with_faithfulness_gate(1.0);
    engine
        .register_template("t", "{name} was modified")
        .unwrap();

    let ctx = ctx! { name: "UserService" };
    let mut session = Session::new();
    // "userservice" is in context, "modified" is in template literal.
    let result = engine.render(&mut session, "t", &ctx);
    assert!(result.is_ok(), "fully faithful output should pass gate; got {:?}", result);
}

#[test]
fn gate_on_permits_above_threshold() {
    // Template: "{old_name} {action|verb:past} {new_name}".
    // context: old_name="UserService", action="rename", new_name="AccountService"
    // Template literal is empty (all slots). Source tokens: "userservice", "rename", "accountservice".
    // Output: "UserService renamed AccountService." — content tokens: "userservice", "renamed", "accountservice".
    // "userservice" → entailed; "accountservice" → entailed; "renamed" → unentailed (singularize gives "renamed").
    // Precision = 2/3 ≈ 0.66. Threshold 0.5 → passes.
    let mut engine = base_engine().with_faithfulness_gate(0.5);
    engine
        .register_template("t", "{old_name} {action|verb:past} {new_name}")
        .unwrap();

    let ctx = ctx! {
        old_name: "UserService",
        action: "rename",
        new_name: "AccountService",
    };
    let mut session = Session::new();
    let result = engine.render(&mut session, "t", &ctx);
    assert!(
        result.is_ok(),
        "precision ≈ 0.66 should pass threshold 0.5; got {:?}",
        result
    );
}

// ── Session state restored on rejection ──────────────────────────────────

#[test]
fn session_state_restored_on_faithfulness_rejection() {
    // After a rejected render, session state is unchanged; subsequent renders
    // behave as if the failed render never happened.
    let mut engine = base_engine().with_faithfulness_gate(1.0);
    engine
        .register_template("ok", "{name} was modified")
        .unwrap();
    engine
        .register_template("bad", "{name} {action|negated}")
        .unwrap();

    let ctx = ctx! { name: "UserService", action: "succeeded" };
    let mut session = Session::new();

    // First valid render
    engine.render(&mut session, "ok", ctx.clone()).unwrap();

    // Rejected render — should fail
    let rejected = engine.render(&mut session, "bad", ctx.clone());
    assert!(
        matches!(rejected, Err(NlgError::FaithfulnessRejection { .. })),
        "second render should be rejected; got {:?}",
        rejected
    );

    // A subsequent valid render should succeed (state was restored).
    let after_rejection = engine.render(&mut session, "ok", ctx.clone());
    assert!(
        after_rejection.is_ok(),
        "render after rejected render should succeed; got {:?}",
        after_rejection
    );
}

// ── Error variant fields ──────────────────────────────────────────────────

#[test]
fn faithfulness_rejection_carries_diagnostic_fields() {
    // Confirm the error variant exposes precision and polarity_match.
    let err = NlgError::FaithfulnessRejection {
        precision: 0.5,
        polarity_match: false,
    };
    let msg = err.to_string();
    assert!(msg.contains("precision"), "error should include precision: {}", msg);
    assert!(msg.contains("polarity_match"), "error should include polarity_match: {}", msg);

    // Precision only failure
    let precision_err = NlgError::FaithfulnessRejection {
        precision: 0.75,
        polarity_match: true,
    };
    let precision_msg = precision_err.to_string();
    assert!(
        precision_msg.contains("0.750"),
        "error should show precision value: {}",
        precision_msg
    );
}
