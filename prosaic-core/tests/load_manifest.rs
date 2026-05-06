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

#[test]
fn load_manifest_applies_engine_settings() {
    let json = r#"
    {
      "schema_version": 1,
      "name": "settings",
      "version": "0.1.0",
      "language": "en",
      "engine": {
        "strictness": "lenient",
        "variation": "fixed",
        "smart_quotes": true,
        "max_sentence_length": 0,
        "faithfulness_min": 0.0,
        "salience_thresholds": { "low_max": 10, "high_min": 20 }
      },
      "templates": [
        {
          "key": "quote",
          "variants": [
            { "salience": "medium", "body": "\"{name}\"" }
          ]
        },
        {
          "key": "impact",
          "variants": [
            { "salience": "low", "body": "low {name}" },
            { "salience": "medium", "body": "medium {name}" }
          ]
        }
      ],
      "partials": []
    }
    "#;

    let mut engine = Engine::new(English::new());
    engine.load_manifest(json).unwrap();

    let mut session = Session::new();
    let missing = engine
        .render(&mut session, "quote", Context::new())
        .unwrap();
    assert!(missing.contains("[missing: name]"), "got: {missing}");
    assert!(missing.contains('\u{201C}'), "got: {missing}");

    let mut ctx = Context::new();
    ctx.insert("name", Value::String("UserService".into()));
    ctx.insert("consumer_count", Value::Number(5));
    let low = engine.render(&mut session, "impact", &ctx).unwrap();
    assert!(low.starts_with("low "), "got: {low}");
}

#[test]
fn load_manifest_rejects_unknown_engine_settings() {
    let bad = r#"
    {
      "schema_version": 1,
      "name": "bad",
      "version": "0.1.0",
      "language": "en",
      "engine": { "strictness": "surprise" },
      "templates": [],
      "partials": []
    }
    "#;

    let mut engine = Engine::new(English::new());
    let err = engine.load_manifest(bad).unwrap_err();
    assert!(err.to_string().contains("unknown strictness"));
}
