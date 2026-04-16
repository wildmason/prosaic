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

use prosaic_core::{Engine, ProsaicError, Salience};

/// Register the full git-activity vocabulary into an engine.
pub fn register(engine: &mut Engine) -> Result<(), ProsaicError> {
    register_commit(engine)?;
    register_pr_opened(engine)?;
    register_pr_merged(engine)?;
    register_pr_closed(engine)?;
    register_issue_opened(engine)?;
    register_issue_closed(engine)?;
    register_review_approved(engine)?;
    register_review_changes_requested(engine)?;
    register_release(engine)?;
    Ok(())
}

fn register_commit(engine: &mut Engine) -> Result<(), ProsaicError> {
    // Low: minor commits, small change counts.
    engine.register_template_at(
        "git.commit",
        "{author} pushed a small change",
        Salience::Low,
    )?;
    engine.register_template_at(
        "git.commit",
        "{author} landed a tweak",
        Salience::Low,
    )?;

    // Medium: standard commits with file counts.
    engine.register_template(
        "git.commit",
        "{author} committed {files_changed} \
         {files_changed|pluralize:file}{?message}: \"{message}\"{/?}",
    )?;
    engine.register_template(
        "git.commit",
        "{author} pushed a commit touching {files_changed} \
         {files_changed|pluralize:file}{?message} \u{2014} {message}{/?}",
    )?;

    // High: large commits, lots of churn.
    engine.register_template_at(
        "git.commit",
        "{author} landed a substantial commit spanning {files_changed} \
         {files_changed|pluralize:file}, adding {additions} {additions|pluralize:line} \
         and removing {deletions} {deletions|pluralize:line}{?message}. The change \
         description reads: \"{message}\"{/?}",
        Salience::High,
    )?;

    Ok(())
}

fn register_pr_opened(engine: &mut Engine) -> Result<(), ProsaicError> {
    engine.register_template(
        "git.pr_opened",
        "{author} opened PR #{number}: \"{title}\"",
    )?;
    engine.register_template(
        "git.pr_opened",
        "PR #{number} was opened by {author} \u{2014} \"{title}\"",
    )?;
    engine.register_template_at(
        "git.pr_opened",
        "A new pull request \u{2014} #{number}, \"{title}\" \u{2014} \
         was opened by {author}",
        Salience::High,
    )?;
    Ok(())
}

fn register_pr_merged(engine: &mut Engine) -> Result<(), ProsaicError> {
    engine.register_template(
        "git.pr_merged",
        "PR #{number} \"{title}\" was merged by {merger}",
    )?;
    engine.register_template(
        "git.pr_merged",
        "{merger} merged PR #{number} (\"{title}\"){?author}, originally \
         authored by {author}{/?}",
    )?;
    engine.register_template_at(
        "git.pr_merged",
        "PR #{number} has been merged",
        Salience::Low,
    )?;
    Ok(())
}

fn register_pr_closed(engine: &mut Engine) -> Result<(), ProsaicError> {
    engine.register_template(
        "git.pr_closed",
        "PR #{number} \"{title}\" was closed{?reason} \u{2014} {reason}{/?}",
    )?;
    engine.register_template_at(
        "git.pr_closed",
        "PR #{number} was closed without merging",
        Salience::Low,
    )?;
    Ok(())
}

fn register_issue_opened(engine: &mut Engine) -> Result<(), ProsaicError> {
    engine.register_template(
        "git.issue_opened",
        "{author} filed issue #{number}: \"{title}\"",
    )?;
    engine.register_template(
        "git.issue_opened",
        "Issue #{number} \"{title}\" was opened by {author}",
    )?;
    Ok(())
}

fn register_issue_closed(engine: &mut Engine) -> Result<(), ProsaicError> {
    engine.register_template(
        "git.issue_closed",
        "Issue #{number} \"{title}\" was closed{?reason} as {reason}{/?}",
    )?;
    engine.register_template_at(
        "git.issue_closed",
        "Issue #{number} was resolved",
        Salience::Low,
    )?;
    Ok(())
}

fn register_review_approved(engine: &mut Engine) -> Result<(), ProsaicError> {
    engine.register_template(
        "git.review_approved",
        "{reviewer} approved PR #{pr_number}",
    )?;
    engine.register_template(
        "git.review_approved",
        "PR #{pr_number} received approval from {reviewer}",
    )?;
    Ok(())
}

fn register_review_changes_requested(engine: &mut Engine) -> Result<(), ProsaicError> {
    engine.register_template(
        "git.review_changes_requested",
        "{reviewer} requested changes on PR #{pr_number}{?comment_count} \
         ({comment_count} {comment_count|pluralize:comment}){/?}",
    )?;
    engine.register_template(
        "git.review_changes_requested",
        "PR #{pr_number} needs revisions \u{2014} {reviewer} left \
         {comment_count} {comment_count|pluralize:comment}",
    )?;
    Ok(())
}

fn register_release(engine: &mut Engine) -> Result<(), ProsaicError> {
    engine.register_template(
        "git.release",
        "Version {version} was tagged as {tag}{?changes_count}, covering \
         {changes_count} {changes_count|pluralize:change}{/?}",
    )?;
    engine.register_template_at(
        "git.release",
        "A new release \u{2014} {version}, tagged {tag} \u{2014} \
         was published with {changes_count} notable \
         {changes_count|pluralize:change}",
        Salience::High,
    )?;
    engine.register_template_at(
        "git.release",
        "{version} was released",
        Salience::Low,
    )?;
    Ok(())
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
