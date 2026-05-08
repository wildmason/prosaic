//! Before/after prose evidence for the sentence-rhythm variance pass.
//!
//! Renders 10 deterministic narratives with `Engine::sentence_rhythm`
//! disabled (BEFORE) and enabled (AFTER) using the same seed and the
//! same input events, so the only difference between the two columns is
//! whether the cadence penalty layered on top of choose-best fires.
//!
//! Run with `cargo run -p prosaic-core --example sentence_rhythm_demo`.

use prosaic_core::{Context, Engine, Session, Strictness, Value, Variation};
use prosaic_grammar_en::English;

const SEED: u64 = 17;
const MAX_SENTENCE_LENGTH: usize = 140;

/// Each fixture is a self-contained narrative — a sequence of `(template_key,
/// entity_name)` events rendered as one connected paragraph. The same fixtures
/// are rendered with `sentence_rhythm(false)` and `sentence_rhythm(true)`.
struct Fixture {
    title: &'static str,
    template_key: &'static str,
    entity_type: &'static str,
    entities: &'static [&'static str],
}

const FIXTURES: &[Fixture] = &[
    Fixture {
        title: "1. Service rename batch",
        template_key: "code.touched",
        entity_type: "service",
        entities: &[
            "BillingService",
            "PaymentGateway",
            "RefundProcessor",
            "ReceiptDispatcher",
            "InvoiceLedger",
            "AuditTrail",
        ],
    },
    Fixture {
        title: "2. Component rewrites",
        template_key: "code.touched",
        entity_type: "component",
        entities: &[
            "LoginPanel",
            "SessionDrawer",
            "ProfileCard",
            "SettingsModal",
            "ToastDispatcher",
            "BreadcrumbTrail",
        ],
    },
    Fixture {
        title: "3. Module migrations",
        template_key: "code.touched",
        entity_type: "module",
        entities: &[
            "AccountsCore",
            "AccountsApi",
            "AccountsCli",
            "AccountsDocs",
            "AccountsFixtures",
            "AccountsBench",
        ],
    },
    Fixture {
        title: "4. Class refactors",
        template_key: "code.touched",
        entity_type: "class",
        entities: &[
            "OrderCart",
            "OrderHistory",
            "OrderInvoice",
            "OrderRefund",
            "OrderDispatcher",
            "OrderArchive",
        ],
    },
    Fixture {
        title: "5. Function tweaks",
        template_key: "code.touched",
        entity_type: "function",
        entities: &[
            "loadConfig",
            "parseConfig",
            "validateConfig",
            "mergeConfig",
            "writeConfig",
            "rotateConfig",
        ],
    },
    Fixture {
        title: "6. Endpoint reorgs",
        template_key: "code.touched",
        entity_type: "endpoint",
        entities: &[
            "GetOrders",
            "PostOrder",
            "PatchOrder",
            "DeleteOrder",
            "ListOrderItems",
            "PostOrderItem",
        ],
    },
    Fixture {
        title: "7. Pipeline stages",
        template_key: "code.touched",
        entity_type: "stage",
        entities: &[
            "FetchStage",
            "ParseStage",
            "ValidateStage",
            "TransformStage",
            "PublishStage",
            "ArchiveStage",
        ],
    },
    Fixture {
        title: "8. Workflow audit",
        template_key: "code.touched",
        entity_type: "workflow",
        entities: &[
            "OnboardingFlow",
            "OffboardingFlow",
            "PromotionFlow",
            "DemotionFlow",
            "ResetFlow",
            "ImpersonationFlow",
        ],
    },
    Fixture {
        title: "9. Job restructure",
        template_key: "code.touched",
        entity_type: "job",
        entities: &[
            "NightlyClean",
            "HourlyMetrics",
            "DailyDigest",
            "WeeklyReport",
            "MonthlyClose",
            "AnnualAudit",
        ],
    },
    Fixture {
        title: "10. Page rewrites",
        template_key: "code.touched",
        entity_type: "page",
        entities: &[
            "DashboardPage",
            "BillingPage",
            "ProfilePage",
            "SettingsPage",
            "AdminPage",
            "AuditPage",
        ],
    },
];

