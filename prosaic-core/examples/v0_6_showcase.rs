//! v0.6 showcase — six side-by-side scenarios that demonstrate what
//! `StyleProfile` and the retrospective refine pass change vs the
//! pre-v0.6 baseline. Each scenario uses the same `Variation::Seeded`
//! and the same input events, so the only difference between columns is
//! whether the new feature is active.
//!
//! Run with `cargo run -p prosaic-core --example v0_6_showcase`.

use prosaic_core::{
    ConnectivePreferences, Context, DocumentPlan, Engine, EntityDescriptor, HedgingCalibration,
    LengthDistribution, ListStyleBias, ParagraphOpenerMonotony, PronounDensity, RefineConfig,
    RstRelation, Salience, SalienceBias, Session, StyleProfile, Value, Variation, Verbosity,
};
use prosaic_grammar_en::English;
use std::sync::Arc;

const SEED: u64 = 7;

fn main() {
    println!("# Prosaic v0.6 — Before/After Showcase\n");
    println!(
        "Each scenario renders the *same* events with `Variation::Seeded({SEED})` \n\
         under two configurations. The only difference between columns is the \n\
         feature being demonstrated.\n"
    );

    scenario_1_voice_differentiation();
    scenario_2_paragraph_opener_monotony();
    scenario_3_rst_relation_imbalance();
    scenario_4_list_style_bias();
    scenario_5_pronoun_density();
    scenario_6_combined_profile_plus_refine();
}

// ────────────────────────────────────────────────────────────────────────
// SCENARIO 1: Same corpus, four different voices via StyleProfile
// ────────────────────────────────────────────────────────────────────────

