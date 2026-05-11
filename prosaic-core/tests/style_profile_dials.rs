//! Dial-by-dial behavior tests for `StyleProfile`.
//!
//! Each test holds six dials at neutral and asserts the seventh produces a
//! visible, documented change in output. The byte-equality gate at
//! `tests/style_profile_neutral.rs` covers the all-neutral baseline; this
//! file covers the per-dial deltas off that baseline.

use prosaic_core::{
    ConnectivePreferences, Context, Engine, HedgingCalibration, LengthDistribution, ListStyleBias,
    PronounDensity, RstRelation, Salience, SalienceBias, Session, StyleProfile, Value, Variation,
    Verbosity,
};
use prosaic_grammar_en::English;

fn engine_with(profile: StyleProfile) -> Engine {
    let mut engine = Engine::new(English::new())
        .variation(Variation::Fixed)
        .style_profile(profile);
    register_three_tier(&mut engine);
    engine
}

fn register_three_tier(engine: &mut Engine) {
    engine
        .register_template_at("evt", "TERSE: {name}", Salience::Low)
        .unwrap();
    engine
        .register_template_at("evt", "MEDIUM: {name} was modified", Salience::Medium)
        .unwrap();
    engine
        .register_template_at(
            "evt",
            "VERBOSE: {name} was modified with consequences",
            Salience::High,
        )
        .unwrap();
}

fn render(engine: &Engine, ctx: Context) -> String {
    let mut session = Session::new();
    engine.render(&mut session, "evt", &ctx).unwrap()
}

fn ctx_with_count(count: i64) -> Context {
    let mut c = Context::new();
    c.insert("name", Value::String("UserService".into()));
    c.insert("entity_type", Value::String("class".into()));
    c.insert("count", Value::Number(count));
    c.insert("consumer_count", Value::Number(count));
    c
}

#[test]
fn verbosity_terse_picks_lowest_tier_below_context_salience() {
    // consumer_count = 5 lands the context in Medium tier without a profile.
    // Terse shifts Medium → Low, so the engine should pick the Low variant.
    let engine = engine_with(
        StyleProfile::builder("terse")
            .verbosity(Verbosity::Terse)
            .build()
            .unwrap(),
    );
    let out = render(&engine, ctx_with_count(5));
    assert!(
        out.starts_with("TERSE:"),
        "expected Low-tier variant; got {out:?}"
    );
}

#[test]
fn verbosity_verbose_picks_higher_tier_above_context_salience() {
    // consumer_count = 5 lands the context in Medium. Verbose shifts to High;
    // the engine should pick the High-tier variant.
    let engine = engine_with(
        StyleProfile::builder("verbose")
            .verbosity(Verbosity::Verbose)
            .build()
            .unwrap(),
    );
    let out = render(&engine, ctx_with_count(5));
    assert!(
        out.starts_with("VERBOSE:"),
        "expected High-tier variant; got {out:?}"
    );
}

#[test]
fn verbosity_neutral_picks_context_tier_unchanged() {
    let engine = engine_with(StyleProfile::neutral());
    let out = render(&engine, ctx_with_count(5));
    assert!(
        out.starts_with("MEDIUM:"),
        "expected Medium-tier variant; got {out:?}"
    );
}

#[test]
fn verbosity_terse_at_low_tier_stays_low() {
    // consumer_count = 0 lands in Low. Terse can't go lower; output stays Low.
    let engine = engine_with(
        StyleProfile::builder("terse")
            .verbosity(Verbosity::Terse)
            .build()
            .unwrap(),
    );
    let out = render(&engine, ctx_with_count(0));
    assert!(
        out.starts_with("TERSE:"),
        "expected Low-tier variant; got {out:?}"
    );
}

