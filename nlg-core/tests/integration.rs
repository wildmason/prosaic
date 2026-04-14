use nlg_core::{Context, Engine, Strictness, Value, Variation, Sentence, Clause, Voice, entity, named, Tense};
use nlg_derive::IntoContext;
use nlg_grammar_en::English;

fn engine() -> Engine {
    Engine::new(English::new())
        .strictness(Strictness::Strict)
        .variation(Variation::Fixed)
}

// ── End-to-end template rendering ────────────────────────────────────────

#[test]
fn full_rename_scenario() {
    let mut engine = engine();
    engine
        .register_template(
            "renamed",
            "The {entity_type} {old_name} was renamed to {new_name} \
             which impacts {count} direct {count|pluralize:consumer} \
             {consumers|truncate:3|join:bracketed}",
        )
        .unwrap();

    let mut ctx = Context::new();
    ctx.insert("entity_type", Value::String("class".into()));
    ctx.insert("old_name", Value::String("Foo".into()));
    ctx.insert("new_name", Value::String("Foobar".into()));
    ctx.insert("count", Value::Number(6));
    ctx.insert(
        "consumers",
        Value::List(vec![
            "Baz".into(),
            "Qux".into(),
            "Quux".into(),
            "Corge".into(),
            "Grault".into(),
            "Garply".into(),
        ]),
    );

    let result = engine.render("renamed", &ctx).unwrap();
    assert_eq!(
        result,
        "The class Foo was renamed to Foobar which impacts 6 direct consumers \
         [Baz, Qux, Quux, and 3 more]"
    );
}

#[test]
fn full_rename_scenario_single_consumer() {
    let mut engine = engine();
    engine
        .register_template(
            "renamed",
            "The {entity_type} {old_name} was renamed to {new_name} \
             which impacts {count} direct {count|pluralize:consumer} \
             [{consumers|join}]",
        )
        .unwrap();

    let mut ctx = Context::new();
    ctx.insert("entity_type", Value::String("interface".into()));
    ctx.insert("old_name", Value::String("IUser".into()));
    ctx.insert("new_name", Value::String("User".into()));
    ctx.insert("count", Value::Number(1));
    ctx.insert(
        "consumers",
        Value::List(vec!["UserService".into()]),
    );

    let result = engine.render("renamed", &ctx).unwrap();
    assert_eq!(
        result,
        "The interface IUser was renamed to User which impacts 1 direct consumer [UserService]"
    );
}

// ── Builder API end-to-end ───────────────────────────────────────────────

#[test]
fn builder_full_sentence_passive() {
    let engine = engine();

    let result = Sentence::new()
        .subject(entity("class", "Foo"))
        .verb("rename", Tense::Past)
        .object("Foobar")
        .clause(
            Clause::which("impacts")
                .amount(6)
                .noun("direct consumer")
                .list(&["Baz", "Qux", "Quux", "Corge", "Grault", "Garply"])
                .truncate(3),
        )
        .render(&engine)
        .unwrap();

    assert_eq!(
        result,
        "The class Foo was renamed to Foobar which impacts 6 direct consumers \
         [Baz, Qux, Quux, and 3 more]"
    );
}

#[test]
fn builder_full_sentence_active() {
    let engine = engine();

    let result = Sentence::new()
        .subject(entity("class", "Foo"))
        .verb("rename", Tense::Past)
        .object("Foobar")
        .voice(Voice::Active)
        .clause(
            Clause::which("impacts")
                .amount(6)
                .noun("direct consumer")
                .list(&["Baz", "Qux", "Quux", "Corge", "Grault", "Garply"])
                .truncate(3),
        )
        .render(&engine)
        .unwrap();

    assert_eq!(
        result,
        "The class Foo renamed Foobar which impacts 6 direct consumers \
         [Baz, Qux, Quux, and 3 more]"
    );
}

#[test]
fn builder_simple_deletion() {
    let engine = engine();

    let result = Sentence::new()
        .subject(named("processOrder"))
        .verb("remove", Tense::Past)
        .render(&engine)
        .unwrap();

    assert_eq!(result, "processOrder was removed");
}

#[test]
fn builder_future_tense_passive() {
    let engine = engine();

    let result = Sentence::new()
        .subject(entity("method", "fetchData"))
        .verb("break", Tense::Future)
        .clause(
            Clause::with_intro("in")
                .amount(3)
                .noun("test"),
        )
        .render(&engine)
        .unwrap();

    assert_eq!(result, "The method fetchData will be broken in 3 tests");
}

#[test]
fn builder_future_tense_active() {
    let engine = engine();

    let result = Sentence::new()
        .subject(entity("method", "fetchData"))
        .verb("break", Tense::Future)
        .voice(Voice::Active)
        .clause(
            Clause::with_intro("in")
                .amount(3)
                .noun("test"),
        )
        .render(&engine)
        .unwrap();

    assert_eq!(result, "The method fetchData will break in 3 tests");
}

