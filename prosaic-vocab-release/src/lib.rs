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

use prosaic_core::{Engine, ProsaicError};

/// English-language templates for release-note events.
///
/// Each locale module exposes a `register` function with the same signature,
/// enabling locale-aware dispatch without changing callers.
pub mod en;

/// Spanish-language templates for release-note events.
pub mod es;

/// Register the full release-note vocabulary into an engine.
///
/// Delegates to the English locale. When multilingual support is added,
/// callers can opt into a specific locale via `register_locale`.
pub fn register(engine: &mut Engine) -> Result<(), ProsaicError> {
    en::register(engine)
}

/// Register Spanish release-note vocabulary templates into an engine.
///
/// Provides the same template keys as [`register`] but with idiomatic
/// Spanish surface text for use with a Spanish grammar layer.
pub fn register_es(engine: &mut Engine) -> Result<(), ProsaicError> {
    es::register(engine)
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

#[cfg(test)]
mod tests_es {
    use super::*;
    use prosaic_core::{Context, Session, Strictness, Value, Variation};
    use prosaic_grammar_es::Spanish;

    fn engine() -> Engine {
        let mut e = Engine::new(Spanish::new())
            .strictness(Strictness::Strict)
            .variation(Variation::Fixed);
        register_es(&mut e).unwrap();
        e
    }

    #[test]
    fn tagged_with_title() {
        let engine = engine();
        let mut ctx = Context::new();
        ctx.insert("version", Value::String("1.2.0".into()));
        ctx.insert("title", Value::String("Pipeline de reintentos".into()));
        ctx.insert("date", Value::String("2026-04-01".into()));
        let mut session = Session::new();
        let out = engine.render(&mut session, "release.tagged", &ctx).unwrap();
        assert!(out.contains("1.2.0"), "got: {out}");
        assert!(
            out.contains("etiquetada") || out.contains("etiquetada"),
            "Expected Spanish tagged phrase, got: {out}"
        );
    }

    #[test]
    fn feature_added_with_description() {
        let engine = engine();
        let mut ctx = Context::new();
        ctx.insert("name", Value::String("API de streaming".into()));
        ctx.insert(
            "description",
            Value::String("eventos enviados por el servidor vía HTTP".into()),
        );
        let mut session = Session::new();
        let out = engine
            .render(&mut session, "release.feature_added", &ctx)
            .unwrap();
        assert!(out.contains("API de streaming"), "got: {out}");
        assert!(
            out.contains("funcionalidad") || out.contains("Nueva") || out.contains("introduce"),
            "Expected Spanish feature phrase, got: {out}"
        );
    }

    #[test]
    fn breaking_change_with_migration() {
        let engine = engine();
        let mut ctx = Context::new();
        ctx.insert("name", Value::String("Config::load()".into()));
        ctx.insert(
            "migration_path",
            Value::String("use Config::from_file() en su lugar".into()),
        );
        ctx.insert("salience", Value::String("high".into()));
        let mut session = Session::new();
        let out = engine
            .render(&mut session, "release.breaking_change", &ctx)
            .unwrap();
        assert!(out.contains("Config::load()"), "got: {out}");
        assert!(
            out.contains("disruptivo") || out.contains("Disruptivo"),
            "Expected 'disruptivo', got: {out}"
        );
    }

    #[test]
    fn bugfix_with_issue_number() {
        let engine = engine();
        let mut ctx = Context::new();
        ctx.insert(
            "description",
            Value::String("agotamiento del pool de conexiones bajo carga".into()),
        );
        ctx.insert("issue_number", Value::Number(512));
        let mut session = Session::new();
        let out = engine.render(&mut session, "release.bugfix", &ctx).unwrap();
        assert!(out.contains("512"), "got: {out}");
        assert!(
            out.contains("Corregido") || out.contains("error") || out.contains("incidencia"),
            "Expected Spanish bugfix phrase, got: {out}"
        );
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
        assert!(
            out.contains("7 colaboradores"),
            "Expected '7 colaboradores', got: {out}"
        );
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
        assert!(out.contains("42"), "got: {out}");
        assert!(out.contains("18"), "got: {out}");
        assert!(
            out.contains("archivo"),
            "Expected Spanish 'archivo', got: {out}"
        );
    }

    #[test]
    fn summary_renders_version_and_headline() {
        let engine = engine();
        let mut ctx = Context::new();
        ctx.insert("version", Value::String("3.0.0".into()));
        ctx.insert(
            "headline",
            Value::String("Revisión arquitectónica importante para escalabilidad".into()),
        );
        let mut session = Session::new();
        let out = engine
            .render(&mut session, "release.summary", &ctx)
            .unwrap();
        assert!(out.contains("3.0.0"), "got: {out}");
        assert!(out.contains("Revisión"), "got: {out}");
    }
}
