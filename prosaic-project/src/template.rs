//! `templates/*.toml` schema.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateFile {
    pub key: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub slots_required: Vec<String>,
    #[serde(default)]
    pub slots_optional: Vec<String>,
    pub variants: Vec<Variant>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Variant {
    /// "low" | "medium" | "high"
    #[serde(default = "default_salience")]
    pub salience: String,
    /// BCP-47 lang code; defaults to project language if unspecified.
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub description: String,
    pub body: String,
}

fn default_salience() -> String {
    "medium".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_template() {
        let toml_str = r#"
            key = "code.modified"
            variants = [
              { body = "{name|refer} was modified" },
            ]
        "#;
        let t: TemplateFile = toml::from_str(toml_str).unwrap();
        assert_eq!(t.key, "code.modified");
        assert_eq!(t.variants.len(), 1);
        assert_eq!(t.variants[0].salience, "medium");
        assert_eq!(t.variants[0].body, "{name|refer} was modified");
        assert!(t.variants[0].language.is_none());
    }

    #[test]
    fn parse_multi_variant_with_salience() {
        let toml_str = r#"
            key = "code.modified"
            description = "Render a 'class X was modified' event."
            slots_required = ["name"]
            slots_optional = ["consumer_count"]

            [[variants]]
            salience = "low"
            body = "{name|refer} was modified"

            [[variants]]
            salience = "medium"
            body = """{name|refer} was modified, affecting {consumer_count}"""

            [[variants]]
            salience = "high"
            language = "en"
            body = "{name|refer} has been substantially modified"
        "#;
        let t: TemplateFile = toml::from_str(toml_str).unwrap();
        assert_eq!(t.variants.len(), 3);
        assert_eq!(t.variants[0].salience, "low");
        assert_eq!(t.variants[1].salience, "medium");
        assert_eq!(t.variants[2].salience, "high");
        assert_eq!(t.variants[2].language.as_deref(), Some("en"));
        assert_eq!(t.slots_required, vec!["name"]);
        assert_eq!(t.slots_optional, vec!["consumer_count"]);
    }

    #[test]
    fn parse_multi_language_variants() {
        let toml_str = r#"
            key = "code.modified"

            [[variants]]
            salience = "medium"
            language = "en"
            body = "{name} was modified"

            [[variants]]
            salience = "medium"
            language = "es"
            body = "{name} fue modificado"
        "#;
        let t: TemplateFile = toml::from_str(toml_str).unwrap();
        assert_eq!(t.variants.len(), 2);
        assert_eq!(t.variants[0].language.as_deref(), Some("en"));
        assert_eq!(t.variants[1].language.as_deref(), Some("es"));
    }

    #[test]
    fn missing_key_errors() {
        let toml_str = r#"
            variants = [{ body = "x" }]
        "#;
        let res = toml::from_str::<TemplateFile>(toml_str);
        assert!(res.is_err(), "expected error for missing `key` field");
    }

    #[test]
    fn missing_variant_body_errors() {
        let toml_str = r#"
            key = "x"
            [[variants]]
            salience = "medium"
        "#;
        let res = toml::from_str::<TemplateFile>(toml_str);
        assert!(res.is_err(), "expected error for missing variant `body` field");
    }

    #[test]
    fn round_trip_serialize() {
        let original: TemplateFile = toml::from_str(
            r#"
            key = "code.added"
            description = "Add event."
            variants = [
              { salience = "medium", body = "{name|refer} was added" },
            ]
            "#,
        )
        .unwrap();
        let serialized = toml::to_string(&original).unwrap();
        let reparsed: TemplateFile = toml::from_str(&serialized).unwrap();
        assert_eq!(reparsed.key, "code.added");
        assert_eq!(reparsed.variants.len(), 1);
        assert_eq!(reparsed.variants[0].body, "{name|refer} was added");
    }
}