#[test]
fn verbosity_verbose_at_high_tier_stays_high() {
    // consumer_count = 50 lands in High. Verbose can't go higher; stays High.
    let engine = engine_with(
        StyleProfile::builder("verbose")
            .verbosity(Verbosity::Verbose)
            .build()
            .unwrap(),
    );
    let out = render(&engine, ctx_with_count(50));
    assert!(
        out.starts_with("VERBOSE:"),
        "expected High-tier variant; got {out:?}"
    );
}

// ── ListStyleBias ───────────────────────────────────────────────────────

fn engine_for_lists(profile: StyleProfile) -> Engine {
    let mut engine = Engine::new(English::new())
        .variation(Variation::Fixed)
        .style_profile(profile);
    engine
        .register_template("evt", "Affecting {items|truncate:3|join}")
        .unwrap();
    engine
}

fn ctx_with_items(items: &[&str]) -> Context {
    let mut c = Context::new();
    c.insert(
        "items",
        Value::List(items.iter().map(|s| s.to_string()).collect()),
    );
    c
}

fn render_list_once(engine: &Engine) -> String {
    let mut session = Session::new();
    engine
        .render(
            &mut session,
            "evt",
            ctx_with_items(&["A", "B", "C", "D", "E", "F", "G"]),
        )
        .unwrap()
}

#[test]
fn list_style_bias_auto_preserves_default_rotation() {
    let neutral = render_list_once(&engine_for_lists(StyleProfile::neutral()));
    let auto = render_list_once(&engine_for_lists(
        StyleProfile::builder("auto")
            .list_style_bias(ListStyleBias::Auto)
            .build()
            .unwrap(),
    ));
    assert_eq!(neutral, auto);
}

#[test]
fn list_style_bias_such_as_picks_such_as_opener() {
    let engine = engine_for_lists(
        StyleProfile::builder("sa")
            .list_style_bias(ListStyleBias::SuchAs)
            .build()
            .unwrap(),
    );
    let out = render_list_once(&engine);
    assert!(
        out.contains("such as"),
        "expected such-as opener under SuchAs bias; got {out:?}"
    );
}

#[test]
fn list_style_bias_dash_picks_dash_opener() {
    let engine = engine_for_lists(
        StyleProfile::builder("dash")
            .list_style_bias(ListStyleBias::Dash)
            .build()
            .unwrap(),
    );
    let out = render_list_once(&engine);
    assert!(
        out.contains("— "),
        "expected em-dash opener under Dash bias; got {out:?}"
    );
}

#[test]
fn list_style_bias_bracketed_picks_bracketed_opener() {
    let engine = engine_for_lists(
        StyleProfile::builder("br")
            .list_style_bias(ListStyleBias::Bracketed)
            .build()
            .unwrap(),
    );
    let out = render_list_once(&engine);
    assert!(
        out.contains("[A"),
        "expected bracketed opener under Bracketed bias; got {out:?}"
    );
}

// ── Multi-dial composition ─────────────────────────────────────────────

#[test]
fn connective_preferences_compose_with_seeded_variation() {
    // Variation::Seeded picks among scored alternatives; ConnectivePreferences
    // narrows the connective pool. The two compose cleanly: filter applies
    // to connectives only, so multi-template variant selection still rotates
    // under seeded scoring while connectives stay constrained.
    let mut prefs = ConnectivePreferences::neutral();
    prefs
        .allowed
        .insert(RstRelation::Elaboration, vec!["Furthermore,".to_string()]);
    let mut engine = Engine::new(English::new())
        .variation(Variation::Seeded(7))
        .style_profile(
            StyleProfile::builder("furth")
                .connectives(prefs)
                .build()
                .unwrap(),
        );
    engine
        .register_template("code.modified", "{name|refer} was modified")
        .unwrap();
    engine
        .register_template("code.modified", "Modifications landed on {name|refer}")
        .unwrap();
    engine
        .register_template("code.renamed", "{name|refer} was renamed")
        .unwrap();

    let mut session = Session::new();
    engine
        .render(&mut session, "code.modified", ctx_class("UserService"))
        .unwrap();
    let second = engine
        .render(&mut session, "code.renamed", ctx_class("UserService"))
        .unwrap();
    assert!(
        second.starts_with("Furthermore,"),
        "filtered connective pool must apply under seeded variation; got {second:?}"
    );
}