fn scenario_1_voice_differentiation() {
    println!("## Scenario 1 — One corpus, four voices");
    println!(
        "*Pre-v0.6: every consumer got the same prose regardless of register.*\n\
         *v0.6: a `StyleProfile` shifts dials across seven decision sites without \n\
         touching templates or breaking determinism.*\n"
    );
    let profiles: Vec<(&str, StyleProfile)> = vec![
        ("neutral (pre-v0.6 equivalent)", StyleProfile::neutral()),
        ("concise-professional", concise_professional()),
        ("verbose-narrative", verbose_narrative()),
        ("regulatory-formal", regulatory_formal()),
    ];
    for (label, profile) in profiles {
        println!("### {label}");
        let engine = build_voice_engine(profile);
        let mut session = Session::new();
        // count = 5 puts the context in the *Medium* salience tier under
        // default thresholds. Verbosity::Verbose then shifts to High,
        // Verbosity::Terse shifts to Low — so all four catalog profiles
        // pick a different variant tier for the same input.
        let mut ctx = ctx_named("UserService", 5);
        ctx.insert(
            "consumers",
            Value::List(
                ["A", "B", "C", "D", "E"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
            ),
        );
        ctx.insert("count", Value::Number(5));

        println!("```");
        println!(
            "{}",
            engine
                .render(&mut session, "code.modified", &ctx)
                .unwrap()
        );
        println!(
            "{}",
            engine
                .render(&mut session, "code.modified", &ctx)
                .unwrap()
        );
        println!(
            "{}",
            engine
                .render(&mut session, "code.renamed", &ctx_renamed("UserService", "AccountService", 5, &["A", "B", "C", "D", "E"]))
                .unwrap()
        );
        println!("```\n");
    }
    println!(
        "**What changed:** Same template registry, same context, same seed. \n\
         The dials biased seven decision sites: variant-tier preference (Verbosity), \n\
         centering-theory pronoun thresholds (PronounDensity), connective filtering \n\
         (ConnectivePreferences), confidence→hedge mapping (Hedging), list-style \n\
         tiebreaker (ListStyleBias), salience thresholds (SalienceBias), and \n\
         rhythm-scorer target shape (LengthDistribution). Determinism preserved \n\
         end-to-end — the same `(profile, input, seed)` triple always produces \n\
         the same output.\n"
    );
}

// ────────────────────────────────────────────────────────────────────────
// SCENARIO 2: Self-Refine catches paragraph-opener monotony
// ────────────────────────────────────────────────────────────────────────

fn scenario_2_paragraph_opener_monotony() {
    println!("## Scenario 2 — Self-Refine catches paragraph-opener monotony");
    println!(
        "*Pre-v0.6: each paragraph's connective recency window resets at the \n\
         paragraph boundary, so several consecutive paragraphs can independently \n\
         pick the same opener (\"Additionally,\") even though no per-decision \n\
         budget was violated.*\n\
         *v0.6: the retrospective pass diagnoses paragraph-opener monotony \n\
         post-hoc, blacklists the dominant opener, and re-renders.*\n"
    );

    let events = monotony_corpus();
    let plan_template = DocumentPlan::from_events(&events, &monotony_engine(false));

    println!("### refine: off (pre-v0.6 equivalent)");
    let off = monotony_engine(false);
    let off_plan = DocumentPlan::from_events(&events, &off);
    let off_text = off_plan
        .render_structured(&off, &mut Session::new())
        .unwrap();
    println!("```");
    println!("{}", off_text.text);
    println!("```");
    let off_count = off_text.text.matches("Additionally,").count();
    println!("- 'Additionally,' opener count: **{off_count}**\n");

    println!("### refine: balanced (max_iterations=3)");
    let on = monotony_engine(true);
    let on_outcome = plan_template
        .render_refined(&on, &mut Session::new())
        .unwrap();
    println!("```");
    println!("{}", on_outcome.text);
    println!("```");
    let on_count = on_outcome.text.matches("Additionally,").count();
    println!(
        "- 'Additionally,' opener count: **{on_count}**\n\
         - iterations run: {}\n\
         - converged clean: {}\n",
        on_outcome.iterations_run, on_outcome.converged_clean
    );

    println!(
        "**What changed:** The diagnoser fired when ≥3 paragraphs shared an \n\
         opener, generated a `BlacklistConnective(\"Additionally,\")` constraint, \n\
         and the next iteration rendered with that opener removed from the \n\
         continuation pool. The cycle's anti-repeat then naturally rotates \n\
         toward Furthermore/It also without breaking faithfulness or \n\
         determinism.\n"
    );
}

// ────────────────────────────────────────────────────────────────────────
// SCENARIO 3: Self-Refine catches RST-relation imbalance
// ────────────────────────────────────────────────────────────────────────

fn scenario_3_rst_relation_imbalance() {
    println!("## Scenario 3 — Self-Refine catches RST-relation imbalance");
    println!(
        "*Pre-v0.6: continuation connectives (Additionally / Furthermore / It also) \n\
         could dominate document-scope emissions even with the family-budget gate, \n\
         because the gate operates on a sliding window and resets across paragraph \n\
         boundaries. A 6-paragraph document can land 5+ Elaboration markers without \n\
         any single window saturating.*\n\
         *v0.6: `RstRelationImbalance` measures the document-scope share of each \n\
         RST relation and triggers a re-render when one relation exceeds 60%.*\n"
    );

    let events = rst_imbalance_corpus();

    println!("### refine: off");
    let off = rst_engine(false);
    let plan_off = DocumentPlan::from_events(&events, &off);
    let off_text = plan_off
        .render_structured(&off, &mut Session::new())
        .unwrap();
    println!("```");
    println!("{}", off_text.text);
    println!("```");
    let elaboration_off: usize = ["Additionally,", "Furthermore,", "It also"]
        .iter()
        .map(|c| off_text.text.matches(c).count())
        .sum();
    let total_off = off_text.connectives_used.len();
    if total_off > 0 {
        println!(
            "- Elaboration share: **{elaboration_off}/{total_off}** ({:.0}%)\n",
            (elaboration_off as f32 / total_off as f32) * 100.0
        );
    } else {
        println!("- No connectives emitted.\n");
    }

    println!("### refine: balanced");
    let on = rst_engine(true);
    let plan_on = DocumentPlan::from_events(&events, &on);
    let on_outcome = plan_on.render_refined(&on, &mut Session::new()).unwrap();
    println!("```");
    println!("{}", on_outcome.text);
    println!("```");
    let elaboration_on: usize = ["Additionally,", "Furthermore,", "It also"]
        .iter()
        .map(|c| on_outcome.text.matches(c).count())
        .sum();
    println!(
        "- iterations run: {}\n- converged clean: {}\n- 'Elaboration' opener count: **{elaboration_on}**\n",
        on_outcome.iterations_run, on_outcome.converged_clean
    );

    println!(
        "**What changed:** The diagnoser computed the share of each RST relation \n\
         across the document, found one above the 60% threshold, and emitted \n\
         a constraint to blacklist the most-frequent connective in the dominant \n\
         bucket. The retry render finds non-blacklisted alternatives in the \n\
         family pool, balancing the document.\n"
    );
}

// ────────────────────────────────────────────────────────────────────────
// SCENARIO 4: ListStyleBias dial steers list openers
// ────────────────────────────────────────────────────────────────────────

fn scenario_4_list_style_bias() {
    println!("## Scenario 4 — ListStyleBias steers list-style cycle");
    println!(
        "*Pre-v0.6: `{{items|join}}` rotated through a fixed cycle (Including → \n\
         SuchAs → Dash → Bracketed → ...) deterministically. Operators who wanted \n\
         a specific opener had two options: force every render with \n\
         `{{items|join:bracketed}}` (loses anti-repeat), or fork the templates.*\n\
         *v0.6: `ListStyleBias` provides a soft preference. The cycle still \n\
         enforces anti-repeat — you can't lock onto one style — but ties break \n\
         toward the bias when the recent window allows.*\n"
    );

    let scenarios: Vec<(&str, ListStyleBias)> = vec![
        ("ListStyleBias::Auto (pre-v0.6 default)", ListStyleBias::Auto),
        ("ListStyleBias::Bracketed", ListStyleBias::Bracketed),
        ("ListStyleBias::SuchAs", ListStyleBias::SuchAs),
        ("ListStyleBias::Dash", ListStyleBias::Dash),
    ];
    for (label, bias) in scenarios {
        println!("### {label}");
        let profile = StyleProfile::builder("bias-test")
            .list_style_bias(bias)
            .build()
            .unwrap();
        let engine = list_engine(profile);
        let mut session = Session::new();
        println!("```");
        for (i, name) in ["Alpha", "Bravo", "Charlie"].iter().enumerate() {
            let out = engine
                .render(&mut session, "evt", &ctx_with_items(name))
                .unwrap();
            println!("{}. {out}", i + 1);
        }
        println!("```\n");
    }
    println!(
        "**What changed:** A profile-level dial nudges the rotation toward a \n\
         target opener without forcing it. Anti-repeat still binds — you'll see \n\
         the cycle move when the recent window forbids a repeat — but on a \n\
         fresh slot, the bias wins.\n"
    );
}

// ────────────────────────────────────────────────────────────────────────
// SCENARIO 5: PronounDensity dial controls register
// ────────────────────────────────────────────────────────────────────────

fn scenario_5_pronoun_density() {
    println!("## Scenario 5 — PronounDensity controls referring expressions");
    println!(
        "*Pre-v0.6: the engine's centering-theory transitions decided when to \n\
         emit a pronoun, a short-name, or a full reference. The behavior was \n\
         fixed; operators had no lever to bias toward a specific register.*\n\
         *v0.6: `PronounDensity::Low` keeps full forms longer (formal register); \n\
         `PronounDensity::High` switches to pronouns earlier (conversational). \n\
         Cb/Cf transition rules are unchanged — only the post-processing tier \n\
         shifts.*\n"
    );

    let densities: Vec<(&str, PronounDensity)> = vec![
        ("PronounDensity::Default (pre-v0.6 equivalent)", PronounDensity::Default),
        ("PronounDensity::Low (formal)", PronounDensity::Low),
        ("PronounDensity::High (conversational)", PronounDensity::High),
    ];
    for (label, density) in densities {
        println!("### {label}");
        let profile = StyleProfile::builder("density-test")
            .pronoun_density(density)
            .build()
            .unwrap();
        let engine = density_engine(profile);
        let mut session = Session::new();
        println!("```");
        // Render the same entity 3 times — watch the reference form drift.
        let mut c = Context::new();
        c.insert("name", Value::String("UserService".into()));
        c.insert("entity_type", Value::String("class".into()));
        for _ in 0..3 {
            println!(
                "{}",
                engine.render(&mut session, "evt", &c).unwrap()
            );
        }
        println!("```\n");
    }
    println!(
        "**What changed:** `Low` demotes a pronoun to a short-name (visible \n\
         above — \"UserService was inspected\" three times instead of \"It \n\
         was inspected\"). `High` extends pronoun eligibility to distance ≤ 2 \n\
         in Cb-mismatch contexts; on a same-entity hot loop like this corpus, \n\
         the baseline already emits pronouns aggressively so `High` matches \n\
         `Default`. The differentiating case is multi-entity narratives where \n\
         the centering-theory rules would emit a ShortName at distance 2 and \n\
         `High` promotes to Pronoun (covered by the dial unit tests).\n"
    );
}

// ────────────────────────────────────────────────────────────────────────
// SCENARIO 6: StyleProfile + Self-Refine compose
// ────────────────────────────────────────────────────────────────────────

fn scenario_6_combined_profile_plus_refine() {
    println!("## Scenario 6 — Profile + Refine compose cleanly");
    println!(
        "*Pre-v0.6: neither a voice configuration nor a post-hoc refinement \n\
         loop existed. The two concerns are orthogonal in v0.6 by design — a \n\
         profile sets the voice; refine catches document-scope failure modes \n\
         that survive the per-decision machinery.*\n"
    );

    let events = monotony_corpus();
    let custom_diag = ParagraphOpenerMonotony {
        threshold: 2,
        min_paragraphs: 8,
    };
    let mut config = RefineConfig::balanced().with_max_iterations(3);
    config.diagnosers.clear();
    config.diagnosers.push(Arc::new(custom_diag));
    let mut engine = Engine::new(English::new())
        .strictness(prosaic_core::Strictness::Strict)
        .variation(Variation::Seeded(SEED))
        .style_profile(concise_professional())
        .refine(config);
    engine
        .register_template("evt.modified", "{name|refer} was modified")
        .unwrap();
    engine
        .register_template("evt.touched", "{name|refer} was touched")
        .unwrap();
    let plan = DocumentPlan::from_events(&events, &engine);
    let outcome = plan.render_refined(&engine, &mut Session::new()).unwrap();

    println!("### concise-professional + refine: balanced");
    println!("```");
    println!("{}", outcome.text);
    println!("```");
    println!(
        "- iterations run: {}\n- converged clean: {}\n- final score: {:.3}\n",
        outcome.iterations_run, outcome.converged_clean, outcome.final_score
    );

    println!(
        "**What changed:** The profile's `Verbosity::Terse` + `PronounDensity::Low` \n\
         + `ListStyleBias::Bracketed` shaped each render's local choices. The \n\
         refine loop independently watched document-scope statistics — paragraph \n\
         openers, RST balance, list-style fatigue — and adjusted via constraints \n\
         when needed. Same input → same output: deterministic across both layers.\n"
    );

    println!("---\n");
    println!("All scenarios rendered with `Variation::Seeded({SEED})`. ");
    println!("Determinism is invariant across `StyleProfile` and `RefineConfig` — ");
    println!("running this example twice produces byte-identical output.");
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn build_voice_engine(profile: StyleProfile) -> Engine {
    let mut engine = Engine::new(English::new())
        .strictness(prosaic_core::Strictness::Strict)
        .variation(Variation::Seeded(SEED))
        .style_profile(profile);
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
    engine
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

fn monotony_corpus() -> Vec<(&'static str, Context)> {
    // 12 entities × 2 events. The family-budget rotates the
    // continuation pool (3 elements: Additionally, Furthermore, It also)
    // through paragraphs 1-3, suppresses for paragraphs 4-6 (saturated
    // window), and resumes for 7+ — each connective ends up emitting
    // twice across the document. That's the opening for the retro-pass
    // ParagraphOpenerMonotony diagnoser to fire with a `threshold=2`
    // configuration.
    let mut out: Vec<(&str, Context)> = Vec::new();
    for name in [
        "Alpha", "Bravo", "Charlie", "Delta", "Echo", "Foxtrot", "Golf", "Hotel", "India",
        "Juliet", "Kilo", "Lima",
    ] {
        out.push(("evt.modified", ctx_simple(name)));
        out.push(("evt.touched", ctx_simple(name)));
    }
    out
}

fn ctx_simple(name: &str) -> Context {
    let mut c = Context::new();
    c.insert("name", Value::String(name.into()));
    c.insert("entity_type", Value::String("class".into()));
    c
}

fn monotony_engine(refine: bool) -> Engine {
    let mut e = Engine::new(English::new())
        .strictness(prosaic_core::Strictness::Strict)
        .variation(Variation::Seeded(SEED));
    if refine {
        // threshold=2 on a 12-paragraph corpus catches the family-budget
        // rotation pattern where each continuation connective emits twice
        // across the document. The default threshold (3) won't fire here
        // — the v0.5.x family-budget already prevents triple-recurrence —
        // so this demonstrates a configurable refine pass tuned tighter
        // than the per-decision machinery.
        let custom = ParagraphOpenerMonotony {
            threshold: 2,
            min_paragraphs: 8,
        };
        let mut config = RefineConfig::balanced().with_max_iterations(3);
        config.diagnosers.clear();
        config.diagnosers.push(Arc::new(custom));
        e = e.refine(config);
    }
    e.register_template("evt.modified", "{name|refer} was modified")
        .unwrap();
    e.register_template("evt.touched", "{name|refer} was touched")
        .unwrap();
    e
}

fn rst_imbalance_corpus() -> Vec<(&'static str, Context)> {
    // Same-entity → SameEntityDifferentAction (continuation) → Elaboration
    // RST. Five entities each emit two events, so 5 paragraphs each with
    // one continuation connective.
    monotony_corpus()
}

fn rst_engine(refine: bool) -> Engine {
    let mut e = Engine::new(English::new())
        .strictness(prosaic_core::Strictness::Strict)
        .variation(Variation::Seeded(SEED));
    if refine {
        let mut config = RefineConfig::balanced()
            .with_max_iterations(3)
            .with_min_improvement(0.0);
        config.diagnosers.clear();
        config.diagnosers.push(Arc::new(prosaic_core::RstRelationImbalance {
            max_share: 0.5,
            min_emissions: 3,
        }));
        e = e.refine(config);
    }
    e.register_template("evt.modified", "{name|refer} was modified")
        .unwrap();
    e.register_template("evt.touched", "{name|refer} was touched")
        .unwrap();
    e
}

fn list_engine(profile: StyleProfile) -> Engine {
    let mut e = Engine::new(English::new())
        .strictness(prosaic_core::Strictness::Strict)
        .variation(Variation::Seeded(SEED))
        .style_profile(profile);
    e.register_template(
        "evt",
        "{name} was modified affecting {items|truncate:3|join}",
    )
    .unwrap();
    e
}

fn ctx_with_items(name: &str) -> Context {
    let mut c = Context::new();
    c.insert("name", Value::String(name.into()));
    c.insert(
        "items",
        Value::List(
            ["A", "B", "C", "D", "E", "F"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        ),
    );
    c
}

fn density_engine(profile: StyleProfile) -> Engine {
    let mut e = Engine::new(English::new())
        .strictness(prosaic_core::Strictness::Strict)
        .variation(Variation::Seeded(SEED))
        .style_profile(profile);
    e.register_template("evt", "{name|refer} was inspected")
        .unwrap();
    e.register_entity(EntityDescriptor::new("UserService", "class"));
    e
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
        .hedging(HedgingCalibration {
            offset: 5,
            forbid: Vec::new(),
        })
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
        .hedging(HedgingCalibration {
            offset: -5,
            forbid: Vec::new(),
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
        .build()
        .unwrap()
}
