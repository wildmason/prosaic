//! English-language release-note templates.
//!
//! This module holds the concrete template strings for the `en` (English)
//! locale. When additional locales are added (e.g. `es`, `de`), they will
//! each live in a sibling module and expose the same `register` signature,
//! making locale-aware dispatch straightforward without touching callers.

use prosaic_core::{Engine, ProsaicError, Salience};

/// Register the full English release-note vocabulary into an engine.
pub fn register(engine: &mut Engine) -> Result<(), ProsaicError> {
    register_tagged(engine)?;
    register_feature_added(engine)?;
    register_breaking_change(engine)?;
    register_bugfix(engine)?;
    register_security_fix(engine)?;
    register_deprecation(engine)?;
    register_dependency_update(engine)?;
    register_contributor_summary(engine)?;
    register_stats(engine)?;
    register_summary(engine)?;
    Ok(())
}

fn register_tagged(engine: &mut Engine) -> Result<(), ProsaicError> {
    // Low: bare tag announcement.
    engine.register_template_at(
        "release.tagged",
        "Version {version} was tagged",
        Salience::Low,
    )?;

    // Medium: includes optional title.
    engine.register_template(
        "release.tagged",
        "Release {version}{?title} \u{2014} {title}{/?} is now tagged",
    )?;

    // High: full detail with optional commit sha and date.
    engine.register_template_at(
        "release.tagged",
        "Version {version}{?title} (\u{201c}{title}\u{201d}){/?} has been tagged \
         and published{?date} on {date}{/?}{?commit_sha} at commit {commit_sha}{/?}",
        Salience::High,
    )?;

    Ok(())
}

fn register_feature_added(engine: &mut Engine) -> Result<(), ProsaicError> {
    // Low: terse.
    engine.register_template_at(
        "release.feature_added",
        "Added {name}",
        Salience::Low,
    )?;

    // Medium: names the feature, surfaces description if present.
    engine.register_template(
        "release.feature_added",
        "New feature: {name}{?description} \u{2014} {description}{/?}",
    )?;

    // High: elaborated announcement encouraging adoption.
    engine.register_template_at(
        "release.feature_added",
        "This release introduces {name}{?description}. {description}{/?} \
         Existing users may want to try it.",
        Salience::High,
    )?;

    Ok(())
}

fn register_breaking_change(engine: &mut Engine) -> Result<(), ProsaicError> {
    // Breaking changes are always high salience — only one tier registered.
    engine.register_template_at(
        "release.breaking_change",
        "{name} introduces a breaking change{?migration_path}; \
         migration: {migration_path}{/?}",
        Salience::High,
    )?;
    engine.register_template_at(
        "release.breaking_change",
        "Breaking: {name}{?migration_path}. To migrate: {migration_path}{/?}",
        Salience::High,
    )?;

    Ok(())
}

fn register_bugfix(engine: &mut Engine) -> Result<(), ProsaicError> {
    // Low: just the description.
    engine.register_template_at(
        "release.bugfix",
        "Fixed: {description}",
        Salience::Low,
    )?;

    // Medium: links to issue number when present.
    engine.register_template(
        "release.bugfix",
        "Bug fix: {description}{?issue_number} (issue #{issue_number}){/?}",
    )?;

    Ok(())
}

fn register_security_fix(engine: &mut Engine) -> Result<(), ProsaicError> {
    // Security fixes are always high salience.
    engine.register_template_at(
        "release.security_fix",
        "Security fix: {description}{?severity} (severity: {severity}){/?}\
         {?cve} \u{2014} {cve}{/?}",
        Salience::High,
    )?;
    engine.register_template_at(
        "release.security_fix",
        "A security vulnerability has been addressed: {description}{?cve} ({cve}){/?}",
        Salience::High,
    )?;

    Ok(())
}

fn register_deprecation(engine: &mut Engine) -> Result<(), ProsaicError> {
    // Medium: names the deprecated item.
    engine.register_template(
        "release.deprecation",
        "{name} is deprecated{?replacement} in favour of {replacement}{/?}\
         {?removal_version} and will be removed in {removal_version}{/?}",
    )?;

    // High: more emphatic warning.
    engine.register_template_at(
        "release.deprecation",
        "Deprecation notice: {name} is now deprecated{?replacement} \
         \u{2014} use {replacement} instead{/?}{?removal_version}. \
         It will be removed in {removal_version}{/?}",
        Salience::High,
    )?;

    Ok(())
}

fn register_dependency_update(engine: &mut Engine) -> Result<(), ProsaicError> {
    // Low: terse version bump.
    engine.register_template_at(
        "release.dependency_update",
        "{name} updated from {from_version} to {to_version}",
        Salience::Low,
    )?;

    // Medium: adds optional reason.
    engine.register_template(
        "release.dependency_update",
        "Dependency {name} bumped from {from_version} to {to_version}\
         {?reason} \u{2014} {reason}{/?}",
    )?;

    Ok(())
}

fn register_contributor_summary(engine: &mut Engine) -> Result<(), ProsaicError> {
    // Low: bare count.
    engine.register_template_at(
        "release.contributor_summary",
        "{count} {count|pluralize:contributor} contributed to this release",
        Salience::Low,
    )?;

    // Medium: names top contributors when available.
    engine.register_template(
        "release.contributor_summary",
        "This release had {count} {count|pluralize:contributor}\
         {?top_contributors}: {top_contributors|truncate:3|join}{/?}",
    )?;

    // High: recognition-focused phrasing.
    engine.register_template_at(
        "release.contributor_summary",
        "Thanks to all {count} {count|pluralize:contributor} who made this \
         release possible{?top_contributors}, including {top_contributors|truncate:3|join}{/?}",
        Salience::High,
    )?;

    Ok(())
}

fn register_stats(engine: &mut Engine) -> Result<(), ProsaicError> {
    // Low: commit count only.
    engine.register_template_at(
        "release.stats",
        "{commits} {commits|pluralize:commit} since the last release",
        Salience::Low,
    )?;

    // Medium: full diff stats.
    engine.register_template(
        "release.stats",
        "{commits} {commits|pluralize:commit} across {files_changed} \
         {files_changed|pluralize:file}, adding {insertions} \
         {insertions|pluralize:line} and removing {deletions} {deletions|pluralize:line}",
    )?;

    Ok(())
}

fn register_summary(engine: &mut Engine) -> Result<(), ProsaicError> {
    // Medium: concise headline.
    engine.register_template(
        "release.summary",
        "{version}: {headline}",
    )?;

    // High: elaborated release announcement.
    engine.register_template_at(
        "release.summary",
        "Version {version} is out. {headline}",
        Salience::High,
    )?;

    Ok(())
}