// ── Derive macro integration ─────────────────────────────────────────────

#[derive(IntoContext)]
struct ChangeEvent {
    entity_type: String,
    name: String,
    consumer_count: i64,
    consumers: Vec<String>,
}

#[test]
fn derive_with_template_rendering() {
    let mut engine = engine();
    engine
        .register_template(
            "changed",
            "{name} ({entity_type}) was changed, affecting {consumer_count} \
             {consumer_count|pluralize:consumer}",
        )
        .unwrap();

    let event = ChangeEvent {
        entity_type: "service".into(),
        name: "AuthService".into(),
        consumer_count: 3,
        consumers: vec!["LoginPage".into(), "SignupPage".into(), "AdminDashboard".into()],
    };

    let result = engine.render("changed", event).unwrap();
    assert_eq!(
        result,
        "AuthService (service) was changed, affecting 3 consumers"
    );
}

// ── Variation determinism ────────────────────────────────────────────────

#[test]
fn seeded_variation_is_deterministic_from_fresh_state() {
    let mut engine = Engine::new(English::new())
        .strictness(Strictness::Strict)
        .variation(Variation::Seeded(42));

    engine.register_template("t", "first variant").unwrap();
    engine.register_template("t", "second variant").unwrap();
    engine.register_template("t", "third variant").unwrap();

    let ctx = Context::new();

    // From fresh state, same seed produces same result
    let result1 = engine.render("t", &ctx).unwrap();
    engine.reset();
    let result2 = engine.render("t", &ctx).unwrap();
    engine.reset();
    let result3 = engine.render("t", &ctx).unwrap();

    assert_eq!(result1, result2);
    assert_eq!(result2, result3);
}

#[test]
fn fixed_variation_picks_first_on_fresh_render() {
    let mut engine = Engine::new(English::new())
        .strictness(Strictness::Strict)
        .variation(Variation::Fixed);

    engine.register_template("t", "alpha").unwrap();
    engine.register_template("t", "beta").unwrap();
    engine.register_template("t", "gamma").unwrap();

    let ctx = Context::new();

    // First render always picks first variant
    assert_eq!(engine.render("t", &ctx).unwrap(), "alpha");

    // After reset, picks first again
    engine.reset();
    assert_eq!(engine.render("t", &ctx).unwrap(), "alpha");
}

#[test]
fn discourse_avoids_repeating_same_variant() {
    let mut engine = Engine::new(English::new())
        .strictness(Strictness::Strict)
        .variation(Variation::Fixed);

    engine.register_template("t", "alpha").unwrap();
    engine.register_template("t", "beta").unwrap();
    engine.register_template("t", "gamma").unwrap();

    let ctx = Context::new();

    let r1 = engine.render("t", &ctx).unwrap();
    let r2 = engine.render("t", &ctx).unwrap();
    let r3 = engine.render("t", &ctx).unwrap();

    // Discourse-aware engine avoids immediate repetition
    assert_ne!(r1, r2);
    assert_ne!(r2, r3);
}

// ── Strictness modes end-to-end ──────────────────────────────────────────

#[test]
fn strict_mode_fails_on_missing_slot() {
    let mut engine = Engine::new(English::new()).strictness(Strictness::Strict);
    engine
        .register_template("t", "Hello {name}, you have {count} items")
        .unwrap();

    let mut ctx = Context::new();
    ctx.insert("name", Value::String("Alice".into()));
    // "count" is missing

    let result = engine.render("t", &ctx);
    assert!(result.is_err());
}

#[test]
fn lenient_mode_shows_placeholder() {
    let mut engine = Engine::new(English::new()).strictness(Strictness::Lenient);
    engine
        .register_template("t", "Hello {name}, you have {count} items")
        .unwrap();

    let mut ctx = Context::new();
    ctx.insert("name", Value::String("Alice".into()));

    let result = engine.render("t", &ctx).unwrap();
    assert_eq!(result, "Hello Alice, you have [missing: count] items");
}

#[test]
fn silent_mode_omits_missing() {
    let mut engine = Engine::new(English::new()).strictness(Strictness::Silent);
    engine
        .register_template("t", "Hello {name}, you have {count} items")
        .unwrap();

    let mut ctx = Context::new();
    ctx.insert("name", Value::String("Alice".into()));

    let result = engine.render("t", &ctx).unwrap();
    assert_eq!(result, "Hello Alice, you have  items");
}

// ── Pipe chaining edge cases ─────────────────────────────────────────────

#[test]
fn empty_list_join() {
    let engine = engine();
    let mut ctx = Context::new();
    ctx.insert("items", Value::List(vec![]));

    let result = engine.render_inline("{items|join}", &ctx).unwrap();
    assert_eq!(result, "");
}