#[test]
fn multi_dial_profile_render_is_deterministic_across_repeats() {
    let mut prefs = ConnectivePreferences::neutral();
    prefs
        .allowed
        .insert(RstRelation::Elaboration, vec!["Furthermore,".to_string()]);
    let profile = StyleProfile::builder("kitchen-sink")
        .verbosity(Verbosity::Terse)
        .salience(SalienceBias::Higher)
        .list_style_bias(ListStyleBias::Bracketed)
        .pronoun_density(PronounDensity::Low)
        .hedging_offset(-10)
        .connectives(prefs)
        .build()
        .unwrap();
    let mut engine = Engine::new(English::new())
        .variation(Variation::Seeded(3))
        .style_profile(profile);
    engine.register_template("warm", "ok").unwrap();
    engine
        .register_template_at(
            "evt",
            "TERSE: {name|refer} {confidence|hedge}",
            Salience::Low,
        )
        .unwrap();
    engine
        .register_template_at(
            "evt",
            "MEDIUM: {name|refer} {confidence|hedge} broke things",
            Salience::Medium,
        )
        .unwrap();

    let render_once = || -> String {
        let mut session = Session::new();
        let mut c = Context::new();
        c.insert("name", Value::String("UserService".into()));
        c.insert("entity_type", Value::String("class".into()));
        c.insert("confidence", Value::Number(60));
        engine.render(&mut session, "warm", &c).unwrap();
        engine.render(&mut session, "evt", &c).unwrap()
    };

    let first = render_once();
    for _ in 0..30 {
        assert_eq!(render_once(), first);
    }
}

#[test]
fn each_dial_independently_perturbs_baseline() {
    // Sanity: each non-neutral dial produces a different output than the
    // all-neutral baseline. Catches accidental no-op wiring.
    let mut engine_baseline = Engine::new(English::new()).variation(Variation::Fixed);
    engine_baseline.register_template("warm", "ok").unwrap();
    engine_baseline
        .register_template_at("evt", "Short {name}", Salience::Low)
        .unwrap();
    engine_baseline
        .register_template_at(
            "evt",
            "The {entity_type} {name} was significantly modified",
            Salience::Medium,
        )
        .unwrap();

    let baseline_render = |engine: &Engine| -> String {
        let mut session = Session::new();
        let mut c = Context::new();
        c.insert("name", Value::String("UserService".into()));
        c.insert("entity_type", Value::String("class".into()));
        c.insert("consumer_count", Value::Number(3));
        engine.render(&mut session, "warm", &c).unwrap();
        engine.render(&mut session, "evt", &c).unwrap()
    };

    let baseline = baseline_render(&engine_baseline);
    // count=3 lands in Medium under default thresholds.
    // - Verbosity::Terse shifts to Low.
    // - SalienceBias::Higher shifts low_max to 4 (count=3 → Low).
    // - SalienceBias::Lower shifts high_min to 15 (count=3 stays Medium —
    //   skipped from this perturbation set since Lower at this count doesn't
    //   cross a tier; the dedicated `salience_bias_*` tests cover Lower).
    let dial_perturbations: Vec<(&str, StyleProfile)> = vec![
        (
            "verbosity",
            StyleProfile::builder("v")
                .verbosity(Verbosity::Terse)
                .build()
                .unwrap(),
        ),
        (
            "salience_bias",
            StyleProfile::builder("s")
                .salience(SalienceBias::Higher)
                .build()
                .unwrap(),
        ),
    ];

    for (label, profile) in dial_perturbations {
        let mut engine = Engine::new(English::new())
            .variation(Variation::Fixed)
            .style_profile(profile);
        engine.register_template("warm", "ok").unwrap();
        engine
            .register_template_at("evt", "Short {name}", Salience::Low)
            .unwrap();
        engine
            .register_template_at(
                "evt",
                "The {entity_type} {name} was significantly modified",
                Salience::Medium,
            )
            .unwrap();
        let perturbed = baseline_render(&engine);
        assert_ne!(
            baseline, perturbed,
            "{label} dial should perturb output but didn't"
        );
    }
}

