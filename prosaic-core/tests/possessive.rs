use prosaic_core::{Engine, IntoValue, Session, Strictness, Value, Variation, ctx, entity};
use prosaic_grammar_en::English;

fn engine() -> Engine {
    Engine::new(English::new())
        .strictness(Strictness::Strict)
        .variation(Variation::Fixed)
}

#[test]
fn first_possessive_mention_uses_name_possessive() {
    let mut engine = engine();
    engine
        .register_template("impact", "{name|possessive} consumers were updated")
        .unwrap();

    let mut session = Session::new();
    let context = ctx! {
        entity_type: "class",
        name: "UserService",
    };

    let output = engine.render(&mut session, "impact", &context).unwrap();
    assert_eq!(output, "UserService's consumers were updated.");
}

#[test]
fn focused_second_possessive_mention_uses_possessive_pronoun() {
    let mut engine = engine();
    engine
        .register_template("intro", "{name|refer} was modified")
        .unwrap();
    engine
        .register_template("impact", "{name|possessive} consumers were updated")
        .unwrap();

    let mut session = Session::new();
    let context = ctx! {
        entity_type: "class",
        name: "UserService",
    };

    engine.render(&mut session, "intro", &context).unwrap();
    let output = engine.render(&mut session, "impact", &context).unwrap();
    assert!(
        output.contains("its consumers were updated")
            || output.contains("Its consumers were updated"),
        "expected possessive pronoun, got: {output}"
    );
}

#[test]
fn ambiguous_possessive_reference_falls_back_to_name() {
    let mut engine = engine();
    engine
        .register_template("intro", "{name|refer} was modified")
        .unwrap();
    engine
        .register_template("impact", "{name|possessive} consumers were updated")
        .unwrap();

    let mut session = Session::new();
    let service_a = ctx! {
        entity_type: "class",
        name: "ServiceA",
    };
    let service_b = ctx! {
        entity_type: "class",
        name: "ServiceB",
    };

    engine.render(&mut session, "intro", &service_a).unwrap();
    engine.render(&mut session, "intro", &service_b).unwrap();
    let output = engine.render(&mut session, "impact", &service_a).unwrap();

    assert!(
        output.contains("ServiceA's consumers were updated"),
        "expected name possessive, got: {output}"
    );
    assert!(
        !output.contains("its consumers were updated")
            && !output.contains("Its consumers were updated"),
        "ambiguous possessive should not use pronoun, got: {output}"
    );
}

#[test]
fn plural_entity_features_use_their_for_possessive_pronoun() {
    let mut engine = engine();
    engine
        .register_template("intro", "{name|refer} were modified")
        .unwrap();
    engine
        .register_template("impact", "{name|possessive} release notes changed")
        .unwrap();

    let mut session = Session::new();
    let mut context = prosaic_core::Context::new();
    context.insert("entity_type", Value::String("service".into()));
    context.insert("name", entity("CoreServices").plur().into_value());

    engine.render(&mut session, "intro", &context).unwrap();
    let output = engine.render(&mut session, "impact", &context).unwrap();
    assert!(
        output.contains("their release notes changed")
            || output.contains("Their release notes changed"),
        "expected plural possessive pronoun, got: {output}"
    );
}
