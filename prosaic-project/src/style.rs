//! `prosaic.toml` schema for the `[style_profile]` section.
//!
//! `StyleProfileConfig` is the TOML-friendly mirror of
//! [`prosaic_core::StyleProfile`]. It uses string keys for RST-relation
//! pools (TOML can't key tables by Rust enum variants directly), keeps
//! every field optional so projects can declare just the dials they care
//! about, and supports a single `extends = "path"` reference to another
//! profile TOML for file-level composition.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use prosaic_core::{
    ConnectivePreferences, HedgingCalibration, LengthDistribution, ListStyleBias, PronounDensity,
    RstRelation, SalienceBias, StyleProfile, StyleProfileError, Verbosity,
};
use serde::{Deserialize, Serialize};

use crate::error::ProjectError;

/// TOML representation of a [`StyleProfile`].
///
/// Every field is optional so an authoring file can declare just the dials
/// it cares about; missing fields fall through to the neutral default.
/// `extends = "path"` loads another `StyleProfileConfig` as the base; the
/// inline `Some(_)` fields then override per-dial.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StyleProfileConfig {
    /// Optional path (relative to the manifest's directory) to another
    /// profile TOML to use as the base. Inline fields below override the
    /// base per-dial.
    pub extends: Option<String>,
    pub name: Option<String>,
    pub verbosity: Option<String>,
    pub list_style_bias: Option<String>,
    pub pronoun_density: Option<String>,
    pub salience: Option<String>,
    pub sentence_length: Option<LengthDistributionConfig>,
    pub connectives: Option<ConnectivePreferencesConfig>,
    pub hedging: Option<HedgingCalibrationConfig>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LengthDistributionConfig {
    pub short: Option<f32>,
    pub medium: Option<f32>,
    pub long: Option<f32>,
    pub short_max_words: Option<u16>,
    pub medium_max_words: Option<u16>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ConnectivePreferencesConfig {
    /// Per-RST-relation allowed connective pools. Keys are lowercase
    /// `RstRelation` variant names: `elaboration`, `contrast`, `cause`,
    /// `result`, `concession`, `sequence`, `condition`, `background`,
    /// `summary`. Unknown keys are rejected at parse time so typos
    /// surface as clear errors instead of silently no-op'ing.
    pub allowed: Option<HashMap<String, Vec<String>>>,
    /// Per-RST-relation tie-breaker weights. Inner shape is array of
    /// `[connective, weight]` pairs.
    pub preferred: Option<HashMap<String, Vec<(String, f32)>>>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HedgingCalibrationConfig {
    pub offset: Option<i8>,
    pub forbid: Option<Vec<String>>,
}

impl StyleProfileConfig {
    /// Convert this TOML config into a validated [`StyleProfile`].
    ///
    /// `manifest_dir` is the directory containing the project's
    /// `prosaic.toml`; relative `extends` paths resolve from there. The
    /// returned profile has been run through [`StyleProfile::validate`]
    /// — invalid configurations surface as `ProjectError::ManifestStyle`
    /// rather than silently producing a half-built profile.
    pub fn into_style_profile(self, manifest_dir: &Path) -> Result<StyleProfile, ProjectError> {
        let merged = self.resolve(manifest_dir, &mut Vec::new())?;
        merged.build_profile()
    }

    fn resolve(
        self,
        manifest_dir: &Path,
        seen: &mut Vec<PathBuf>,
    ) -> Result<StyleProfileConfig, ProjectError> {
        // Walk `extends` chain depth-first, base-first. The accumulated
        // base config gets overlaid by self's `Some(_)` fields at the end.
        let base = if let Some(ext_path) = &self.extends {
            let mut path = manifest_dir.join(ext_path);
            if !path.is_absolute() {
                path = manifest_dir.join(ext_path);
            }
            let canonical = path.canonicalize().unwrap_or(path.clone());
            if seen.iter().any(|p| p == &canonical) {
                return Err(ProjectError::ManifestStyle {
                    reason: format!(
                        "extends cycle detected: `{}` is already in the resolution chain",
                        path.display()
                    ),
                });
            }
            seen.push(canonical);
            let text = std::fs::read_to_string(&path).map_err(|e| ProjectError::Io {
                path: path.display().to_string(),
                cause: e.to_string(),
            })?;
            let parent = path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| manifest_dir.to_path_buf());
            let parsed: StyleProfileConfig =
                toml::from_str(&text).map_err(|e| ProjectError::TomlParse {
                    file: path.display().to_string(),
                    cause: e.to_string(),
                })?;
            Some(parsed.resolve(&parent, seen)?)
        } else {
            None
        };

        Ok(merge_overlay(base.unwrap_or_default(), self))
    }

    fn build_profile(self) -> Result<StyleProfile, ProjectError> {
        let mut builder =
            StyleProfile::builder(self.name.unwrap_or_else(|| String::from("default")));
        if let Some(v) = self.verbosity {
            builder = builder.verbosity(parse_verbosity(&v)?);
        }
        if let Some(l) = self.list_style_bias {
            builder = builder.list_style_bias(parse_list_style_bias(&l)?);
        }
        if let Some(p) = self.pronoun_density {
            builder = builder.pronoun_density(parse_pronoun_density(&p)?);
        }
        if let Some(s) = self.salience {
            builder = builder.salience(parse_salience_bias(&s)?);
        }
        if let Some(sl) = self.sentence_length {
            builder = builder.sentence_length(build_length_distribution(sl));
        }
        if let Some(c) = self.connectives {
            builder = builder.connectives(build_connective_preferences(c)?);
        }
        if let Some(h) = self.hedging {
            builder = builder.hedging(build_hedging_calibration(h));
        }
        builder.build().map_err(map_style_error)
    }
}

fn merge_overlay(base: StyleProfileConfig, overlay: StyleProfileConfig) -> StyleProfileConfig {
    StyleProfileConfig {
        extends: None, // resolved already
        name: overlay.name.or(base.name),
        verbosity: overlay.verbosity.or(base.verbosity),
        list_style_bias: overlay.list_style_bias.or(base.list_style_bias),
        pronoun_density: overlay.pronoun_density.or(base.pronoun_density),
        salience: overlay.salience.or(base.salience),
        sentence_length: merge_length(base.sentence_length, overlay.sentence_length),
        connectives: merge_connectives(base.connectives, overlay.connectives),
        hedging: merge_hedging(base.hedging, overlay.hedging),
    }
}

fn merge_length(
    base: Option<LengthDistributionConfig>,
    overlay: Option<LengthDistributionConfig>,
) -> Option<LengthDistributionConfig> {
    match (base, overlay) {
        (None, o) => o,
        (b, None) => b,
        (Some(b), Some(o)) => Some(LengthDistributionConfig {
            short: o.short.or(b.short),
            medium: o.medium.or(b.medium),
            long: o.long.or(b.long),
            short_max_words: o.short_max_words.or(b.short_max_words),
            medium_max_words: o.medium_max_words.or(b.medium_max_words),
        }),
    }
}

fn merge_connectives(
    base: Option<ConnectivePreferencesConfig>,
    overlay: Option<ConnectivePreferencesConfig>,
) -> Option<ConnectivePreferencesConfig> {
    match (base, overlay) {
        (None, o) => o,
        (b, None) => b,
        (Some(b), Some(o)) => Some(ConnectivePreferencesConfig {
            allowed: o.allowed.or(b.allowed),
            preferred: o.preferred.or(b.preferred),
        }),
    }
}

fn merge_hedging(
    base: Option<HedgingCalibrationConfig>,
    overlay: Option<HedgingCalibrationConfig>,
) -> Option<HedgingCalibrationConfig> {
    match (base, overlay) {
        (None, o) => o,
        (b, None) => b,
        (Some(b), Some(o)) => Some(HedgingCalibrationConfig {
            offset: o.offset.or(b.offset),
            forbid: o.forbid.or(b.forbid),
        }),
    }
}

fn build_length_distribution(c: LengthDistributionConfig) -> LengthDistribution {
    let neutral = LengthDistribution::neutral();
    LengthDistribution {
        short: c.short.unwrap_or(neutral.short),
        medium: c.medium.unwrap_or(neutral.medium),
        long: c.long.unwrap_or(neutral.long),
        short_max_words: c.short_max_words.unwrap_or(neutral.short_max_words),
        medium_max_words: c.medium_max_words.unwrap_or(neutral.medium_max_words),
    }
}

fn build_connective_preferences(
    c: ConnectivePreferencesConfig,
) -> Result<ConnectivePreferences, ProjectError> {
    let mut prefs = ConnectivePreferences::neutral();
    if let Some(allowed) = c.allowed {
        for (k, v) in allowed {
            let rst = parse_rst_relation(&k)?;
            prefs.allowed.insert(rst, v);
        }
    }
    if let Some(preferred) = c.preferred {
        for (k, v) in preferred {
            let rst = parse_rst_relation(&k)?;
            prefs.preferred.insert(rst, v);
        }
    }
    Ok(prefs)
}

fn build_hedging_calibration(c: HedgingCalibrationConfig) -> HedgingCalibration {
    HedgingCalibration {
        offset: c.offset.unwrap_or(0),
        forbid: c.forbid.unwrap_or_default(),
    }
}

fn parse_verbosity(s: &str) -> Result<Verbosity, ProjectError> {
    match s {
        "terse" => Ok(Verbosity::Terse),
        "neutral" => Ok(Verbosity::Neutral),
        "verbose" => Ok(Verbosity::Verbose),
        other => Err(ProjectError::ManifestStyle {
            reason: format!(
                "unknown verbosity `{other}` — expected one of terse, neutral, verbose"
            ),
        }),
    }
}

fn parse_list_style_bias(s: &str) -> Result<ListStyleBias, ProjectError> {
    match s {
        "auto" => Ok(ListStyleBias::Auto),
        "including" => Ok(ListStyleBias::Including),
        "such_as" => Ok(ListStyleBias::SuchAs),
        "dash" => Ok(ListStyleBias::Dash),
        "bracketed" => Ok(ListStyleBias::Bracketed),
        other => Err(ProjectError::ManifestStyle {
            reason: format!(
                "unknown list_style_bias `{other}` — expected one of auto, including, such_as, dash, bracketed"
            ),
        }),
    }
}

fn parse_pronoun_density(s: &str) -> Result<PronounDensity, ProjectError> {
    match s {
        "low" => Ok(PronounDensity::Low),
        "default" => Ok(PronounDensity::Default),
        "high" => Ok(PronounDensity::High),
        other => Err(ProjectError::ManifestStyle {
            reason: format!(
                "unknown pronoun_density `{other}` — expected one of low, default, high"
            ),
        }),
    }
}

fn parse_salience_bias(s: &str) -> Result<SalienceBias, ProjectError> {
    match s {
        "lower" => Ok(SalienceBias::Lower),
        "auto" => Ok(SalienceBias::Auto),
        "higher" => Ok(SalienceBias::Higher),
        other => Err(ProjectError::ManifestStyle {
            reason: format!(
                "unknown salience bias `{other}` — expected one of lower, auto, higher"
            ),
        }),
    }
}

fn parse_rst_relation(s: &str) -> Result<RstRelation, ProjectError> {
    match s {
        "elaboration" => Ok(RstRelation::Elaboration),
        "contrast" => Ok(RstRelation::Contrast),
        "cause" => Ok(RstRelation::Cause),
        "result" => Ok(RstRelation::Result),
        "concession" => Ok(RstRelation::Concession),
        "sequence" => Ok(RstRelation::Sequence),
        "condition" => Ok(RstRelation::Condition),
        "background" => Ok(RstRelation::Background),
        "summary" => Ok(RstRelation::Summary),
        other => Err(ProjectError::ManifestStyle {
            reason: format!(
                "unknown RST relation key `{other}` — expected one of elaboration, contrast, cause, result, concession, sequence, condition, background, summary"
            ),
        }),
    }
}

fn map_style_error(err: StyleProfileError) -> ProjectError {
    ProjectError::ManifestStyle {
        reason: err.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn parses_minimal_inline_profile() {
        let toml_str = r#"
            name = "concise"
            verbosity = "terse"
            list_style_bias = "bracketed"
        "#;
        let cfg: StyleProfileConfig = toml::from_str(toml_str).unwrap();
        let dir = tempdir().unwrap();
        let profile = cfg.into_style_profile(dir.path()).unwrap();
        assert_eq!(profile.name, "concise");
        assert_eq!(profile.verbosity, Verbosity::Terse);
        assert_eq!(profile.list_style_bias, ListStyleBias::Bracketed);
        assert!(profile.connectives.is_neutral());
    }

    #[test]
    fn parses_per_relation_connective_pools() {
        let toml_str = r#"
            name = "tight-contrast"
            [connectives.allowed]
            elaboration = ["Furthermore,", "Additionally,"]
            contrast = ["However,"]
            [connectives.preferred]
            elaboration = [["Furthermore,", 1.0], ["Additionally,", 0.5]]
        "#;
        let cfg: StyleProfileConfig = toml::from_str(toml_str).unwrap();
        let dir = tempdir().unwrap();
        let profile = cfg.into_style_profile(dir.path()).unwrap();
        assert_eq!(
            profile
                .connectives
                .allowed
                .get(&RstRelation::Elaboration)
                .map(Vec::len),
            Some(2)
        );
        assert_eq!(
            profile
                .connectives
                .allowed
                .get(&RstRelation::Contrast)
                .map(Vec::len),
            Some(1)
        );
        assert_eq!(
            profile
                .connectives
                .preferred
                .get(&RstRelation::Elaboration)
                .map(Vec::len),
            Some(2)
        );
    }

    #[test]
    fn unknown_rst_relation_key_is_rejected() {
        let toml_str = r#"
            name = "bad"
            [connectives.allowed]
            shrubbery = ["foo"]
        "#;
        let cfg: StyleProfileConfig = toml::from_str(toml_str).unwrap();
        let dir = tempdir().unwrap();
        let result = cfg.into_style_profile(dir.path());
        assert!(matches!(
            result,
            Err(ProjectError::ManifestStyle { reason }) if reason.contains("shrubbery")
        ));
    }

    #[test]
    fn unknown_verbosity_value_is_rejected() {
        let toml_str = r#"
            name = "bad"
            verbosity = "yelly"
        "#;
        let cfg: StyleProfileConfig = toml::from_str(toml_str).unwrap();
        let dir = tempdir().unwrap();
        let result = cfg.into_style_profile(dir.path());
        assert!(matches!(
            result,
            Err(ProjectError::ManifestStyle { reason }) if reason.contains("yelly")
        ));
    }

    #[test]
    fn extends_loads_referenced_profile_and_overlays() {
        let dir = tempdir().unwrap();
        let base_path = dir.path().join("base.toml");
        fs::write(
            &base_path,
            r#"
                name = "base"
                verbosity = "terse"
                list_style_bias = "bracketed"
            "#,
        )
        .unwrap();

        // Inline overrides only verbosity; list_style_bias stays from base.
        let overlay_toml = r#"
            extends = "base.toml"
            name = "child"
            verbosity = "verbose"
        "#;
        let cfg: StyleProfileConfig = toml::from_str(overlay_toml).unwrap();
        let profile = cfg.into_style_profile(dir.path()).unwrap();
        assert_eq!(profile.name, "child");
        assert_eq!(profile.verbosity, Verbosity::Verbose);
        assert_eq!(profile.list_style_bias, ListStyleBias::Bracketed);
    }

    #[test]
    fn extends_cycle_is_rejected() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("a.toml"),
            r#"
                extends = "b.toml"
                name = "a"
            "#,
        )
        .unwrap();
        fs::write(
            dir.path().join("b.toml"),
            r#"
                extends = "a.toml"
                name = "b"
            "#,
        )
        .unwrap();
        let cfg = StyleProfileConfig {
            extends: Some("a.toml".to_string()),
            ..Default::default()
        };
        let result = cfg.into_style_profile(dir.path());
        assert!(matches!(
            result,
            Err(ProjectError::ManifestStyle { reason }) if reason.contains("cycle")
        ));
    }

    #[test]
    fn validation_errors_propagate() {
        let toml_str = r#"
            name = "bad"
            [hedging]
            offset = 75
        "#;
        let cfg: StyleProfileConfig = toml::from_str(toml_str).unwrap();
        let dir = tempdir().unwrap();
        let result = cfg.into_style_profile(dir.path());
        assert!(matches!(
            result,
            Err(ProjectError::ManifestStyle { reason }) if reason.contains("75")
        ));
    }

    #[test]
    fn empty_config_produces_neutral_profile() {
        let cfg = StyleProfileConfig::default();
        let dir = tempdir().unwrap();
        let profile = cfg.into_style_profile(dir.path()).unwrap();
        assert!(profile.is_neutral());
    }
}
