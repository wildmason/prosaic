//! English-language PR-narrative templates.
//!
//! This module holds the concrete template strings for the `en` (English)
//! locale. When additional locales are added (e.g. `es`, `de`), they will
//! each live in a sibling module and expose the same `register` signature,
//! making locale-aware dispatch straightforward without touching callers.

use prosaic_core::{Engine, ProsaicError, Salience};

/// Register the full English PR-narrative vocabulary into an engine.
pub fn register(engine: &mut Engine) -> Result<(), ProsaicError> {
    register_summary(engine)?;
    register_review_state(engine)?;
    register_scope(engine)?;
    register_diff_stats(engine)?;
    register_age(engine)?;
    register_merge_readiness(engine)?;
    register_ci_status(engine)?;
    register_related_prs(engine)?;
    Ok(())
}

fn register_summary(engine: &mut Engine) -> Result<(), ProsaicError> {
    // Low: terse title reference only.
    engine.register_template_at(
        "pr.summary",
        "PR #{number}: {title}",
        Salience::Low,
    )?;

    // Medium: adds author.
    engine.register_template(
        "pr.summary",
        "PR #{number} \u{2014} \u{201c}{title}\u{201d} by {author}",
    )?;

    // High: full detail including commit and file counts.
    engine.register_template_at(
        "pr.summary",
        "PR #{number} by {author}: \u{201c}{title}\u{201d} \u{2014} \
         {commit_count} {commit_count|pluralize:commit} across \
         {files_changed} {files_changed|pluralize:file}",
        Salience::High,
    )?;

    Ok(())
}

fn register_review_state(engine: &mut Engine) -> Result<(), ProsaicError> {
    // Low: approval count only.
    engine.register_template_at(
        "pr.review_state",
        "PR #{number} has {approvals} {approvals|pluralize:approval}",
        Salience::Low,
    )?;

    // Medium: full review picture including requested changes and pending.
    engine.register_template(
        "pr.review_state",
        "PR #{number} has {approvals} {approvals|pluralize:approval}\
         {?requested_changes}, {requested_changes} \
         {requested_changes|pluralize:request} for changes{/?}\
         {?pending}, and {pending} pending {pending|pluralize:review}{/?}",
    )?;

    Ok(())
}

fn register_scope(engine: &mut Engine) -> Result<(), ProsaicError> {
    // Low: terse area count.
    engine.register_template_at(
        "pr.scope",
        "PR #{number} touches {areas|truncate:3|join}",
        Salience::Low,
    )?;

    // Medium: framed as scope statement.
    engine.register_template(
        "pr.scope",
        "PR #{number} spans {areas|truncate:3|join}",
    )?;

    Ok(())
}

fn register_diff_stats(engine: &mut Engine) -> Result<(), ProsaicError> {
    // Low only — diff stats are supplementary detail.
    engine.register_template_at(
        "pr.diff_stats",
        "PR #{number}: +{insertions} \u{2212}{deletions} {deletions|pluralize:line}",
        Salience::Low,
    )?;

    engine.register_template(
        "pr.diff_stats",
        "PR #{number} adds {insertions} {insertions|pluralize:line} \
         and removes {deletions} {deletions|pluralize:line}",
    )?;

    Ok(())
}

fn register_age(engine: &mut Engine) -> Result<(), ProsaicError> {
    // Low: bare age.
    engine.register_template_at(
        "pr.age",
        "PR #{number} has been open for {days_open} {days_open|pluralize:day}",
        Salience::Low,
    )?;

    // Medium: age plus stale flag via |choose.
    engine.register_template(
        "pr.age",
        "PR #{number} has been open for {days_open} \
         {days_open|pluralize:day}{stale|choose: 1= and is stale, default=}",
    )?;

    Ok(())
}

fn register_merge_readiness(engine: &mut Engine) -> Result<(), ProsaicError> {
    // Medium: ready/not-ready with optional blockers.
    engine.register_template(
        "pr.merge_readiness",
        "PR #{number} is {ready|choose: 1=ready to merge, default=not yet ready}\
         {?blockers}, blocked by {blockers|truncate:3|join}{/?}",
    )?;

    // High: more emphatic framing.
    engine.register_template_at(
        "pr.merge_readiness",
        "PR #{number} \u{2014} {ready|choose: 1=this PR is ready to merge, \
         default=this PR is not yet ready}{?blockers}. \
         Blockers: {blockers|truncate:3|join}{/?}",
        Salience::High,
    )?;

    Ok(())
}

fn register_ci_status(engine: &mut Engine) -> Result<(), ProsaicError> {
    // Low: passing count only.
    engine.register_template_at(
        "pr.ci_status",
        "PR #{number}: {passing} CI {passing|pluralize:check} passing",
        Salience::Low,
    )?;

    // Medium: passing plus failing counts.
    engine.register_template(
        "pr.ci_status",
        "PR #{number} has {passing} {passing|pluralize:check} passing\
         {?failing} and {failing} {failing|pluralize:check} failing{/?}",
    )?;

    Ok(())
}

fn register_related_prs(engine: &mut Engine) -> Result<(), ProsaicError> {
    // Medium only — relational context is inherently mid-verbosity.
    engine.register_template(
        "pr.related_prs",
        "PR #{number}{?depends_on} depends on {depends_on|truncate:3|join}{/?}\
         {?blocks} and blocks {blocks|truncate:3|join}{/?}",
    )?;

    Ok(())
}