// ── HedgingCalibration ──────────────────────────────────────────────────

fn engine_for_hedges(profile: StyleProfile) -> Engine {
    let mut engine = Engine::new(English::new())
        .variation(Variation::Fixed)
        .style_profile(profile);
    engine.register_template("evt", "{c|hedge}").unwrap();
    engine
}

fn render_hedge(engine: &Engine, confidence: i64) -> String {
    let mut session = Session::new();
    let mut c = Context::new();
    c.insert("c", Value::Number(confidence));
    engine.render(&mut session, "evt", &c).unwrap()
}

#[test]
fn hedging_offset_shifts_bucket_upward() {
    let engine = engine_for_hedges(
        StyleProfile::builder("more-confident")
            .hedging_offset(20)
            .build()
            .unwrap(),
    );
    // confidence=60 (probable=probably). With +20 offset → 80 → likely.
    let out = render_hedge(&engine, 60);
    assert!(
        out.contains("likely"),
        "expected likely under +20 offset; got {out:?}"
    );
}

#[test]
fn hedging_offset_shifts_bucket_downward() {
    let engine = engine_for_hedges(
        StyleProfile::builder("less-confident")
            .hedging_offset(-30)
            .build()
            .unwrap(),
    );
    // confidence=60 (probable=probably). With -30 offset → 30 → possibly.
    let out = render_hedge(&engine, 60);
    assert!(
        out.contains("possibly"),
        "expected possibly under -30 offset; got {out:?}"
    );
}

#[test]
fn hedging_forbid_falls_through_toward_more_confident() {
    let mut calib = HedgingCalibration::neutral();
    calib.forbid.push("perhaps".to_string());
    let engine = engine_for_hedges(
        StyleProfile::builder("no-perhaps")
            .hedging(calib)
            .build()
            .unwrap(),
    );
    // confidence=10 (unlikely=perhaps). Forbid → walk up to next bucket
    // (possible=possibly).
    let out = render_hedge(&engine, 10);
    assert!(
        out.contains("possibly"),
        "forbid perhaps should fall through to possibly; got {out:?}"
    );
}

#[test]
fn hedging_forbid_at_top_falls_back_to_original() {
    let mut calib = HedgingCalibration::neutral();
    calib.forbid.push("certainly".to_string());
    let engine = engine_for_hedges(
        StyleProfile::builder("no-certain")
            .hedging(calib)
            .build()
            .unwrap(),
    );
    // confidence=95 (certain=certainly). Forbid + no higher bucket → fall
    // back to the calibrated original. (Spec: never error out — degrade.)
    let out = render_hedge(&engine, 95);
    assert!(
        out.contains("certainly"),
        "no fallthrough above certain → keep original; got {out:?}"
    );
}

// ── PronounDensity ──────────────────────────────────────────────────────

fn engine_for_pronouns(profile: StyleProfile) -> Engine {
    let mut engine = Engine::new(English::new())
        .variation(Variation::Fixed)
        .style_profile(profile);
    engine
        .register_template("evt", "{name|refer} was inspected")
        .unwrap();
    engine
}

fn render_two_same_entity(engine: &Engine) -> (String, String) {
    let mut session = Session::new();
    let mut c = Context::new();
    c.insert("name", Value::String("UserService".into()));
    c.insert("entity_type", Value::String("class".into()));
    let first = engine.render(&mut session, "evt", &c).unwrap();
    let second = engine.render(&mut session, "evt", &c).unwrap();
    (first, second)
}

