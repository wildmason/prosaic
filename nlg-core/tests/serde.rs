//! Round-trip tests for the `serde` feature. Compiled only when the
//! feature is enabled: `cargo test --features serde`.

#![cfg(feature = "serde")]

use nlg_core::{
    Context, EntityDescriptor, GroupingStrategy, HedgeMode, ListStyle, QuantifyMode,
    ReferenceForm, RhetoricalCategory, Salience, Tense, Value, VerbForm, Voice,
};

#[test]
fn value_roundtrips_through_json() {
    let v = Value::List(vec!["Baz".into(), "Qux".into()]);
    let json = serde_json::to_string(&v).unwrap();
    let back: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v, back);
}

#[test]
fn context_roundtrips_through_json() {
    let mut ctx = Context::new();
    ctx.insert("name", Value::String("UserService".into()));
    ctx.insert("count", Value::Number(6));
    ctx.insert(
        "consumers",
        Value::List(vec!["A".into(), "B".into(), "C".into()]),
    );

    let json = serde_json::to_string(&ctx).unwrap();
    let back: Context = serde_json::from_str(&json).unwrap();
    assert_eq!(
        back.get("name"),
        Some(&Value::String("UserService".into()))
    );
    assert_eq!(back.get("count"), Some(&Value::Number(6)));
}

#[test]
fn entity_descriptor_roundtrips() {
    let desc = EntityDescriptor::new("UserService", "class")
        .with_attribute("layer", "domain");
    let json = serde_json::to_string(&desc).unwrap();
    let back: EntityDescriptor = serde_json::from_str(&json).unwrap();
    assert_eq!(desc, back);
}

#[test]
fn enums_serialize_to_names() {
    // Spot check a handful of config enums to confirm they serialize
    // as their variant names (useful for JSON APIs).
    assert_eq!(serde_json::to_string(&Salience::High).unwrap(), "\"High\"");
    assert_eq!(
        serde_json::to_string(&GroupingStrategy::ByAction).unwrap(),
        "\"ByAction\""
    );
    assert_eq!(
        serde_json::to_string(&RhetoricalCategory::Removal).unwrap(),
        "\"Removal\""
    );
    assert_eq!(
        serde_json::to_string(&HedgeMode::Modal).unwrap(),
        "\"Modal\""
    );
    assert_eq!(
        serde_json::to_string(&QuantifyMode::Hedged).unwrap(),
        "\"Hedged\""
    );
    assert_eq!(
        serde_json::to_string(&Voice::Passive).unwrap(),
        "\"Passive\""
    );
    assert_eq!(
        serde_json::to_string(&Tense::Past).unwrap(),
        "\"Past\""
    );
    assert_eq!(
        serde_json::to_string(&VerbForm::PresentPerfect).unwrap(),
        "\"PresentPerfect\""
    );
    assert_eq!(
        serde_json::to_string(&ReferenceForm::Pronoun).unwrap(),
        "\"Pronoun\""
    );
    assert_eq!(
        serde_json::to_string(&ListStyle::Bracketed).unwrap(),
        "\"Bracketed\""
    );
}
