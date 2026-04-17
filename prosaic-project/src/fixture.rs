//! `fixtures/*.json` loader.
//!
//! A fixture is a single-event context map serialized as JSON.

use prosaic_core::{Context, Value};
use serde_json::Value as JsonValue;

use crate::error::ProjectError;

/// Parse a fixture JSON string into a `Context`.
pub fn parse_fixture(name: &str, json: &str) -> Result<Context, ProjectError> {
    let v: JsonValue = serde_json::from_str(json).map_err(|e| ProjectError::JsonParse {
        file: name.to_string(),
        cause: e.to_string(),
    })?;
    let obj = v
        .as_object()
        .ok_or_else(|| ProjectError::FixtureValidation {
            name: name.to_string(),
            reason: "fixture must be a JSON object".to_string(),
        })?;
    let mut ctx = Context::new();
    for (k, v) in obj {
        ctx.insert(k.clone(), json_to_value(v));
    }
    Ok(ctx)
}

fn json_to_value(v: &JsonValue) -> Value {
    match v {
        JsonValue::Null => Value::String(String::new()),
        JsonValue::Bool(b) => Value::Number(if *b { 1 } else { 0 }),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Number(i)
            } else if let Some(f) = n.as_f64() {
                Value::Number(f as i64)
            } else {
                Value::Number(0)
            }
        }
        JsonValue::String(s) => Value::String(s.clone()),
        JsonValue::Array(items) => Value::List(
            items
                .iter()
                .map(|i| match i {
                    JsonValue::String(s) => s.clone(),
                    other => other.to_string(),
                })
                .collect(),
        ),
        JsonValue::Object(_) => Value::String(v.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_fixture() {
        let json = r#"{"name": "UserService", "entity_type": "class", "consumer_count": 6}"#;
        let ctx = parse_fixture("test", json).unwrap();
        assert_eq!(ctx.get("name").unwrap().as_display(), "UserService");
        assert_eq!(ctx.get("consumer_count").unwrap().as_number().unwrap(), 6);
    }

    #[test]
    fn parse_fixture_with_array() {
        let json = r#"{"consumers": ["A", "B", "C"]}"#;
        let ctx = parse_fixture("test", json).unwrap();
        let list = ctx.get("consumers").unwrap().as_list().unwrap();
        assert_eq!(
            list,
            vec!["A".to_string(), "B".to_string(), "C".to_string()]
        );
    }

    #[test]
    fn invalid_json_errors() {
        let res = parse_fixture("bad", "{not json");
        assert!(matches!(res, Err(ProjectError::JsonParse { .. })));
    }

    #[test]
    fn non_object_errors() {
        let res = parse_fixture("bad", "[1, 2, 3]");
        assert!(matches!(res, Err(ProjectError::FixtureValidation { .. })));
    }
}