#[test]
fn pronoun_density_low_demotes_pronoun_to_short_name() {
    let neutral_engine = engine_for_pronouns(StyleProfile::neutral());
    let (_n_first, n_second) = render_two_same_entity(&neutral_engine);

    let low_engine = engine_for_pronouns(
        StyleProfile::builder("low")
            .pronoun_density(PronounDensity::Low)
            .build()
            .unwrap(),
    );
    let (_l_first, l_second) = render_two_same_entity(&low_engine);

    // Neutral path normally emits a pronoun on the second mention.
    // Low must produce something with a noun (the entity name), not the pronoun.
    assert_ne!(n_second, l_second);
    assert!(
        l_second.contains("UserService"),
        "Low density should keep entity name visible; got {l_second:?}"
    );
}

#[test]
fn pronoun_density_default_matches_unspecified() {
    let unspecified = engine_for_pronouns(StyleProfile::neutral());
    let default_dial = engine_for_pronouns(
        StyleProfile::builder("default")
            .pronoun_density(PronounDensity::Default)
            .build()
            .unwrap(),
    );
    assert_eq!(
        render_two_same_entity(&unspecified),
        render_two_same_entity(&default_dial)
    );
}

// ── ConnectivePreferences ───────────────────────────────────────────────

fn engine_for_connectives(profile: StyleProfile) -> Engine {
    let mut engine = Engine::new(English::new())
        .variation(Variation::Fixed)
        .style_profile(profile);
    engine
        .register_template("code.modified", "{name|refer} was modified")
        .unwrap();
    engine
        .register_template("code.renamed", "{name|refer} was renamed")
        .unwrap();
    engine
}

fn ctx_class(name: &str) -> Context {
    let mut c = Context::new();
    c.insert("name", Value::String(name.into()));
    c.insert("entity_type", Value::String("class".into()));
    c
}

fn render_same_entity_different_action(engine: &Engine) -> String {
    let mut session = Session::new();
    engine
        .render(&mut session, "code.modified", ctx_class("UserService"))
        .unwrap();
    engine
        .render(&mut session, "code.renamed", ctx_class("UserService"))
        .unwrap()
}

#[test]
fn connective_allowed_filter_picks_only_listed_pool_member() {
    let mut prefs = ConnectivePreferences::neutral();
    prefs
        .allowed
        .insert(RstRelation::Elaboration, vec!["Furthermore,".to_string()]);
    let engine = engine_for_connectives(
        StyleProfile::builder("furth")
            .connectives(prefs)
            .build()
            .unwrap(),
    );
    let out = render_same_entity_different_action(&engine);
    assert!(
        out.starts_with("Furthermore,"),
        "expected Furthermore, under restricted Elaboration pool; got {out:?}"
    );
}

#[test]
fn connective_neutral_picks_default_pool_member() {
    let engine = engine_for_connectives(StyleProfile::neutral());
    let out = render_same_entity_different_action(&engine);
    assert!(
        out.starts_with("Additionally,"),
        "expected default pool's first entry; got {out:?}"
    );
}

#[test]
fn connective_preferred_weight_biases_within_full_pool() {
    let mut prefs = ConnectivePreferences::neutral();
    // Apply heavy weight to the third pool entry. With Variation::Fixed and
    // an empty connective_history, all three candidates have equal distance
    // — preferred-weight breaks the tie toward "It also".
    prefs
        .preferred
        .insert(RstRelation::Elaboration, vec![("It also".to_string(), 1.0)]);
    let engine = engine_for_connectives(
        StyleProfile::builder("italso")
            .connectives(prefs)
            .build()
            .unwrap(),
    );
    let out = render_same_entity_different_action(&engine);
    assert!(
        out.starts_with("It also"),
        "expected It also under heavy preferred weight; got {out:?}"
    );
}

