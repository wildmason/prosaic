//! Pull-request narrative vocabulary for the `prosaic` engine.
//!
//! Registers higher-level PR narrative templates that complement the raw event
//! coverage in `prosaic-vocab-git`. Where `prosaic-vocab-git` covers discrete events
//! (PR opened, PR merged), this crate covers aggregate state: review posture,
//! scope, merge readiness, CI health, and cross-PR relationships.
//!
//! Every event type is registered at two or three salience levels so the
//! engine's importance-aware verbosity picks appropriate phrasing automatically.
//!
//! # Context keys per event
//!
//! - `pr.summary`        — `number`, `title`, `author`, `commit_count`, `files_changed`
//! - `pr.review_state`   — `number`, `approvals`, `requested_changes` (opt), `pending` (opt)
//! - `pr.scope`          — `number`, `areas` (list)
//! - `pr.diff_stats`     — `number`, `insertions`, `deletions`
//! - `pr.age`            — `number`, `days_open`, `stale` (0/1)
//! - `pr.merge_readiness` — `number`, `ready` (0/1), `blockers` (list, opt)
//! - `pr.ci_status`      — `number`, `passing`, `failing` (opt)
//! - `pr.related_prs`    — `number`, `depends_on` (list, opt), `blocks` (list, opt)

use prosaic_core::{Engine, ProsaicError};

/// English-language templates for PR-narrative events.
///
/// Each locale module exposes a `register` function with the same signature,
/// enabling future locale-aware dispatch (e.g. `es::register`, `de::register`)
/// without changing callers.
pub mod en;

/// Register the full PR-narrative vocabulary into an engine.
///
/// Delegates to the English locale. When multilingual support is added,
/// callers can opt into a specific locale via `register_locale`.
pub fn register(engine: &mut Engine) -> Result<(), ProsaicError> {
    en::register(engine)
}

#[cfg(test)]
mod tests {
    use super::*;
    use prosaic_core::{Context, Session, Strictness, Value, Variation};
    use prosaic_grammar_en::English;

    fn engine() -> Engine {
        let mut e = Engine::new(English::new())
            .strictness(Strictness::Strict)
            .variation(Variation::Fixed);
        register(&mut e).unwrap();
        e
    }

    #[test]
    fn summary_low_salience() {
        let engine = engine();
        let mut ctx = Context::new();
        ctx.insert("number", Value::Number(42));
        ctx.insert("title", Value::String("Add streaming API".into()));
        ctx.insert("author", Value::String("Alice".into()));
        ctx.insert("commit_count", Value::Number(3));
        ctx.insert("files_changed", Value::Number(5));
        ctx.insert("salience", Value::String("low".into()));
        let mut session = Session::new();
        let out = engine.render(&mut session, "pr.summary", &ctx).unwrap();
        assert!(out.contains("#42"), "got: {out}");
        assert!(out.contains("streaming API"), "got: {out}");
    }

    #[test]
    fn summary_high_salience_includes_counts() {
        let engine = engine();
        let mut ctx = Context::new();
        ctx.insert("number", Value::Number(7));
        ctx.insert("title", Value::String("Refactor auth layer".into()));
        ctx.insert("author", Value::String("Bob".into()));
        ctx.insert("commit_count", Value::Number(12));
        ctx.insert("files_changed", Value::Number(20));
        ctx.insert("salience", Value::String("high".into()));
        let mut session = Session::new();
        let out = engine.render(&mut session, "pr.summary", &ctx).unwrap();
        assert!(out.contains("#7"), "got: {out}");
        assert!(out.contains("12 commits"), "got: {out}");
        assert!(out.contains("20 files"), "got: {out}");
    }

    #[test]
    fn review_state_with_requested_changes_and_pending() {
        let engine = engine();
        let mut ctx = Context::new();
        ctx.insert("number", Value::Number(99));
        ctx.insert("approvals", Value::Number(2));
        ctx.insert("requested_changes", Value::Number(1));
        ctx.insert("pending", Value::Number(3));
        let mut session = Session::new();
        let out = engine.render(&mut session, "pr.review_state", &ctx).unwrap();
        assert!(out.contains("#99"), "got: {out}");
        assert!(out.contains("2 approvals"), "got: {out}");
        assert!(out.contains("1 request"), "got: {out}");
        assert!(out.contains("3 pending reviews"), "got: {out}");
    }

    #[test]
    fn review_state_approvals_only() {
        let engine = engine();
        let mut ctx = Context::new();
        ctx.insert("number", Value::Number(5));
        ctx.insert("approvals", Value::Number(1));
        let mut session = Session::new();
        let out = engine.render(&mut session, "pr.review_state", &ctx).unwrap();
        assert!(out.contains("1 approval"), "got: {out}");
        // No "None" or phantom text for absent optional slots.
        assert!(!out.contains("None"), "got: {out}");
    }

