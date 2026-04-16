//! Round-trip tests for the `serde` feature. Compiled only when the
//! feature is enabled: `cargo test --features serde`.

#![cfg(feature = "serde")]

#[cfg(feature = "reg")]
use prosaic_core::RegAlgorithm;
use prosaic_core::{
    Context, EntityDescriptor, GroupingStrategy, HedgeMode, ListStyle, QuantifyMode, ReferenceForm,
    RhetoricalCategory, Salience, Tense, Value, VerbForm, Voice,
    agreement::{AgreementFeatures, Definiteness, Gender, Number},
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
    assert_eq!(back.get("name"), Some(&Value::String("UserService".into())));
    assert_eq!(back.get("count"), Some(&Value::Number(6)));
}

#[test]
fn entity_descriptor_roundtrips() {
    let desc = EntityDescriptor::new("UserService", "class").with_attribute("layer", "domain");
    let json = serde_json::to_string(&desc).unwrap();
    let back: EntityDescriptor = serde_json::from_str(&json).unwrap();
    assert_eq!(desc, back);
}

#[test]
#[cfg(feature = "reg")]
fn entity_descriptor_without_relations_field_deserializes() {
    // Simulate a serialized EntityDescriptor from before the relations field
    // was added. The serde(default) annotation must handle missing-field
    // deserialization cleanly.
    let legacy_json =
        r#"{"name":"UserService","entity_type":"class","attributes":[["layer","domain"]]}"#;
    let back: EntityDescriptor = serde_json::from_str(legacy_json).unwrap();
    assert_eq!(back.name, "UserService");
    assert_eq!(back.entity_type, "class");
    assert_eq!(back.attribute("layer"), Some("domain"));
    assert!(
        back.relations.is_empty(),
        "relations should default to empty"
    );
}

#[test]
#[cfg(feature = "reg")]
fn entity_descriptor_with_relations_roundtrips() {
    let desc = EntityDescriptor::new("LoginHandler", "function")
        .with_attribute("layer", "api")
        .with_relation("that calls", "AuthService");
    let json = serde_json::to_string(&desc).unwrap();
    let back: EntityDescriptor = serde_json::from_str(&json).unwrap();
    assert_eq!(desc, back);
    assert_eq!(back.relation("that calls"), Some("AuthService"));
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
    assert_eq!(serde_json::to_string(&Tense::Past).unwrap(), "\"Past\"");
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
    #[cfg(feature = "reg")]
    {
        assert_eq!(
            serde_json::to_string(&RegAlgorithm::DaleReiter).unwrap(),
            "\"DaleReiter\""
        );
        assert_eq!(
            serde_json::to_string(&RegAlgorithm::GraphBased).unwrap(),
            "\"GraphBased\""
        );
    }
}

// ── Value::Entity serde tests ─────────────────────────────────────────────────

#[test]
fn value_entity_roundtrips() {
    let v = Value::Entity {
        name: "UserService".into(),
        features: AgreementFeatures::default()
            .with_gender(Gender::Fem)
            .with_number(Number::Singular)
            .with_definiteness(Definiteness::Definite),
    };
    let json = serde_json::to_string(&v).unwrap();
    let back: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v, back);
}

#[test]
fn value_entity_without_features_deserializes() {
    // Legacy-style payload without the features field — serde(default) must handle it.
    let json = r#"{"Entity":{"name":"X"}}"#;
    let v: Value = serde_json::from_str(json).unwrap();
    match v {
        Value::Entity { name, features } => {
            assert_eq!(name, "X");
            assert_eq!(features, AgreementFeatures::default());
        }
        _ => panic!("expected Value::Entity"),
    }
}

#[test]
fn value_entity_features_field_serializes() {
    // Confirm the JSON shape so the format is explicit and won't silently change.
    let v = Value::Entity {
        name: "Foo".into(),
        features: AgreementFeatures::new().with_gender(Gender::Masc),
    };
    let json = serde_json::to_string(&v).unwrap();
    // Must contain the entity name
    assert!(json.contains("\"name\":\"Foo\""), "json: {json}");
    // Must contain the features field (even though most are default/Unknown)
    assert!(json.contains("\"features\""), "json: {json}");
    // Must be the Entity variant
    assert!(json.contains("\"Entity\""), "json: {json}");
}

#[test]
fn agreement_features_roundtrip() {
    let f = AgreementFeatures::new()
        .with_gender(Gender::Fem)
        .with_number(Number::Plural)
        .with_definiteness(Definiteness::Indefinite);
    let json = serde_json::to_string(&f).unwrap();
    let back: AgreementFeatures = serde_json::from_str(&json).unwrap();
    assert_eq!(f, back);
}

#[test]
fn agreement_enums_serialize_to_variant_names() {
    assert_eq!(serde_json::to_string(&Gender::Fem).unwrap(), "\"Fem\"");
    assert_eq!(serde_json::to_string(&Gender::Masc).unwrap(), "\"Masc\"");
    assert_eq!(
        serde_json::to_string(&Number::Singular).unwrap(),
        "\"Singular\""
    );
    assert_eq!(
        serde_json::to_string(&Definiteness::Definite).unwrap(),
        "\"Definite\""
    );
}