#[test]
fn single_item_join() {
    let engine = engine();
    let mut ctx = Context::new();
    ctx.insert("items", Value::List(vec!["only".into()]));

    let result = engine.render_inline("{items|join}", &ctx).unwrap();
    assert_eq!(result, "only");
}

#[test]
fn two_item_join() {
    let engine = engine();
    let mut ctx = Context::new();
    ctx.insert("items", Value::List(vec!["alpha".into(), "beta".into()]));

    let result = engine.render_inline("{items|join}", &ctx).unwrap();
    assert_eq!(result, "alpha and beta");
}

#[test]
fn truncate_when_under_limit_is_noop() {
    let engine = engine();
    let mut ctx = Context::new();
    ctx.insert("items", Value::List(vec!["a".into(), "b".into()]));

    let result = engine.render_inline("{items|truncate:5|join}", &ctx).unwrap();
    assert_eq!(result, "a and b");
}

#[test]
fn number_as_words() {
    let engine = engine();
    let mut ctx = Context::new();
    ctx.insert("n", Value::Number(42));

    let result = engine.render_inline("{n|words}", &ctx).unwrap();
    assert_eq!(result, "forty-two");
}

#[test]
fn ordinal_rendering() {
    let engine = engine();
    let mut ctx = Context::new();
    ctx.insert("n", Value::Number(3));

    let result = engine.render_inline("{n|ordinal}", &ctx).unwrap();
    assert_eq!(result, "3rd");
}

#[test]
fn capitalize_rendering() {
    let engine = engine();
    let mut ctx = Context::new();
    ctx.insert("word", Value::String("hello world".into()));

    let result = engine.render_inline("{word|capitalize}", &ctx).unwrap();
    assert_eq!(result, "Hello world");
}

#[test]
fn article_rendering() {
    let engine = engine();

    let mut ctx = Context::new();
    ctx.insert("thing", Value::String("apple".into()));
    assert_eq!(engine.render_inline("{thing|article}", &ctx).unwrap(), "an apple");

    ctx.insert("thing", Value::String("banana".into()));
    assert_eq!(engine.render_inline("{thing|article}", &ctx).unwrap(), "a banana");

    ctx.insert("thing", Value::String("hour".into()));
    assert_eq!(engine.render_inline("{thing|article}", &ctx).unwrap(), "an hour");

    ctx.insert("thing", Value::String("university".into()));
    assert_eq!(engine.render_inline("{thing|article}", &ctx).unwrap(), "a university");
}

// ── Snapshot-style tests: exact output for realistic scenarios ────────────

#[test]
fn snapshot_angular_service_rename() {
    let mut engine = engine();
    engine
        .register_template(
            "change",
            "The {entity_type} {old_name} was renamed to {new_name} which impacts \
             {count} direct {count|pluralize:consumer} {consumers|truncate:3|join:bracketed}",
        )
        .unwrap();

    let mut ctx = Context::new();
    ctx.insert("entity_type", Value::String("class".into()));
    ctx.insert("old_name", Value::String("UserService".into()));
    ctx.insert("new_name", Value::String("AccountService".into()));
    ctx.insert("count", Value::Number(4));
    ctx.insert(
        "consumers",
        Value::List(vec![
            "ProfileComponent".into(),
            "SettingsComponent".into(),
            "AdminModule".into(),
            "AuthGuard".into(),
        ]),
    );

    assert_eq!(
        engine.render("change", &ctx).unwrap(),
        "The class UserService was renamed to AccountService which impacts \
         4 direct consumers [ProfileComponent, SettingsComponent, AdminModule, and 1 more]"
    );
}

#[test]
fn snapshot_interface_deleted_no_consumers() {
    let mut engine = engine();
    engine
        .register_template(
            "deleted",
            "The {entity_type} {name} was removed with no remaining consumers",
        )
        .unwrap();

    let mut ctx = Context::new();
    ctx.insert("entity_type", Value::String("interface".into()));
    ctx.insert("name", Value::String("LegacyConfig".into()));

    assert_eq!(
        engine.render("deleted", &ctx).unwrap(),
        "The interface LegacyConfig was removed with no remaining consumers"
    );
}

#[test]
fn snapshot_method_signature_change() {
    let mut engine = engine();
    engine
        .register_template(
            "sig",
            "The signature of {name} was changed, requiring updates in \
             {count} {count|pluralize:caller} [{callers|join}]",
        )
        .unwrap();

    let mut ctx = Context::new();
    ctx.insert("name", Value::String("getUser".into()));
    ctx.insert("count", Value::Number(2));
    ctx.insert(
        "callers",
        Value::List(vec!["ProfileController".into(), "AdminPanel".into()]),
    );

    assert_eq!(
        engine.render("sig", &ctx).unwrap(),
        "The signature of getUser was changed, requiring updates in \
         2 callers [ProfileController and AdminPanel]"
    );
}