    #[test]
    fn scope_renders_area_list() {
        let engine = engine();
        let mut ctx = Context::new();
        ctx.insert("number", Value::Number(11));
        ctx.insert(
            "areas",
            Value::List(vec!["auth".into(), "api".into(), "db".into()]),
        );
        let mut session = Session::new();
        let out = engine.render(&mut session, "pr.scope", &ctx).unwrap();
        assert!(out.contains("#11"), "got: {out}");
        assert!(out.contains("auth"), "got: {out}");
    }

    #[test]
    fn diff_stats_pluralizes_lines() {
        let engine = engine();
        let mut ctx = Context::new();
        ctx.insert("number", Value::Number(33));
        ctx.insert("insertions", Value::Number(150));
        ctx.insert("deletions", Value::Number(1));
        let mut session = Session::new();
        let out = engine.render(&mut session, "pr.diff_stats", &ctx).unwrap();
        assert!(out.contains("#33"), "got: {out}");
        assert!(out.contains("150"), "got: {out}");
        assert!(out.contains("1 line"), "got: {out}");
    }

    #[test]
    fn age_stale_flag_via_choose() {
        let engine = engine();
        let mut ctx = Context::new();
        ctx.insert("number", Value::Number(8));
        ctx.insert("days_open", Value::Number(30));
        ctx.insert("stale", Value::Number(1));
        let mut session = Session::new();
        let out = engine.render(&mut session, "pr.age", &ctx).unwrap();
        assert!(out.contains("30 days"), "got: {out}");
        assert!(out.contains("stale"), "got: {out}");
    }

    #[test]
    fn age_not_stale_choose_produces_no_stale_text() {
        let engine = engine();
        let mut ctx = Context::new();
        ctx.insert("number", Value::Number(9));
        ctx.insert("days_open", Value::Number(2));
        ctx.insert("stale", Value::Number(0));
        let mut session = Session::new();
        let out = engine.render(&mut session, "pr.age", &ctx).unwrap();
        assert!(out.contains("2 days"), "got: {out}");
        assert!(!out.contains("stale"), "got: {out}");
    }

    #[test]
    fn merge_readiness_ready_no_blockers() {
        let engine = engine();
        let mut ctx = Context::new();
        ctx.insert("number", Value::Number(55));
        ctx.insert("ready", Value::Number(1));
        let mut session = Session::new();
        let out = engine
            .render(&mut session, "pr.merge_readiness", &ctx)
            .unwrap();
        assert!(out.contains("ready to merge"), "got: {out}");
        assert!(!out.contains("blocked"), "got: {out}");
    }

    #[test]
    fn merge_readiness_not_ready_with_blockers() {
        let engine = engine();
        let mut ctx = Context::new();
        ctx.insert("number", Value::Number(56));
        ctx.insert("ready", Value::Number(0));
        ctx.insert(
            "blockers",
            Value::List(vec!["failing CI".into(), "pending review".into()]),
        );
        let mut session = Session::new();
        let out = engine
            .render(&mut session, "pr.merge_readiness", &ctx)
            .unwrap();
        assert!(out.contains("not yet ready"), "got: {out}");
        assert!(out.contains("blocked"), "got: {out}");
        assert!(out.contains("failing CI"), "got: {out}");
    }

    #[test]
    fn ci_status_with_failing_checks() {
        let engine = engine();
        let mut ctx = Context::new();
        ctx.insert("number", Value::Number(77));
        ctx.insert("passing", Value::Number(8));
        ctx.insert("failing", Value::Number(2));
        let mut session = Session::new();
        let out = engine.render(&mut session, "pr.ci_status", &ctx).unwrap();
        assert!(out.contains("8 checks"), "got: {out}");
        assert!(out.contains("2 checks"), "got: {out}");
        assert!(out.contains("failing"), "got: {out}");
    }

    #[test]
    fn ci_status_all_passing() {
        let engine = engine();
        let mut ctx = Context::new();
        ctx.insert("number", Value::Number(78));
        ctx.insert("passing", Value::Number(5));
        let mut session = Session::new();
        let out = engine.render(&mut session, "pr.ci_status", &ctx).unwrap();
        assert!(out.contains("5 checks"), "got: {out}");
        assert!(!out.contains("failing"), "got: {out}");
    }

    #[test]
    fn related_prs_with_depends_on_and_blocks() {
        let engine = engine();
        let mut ctx = Context::new();
        ctx.insert("number", Value::Number(100));
        ctx.insert(
            "depends_on",
            Value::List(vec!["#98".into(), "#97".into()]),
        );
        ctx.insert("blocks", Value::List(vec!["#101".into()]));
        let mut session = Session::new();
        let out = engine
            .render(&mut session, "pr.related_prs", &ctx)
            .unwrap();
        assert!(out.contains("#100"), "got: {out}");
        assert!(out.contains("#98"), "got: {out}");
        assert!(out.contains("#101"), "got: {out}");
    }

    #[test]
    fn related_prs_no_relationships() {
        let engine = engine();
        let mut ctx = Context::new();
        ctx.insert("number", Value::Number(200));
        let mut session = Session::new();
        let out = engine
            .render(&mut session, "pr.related_prs", &ctx)
            .unwrap();
        // Must render without panic — conditional sections should suppress absent slots.
        assert!(out.contains("#200"), "got: {out}");
    }
}
