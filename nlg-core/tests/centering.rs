/// Integration tests for Centering Theory Rule 1 enforcement.
///
/// Rule 1 (Grosz, Joshi & Weinstein 1995): if any element of Cf(Ui) is
/// realized as a pronoun in Ui+1, then the Cb(Ui+1) must also be realized
/// as a pronoun. In practice: pronominalize an entity only when it IS the
/// backward-looking center (Cb). If the entity to be pronominalized is not
/// the Cb, demote to ShortName to prevent ambiguous pronoun resolution.
use nlg_core::{Context, Engine, Session, Strictness, Value, Variation};
use nlg_grammar_en::English;

fn engine() -> Engine {
    Engine::new(English::new())
        .strictness(Strictness::Strict)
        .variation(Variation::Fixed)
}

/// A single entity continuously focused: Cb = Foo throughout.
/// Rule 1 permits pronominalization because the referent IS the Cb.
#[test]
fn rule_1_single_entity_pronominalizes_normally() {
    let mut engine = engine();
    engine
        .register_template("t", "{name|refer} was modified")
        .unwrap();

    let mut session = Session::new();
    let mut ctx = Context::new();
    ctx.insert("entity_type", Value::String("class".into()));
    ctx.insert("name", Value::String("Foo".into()));

    let r1 = engine.render(&mut session, "t", &ctx).unwrap();
    let r2 = engine.render(&mut session, "t", &ctx).unwrap();

    // First mention is always full form.
    assert!(r1.contains("Foo"), "r1 should contain Foo; got: {r1}");
    // Second mention: Cb=Foo, referent=Foo → Rule 1 allows pronoun.
    assert!(
        r2.to_lowercase().starts_with("it "),
        "expected pronoun for single-entity continuation; got: {r2}"
    );
}

/// Rule 1 demotion: Render 1 = Foo (Cb becomes Foo). Render 2 = Bar (new
/// entity; Smooth Shift keeps Cb=Foo). Render 3 = Bar again. Bar ≠ Cb Foo
/// → Rule 1 demotes what would otherwise be a pronoun to ShortName.
#[test]
fn rule_1_demotes_pronoun_when_referent_not_cb() {
    let mut engine = engine();
    engine
        .register_template("t", "{name|refer} was modified")
        .unwrap();

    let mut session = Session::new();

    let mut c_foo = Context::new();
    c_foo.insert("entity_type", Value::String("class".into()));
    c_foo.insert("name", Value::String("Foo".into()));

    let mut c_bar = Context::new();
    c_bar.insert("entity_type", Value::String("class".into()));
    c_bar.insert("name", Value::String("Bar".into()));

    let _r1 = engine.render(&mut session, "t", &c_foo).unwrap(); // Foo; Cb→Foo
    let _r2 = engine.render(&mut session, "t", &c_bar).unwrap(); // Bar; Smooth Shift → Cb stays Foo
    let r3 = engine.render(&mut session, "t", &c_bar).unwrap(); // Bar again; Bar≠Cb(Foo) → demote

    // r3 would naively pronominalize Bar ("it was modified"), but Rule 1
    // requires the Cb (Foo) to be pronominalized before Bar can be. Since
    // we are asking for Bar's reference form and Bar ≠ Cb, demote to ShortName.
    assert!(
        !r3.to_lowercase().starts_with("it"),
        "Rule 1 should demote Bar to ShortName (Cb is Foo); got: {r3}"
    );
    assert!(r3.contains("Bar"), "expected ShortName 'Bar'; got: {r3}");
}

/// After session.reset(), Cb is None. The very next render sees no Cb and
/// is treated as a fresh discourse scope — first mention is Full form again.
#[test]
fn rule_1_resets_cb_on_session_reset() {
    let mut engine = engine();
    engine
        .register_template("t", "{name|refer} was modified")
        .unwrap();

    let mut session = Session::new();
    let mut ctx = Context::new();
    ctx.insert("entity_type", Value::String("class".into()));
    ctx.insert("name", Value::String("Foo".into()));

    // Establish Cb = Foo.
    engine.render(&mut session, "t", &ctx).unwrap();

    // Reset: Cb should return to None.
    session.reset();

    // Post-reset, Foo is unknown again — full form expected.
    let r = engine.render(&mut session, "t", &ctx).unwrap();
    assert!(
        r.contains("The class Foo") || r.contains("the class Foo"),
        "post-reset should produce full reference form; got: {r}"
    );
}
