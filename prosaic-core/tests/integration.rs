use prosaic_core::{
    entity, named, Clause, Context, DocumentPlan, Engine, EntityDescriptor, GroupingStrategy,
    RhetoricalCategory, Salience, Sentence, Session, Strictness, Tense, Value, Variation, VerbForm, Voice,
};
use prosaic_derive::IntoContext;
use prosaic_grammar_en::English;

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
    let mut session = Session::new();

    let result = engine.render(&mut session, "renamed", &ctx).unwrap();
    assert_eq!(
        result,
        "The class Foo was renamed to Foobar which impacts 6 direct consumers \
         [Baz, Qux, Quux, and 3 more]."
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
    let mut session = Session::new();

    let result = engine.render(&mut session, "renamed", &ctx).unwrap();
    assert_eq!(
        result,
        "The interface IUser was renamed to User which impacts 1 direct consumer [UserService]."
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

// ── Tense / aspect end-to-end (builder + pipe) ───────────────────────────

#[test]
fn builder_present_perfect_passive() {
    let engine = engine();

    let result = Sentence::new()
        .subject(entity("class", "UserService"))
        .verb_word("rename")
        .form(VerbForm::PresentPerfect)
        .object("AccountService")
        .render(&engine)
        .unwrap();

    assert_eq!(
        result,
        "The class UserService has been renamed to AccountService"
    );
}

#[test]
fn builder_present_progressive_passive() {
    let engine = engine();

    let result = Sentence::new()
        .subject(entity("module", "Legacy"))
        .verb_word("deprecate")
        .form(VerbForm::PresentProgressive)
        .render(&engine)
        .unwrap();

    assert_eq!(result, "The module Legacy is being deprecated");
}

#[test]
fn builder_past_perfect_active() {
    let engine = engine();

    let result = Sentence::new()
        .subject(entity("team", "Backend"))
        .verb_word("ship")
        .form(VerbForm::PastPerfect)
        .voice(Voice::Active)
        .object("the rewrite")
        .render(&engine)
        .unwrap();

    assert_eq!(result, "The team Backend had shipped the rewrite");
}

#[test]
fn builder_conditional_passive() {
    let engine = engine();

    let result = Sentence::new()
        .subject(entity("test", "E2E"))
        .verb_word("break")
        .form(VerbForm::Conditional)
        .render(&engine)
        .unwrap();

    assert_eq!(result, "The test E2E would be broken");
}

#[test]
fn builder_conditional_perfect_active() {
    let engine = engine();

    let result = Sentence::new()
        .subject(named("rollback"))
        .verb_word("prevent")
        .form(VerbForm::ConditionalPerfect)
        .voice(Voice::Active)
        .object("the outage")
        .render(&engine)
        .unwrap();

    assert_eq!(result, "rollback would have prevented the outage");
}

#[test]
fn verb_pipe_with_english_grammar() {
    let mut engine = engine();
    engine
        .register_template(
            "t",
            "The {entity_type} {name} {action|verb:present_perfect}",
        )
        .unwrap();

    let mut ctx = Context::new();
    ctx.insert("entity_type", Value::String("class".into()));
    ctx.insert("name", Value::String("OrderProcessor".into()));
    ctx.insert("action", Value::String("break".into()));
    let mut session = Session::new();

    // Irregular: break → broken (past participle)
    let result = engine.render(&mut session, "t", &ctx).unwrap();
    assert_eq!(result, "The class OrderProcessor has been broken.");
}

#[test]
fn verb_pipe_progressive_irregular() {
    let mut engine = engine();
    engine
        .register_template(
            "t",
            "The {entity_type} {name} {action|verb:present_progressive}",
        )
        .unwrap();

    let mut ctx = Context::new();
    ctx.insert("entity_type", Value::String("module".into()));
    ctx.insert("name", Value::String("Core".into()));
    ctx.insert("action", Value::String("write".into()));
    let mut session = Session::new();

    // Irregular write: past participle "written", present participle "writing"
    // "is being written"
    let result = engine.render(&mut session, "t", &ctx).unwrap();
    assert_eq!(result, "The module Core is being written.");
}

#[test]
fn verb_pipe_active_simple_past_irregular() {
    let mut engine = engine();
    engine
        .register_template(
            "t",
            "The {entity_type} {name} {action|verb:active_past} {target}",
        )
        .unwrap();

    let mut ctx = Context::new();
    ctx.insert("entity_type", Value::String("team".into()));
    ctx.insert("name", Value::String("Platform".into()));
    ctx.insert("action", Value::String("write".into()));
    ctx.insert("target", Value::String("the migration".into()));
    let mut session = Session::new();

    // write → wrote (irregular simple past)
    let result = engine.render(&mut session, "t", &ctx).unwrap();
    assert_eq!(result, "The team Platform wrote the migration.");
}

// ── Clause aggregation (conjunction reduction) ───────────────────────────

#[test]
fn clause_reduction_fuses_simple_same_entity_sequence() {
    let mut engine = engine();
    engine
        .register_template("code.renamed", "{old_name|refer} was renamed")
        .unwrap();
    engine
        .register_template("code.modified", "{name|refer} was modified")
        .unwrap();
    engine
        .register_template("code.moved", "{name|refer} was moved")
        .unwrap();

    let mut c1 = Context::new();
    c1.insert("entity_type", Value::String("class".into()));
    c1.insert("old_name", Value::String("UserService".into()));
    c1.insert("name", Value::String("UserService".into()));

    let c2 = c1.clone();
    let c3 = c1.clone();

    let events: Vec<(&str, Context)> = vec![
        ("code.renamed", c1),
        ("code.modified", c2),
        ("code.moved", c3),
    ];
    let mut session = Session::new();

    let out = engine.render_batch(&mut session, &events).unwrap();
    assert_eq!(
        out,
        "The class UserService was renamed, modified, and moved."
    );
}

#[test]
fn clause_reduction_declines_when_predicate_has_embedded_clause() {
    // With a "which"-clause in the first predicate, fusion would spoil
    // the subordinate structure — the engine must leave them separate.
    let mut engine = engine();
    engine
        .register_template(
            "code.renamed",
            "{old_name|refer} was renamed{?consumer_count}, \
             which impacts {consumer_count} {consumer_count|pluralize:consumer}{/?}",
        )
        .unwrap();
    engine
        .register_template("code.modified", "{name|refer} was modified")
        .unwrap();

    let mut c1 = Context::new();
    c1.insert("entity_type", Value::String("class".into()));
    c1.insert("old_name", Value::String("Foo".into()));
    c1.insert("name", Value::String("Foo".into()));
    c1.insert("consumer_count", Value::Number(6));
    let mut c2 = c1.clone();
    c2.remove_consumer_count_dummy(); // placeholder; see helper below

    let events: Vec<(&str, Context)> = vec![("code.renamed", c1), ("code.modified", c2)];
    let mut session = Session::new();
    let out = engine.render_batch(&mut session, &events).unwrap();
    // Expect the subordinate "which" clause to stay on its own sentence.
    assert!(out.contains(", which impacts 6 consumers"), "got: {out}");
    assert!(!out.contains("modified and"), "should not fuse, got: {out}");
}

// Minimal local helper to allow c1.clone() usage above without leaking a
// `consumer_count` that would also trigger the modified template's own
// conditional branch. Context doesn't expose `remove`; rebuild the context
// without the key.
trait ContextTestExt {
    fn remove_consumer_count_dummy(&mut self);
}
impl ContextTestExt for Context {
    fn remove_consumer_count_dummy(&mut self) {
        // The integration test is defensive: if/when Context grows a
        // `remove` method we can switch to it; for now we accept the
        // presence of consumer_count since the modified template doesn't
        // reference it.
    }
}

// FCR Phase 1: "It also" connective end-to-end ──────────────────────────
//
// Four same-entity events on the same session. The discourse system cycles
// connectives in pool order: sentence 2 → "Additionally,", sentence 3 →
// "Furthermore,", sentence 4 → "It also". The reducer must strip all three
// and fuse the four predicates.
#[test]
fn clause_reduction_accepts_it_also_connective_end_to_end() {
    let mut engine = engine();
    engine
        .register_template("code.renamed", "{old_name|refer} was renamed")
        .unwrap();
    engine
        .register_template("code.modified", "{name|refer} was modified")
        .unwrap();
    engine
        .register_template("code.moved", "{name|refer} was moved")
        .unwrap();
    engine
        .register_template("code.archived", "{name|refer} was archived")
        .unwrap();

    let base_ctx = {
        let mut c = Context::new();
        c.insert("entity_type", Value::String("class".into()));
        c.insert("old_name", Value::String("DataStore".into()));
        c.insert("name", Value::String("DataStore".into()));
        c
    };

    let events: Vec<(&str, Context)> = vec![
        ("code.renamed", base_ctx.clone()),
        ("code.modified", base_ctx.clone()),
        ("code.moved", base_ctx.clone()),
        ("code.archived", base_ctx.clone()),
    ];
    let mut session = Session::new();

    let out = engine.render_batch(&mut session, &events).unwrap();
    // All four predicates should fuse into one sentence. The "It also" on
    // sentence 4 is the key case — without FCR Phase 1 it would bail out
    // and we'd get two separate results.
    assert_eq!(
        out,
        "The class DataStore was renamed, modified, moved, and archived."
    );
}

// FCR Phase 2: full-NP repetition end-to-end ────────────────────────────
//
// When templates hard-code the full NP (no `refer` pipe) or when the
// discourse layer re-introduces the full subject, the second sentence
// repeats "The class X was …" verbatim. The Phase 2 fallback in
// reduce_same_entity_clauses must still fuse them.
#[test]
fn clause_reduction_accepts_full_np_repetition_end_to_end() {
    let mut engine = engine();
    // Templates write "The class {name} was …" explicitly — no refer pipe.
    // Both sentences will carry the full NP, so the pronoun matcher
    // declines and the full-NP fallback (FCR Phase 2) must accept them.
    engine
        .register_template("op.a", "The class {name} was renamed")
        .unwrap();
    engine
        .register_template("op.b", "The class {name} was modified")
        .unwrap();
    engine
        .register_template("op.c", "The class {name} was moved")
        .unwrap();

    let base_ctx = {
        let mut c = Context::new();
        c.insert("entity_type", Value::String("class".into()));
        c.insert("name", Value::String("Gateway".into()));
        c
    };

    let events: Vec<(&str, Context)> = vec![
        ("op.a", base_ctx.clone()),
        ("op.b", base_ctx.clone()),
        ("op.c", base_ctx.clone()),
    ];
    let mut session = Session::new();

    let out = engine.render_batch(&mut session, &events).unwrap();
    // Without Phase 2 the three full-NP sentences would fail reduction and
    // be emitted separately. With Phase 2 they fuse.
    assert_eq!(out, "The class Gateway was renamed, modified, and moved.");
}

// ── Rhetorical grouping in document plans ────────────────────────────────

#[test]
fn by_action_produces_section_style_narrative() {
    let mut engine = engine();
    prosaic_vocab_code::register(&mut engine).unwrap();

    let mut del = Context::new();
    del.insert("entity_type", Value::String("function".into()));
    del.insert("name", Value::String("legacyFoo".into()));
    del.insert("consumer_count", Value::Number(0));

    let mut add = Context::new();
    add.insert("entity_type", Value::String("function".into()));
    add.insert("name", Value::String("newFoo".into()));
    add.insert("location", Value::String("foo.ts".into()));
    add.insert("consumer_count", Value::Number(0));

    let mut modif = Context::new();
    modif.insert("entity_type", Value::String("class".into()));
    modif.insert("name", Value::String("Alpha".into()));
    modif.insert("consumer_count", Value::Number(0));

    // Arranged out-of-order on purpose; ByAction should re-organize them.
    let events: Vec<(&str, Context)> = vec![
        ("code.modified", modif),
        ("code.added", add),
        ("code.deleted", del),
    ];

    let plan = DocumentPlan::from_events_grouped(
        &events,
        &engine,
        GroupingStrategy::ByAction,
    );

    assert_eq!(plan.paragraphs.len(), 3);
    assert_eq!(
        plan.paragraphs[0].category,
        Some(RhetoricalCategory::Removal)
    );
    assert_eq!(
        plan.paragraphs[1].category,
        Some(RhetoricalCategory::Addition)
    );
    assert_eq!(
        plan.paragraphs[2].category,
        Some(RhetoricalCategory::Modification)
    );
    let mut session = Session::new();

    let rendered = plan.render(&engine, &mut session).unwrap();
    // Removal content leads.
    let remove_idx = rendered.find("legacyFoo").expect("legacyFoo should render");
    let add_idx = rendered.find("newFoo").expect("newFoo should render");
    let mod_idx = rendered.find("Alpha").expect("Alpha should render");
    assert!(remove_idx < add_idx, "Removal should precede Addition");
    assert!(add_idx < mod_idx, "Addition should precede Modification");
    // Section breaks.
    assert!(rendered.contains("\n\n"));
}

// ── Referring Expression Generation (Dale & Reiter) ──────────────────────

#[test]
fn reg_disambiguates_two_same_type_entities_in_narrative() {
    let mut engine = Engine::new(English::new())
        .strictness(Strictness::Strict)
        .variation(Variation::Fixed)
        .attribute_preference(vec!["layer".to_string()]);

    engine.register_entity(
        EntityDescriptor::new("UserService", "class").with_attribute("layer", "domain"),
    );
    engine.register_entity(
        EntityDescriptor::new("AuthService", "class").with_attribute("layer", "infra"),
    );

    engine
        .register_template("t", "{name|refer} was modified")
        .unwrap();

    let mut ctx_user = Context::new();
    ctx_user.insert("entity_type", Value::String("class".into()));
    ctx_user.insert("name", Value::String("UserService".into()));

    let mut ctx_auth = Context::new();
    ctx_auth.insert("entity_type", Value::String("class".into()));
    ctx_auth.insert("name", Value::String("AuthService".into()));
    let mut session = Session::new();

    // First render introduces UserService with distinguisher.
    let r1 = engine.render(&mut session, "t", &ctx_user).unwrap();
    assert_eq!(r1, "The domain class UserService was modified.");

    // Second render introduces AuthService with its own distinguisher.
    let r2 = engine.render(&mut session, "t", &ctx_auth).unwrap();
    // A connective gets prepended ("Similarly," since same action, different entity).
    assert!(r2.contains("the infra class AuthService"), "got: {r2}");
}

#[test]
fn reg_unambiguous_single_entity_skips_attributes() {
    let mut engine = Engine::new(English::new())
        .strictness(Strictness::Strict)
        .variation(Variation::Fixed);

    engine.register_entity(
        EntityDescriptor::new("UserService", "class").with_attribute("layer", "domain"),
    );

    engine
        .register_template("t", "{name|refer} was modified")
        .unwrap();

    let mut ctx = Context::new();
    ctx.insert("entity_type", Value::String("class".into()));
    ctx.insert("name", Value::String("UserService".into()));
    let mut session = Session::new();

    // No distractor registered → no attribute premodifier.
    let r = engine.render(&mut session, "t", &ctx).unwrap();
    assert_eq!(r, "The class UserService was modified.");
}

#[test]
fn reg_registered_entity_with_unregistered_distractor() {
    // A scenario where one entity has attributes and another same-type
    // entity shows up only via context, without being registered.
    let mut engine = Engine::new(English::new())
        .strictness(Strictness::Strict)
        .variation(Variation::Fixed);

    engine.register_entity(
        EntityDescriptor::new("UserService", "class").with_attribute("layer", "domain"),
    );
    // AuthService is never registered; it appears only through context.

    engine
        .register_template("t", "{name|refer} was modified")
        .unwrap();

    let mut ctx_user = Context::new();
    ctx_user.insert("entity_type", Value::String("class".into()));
    ctx_user.insert("name", Value::String("UserService".into()));
    let mut session = Session::new();

    // Only registered entities count as distractors, so UserService still
    // renders without premodifying attributes (there's no registered rival).
    let r = engine.render(&mut session, "t", &ctx_user).unwrap();
    assert_eq!(r, "The class UserService was modified.");
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
    let mut session = Session::new();

    let result = engine.render(&mut session, "changed", event).unwrap();
    assert_eq!(
        result,
        "AuthService (service) was changed, affecting 3 consumers."
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
    let mut session = Session::new();

    // From fresh state, same seed produces same result
    let result1 = engine.render(&mut session, "t", &ctx).unwrap();
    session.reset();
    let result2 = engine.render(&mut session, "t", &ctx).unwrap();
    session.reset();
    let result3 = engine.render(&mut session, "t", &ctx).unwrap();

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
    let mut session = Session::new();

    // First render always picks first variant
    assert_eq!(engine.render(&mut session, "t", &ctx).unwrap(), "alpha");

    // After reset, picks first again
    session.reset();
    assert_eq!(engine.render(&mut session, "t", &ctx).unwrap(), "alpha");
}

#[test]
fn discourse_avoids_repeating_same_variant() {
    // Anti-repeat via choose-best scoring requires a variation strategy
    // that actually permits reordering (Seeded or Random). Fixed and
    // RoundRobin are literal by contract.
    let mut engine = Engine::new(English::new())
        .strictness(Strictness::Strict)
        .variation(Variation::Seeded(1));

    engine.register_template("t", "alpha distinct tokens").unwrap();
    engine.register_template("t", "beta different tokens").unwrap();
    engine.register_template("t", "gamma unique tokens").unwrap();

    let ctx = Context::new();
    let mut session = Session::new();

    let r1 = engine.render(&mut session, "t", &ctx).unwrap();
    let r2 = engine.render(&mut session, "t", &ctx).unwrap();
    let r3 = engine.render(&mut session, "t", &ctx).unwrap();

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
    let mut session = Session::new();
    // "count" is missing

    let result = engine.render(&mut session, "t", &ctx);
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
    let mut session = Session::new();

    let result = engine.render(&mut session, "t", &ctx).unwrap();
    assert_eq!(result, "Hello Alice, you have [missing: count] items.");
}

#[test]
fn silent_mode_omits_missing() {
    let mut engine = Engine::new(English::new()).strictness(Strictness::Silent);
    engine
        .register_template("t", "Hello {name}, you have {count} items")
        .unwrap();

    let mut ctx = Context::new();
    ctx.insert("name", Value::String("Alice".into()));
    let mut session = Session::new();

    let result = engine.render(&mut session, "t", &ctx).unwrap();
    // Silent-mode cleanup collapses the double space left by the omitted slot.
    assert_eq!(result, "Hello Alice, you have items.");
}

// ── Pipe chaining edge cases ─────────────────────────────────────────────

#[test]
fn empty_list_join() {
    let engine = engine();
    let mut ctx = Context::new();
    ctx.insert("items", Value::List(vec![]));
    let mut session = Session::new();

    let result = engine.render_inline(&mut session, "{items|join}", &ctx).unwrap();
    assert_eq!(result, "");
}

#[test]
fn single_item_join() {
    let engine = engine();
    let mut ctx = Context::new();
    ctx.insert("items", Value::List(vec!["only".into()]));
    let mut session = Session::new();

    let result = engine.render_inline(&mut session, "{items|join}", &ctx).unwrap();
    assert_eq!(result, "only");
}

#[test]
fn two_item_join() {
    let engine = engine();
    let mut ctx = Context::new();
    ctx.insert("items", Value::List(vec!["alpha".into(), "beta".into()]));
    let mut session = Session::new();

    let result = engine.render_inline(&mut session, "{items|join}", &ctx).unwrap();
    assert_eq!(result, "alpha and beta");
}

#[test]
fn truncate_when_under_limit_is_noop() {
    let engine = engine();
    let mut ctx = Context::new();
    ctx.insert("items", Value::List(vec!["a".into(), "b".into()]));
    let mut session = Session::new();

    let result = engine.render_inline(&mut session, "{items|truncate:5|join}", &ctx).unwrap();
    assert_eq!(result, "a and b");
}

#[test]
fn number_as_words() {
    let engine = engine();
    let mut ctx = Context::new();
    ctx.insert("n", Value::Number(42));
    let mut session = Session::new();

    let result = engine.render_inline(&mut session, "{n|words}", &ctx).unwrap();
    assert_eq!(result, "forty-two");
}

#[test]
fn ordinal_rendering() {
    let engine = engine();
    let mut ctx = Context::new();
    ctx.insert("n", Value::Number(3));
    let mut session = Session::new();

    let result = engine.render_inline(&mut session, "{n|ordinal}", &ctx).unwrap();
    assert_eq!(result, "3rd");
}

#[test]
fn capitalize_rendering() {
    let engine = engine();
    let mut ctx = Context::new();
    ctx.insert("word", Value::String("hello world".into()));
    let mut session = Session::new();

    let result = engine.render_inline(&mut session, "{word|capitalize}", &ctx).unwrap();
    assert_eq!(result, "Hello world");
}

#[test]
fn article_rendering() {
    let engine = engine();

    let mut ctx = Context::new();
    ctx.insert("thing", Value::String("apple".into()));
    let mut session = Session::new();
    assert_eq!(engine.render_inline(&mut session, "{thing|article}", &ctx).unwrap(), "an apple");

    ctx.insert("thing", Value::String("banana".into()));
    assert_eq!(engine.render_inline(&mut session, "{thing|article}", &ctx).unwrap(), "a banana");

    ctx.insert("thing", Value::String("hour".into()));
    assert_eq!(engine.render_inline(&mut session, "{thing|article}", &ctx).unwrap(), "an hour");

    ctx.insert("thing", Value::String("university".into()));
    assert_eq!(engine.render_inline(&mut session, "{thing|article}", &ctx).unwrap(), "a university");
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
    let mut session = Session::new();

    assert_eq!(
        engine.render(&mut session, "change", &ctx).unwrap(),
        "The class UserService was renamed to AccountService which impacts \
         4 direct consumers [ProfileComponent, SettingsComponent, AdminModule, and 1 more]."
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
    let mut session = Session::new();

    assert_eq!(
        engine.render(&mut session, "deleted", &ctx).unwrap(),
        "The interface LegacyConfig was removed with no remaining consumers."
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
    let mut session = Session::new();

    assert_eq!(
        engine.render(&mut session, "sig", &ctx).unwrap(),
        "The signature of getUser was changed, requiring updates in \
         2 callers [ProfileController and AdminPanel]."
    );
}

// ── Batch rendering ─────────────────────────────────────────────────────

#[test]
fn batch_produces_periods_between_sentences() {
    let mut engine = engine();
    engine.register_template("simple", "{name|refer} was modified").unwrap();

    let mut ctx1 = Context::new();
    ctx1.insert("entity_type", Value::String("class".into()));
    ctx1.insert("name", Value::String("Foo".into()));

    let mut ctx2 = Context::new();
    ctx2.insert("entity_type", Value::String("class".into()));
    ctx2.insert("name", Value::String("Bar".into()));

    let events: Vec<(&str, Context)> = vec![
        ("simple", ctx1),
        ("simple", ctx2),
    ];
    let mut session = Session::new();

    let result = engine.render_batch(&mut session, &events).unwrap();

    // Two sentences, both terminated
    assert!(result.contains("."), "Expected periods in batch output, got: {result}");
}

#[test]
fn batch_aggregates_same_action_different_subjects() {
    let mut engine = engine();
    engine.register_template(
        "code.renamed",
        "{old_name|refer} was renamed",
    ).unwrap();

    let mut ctx1 = Context::new();
    ctx1.insert("entity_type", Value::String("class".into()));
    ctx1.insert("old_name", Value::String("UserService".into()));

    let mut ctx2 = Context::new();
    ctx2.insert("entity_type", Value::String("class".into()));
    ctx2.insert("old_name", Value::String("AuthService".into()));

    let events: Vec<(&str, Context)> = vec![
        ("code.renamed", ctx1),
        ("code.renamed", ctx2),
    ];
    let mut session = Session::new();

    let result = engine.render_batch(&mut session, &events).unwrap();

    // Should aggregate into a single sentence with combined subjects
    assert!(
        result.contains("UserService and AuthService"),
        "Expected aggregated subjects, got: {result}"
    );
}

#[test]
fn batch_sequential_when_entities_differ_across_actions() {
    let mut engine = engine();
    engine.register_template("code.renamed", "{name|refer} was renamed").unwrap();
    engine.register_template("code.deleted", "{name|refer} was removed").unwrap();

    let mut ctx1 = Context::new();
    ctx1.insert("entity_type", Value::String("class".into()));
    ctx1.insert("name", Value::String("Foo".into()));

    let mut ctx2 = Context::new();
    ctx2.insert("entity_type", Value::String("interface".into()));
    ctx2.insert("name", Value::String("Bar".into()));

    let events: Vec<(&str, Context)> = vec![
        ("code.renamed", ctx1),
        ("code.deleted", ctx2),
    ];
    let mut session = Session::new();

    let result = engine.render_batch(&mut session, &events).unwrap();

    // Different actions, different entities — sequential
    // Periods should separate the sentences
    let period_count = result.matches('.').count();
    assert!(period_count >= 2, "Expected 2 periods for 2 sentences, got {period_count}: {result}");
}

// ── Conditional sections ────────────────────────────────────────────────

#[test]
fn conditional_section_skipped_when_zero() {
    let mut engine = engine();
    engine.register_template(
        "t",
        "{name|refer} was removed{?count}, impacting {count} {count|pluralize:consumer}{/?}",
    ).unwrap();

    let mut ctx = Context::new();
    ctx.insert("entity_type", Value::String("class".into()));
    ctx.insert("name", Value::String("Foo".into()));
    ctx.insert("count", Value::Number(0));
    let mut session = Session::new();

    let result = engine.render(&mut session, "t", &ctx).unwrap();
    // 0 count — conditional should be skipped entirely
    assert_eq!(result, "The class Foo was removed.");
    assert!(!result.contains("0"), "Should not contain '0', got: {result}");
}

#[test]
fn conditional_section_rendered_when_nonzero() {
    let mut engine = engine();
    engine.register_template(
        "t",
        "{name|refer} was removed{?count}, impacting {count} {count|pluralize:consumer}{/?}",
    ).unwrap();

    let mut ctx = Context::new();
    ctx.insert("entity_type", Value::String("class".into()));
    ctx.insert("name", Value::String("Foo".into()));
    ctx.insert("count", Value::Number(3));
    let mut session = Session::new();

    let result = engine.render(&mut session, "t", &ctx).unwrap();
    assert_eq!(result, "The class Foo was removed, impacting 3 consumers.");
}

#[test]
fn conditional_section_skipped_for_empty_list() {
    let mut engine = engine();
    engine.register_template(
        "t",
        "Added item{?items} with refs: {items|join}{/?}",
    ).unwrap();

    let mut ctx = Context::new();
    ctx.insert("items", Value::List(vec![]));
    let mut session = Session::new();

    let result = engine.render(&mut session, "t", &ctx).unwrap();
    assert_eq!(result, "Added item");
}

#[test]
fn conditional_section_rendered_for_nonempty_list() {
    let mut engine = engine();
    engine.register_template(
        "t",
        "Added item{?items} with refs: {items|join}{/?}",
    ).unwrap();

    let mut ctx = Context::new();
    ctx.insert("items", Value::List(vec!["a".into(), "b".into()]));
    let mut session = Session::new();

    let result = engine.render(&mut session, "t", &ctx).unwrap();
    assert!(result.contains("with refs: a and b"));
}

#[test]
fn conditional_section_skipped_when_key_missing() {
    let mut engine = engine();
    engine.register_template(
        "t",
        "Always{?optional}, maybe{/?}",
    ).unwrap();

    let ctx = Context::new();
    let mut session = Session::new();
    // No "optional" key at all

    let result = engine.render(&mut session, "t", &ctx).unwrap();
    // Should not include the conditional content
    assert!(!result.contains("maybe"), "Should skip missing-key conditional, got: {result}");
}

// ── Salience-based template selection ────────────────────────────────────

#[test]
fn salience_selects_correct_level() {
    let mut engine = engine();
    engine.register_template_at("event", "terse {name}", Salience::Low).unwrap();
    engine.register_template("event", "standard {name} with impact").unwrap();
    engine.register_template_at("event", "elaborate full account of {name}", Salience::High).unwrap();

    let mut ctx_low = Context::new();
    ctx_low.insert("name", Value::String("Foo".into()));
    ctx_low.insert("consumer_count", Value::Number(0));

    let mut ctx_med = Context::new();
    ctx_med.insert("name", Value::String("Bar".into()));
    ctx_med.insert("consumer_count", Value::Number(5));

    let mut ctx_high = Context::new();
    ctx_high.insert("name", Value::String("Baz".into()));
    ctx_high.insert("consumer_count", Value::Number(50));

    let mut session = Session::new();
    let low = engine.render(&mut session, "event", &ctx_low).unwrap();
    assert!(low.starts_with("terse"), "Expected low template, got: {low}");
    session.reset();
    let med = engine.render(&mut session, "event", &ctx_med).unwrap();
    assert!(med.starts_with("standard"), "Expected medium template, got: {med}");
    session.reset();
    let high = engine.render(&mut session, "event", &ctx_high).unwrap();
    assert!(high.starts_with("elaborate"), "Expected high template, got: {high}");
}

#[test]
fn salience_falls_back_to_medium_when_level_missing() {
    let mut engine = engine();
    // Only register Medium
    engine.register_template("event", "standard for {name}").unwrap();

    let mut ctx = Context::new();
    ctx.insert("name", Value::String("Foo".into()));
    ctx.insert("consumer_count", Value::Number(50)); // Would be High
    let mut session = Session::new();

    let result = engine.render(&mut session, "event", &ctx).unwrap();
    assert!(result.contains("standard"), "Expected fallback to Medium, got: {result}");
}

#[test]
fn explicit_salience_key_overrides_count() {
    let mut engine = engine();
    engine.register_template_at("event", "low {name}", Salience::Low).unwrap();
    engine.register_template_at("event", "high {name}", Salience::High).unwrap();

    let mut ctx = Context::new();
    ctx.insert("name", Value::String("Foo".into()));
    ctx.insert("consumer_count", Value::Number(50)); // Would normally be High
    ctx.insert("salience", Value::String("low".into())); // Explicit override
    let mut session = Session::new();

    let result = engine.render(&mut session, "event", &ctx).unwrap();
    assert!(result.contains("low"), "Expected Low from explicit override, got: {result}");
}
