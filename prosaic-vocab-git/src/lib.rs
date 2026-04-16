//! Git/VCS activity vocabulary for the `prosaic` engine.
//!
//! Registers a set of templates covering the common events in a
//! git-based workflow: commits, pull requests, issues, reviews, and
//! releases. Each event is registered at three salience levels (Low,
//! Medium, High) so the engine's importance-aware rendering picks
//! natural verbosity based on event magnitude.
//!
//! Context keys expected per event:
//!
//! - `git.commit`            — author, sha, files_changed, additions, deletions, message
//! - `git.pr_opened`         — number, title, author
//! - `git.pr_merged`         — number, title, author, merger
//! - `git.pr_closed`         — number, title, reason (optional)
//! - `git.issue_opened`      — number, title, author
//! - `git.issue_closed`      — number, title, reason (optional)
//! - `git.review_approved`   — reviewer, pr_number
//! - `git.review_changes_requested` — reviewer, pr_number, comment_count
//! - `git.release`           — version, tag, changes_count (optional)
//!
//! All templates use `{name}` / `{author}` style slots without the
//! `|refer` pipe so consumers who don't care about discourse-aware
//! entity reference still render cleanly. Advanced callers can register
//! entity descriptors for contributors / PRs to get distinguishing
//! references across entities sharing the same name.

use prosaic_core::{Engine, ProsaicError};

/// English-language templates for git-activity events.
///
/// Each locale module exposes a `register` function with the same signature,
/// enabling future locale-aware dispatch (e.g. `es::register`, `de::register`)
/// without changing callers.
pub mod en;

/// Register the full git-activity vocabulary into an engine.
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
    fn commit_event_medium_salience() {
        let engine = engine();
        let mut ctx = Context::new();
        ctx.insert("author", Value::String("Alice".into()));
        ctx.insert("files_changed", Value::Number(5));
        ctx.insert(
            "message",
            Value::String("refactor the parser".into()),
        );
        ctx.insert("additions", Value::Number(40));
        ctx.insert("deletions", Value::Number(10));
        let mut session = Session::new();

        let result = engine.render(&mut session, "git.commit", &ctx).unwrap();
        assert!(
            result.contains("Alice") && result.contains("5 files"),
            "got: {result}"
        );
    }

    #[test]
    fn pr_opened_event() {
        let engine = engine();
        let mut ctx = Context::new();
        ctx.insert("author", Value::String("Bob".into()));
        ctx.insert("number", Value::Number(42));
        ctx.insert("title", Value::String("Add retry logic".into()));
        let mut session = Session::new();

        let result = engine.render(&mut session, "git.pr_opened", &ctx).unwrap();
        assert!(result.contains("#42"), "got: {result}");
        assert!(result.contains("Add retry logic"), "got: {result}");
    }

    #[test]
    fn pr_merged_without_author_still_renders() {
        // The optional {?author}...{/?} block must be skipped.
        let engine = engine();
        let mut ctx = Context::new();
        ctx.insert("merger", Value::String("Charlie".into()));
        ctx.insert("number", Value::Number(17));
        ctx.insert("title", Value::String("Fix flaky test".into()));
        let mut session = Session::new();

        let result = engine.render(&mut session, "git.pr_merged", &ctx).unwrap();
        assert!(result.contains("#17"), "got: {result}");
        assert!(result.contains("Charlie"), "got: {result}");
    }

    #[test]
    fn issue_opened_event() {
        let engine = engine();
        let mut ctx = Context::new();
        ctx.insert("author", Value::String("Dave".into()));
        ctx.insert("number", Value::Number(99));
        ctx.insert("title", Value::String("Server returns 500 on logout".into()));
        let mut session = Session::new();

        let result = engine.render(&mut session, "git.issue_opened", &ctx).unwrap();
        assert!(result.contains("#99"), "got: {result}");
    }

    #[test]
    fn review_changes_requested_pluralizes_comments() {
        let engine = engine();
        let mut ctx = Context::new();
        ctx.insert("reviewer", Value::String("Eve".into()));
        ctx.insert("pr_number", Value::Number(7));
        ctx.insert("comment_count", Value::Number(3));
        let mut session = Session::new();

        let result = engine.render(&mut session, "git.review_changes_requested", &ctx).unwrap();
        assert!(result.contains("3 comments"), "got: {result}");
    }

    #[test]
    fn release_event_with_changes_count() {
        let engine = engine();
        let mut ctx = Context::new();
        ctx.insert("version", Value::String("1.2.0".into()));
        ctx.insert("tag", Value::String("v1.2.0".into()));
        ctx.insert("changes_count", Value::Number(14));
        let mut session = Session::new();

        let result = engine.render(&mut session, "git.release", &ctx).unwrap();
        assert!(result.contains("1.2.0"), "got: {result}");
        assert!(result.contains("14 changes"), "got: {result}");
    }

    #[test]
    fn low_salience_falls_through_for_small_numbers() {
        // Low salience registered for commits.
        let engine = engine();
        // Explicit salience override.
        let mut ctx = Context::new();
        ctx.insert("author", Value::String("Alice".into()));
        ctx.insert("salience", Value::String("low".into()));
        ctx.insert("files_changed", Value::Number(1));
        ctx.insert("additions", Value::Number(2));
        ctx.insert("deletions", Value::Number(1));

        let mut session = Session::new();
        // Reset in case other tests' discourse state leaked into this
        // engine instance (no-op on a fresh session, documents intent).
        session.reset();

        let result = engine.render(&mut session, "git.commit", &ctx).unwrap();
        assert!(
            result.contains("Alice"),
            "got: {result}"
        );
    }
}
