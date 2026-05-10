//! `prosaic.toml` schema — project manifest.

use serde::{Deserialize, Serialize};

use crate::style::StyleProfileConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    pub language: String,
    #[serde(default)]
    pub engine: EngineSettings,
    #[serde(default)]
    pub dependencies: Vec<VocabDependency>,
    /// Optional declarative voice profile applied to the materialized
    /// engine. Missing or all-default fields render byte-equivalent to
    /// `StyleProfile::neutral()`.
    #[serde(default)]
    pub style_profile: Option<StyleProfileConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct EngineSettings {
    pub strictness: String,
    pub variation: String,
    pub smart_quotes: bool,
    pub max_sentence_length: usize,
    pub faithfulness_min: f64,
    pub salience_thresholds: Option<SalienceThresholdsConfig>,
    pub style: Option<String>,
}

impl Default for EngineSettings {
    fn default() -> Self {
        Self {
            strictness: "strict".to_string(),
            variation: "fixed".to_string(),
            smart_quotes: false,
            max_sentence_length: 0,
            faithfulness_min: 0.0,
            salience_thresholds: None,
            style: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SalienceThresholdsConfig {
    pub low_max: i64,
    pub high_min: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VocabDependency {
    #[serde(rename = "crate")]
    pub crate_name: String,
    pub version: String,
    #[serde(default)]
    pub languages: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_manifest() {
        let toml_str = r#"
            name = "demo"
            version = "0.1.0"
            language = "en"
        "#;
        let m: Manifest = toml::from_str(toml_str).unwrap();
        assert_eq!(m.name, "demo");
        assert_eq!(m.version, "0.1.0");
        assert_eq!(m.language, "en");
        assert!(m.dependencies.is_empty());
        assert_eq!(m.engine.strictness, "strict");
        assert_eq!(m.engine.variation, "fixed");
    }

    #[test]
    fn parse_full_manifest() {
        let toml_str = r#"
            name = "changelog"
            version = "1.2.0"
            language = "en"

            [engine]
            strictness = "lenient"
            variation = "round_robin"
            smart_quotes = true
            max_sentence_length = 120
            faithfulness_min = 0.85
            style = "executive"

            [engine.salience_thresholds]
            low_max = 2
            high_min = 30

            [[dependencies]]
            crate = "prosaic-vocab-code"
            version = "0.3"
            languages = ["en", "es"]

            [[dependencies]]
            crate = "prosaic-vocab-git"
            version = "0.3"
        "#;
        let m: Manifest = toml::from_str(toml_str).unwrap();
        assert_eq!(m.engine.max_sentence_length, 120);
        assert_eq!(m.engine.faithfulness_min, 0.85);
        assert_eq!(m.engine.style.as_deref(), Some("executive"));
        let st = m.engine.salience_thresholds.unwrap();
        assert_eq!(st.low_max, 2);
        assert_eq!(st.high_min, 30);
        assert_eq!(m.dependencies.len(), 2);
        assert_eq!(m.dependencies[0].crate_name, "prosaic-vocab-code");
        assert_eq!(m.dependencies[0].languages, vec!["en", "es"]);
        assert!(m.dependencies[1].languages.is_empty());
    }

    #[test]
    fn missing_required_fields_errors() {
        let toml_str = r#"version = "0.1.0""#;
        let res = toml::from_str::<Manifest>(toml_str);
        assert!(res.is_err(), "expected error for missing `name` field");
    }

    #[test]
    fn parse_manifest_with_style_profile_section() {
        let toml_str = r#"
            name = "demo"
            version = "0.1.0"
            language = "en"

            [style_profile]
            name = "concise-professional"
            verbosity = "terse"
            list_style_bias = "bracketed"
            pronoun_density = "high"

            [style_profile.connectives.allowed]
            elaboration = ["Furthermore,", "Additionally,"]
            contrast = ["However,"]

            [style_profile.hedging]
            offset = -10
            forbid = ["perhaps"]
        "#;
        let m: Manifest = toml::from_str(toml_str).unwrap();
        let p = m.style_profile.unwrap();
        assert_eq!(p.name.as_deref(), Some("concise-professional"));
        assert_eq!(p.verbosity.as_deref(), Some("terse"));
        assert_eq!(p.list_style_bias.as_deref(), Some("bracketed"));
        let connectives = p.connectives.unwrap();
        let allowed = connectives.allowed.unwrap();
        assert_eq!(allowed.get("elaboration").map(Vec::len), Some(2));
        assert_eq!(allowed.get("contrast").map(Vec::len), Some(1));
        let hedging = p.hedging.unwrap();
        assert_eq!(hedging.offset, Some(-10));
        assert_eq!(hedging.forbid.as_ref().map(Vec::len), Some(1));
    }

    #[test]
    fn parse_manifest_with_style_profile_extends_only() {
        let toml_str = r#"
            name = "demo"
            version = "0.1.0"
            language = "en"

            [style_profile]
            extends = "profiles/concise-professional.toml"
        "#;
        let m: Manifest = toml::from_str(toml_str).unwrap();
        let p = m.style_profile.unwrap();
        assert_eq!(
            p.extends.as_deref(),
            Some("profiles/concise-professional.toml")
        );
        assert!(p.name.is_none());
    }

    #[test]
    fn manifest_without_style_profile_section_omits_field() {
        let toml_str = r#"
            name = "demo"
            version = "0.1.0"
            language = "en"
        "#;
        let m: Manifest = toml::from_str(toml_str).unwrap();
        assert!(m.style_profile.is_none());
    }

    #[test]
    fn invalid_strictness_preserved_for_downstream_validation() {
        let toml_str = r#"
            name = "demo"
            version = "0.1.0"
            language = "en"
            [engine]
            strictness = "yolo"
        "#;
        let m: Manifest = toml::from_str(toml_str).unwrap();
        assert_eq!(m.engine.strictness, "yolo");
    }
}
