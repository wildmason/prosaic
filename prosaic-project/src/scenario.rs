//! `tests/*.toml` schema — narrative-flow scenarios with discourse assertions.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scenario {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub engine: ScenarioEngineOverride,
    pub events: Vec<ScenarioEvent>,
    #[serde(default)]
    pub expected: Option<Expected>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ScenarioEngineOverride {
    pub variation: Option<String>,
    pub language: Option<String>,
    pub salience_thresholds: Option<crate::manifest::SalienceThresholdsConfig>,
    pub faithfulness_min: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioEvent {
    pub template: String,
    /// Free-form context map. Values can be strings, numbers, bools, or arrays of strings.
    #[serde(default)]
    pub context: HashMap<String, toml::Value>,
    /// RST relation hint (Elaboration, Contrast, Result, etc.). String form for TOML friendliness.
    #[serde(default)]
    pub rst_relation: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Expected {
    pub output: Option<String>,
    pub faithfulness_min: Option<f64>,
    pub discourse: Vec<ExpectedDiscourse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedDiscourse {
    pub event_index: usize,
    #[serde(default)]
    pub reference_form: Option<String>,
    #[serde(default)]
    pub connective_contains: Option<String>,
    #[serde(default)]
    pub transition: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_scenario() {
        let toml_str = r#"
            name = "smoke"
            [[events]]
            template = "code.added"
            context = { name = "Foo", entity_type = "class" }
        "#;
        let s: Scenario = toml::from_str(toml_str).unwrap();
        assert_eq!(s.name, "smoke");
        assert_eq!(s.events.len(), 1);
        assert_eq!(s.events[0].template, "code.added");
        assert_eq!(
            s.events[0].context.get("name").unwrap().as_str().unwrap(),
            "Foo"
        );
    }

    #[test]
    fn parse_full_scenario_with_expectations() {
        let toml_str = r#"
            name = "PR 142"
            description = "auth refactor"

            [engine]
            variation = "fixed"
            language = "en"
            faithfulness_min = 0.9

            [[events]]
            template = "code.modified"
            context = { name = "UserService", consumer_count = 6 }

            [[events]]
            template = "code.moved"
            context = { name = "UserService" }
            rst_relation = "elaboration"

            [expected]
            output = "..."
            faithfulness_min = 0.85

            [[expected.discourse]]
            event_index = 1
            reference_form = "Pronoun"

            [[expected.discourse]]
            event_index = 1
            connective_contains = "also"
        "#;
        let s: Scenario = toml::from_str(toml_str).unwrap();
        assert_eq!(s.events.len(), 2);
        assert_eq!(s.events[1].rst_relation.as_deref(), Some("elaboration"));
        let exp = s.expected.unwrap();
        assert_eq!(exp.faithfulness_min, Some(0.85));
        assert_eq!(exp.discourse.len(), 2);
        assert_eq!(exp.discourse[0].reference_form.as_deref(), Some("Pronoun"));
        assert_eq!(exp.discourse[1].connective_contains.as_deref(), Some("also"));
    }

    #[test]
    fn parse_array_context_value() {
        let toml_str = r#"
            name = "list"
            [[events]]
            template = "code.modified"
            context = { name = "X", consumers = ["a", "b", "c"] }
        "#;
        let s: Scenario = toml::from_str(toml_str).unwrap();
        let arr = s.events[0]
            .context
            .get("consumers")
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[0].as_str().unwrap(), "a");
    }
}
