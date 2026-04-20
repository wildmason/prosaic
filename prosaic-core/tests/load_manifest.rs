#![cfg(feature = "serde")]

use prosaic_core::{Context, Engine, Session, Value};
use prosaic_grammar_en::English;

const SAMPLE: &str = r#"
{
  "schema_version": 1,
  "name": "demo",
  "version": "0.1.0",
  "language": "en",
  "engine": {
    "strictness": "strict",
    "variation": "fixed",
    "smart_quotes": false,
    "max_sentence_length": 0,
    "faithfulness_min": 0.0,
    "salience_thresholds": null
  },
  "templates": [
    {
      "key": "greet",
      "description": "",
      "slots_required": ["name"],
      "slots_optional": [],
      "variants": [
        {
          "salience": "medium",
          "language": null,
          "description": "",
          "body": "Hello {name}"
        }
      ]
    }
  ],
  "partials": []
}
"#;

#[test]
fn load_manifest_renders_template() {
    let mut engine = Engine::new(English::new());
    engine.load_manifest(SAMPLE).unwrap();

    let mut ctx = Context::new();
    ctx.insert("name", Value::String("world".into()));

    let mut session = Session::new();
    let out = engine.render(&mut session, "greet", &ctx).unwrap();
    assert!(out.contains("Hello"));
    assert!(out.contains("world"));
}

#[test]
fn load_manifest_unsupported_schema_version_errors() {
    let mut engine = Engine::new(English::new());
    let bad = r#"{"schema_version": 99, "name": "x", "version": "0", "language": "en", "engine": {}, "templates": [], "partials": []}"#;
    let res = engine.load_manifest(bad);
    assert!(res.is_err());
}
