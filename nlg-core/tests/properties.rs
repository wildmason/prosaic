//! Property-based tests backstopping the heuristic-heavy parts of the
//! engine. These don't exhaustively describe correct output — they
//! assert invariants that must hold for *any* valid input:
//!
//! - Rendering never panics on well-formed templates.
//! - Output never contains unresolved `{…}` markers.
//! - Strict mode produces an error (not a panic) on missing slots.
//! - Silent mode swallows missing slots without panicking.
//! - Clause reduction preserves the entity name somewhere in its output.
//! - Sentence-length budget keeps every final piece ≤ (max + margin).
//!
//! Where a property might be overly strict for edge cases, we cap the
//! test input space so generated data remains within the invariant's
//! domain.

use nlg_core::{Context, Engine, Session, Strictness, Value, Variation};
use nlg_grammar_en::English;
use proptest::prelude::*;

fn base_engine() -> Engine {
    Engine::new(English::new())
        .strictness(Strictness::Strict)
        .variation(Variation::Fixed)
}

fn safe_ident() -> impl Strategy<Value = String> {
    "[A-Za-z]{1,16}".prop_map(|s| s)
}

// The literal `{` and `}` characters confuse `prop_assert!`'s format
// string parser, so we reach them through helpers that never appear
// inside a format string.
fn leaks_open_brace(s: &str) -> bool {
    s.contains('{')
}

fn leaks_close_brace(s: &str) -> bool {
    s.contains('}')
}

proptest! {
    /// A render via a simple slot never panics, always returns Ok, and
    /// never leaks a literal `{` or `}`.
    #[test]
    fn render_never_leaks_slot_markers(name in safe_ident()) {
        let mut engine = base_engine();
        engine.register_template("t", "Hello {name}").unwrap();

        let mut ctx = Context::new();
        ctx.insert("name", Value::String(name.clone()));
        let mut session = Session::new();
        let output = engine.render(&mut session, "t", &ctx).unwrap();

        prop_assert!(!leaks_open_brace(&output));
        prop_assert!(!leaks_close_brace(&output));
        prop_assert!(output.contains(&name));
    }

    /// Strict mode returns Err on a missing slot. No panics.
    #[test]
    fn strict_mode_errors_on_missing_slot(_seed in any::<u32>()) {
        let mut engine = base_engine();
        engine.register_template("t", "needs {absent}").unwrap();
        let ctx = Context::new();
        let mut session = Session::new();
        prop_assert!(engine.render(&mut session, "t", &ctx).is_err());
    }

    /// Silent mode never panics on missing slots and never leaves slot
    /// markers behind. Cleanup also ensures output isn't empty for a
    /// template that has at least one literal character.
    #[test]
    fn silent_mode_never_panics_on_missing_slot(_seed in "[A-Za-z]{1,8}") {
        let mut engine = base_engine().strictness(Strictness::Silent);
        engine.register_template("t", "hello {missing} world").unwrap();

        let ctx = Context::new();
        let mut session = Session::new();
        let output = engine.render(&mut session, "t", &ctx).unwrap();
        prop_assert!(!leaks_open_brace(&output));
        prop_assert!(!leaks_close_brace(&output));
        prop_assert!(!output.is_empty());
    }

    /// Clause reduction on a run of same-entity events keeps the entity
    /// name in the output — the reducer must not drop the subject while
    /// combining predicates.
    #[test]
    fn clause_reduction_preserves_entity(name in safe_ident()) {
        let mut engine = base_engine();
        engine
            .register_template("renamed", "{name|refer} was renamed")
            .unwrap();
        engine
            .register_template("modified", "{name|refer} was modified")
            .unwrap();
        engine
            .register_template("moved", "{name|refer} was moved")
            .unwrap();

        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("class".into()));
        ctx.insert("name", Value::String(name.clone()));

        let events: Vec<(&str, Context)> = vec![
            ("renamed", ctx.clone()),
            ("modified", ctx.clone()),
            ("moved", ctx.clone()),
        ];
        let mut session = Session::new();
        let output = engine.render_batch(&mut session, &events).unwrap();

        prop_assert!(output.contains(&name));
    }

    /// Every piece of a length-budgeted output stays within a bounded
    /// overshoot of the budget (the splitter's search window allows
    /// modest overrun in pursuit of a better natural boundary).
    #[test]
    fn length_budget_keeps_pieces_within_margin(
        budget in 40usize..120,
        count in 1i64..12
    ) {
        let mut engine = base_engine().max_sentence_length(budget);
        engine
            .register_template(
                "t",
                "The class UserService was renamed to AccountService, \
                 which impacts {count} direct {count|pluralize:consumer} including \
                 ProfileComponent, SettingsComponent, AdminModule, DashboardModule",
            )
            .unwrap();

        let mut ctx = Context::new();
        ctx.insert("count", Value::Number(count));
        let mut session = Session::new();
        let output = engine.render(&mut session, "t", &ctx).unwrap();

        for piece in output.split(". ") {
            prop_assert!(piece.chars().count() <= budget + 60);
        }
    }

    /// The quantify pipe is total: any i64 produces non-empty, marker-
    /// free output.
    #[test]
    fn quantify_pipe_is_total(count in -1000i64..1_000_000) {
        let mut engine = base_engine();
        engine.register_template("t", "{n|quantify} caller").unwrap();

        let mut ctx = Context::new();
        ctx.insert("n", Value::Number(count));
        let mut session = Session::new();
        let output = engine.render(&mut session, "t", &ctx).unwrap();

        prop_assert!(!output.is_empty());
        prop_assert!(!leaks_open_brace(&output));
        prop_assert!(!leaks_close_brace(&output));
    }

    /// The hedge pipe clamps out-of-range scores and always produces an
    /// adverb-length string (non-empty).
    #[test]
    fn hedge_pipe_handles_any_score(score in -10_000i64..10_000) {
        let mut engine = base_engine();
        engine.register_template("t", "It {c|hedge} works").unwrap();

        let mut ctx = Context::new();
        ctx.insert("c", Value::Number(score));
        let mut session = Session::new();
        let output = engine.render(&mut session, "t", &ctx).unwrap();
        prop_assert!(!leaks_open_brace(&output));
        prop_assert!(!leaks_close_brace(&output));
        prop_assert!(!output.is_empty());
    }

    /// Relative-time rendering is total across a wide range of
    /// timestamps relative to any reference time.
    #[test]
    fn relative_time_pipe_is_total(
        reference in 0i64..2_000_000_000,
        ts in 0i64..2_000_000_000
    ) {
        let mut engine = base_engine().reference_time(reference);
        engine.register_template("t", "event was {ts|relative}").unwrap();

        let mut ctx = Context::new();
        ctx.insert("ts", Value::Number(ts));
        let mut session = Session::new();
        let output = engine.render(&mut session, "t", &ctx).unwrap();
        prop_assert!(!output.is_empty());
        prop_assert!(!leaks_open_brace(&output));
        prop_assert!(!leaks_close_brace(&output));
    }
}
