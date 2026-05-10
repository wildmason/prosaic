//! Side-by-side prose under each `StyleProfile` catalog preset.
//!
//! Renders one shared event corpus through:
//!
//! - `StyleProfile::neutral()` — the byte-equivalent baseline.
//! - Four catalog presets that each move several dials at once. The
//!   presets ship in `prosaic-project::catalog`; for the purposes of this
//!   demo (which lives in `prosaic-core` and cannot depend on
//!   `prosaic-project`), the same dial values are constructed inline so
//!   the example is self-contained.
//!
//! The same `Variation::Seeded` is used everywhere so the only difference
//! between columns is the active profile.
//!
//! Run with `cargo run -p prosaic-core --example style_profile_demo`.

use prosaic_core::{
    ConnectivePreferences, Context, Engine, EntityDescriptor, HedgingCalibration,
    LengthDistribution, ListStyleBias, PronounDensity, RstRelation, Salience, SalienceBias,
    Session, StyleProfile, Value, Variation, Verbosity,
};
use prosaic_grammar_en::English;

const SEED: u64 = 7;

fn main() {
    let profiles: Vec<(&str, StyleProfile)> = vec![
        ("neutral (baseline)", StyleProfile::neutral()),
        ("concise-professional", concise_professional()),
        ("verbose-narrative", verbose_narrative()),
        ("regulatory-formal", regulatory_formal()),
    ];

    for (label, profile) in profiles {
        println!("=== {label} ===");
        let engine = build_engine(profile);
        print!("{}", render_corpus(&engine));
        println!();
    }

    println!(
        "Notes:\n\
         - All four columns use Variation::Seeded({SEED}); the only differences\n  \
         are profile dials.\n\
         - `concise-professional` shows Verbosity::Terse + ListStyleBias::Bracketed\n  \
         + low pronoun density.\n\
         - `verbose-narrative` shows Verbosity::Verbose + ListStyleBias::Including\n  \
         + high pronoun density.\n\
         - `regulatory-formal` adds hedging.forbid for absolutes and prefers long\n  \
         sentences with bracketed enumeration."
    );
}

fn build_engine(profile: StyleProfile) -> Engine {
    let mut engine = Engine::new(English::new())
        .variation(Variation::Seeded(SEED))
        .style_profile(profile);
    register_corpus(&mut engine);
    engine
}

fn register_corpus(engine: &mut Engine) {
    engine
        .register_template_at(
            "code.modified",
            "{name|refer} was modified",
            Salience::Medium,
        )
        .unwrap();
    engine
        .register_template_at(
            "code.modified",
            "{name|refer} was modified across consumers",
            Salience::High,
        )
        .unwrap();
    engine
        .register_template_at(
            "code.renamed",
            "{old_name|refer} was renamed to {new_name}",
            Salience::Medium,
        )
        .unwrap();
    engine
        .register_template_at(
            "code.renamed",
            "{old_name|refer} was renamed to {new_name} \
             which impacts {count} {count|pluralize:consumer} \
             {consumers|truncate:3|join}",
            Salience::High,
        )
        .unwrap();
    engine
        .register_template_at(
            "code.renamed",
            "{old_name|refer} → {new_name}",
            Salience::Low,
        )
        .unwrap();
    engine.register_entity(
        EntityDescriptor::new("UserService", "class").with_attribute("layer", "domain"),
    );
    engine.register_entity(
        EntityDescriptor::new("AuthService", "class").with_attribute("layer", "infra"),
    );
    engine.register_entity(EntityDescriptor::new("OrderService", "class"));
}

fn render_corpus(engine: &Engine) -> String {
    let mut session = Session::new();
    let mut out = String::new();
    out.push_str(
        &engine
            .render(&mut session, "code.modified", &ctx_named("UserService", 25))
            .unwrap(),
    );
    out.push('\n');
    out.push_str(
        &engine
            .render(&mut session, "code.modified", &ctx_named("UserService", 25))
            .unwrap(),
    );
    out.push('\n');
    out.push_str(
        &engine
            .render(
                &mut session,
                "code.renamed",
                &ctx_renamed("UserService", "AccountService", 25, &["A", "B", "C", "D", "E"]),
            )
            .unwrap(),
    );
    out.push('\n');
    out.push_str(
        &engine
            .render(&mut session, "code.modified", &ctx_named("AuthService", 5))
            .unwrap(),
    );
    out.push('\n');
    out.push_str(
        &engine
            .render(&mut session, "code.modified", &ctx_named("OrderService", 5))
            .unwrap(),
    );
    out.push('\n');
    out
}

fn ctx_named(name: &str, count: i64) -> Context {
    let mut c = Context::new();
    c.insert("name", Value::String(name.into()));
    c.insert("entity_type", Value::String("class".into()));
    c.insert("consumer_count", Value::Number(count));
    c
}

fn ctx_renamed(old: &str, new: &str, count: i64, consumers: &[&str]) -> Context {
    let mut c = Context::new();
    c.insert("old_name", Value::String(old.into()));
    c.insert("new_name", Value::String(new.into()));
    c.insert("entity_type", Value::String("class".into()));
    c.insert("consumer_count", Value::Number(count));
    c.insert("count", Value::Number(consumers.len() as i64));
    c.insert(
        "consumers",
        Value::List(consumers.iter().map(|s| s.to_string()).collect()),
    );
    c
}

fn concise_professional() -> StyleProfile {
    let mut connectives = ConnectivePreferences::neutral();
    connectives.allowed.insert(
        RstRelation::Elaboration,
        vec!["Furthermore,".to_string(), "Additionally,".to_string()],
    );
    connectives
        .allowed
        .insert(RstRelation::Contrast, vec!["However,".to_string()]);
    StyleProfile::builder("concise-professional")
        .verbosity(Verbosity::Terse)
        .list_style_bias(ListStyleBias::Bracketed)
        .pronoun_density(PronounDensity::Low)
        .salience(SalienceBias::Auto)
        .hedging(HedgingCalibration { offset: 5, forbid: Vec::new() })
        .sentence_length(LengthDistribution {
            short: 0.5,
            medium: 0.4,
            long: 0.1,
            short_max_words: 8,
            medium_max_words: 18,
        })
        .connectives(connectives)
        .build()
        .unwrap()
}

fn verbose_narrative() -> StyleProfile {
    StyleProfile::builder("verbose-narrative")
        .verbosity(Verbosity::Verbose)
        .list_style_bias(ListStyleBias::Including)
        .pronoun_density(PronounDensity::High)
        .salience(SalienceBias::Auto)
        .hedging(HedgingCalibration { offset: -5, forbid: Vec::new() })
        .sentence_length(LengthDistribution {
            short: 0.2,
            medium: 0.5,
            long: 0.3,
            short_max_words: 8,
            medium_max_words: 18,
        })
        .build()
        .unwrap()
}

fn regulatory_formal() -> StyleProfile {
    StyleProfile::builder("regulatory-formal")
        .verbosity(Verbosity::Verbose)
        .list_style_bias(ListStyleBias::Bracketed)
        .pronoun_density(PronounDensity::Low)
        .salience(SalienceBias::Auto)
        .hedging(HedgingCalibration {
            offset: -10,
            forbid: vec!["certainly".to_string(), "must".to_string()],
        })
        .sentence_length(LengthDistribution {
            short: 0.1,
            medium: 0.5,
            long: 0.4,
            short_max_words: 8,
            medium_max_words: 18,
        })
        .build()
        .unwrap()
}