#[test]
fn connective_filter_emptying_pool_falls_back_to_base() {
    // Allowed pool that doesn't intersect the base pool at all — engine
    // must fall back to the base pool rather than emit no connective.
    let mut prefs = ConnectivePreferences::neutral();
    prefs
        .allowed
        .insert(RstRelation::Elaboration, vec!["NotInPool".to_string()]);
    let engine = engine_for_connectives(
        StyleProfile::builder("missing")
            .connectives(prefs)
            .build()
            .unwrap(),
    );
    let out = render_same_entity_different_action(&engine);
    assert!(
        out.starts_with("Additionally,"),
        "expected fallback to base pool when filter empties; got {out:?}"
    );
}

// ── LengthDistribution ──────────────────────────────────────────────────

fn engine_for_length(profile: StyleProfile) -> Engine {
    let mut engine = Engine::new(English::new())
        .variation(Variation::Seeded(42))
        .style_profile(profile);
    engine.register_template("warm", "ok").unwrap();
    engine.register_template("evt", "Short {name}").unwrap();
    engine
        .register_template(
            "evt",
            "The class {name} was significantly modified across many concerns",
        )
        .unwrap();
    engine
}

fn ctx_named(name: &str) -> Context {
    let mut c = Context::new();
    c.insert("name", Value::String(name.into()));
    c.insert("entity_type", Value::String("class".into()));
    c
}

fn render_two(engine: &Engine) -> String {
    let mut session = Session::new();
    // Warmup populates rhythm history so choose-best fires on the next call.
    engine
        .render(&mut session, "warm", ctx_named("UserService"))
        .unwrap();
    engine
        .render(&mut session, "evt", ctx_named("UserService"))
        .unwrap()
}

#[test]
fn length_distribution_short_bias_picks_shorter_variant() {
    let target = LengthDistribution {
        short: 1.0,
        medium: 0.0,
        long: 0.0,
        short_max_words: 4,
        medium_max_words: 8,
    };
    let engine = engine_for_length(
        StyleProfile::builder("short")
            .sentence_length(target)
            .build()
            .unwrap(),
    );
    let out = render_two(&engine);
    // The short variant is "Short {name}" — its body says "short UserService".
    // (Auto-discourse connectives like "Additionally," may prepend; we match
    // on body text the long variant cannot produce.)
    assert!(
        out.contains("short UserService") && !out.contains("significantly"),
        "expected short variant under short-biased target; got {out:?}"
    );
}

#[test]
fn length_distribution_long_bias_picks_longer_variant() {
    let target = LengthDistribution {
        short: 0.0,
        medium: 0.0,
        long: 1.0,
        short_max_words: 4,
        medium_max_words: 8,
    };
    let engine = engine_for_length(
        StyleProfile::builder("long")
            .sentence_length(target)
            .build()
            .unwrap(),
    );
    let out = render_two(&engine);
    assert!(
        out.contains("significantly modified across many concerns"),
        "expected long variant under long-biased target; got {out:?}"
    );
}

#[test]
fn length_distribution_neutral_does_not_override_baseline_choice() {
    let baseline = render_two(&engine_for_length(StyleProfile::neutral()));
    // Setting a neutral distribution explicitly must produce identical output.
    let explicit_neutral = render_two(&engine_for_length(
        StyleProfile::builder("neutral")
            .sentence_length(LengthDistribution::neutral())
            .build()
            .unwrap(),
    ));
    assert_eq!(baseline, explicit_neutral);
}

// ── SalienceBias ────────────────────────────────────────────────────────

#[test]
fn salience_bias_lower_promotes_count_into_higher_tier() {
    // Default thresholds: low_max=2, high_min=20. count=15 lands in Medium.
    // Under Lower bias: low_max=1, high_min=15. count=15 → High tier.
    let engine = engine_with(
        StyleProfile::builder("lower")
            .salience(SalienceBias::Lower)
            .build()
            .unwrap(),
    );
    let out = render(&engine, ctx_with_count(15));
    assert!(
        out.starts_with("VERBOSE:"),
        "expected High tier under Lower bias; got {out:?}"
    );
}

