//! Release-note vocabulary for the `prosaic` engine.
//!
//! Registers event templates covering the lifecycle of a software release:
//! version tagging, feature additions, breaking changes, bug fixes, security
//! fixes, deprecations, dependency updates, contributor summaries, stats, and
//! overall release summary.
//!
//! Every event type is registered at two or three salience levels so the
//! engine's importance-aware verbosity picks appropriate phrasing automatically.
//!
//! # Context keys per event
//!
//! - `release.tagged`              — `version`, `title` (opt), `commit_sha` (opt), `date` (opt)
//! - `release.feature_added`       — `name`, `description` (opt)
//! - `release.breaking_change`     — `name`, `migration_path` (opt)
//! - `release.bugfix`              — `description`, `issue_number` (opt)
//! - `release.security_fix`        — `description`, `cve` (opt), `severity` (opt)
//! - `release.deprecation`         — `name`, `replacement` (opt), `removal_version` (opt)
//! - `release.dependency_update`   — `name`, `from_version`, `to_version`, `reason` (opt)
//! - `release.contributor_summary` — `count`, `top_contributors` (list, opt)
//! - `release.stats`               — `commits`, `files_changed`, `insertions`, `deletions`
//! - `release.summary`             — `version`, `headline`

use prosaic_core::{Engine, ProsaicError, Salience};