fn build_engine(rhythm_enabled: bool) -> Engine {
    let mut engine = Engine::new(English::new())
        .strictness(Strictness::Strict)
        .variation(Variation::Seeded(SEED))
        .max_sentence_length(MAX_SENTENCE_LENGTH)
        .sentence_rhythm(rhythm_enabled);

    // Three template variants: short / medium / long. Choose-best with the
    // rhythm penalty has live cadence options on every render.
    engine
        .register_template("code.touched", "The {entity_type} {name} was touched")
        .unwrap();
    engine
        .register_template(
            "code.touched",
            "The {entity_type} {name} was touched and revalidated against the current schema",
        )
        .unwrap();
    engine
        .register_template(
            "code.touched",
            "The {entity_type} {name} was touched after the routine sweep, \
             revalidated against the current schema, and recorded in the engineering ledger",
        )
        .unwrap();

    engine
}

fn render_fixture(engine: &Engine, fixture: &Fixture) -> Vec<String> {
    let mut session = Session::new();
    fixture
        .entities
        .iter()
        .map(|name| {
            let mut ctx = Context::new();
            ctx.insert("entity_type", Value::String(fixture.entity_type.into()));
            ctx.insert("name", Value::String((*name).into()));
            engine.render(&mut session, fixture.template_key, &ctx).unwrap()
        })
        .collect()
}

fn word_count(sentence: &str) -> usize {
    sentence
        .split_whitespace()
        .filter(|word| word.chars().any(|c| c.is_alphanumeric()))
        .count()
}

fn stdev(lengths: &[usize]) -> f64 {
    let mean = lengths.iter().sum::<usize>() as f64 / lengths.len() as f64;
    let variance = lengths
        .iter()
        .map(|len| {
            let delta = *len as f64 - mean;
            delta * delta
        })
        .sum::<f64>()
        / lengths.len() as f64;
    variance.sqrt()
}

fn print_pair(fixture: &Fixture, before: &[String], after: &[String]) {
    let before_lengths: Vec<usize> = before.iter().map(|s| word_count(s)).collect();
    let after_lengths: Vec<usize> = after.iter().map(|s| word_count(s)).collect();

    println!("--- {} ---", fixture.title);
    println!(
        "BEFORE (sentence_rhythm=false, lengths {before_lengths:?}, stdev {:.2}):",
        stdev(&before_lengths)
    );
    println!("    {}", before.join(" "));
    println!(
        "AFTER  (sentence_rhythm=true,  lengths {after_lengths:?}, stdev {:.2}):",
        stdev(&after_lengths)
    );
    println!("    {}", after.join(" "));
    println!();
}

fn main() {
    println!(
        "Sentence-rhythm variance — 10 before/after narratives (seed {SEED}, max sentence {MAX_SENTENCE_LENGTH}).\n"
    );

    let engine_off = build_engine(false);
    let engine_on = build_engine(true);

    let mut total_before_lengths: Vec<usize> = Vec::new();
    let mut total_after_lengths: Vec<usize> = Vec::new();

    for fixture in FIXTURES {
        let before = render_fixture(&engine_off, fixture);
        let after = render_fixture(&engine_on, fixture);
        total_before_lengths.extend(before.iter().map(|s| word_count(s)));
        total_after_lengths.extend(after.iter().map(|s| word_count(s)));
        print_pair(fixture, &before, &after);
    }

    println!("=== Aggregate cadence stdev across all 10 narratives ===");
    println!(
        "BEFORE (rhythm off): stdev {:.3} over {} sentences",
        stdev(&total_before_lengths),
        total_before_lengths.len()
    );
    println!(
        "AFTER  (rhythm on) : stdev {:.3} over {} sentences",
        stdev(&total_after_lengths),
        total_after_lengths.len()
    );
}