#[test]
fn salience_bias_higher_demotes_count_into_lower_tier() {
    // Default thresholds: low_max=2, high_min=20. count=20 lands in High.
    // Under Higher bias: low_max=4, high_min=30. count=20 → Medium tier.
    let engine = engine_with(
        StyleProfile::builder("higher")
            .salience(SalienceBias::Higher)
            .build()
            .unwrap(),
    );
    let out = render(&engine, ctx_with_count(20));
    assert!(
        out.starts_with("MEDIUM:"),
        "expected Medium tier under Higher bias; got {out:?}"
    );
}

#[test]
fn salience_bias_auto_preserves_default_thresholds() {
    let engine = engine_with(StyleProfile::neutral());
    // count=20 with default thresholds is High.
    let high = render(&engine, ctx_with_count(20));
    assert!(high.starts_with("VERBOSE:"));
    // count=1 with default thresholds is Low.
    let low = render(&engine, ctx_with_count(1));
    assert!(low.starts_with("TERSE:"));
}

#[test]
fn salience_bias_lower_then_verbosity_terse_compose_via_documented_order() {
    // SalienceBias runs first: Lower bias on count=15 → High tier.
    // Verbosity::Terse runs second on that High tier → shifts down to Medium.
    // Net result: Medium-tier variant, demonstrating both dials compose
    // without short-circuiting each other.
    let engine = engine_with(
        StyleProfile::builder("composed")
            .salience(SalienceBias::Lower)
            .verbosity(Verbosity::Terse)
            .build()
            .unwrap(),
    );
    let out = render(&engine, ctx_with_count(15));
    assert!(
        out.starts_with("MEDIUM:"),
        "expected SalienceBias::Lower→High then Verbosity::Terse→Medium; got {out:?}"
    );
}

#[test]
fn salience_bias_explicit_salience_string_still_overrides() {
    // Explicit salience="low" in context bypasses count thresholds entirely.
    // SalienceBias must not shadow that override path.
    let engine = engine_with(
        StyleProfile::builder("higher")
            .salience(SalienceBias::Lower) // would normally promote
            .build()
            .unwrap(),
    );
    let mut c = Context::new();
    c.insert("name", Value::String("UserService".into()));
    c.insert("entity_type", Value::String("class".into()));
    c.insert("salience", Value::String("low".into()));
    c.insert("consumer_count", Value::Number(15));
    let out = render(&engine, c);
    assert!(
        out.starts_with("TERSE:"),
        "explicit salience=low must still win; got {out:?}"
    );
}

// ── Verbosity ───────────────────────────────────────────────────────────

#[test]
fn verbosity_terse_falls_through_to_medium_when_low_tier_missing() {
    // Register only Medium and High; Terse on Medium-context shifts to Low,
    // but no Low variant exists → filter_alternatives cascade lands on
    // Medium (its first fallback). The bias must be a preference, never a
    // ProsaicError-by-misconfiguration.
    let mut engine = Engine::new(English::new())
        .variation(Variation::Fixed)
        .style_profile(
            StyleProfile::builder("terse")
                .verbosity(Verbosity::Terse)
                .build()
                .unwrap(),
        );
    engine
        .register_template_at("evt", "MEDIUM: {name}", Salience::Medium)
        .unwrap();
    engine
        .register_template_at("evt", "VERBOSE: {name}", Salience::High)
        .unwrap();

    let mut session = Session::new();
    let out = engine
        .render(&mut session, "evt", ctx_with_count(5))
        .unwrap();
    assert!(
        out.starts_with("MEDIUM:"),
        "expected Medium fallback; got {out:?}"
    );
}