/// Register the full release-note vocabulary into an engine.
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
    fn tagged_with_title_and_date() {
        let engine = engine();
        let mut ctx = Context::new();
        ctx.insert("version", Value::String("1.2.0".into()));
        ctx.insert("title", Value::String("Retry pipeline".into()));
        ctx.insert("date", Value::String("2026-04-01".into()));
        let mut session = Session::new();
        let out = engine.render(&mut session, "release.tagged", &ctx).unwrap();
        assert!(out.contains("1.2.0"), "got: {out}");
    }

    #[test]
    fn tagged_without_optional_slots() {
        let engine = engine();
        let mut ctx = Context::new();
        ctx.insert("version", Value::String("2.0.0".into()));
        let mut session = Session::new();
        let out = engine.render(&mut session, "release.tagged", &ctx).unwrap();
        assert!(out.contains("2.0.0"), "got: {out}");
        // No "None" or "null" in output — optional slots must be suppressed.
        assert!(!out.contains("None"), "got: {out}");
    }

    #[test]
    fn feature_added_with_description() {
        let engine = engine();
        let mut ctx = Context::new();
        ctx.insert("name", Value::String("streaming API".into()));
        ctx.insert(
            "description",
            Value::String("server-sent events over HTTP".into()),
        );
        let mut session = Session::new();
        let out = engine
            .render(&mut session, "release.feature_added", &ctx)
            .unwrap();
        assert!(out.contains("streaming API"), "got: {out}");
    }

    #[test]
    fn feature_added_without_description() {
        let engine = engine();
        let mut ctx = Context::new();
        ctx.insert("name", Value::String("dark mode".into()));
        let mut session = Session::new();
        let out = engine
            .render(&mut session, "release.feature_added", &ctx)
            .unwrap();
        assert!(out.contains("dark mode"), "got: {out}");
    }

    #[test]
    fn breaking_change_with_migration() {
        let engine = engine();
        let mut ctx = Context::new();
        ctx.insert("name", Value::String("Config::load()".into()));
        ctx.insert(
            "migration_path",
            Value::String("use Config::from_file() instead".into()),
        );
        // Breaking changes are High-salience only, so salience override needed
        // for Fixed variation to reach them.
        ctx.insert("salience", Value::String("high".into()));
        let mut session = Session::new();
        let out = engine
            .render(&mut session, "release.breaking_change", &ctx)
            .unwrap();
        assert!(out.contains("Config::load()"), "got: {out}");
        assert!(out.contains("Config::from_file()"), "got: {out}");
    }

    #[test]
    fn bugfix_with_issue_number() {
        let engine = engine();
        let mut ctx = Context::new();
        ctx.insert(
            "description",
            Value::String("connection pool exhaustion under load".into()),
        );
        ctx.insert("issue_number", Value::Number(512));
        let mut session = Session::new();
        let out = engine.render(&mut session, "release.bugfix", &ctx).unwrap();
        assert!(out.contains("connection pool"), "got: {out}");
        assert!(out.contains("512"), "got: {out}");
    }

    #[test]
    fn security_fix_with_cve() {
        let engine = engine();
        let mut ctx = Context::new();
        ctx.insert(
            "description",
            Value::String("path traversal in file upload handler".into()),
        );
        ctx.insert("cve", Value::String("CVE-2026-1234".into()));
        ctx.insert("severity", Value::String("high".into()));
        ctx.insert("salience", Value::String("high".into()));
        let mut session = Session::new();
        let out = engine
            .render(&mut session, "release.security_fix", &ctx)
            .unwrap();
        assert!(out.contains("path traversal"), "got: {out}");
        assert!(out.contains("CVE-2026-1234"), "got: {out}");
    }

    #[test]
    fn deprecation_with_replacement_and_removal() {
        let engine = engine();
        let mut ctx = Context::new();
        ctx.insert("name", Value::String("WidgetFactory::create()".into()));
        ctx.insert(
            "replacement",
            Value::String("WidgetBuilder::build()".into()),
        );
        ctx.insert("removal_version", Value::String("3.0.0".into()));
        let mut session = Session::new();
        let out = engine
            .render(&mut session, "release.deprecation", &ctx)
            .unwrap();
        assert!(out.contains("WidgetFactory"), "got: {out}");
        assert!(out.contains("WidgetBuilder"), "got: {out}");
        assert!(out.contains("3.0.0"), "got: {out}");
    }

    #[test]
    fn dependency_update_with_reason() {
        let engine = engine();
        let mut ctx = Context::new();
        ctx.insert("name", Value::String("tokio".into()));
        ctx.insert("from_version", Value::String("1.35".into()));
        ctx.insert("to_version", Value::String("1.40".into()));
        ctx.insert(
            "reason",
            Value::String("security patch for task scheduler".into()),
        );
        let mut session = Session::new();
        let out = engine
            .render(&mut session, "release.dependency_update", &ctx)
            .unwrap();
        assert!(out.contains("tokio"), "got: {out}");
        assert!(out.contains("1.35"), "got: {out}");
        assert!(out.contains("1.40"), "got: {out}");
    }

    #[test]
    fn contributor_summary_pluralizes() {
        let engine = engine();
        let mut ctx = Context::new();
        ctx.insert("count", Value::Number(7));
        ctx.insert(
            "top_contributors",
            Value::List(vec![
                "Alice".into(),
                "Bob".into(),
                "Carol".into(),
                "Dave".into(),
            ]),
        );
        let mut session = Session::new();
        let out = engine
            .render(&mut session, "release.contributor_summary", &ctx)
            .unwrap();
        assert!(out.contains("7 contributors"), "got: {out}");
    }

    #[test]
    fn contributor_summary_singular() {
        let engine = engine();
        let mut ctx = Context::new();
        ctx.insert("count", Value::Number(1));
        let mut session = Session::new();
        let out = engine
            .render(&mut session, "release.contributor_summary", &ctx)
            .unwrap();
        assert!(out.contains("1 contributor"), "got: {out}");
    }

    #[test]
    fn stats_full() {
        let engine = engine();
        let mut ctx = Context::new();
        ctx.insert("commits", Value::Number(42));
        ctx.insert("files_changed", Value::Number(18));
        ctx.insert("insertions", Value::Number(300));
        ctx.insert("deletions", Value::Number(50));
        let mut session = Session::new();
        let out = engine.render(&mut session, "release.stats", &ctx).unwrap();
        assert!(out.contains("42 commits"), "got: {out}");
        assert!(out.contains("18 files"), "got: {out}");
    }

    #[test]
    fn summary_renders_version_and_headline() {
        let engine = engine();
        let mut ctx = Context::new();
        ctx.insert("version", Value::String("3.0.0".into()));
        ctx.insert(
            "headline",
            Value::String("Major architecture overhaul for scalability".into()),
        );
        let mut session = Session::new();
        let out = engine
            .render(&mut session, "release.summary", &ctx)
            .unwrap();
        assert!(out.contains("3.0.0"), "got: {out}");
        assert!(out.contains("overhaul"), "got: {out}");
    }
}
