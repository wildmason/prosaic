//! English-language git-activity templates.
//!
//! This module holds the concrete template strings for the `en` (English)
//! locale. When additional locales are added (e.g. `es`, `de`), they will
//! each live in a sibling module and expose the same `register` signature,
//! making locale-aware dispatch straightforward without touching callers.

use prosaic_core::{Engine, ProsaicError, Salience};

/// Register the full English git-activity vocabulary into an engine.
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
    engine.register_template_at("git.commit", "{author} landed a tweak", Salience::Low)?;

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
    engine.register_template("git.pr_opened", "{author} opened PR #{number}: \"{title}\"")?;
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
    engine.register_template("git.review_approved", "{reviewer} approved PR #{pr_number}")?;
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
    engine.register_template_at("git.release", "{version} was released", Salience::Low)?;
    Ok(())
}
