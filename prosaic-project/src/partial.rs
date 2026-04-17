//! `partials/*.toml` schema.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartialFile {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub body: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_partial() {
        let toml_str = r#"
            name = "impact_tail"
            body = "{?consumer_count}, affecting {consumer_count}{/?}"
        "#;
        let p: PartialFile = toml::from_str(toml_str).unwrap();
        assert_eq!(p.name, "impact_tail");
        assert!(p.body.contains("affecting"));
    }

    #[test]
    fn missing_name_errors() {
        let res = toml::from_str::<PartialFile>(r#"body = "x""#);
        assert!(res.is_err());
    }

    #[test]
    fn missing_body_errors() {
        let res = toml::from_str::<PartialFile>(r#"name = "x""#);
        assert!(res.is_err());
    }
}
