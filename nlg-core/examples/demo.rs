use nlg_core::{
    Strictness, Clause, Context, Engine, Sentence, Tense, Value, Variation, Voice, entity, named,
};
use nlg_derive::IntoContext;
use nlg_grammar_en::English;

fn main() {
    println!("=== nlg crate demo ===\n");

    referring_expression_demo();
    discourse_aware_demos();
    batch_rendering_demo();
    template_api_demos();
    builder_api_demos();
    vocab_code_demos();
    variation_demos();
    strictness_demos();
    derive_macro_demo();
    grammar_showcase();
}

fn header(title: &str) {
    println!("── {title} ──\n");
}

fn show(label: &str, result: &str) {
    println!("  {label}:");
    println!("    \"{result}\"\n");
}

// ── Referring Expressions ─────────────────────────────────────────────────

fn referring_expression_demo() {
    header("Referring Expressions (Entity Tracking)");

    let mut engine = Engine::new(English::new())
        .strictness(Strictness::Strict)
        .variation(Variation::Fixed);
    nlg_vocab_code::register(&mut engine).unwrap();

    println!("  Four renders about the SAME entity (UserService):");
    println!("  Watch how the engine refers to it differently each time.\n");

    let mut ctx = Context::new();
    ctx.insert("entity_type", Value::String("class".into()));
    ctx.insert("old_name", Value::String("UserService".into()));
    ctx.insert("name", Value::String("UserService".into()));
    ctx.insert("new_name", Value::String("AccountService".into()));
    ctx.insert("consumer_count", Value::Number(3));
    ctx.insert("consumers", Value::List(vec![
        "ProfilePage".into(), "SettingsPage".into(), "AuthModule".into(),
    ]));

    let r1 = engine.render("code.renamed", &ctx).unwrap();
    println!("    1. \"{r1}\"");

    // Update context to use new name going forward (though for demo we keep same entity)
    ctx.insert("name", Value::String("UserService".into()));
    let r2 = engine.render("code.modified", &ctx).unwrap();
    println!("    2. \"{r2}\"");

    let r3 = engine.render("code.modified", &ctx).unwrap();
    println!("    3. \"{r3}\"");

    // Introduce a different entity — should break pronoun chain
    let mut other_ctx = Context::new();
    other_ctx.insert("entity_type", Value::String("service".into()));
    other_ctx.insert("name", Value::String("AuthGuard".into()));
    other_ctx.insert("location", Value::String("src/guards/".into()));
    let r4 = engine.render("code.added", &other_ctx).unwrap();
    println!("    4. \"{r4}\"");

    // Back to UserService — now there's ambiguity, so no pronoun
    let r5 = engine.render("code.modified", &ctx).unwrap();
    println!("    5. \"{r5}\"");

    println!();
    println!("  1st mention: full form \"The class UserService\"");
    println!("  2nd mention (same focus): pronoun \"it\"");
    println!("  3rd mention (continued focus): pronoun \"it\"");
    println!("  4th: different entity introduced (full form)");
    println!("  5th: back to UserService but ambiguity prevents pronoun\n");
}

// ── Discourse-Aware Rendering ─────────────────────────────────────────────

fn discourse_aware_demos() {
    header("Discourse-Aware Sequential Rendering");

    let mut engine = Engine::new(English::new())
        .strictness(Strictness::Strict)
        .variation(Variation::Fixed);
    nlg_vocab_code::register(&mut engine).unwrap();

    println!("  Rendering 5 related events sequentially (no reset between them):\n");

    // Event 1: rename
    let mut ctx = Context::new();
    ctx.insert("entity_type", Value::String("class".into()));
    ctx.insert("old_name", Value::String("UserService".into()));
    ctx.insert("new_name", Value::String("AccountService".into()));
    ctx.insert("consumer_count", Value::Number(6));
    ctx.insert("consumers", Value::List(vec![
        "ProfileComponent".into(), "SettingsComponent".into(), "AdminModule".into(),
        "AuthGuard".into(), "DashboardWidget".into(), "NotificationService".into(),
    ]));
    let r1 = engine.render("code.renamed", &ctx).unwrap();
    println!("    1. \"{r1}\"");

    // Event 2: delete (different entity, triggers connective)
    let mut ctx = Context::new();
    ctx.insert("entity_type", Value::String("interface".into()));
    ctx.insert("name", Value::String("LegacyConfig".into()));
    ctx.insert("consumer_count", Value::Number(3));
    ctx.insert("consumers", Value::List(vec![
        "ConfigLoader".into(), "BootstrapModule".into(), "MigrationScript".into(),
    ]));
    let r2 = engine.render("code.deleted", &ctx).unwrap();
    println!("    2. \"{r2}\"");

    // Event 3: modify (different entity, different action)
    let mut ctx = Context::new();
    ctx.insert("entity_type", Value::String("method".into()));
    ctx.insert("name", Value::String("calculateTotal".into()));
    ctx.insert("consumer_count", Value::Number(4));
    ctx.insert("consumers", Value::List(vec![
        "CartComponent".into(), "CheckoutFlow".into(),
        "InvoiceService".into(), "PricingEngine".into(),
    ]));
    let r3 = engine.render("code.modified", &ctx).unwrap();
    println!("    3. \"{r3}\"");

    // Event 4: another rename (same action as #1, triggers "Similarly")
    let mut ctx = Context::new();
    ctx.insert("entity_type", Value::String("class".into()));
    ctx.insert("old_name", Value::String("AuthService".into()));
    ctx.insert("new_name", Value::String("IdentityService".into()));
    ctx.insert("consumer_count", Value::Number(2));
    ctx.insert("consumers", Value::List(vec![
        "LoginPage".into(), "TokenManager".into(),
    ]));
    let r4 = engine.render("code.renamed", &ctx).unwrap();
    println!("    4. \"{r4}\"");

    // Event 5: add (new entity)
    let mut ctx = Context::new();
    ctx.insert("entity_type", Value::String("service".into()));
    ctx.insert("name", Value::String("TelemetryService".into()));
    ctx.insert("location", Value::String("src/services/telemetry.service.ts".into()));
    let r5 = engine.render("code.added", &ctx).unwrap();
    println!("    5. \"{r5}\"");

    println!();
    println!("  Note: list styles cycle (including/such as/dash/bracketed),");
    println!("  template variants anti-repeat, and discourse connectives");
    println!("  link related events.\n");
}

fn batch_rendering_demo() {
    header("Batch Rendering with Aggregation");

    let mut engine = Engine::new(English::new())
        .strictness(Strictness::Strict)
        .variation(Variation::Fixed);
    nlg_vocab_code::register(&mut engine).unwrap();

    // Events about the same entity (should aggregate)
    let mut ctx1 = Context::new();
    ctx1.insert("entity_type", Value::String("class".into()));
    ctx1.insert("old_name", Value::String("DataManager".into()));
    ctx1.insert("new_name", Value::String("DataService".into()));
    ctx1.insert("consumer_count", Value::Number(3));
    ctx1.insert("consumers", Value::List(vec![
        "Dashboard".into(), "Reports".into(), "Export".into(),
    ]));

    let mut ctx2 = Context::new();
    ctx2.insert("entity_type", Value::String("class".into()));
    ctx2.insert("name", Value::String("DataManager".into()));
    ctx2.insert("old_location", Value::String("src/utils/".into()));
    ctx2.insert("new_location", Value::String("src/core/".into()));
    ctx2.insert("consumer_count", Value::Number(3));
    ctx2.insert("consumers", Value::List(vec![
        "Dashboard".into(), "Reports".into(), "Export".into(),
    ]));

    // Unrelated entity
    let mut ctx3 = Context::new();
    ctx3.insert("entity_type", Value::String("service".into()));
    ctx3.insert("name", Value::String("AuthGuard".into()));
    ctx3.insert("location", Value::String("src/guards/auth.guard.ts".into()));

    let events: Vec<(&str, Context)> = vec![
        ("code.renamed", ctx1),
        ("code.moved", ctx2),
        ("code.added", ctx3),
    ];

    let paragraph = engine.render_batch(&events).unwrap();
    println!("  Batch output (3 events, first 2 share an entity):\n");
    println!("    \"{paragraph}\"\n");

    // Same-action aggregation: 3 renames of different classes
    let mut engine2 = Engine::new(English::new()).variation(Variation::Fixed);
    nlg_vocab_code::register(&mut engine2).unwrap();

    let make_rename = |old: &str, new: &str| {
        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("class".into()));
        ctx.insert("old_name", Value::String(old.into()));
        ctx.insert("new_name", Value::String(new.into()));
        ctx.insert("consumer_count", Value::Number(1));
        ctx.insert("consumers", Value::List(vec!["App".into()]));
        ctx
    };

    let events: Vec<(&str, Context)> = vec![
        ("code.renamed", make_rename("UserService", "AccountService")),
        ("code.renamed", make_rename("AuthService", "IdentityService")),
        ("code.renamed", make_rename("DataService", "StorageService")),
    ];

    let aggregated = engine2.render_batch(&events).unwrap();
    println!("  Same action with differing details (falls back to sequential):\n");
    println!("    \"{aggregated}\"\n");

    // True aggregation: same action, same secondary context (deletions in same module)
    let mut engine3 = Engine::new(English::new()).variation(Variation::Fixed);
    nlg_vocab_code::register(&mut engine3).unwrap();

    let make_delete = |name: &str| {
        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("class".into()));
        ctx.insert("name", Value::String(name.into()));
        ctx.insert("consumer_count", Value::Number(0));
        ctx.insert("consumers", Value::List(vec![]));
        ctx
    };

    let events: Vec<(&str, Context)> = vec![
        ("code.deleted", make_delete("OldUtils")),
        ("code.deleted", make_delete("LegacyHelpers")),
        ("code.deleted", make_delete("DeprecatedShims")),
    ];

    let aggregated = engine3.render_batch(&events).unwrap();
    println!("  True aggregation (3 deletions with matching context):\n");
    println!("    \"{aggregated}\"\n");
}

// ── Template API ─────────────────────────────────────────────────────────

fn template_api_demos() {
    header("Template API");

    let mut engine = Engine::new(English::new())
        .strictness(Strictness::Strict)
        .variation(Variation::Fixed);

    engine
        .register_template(
            "renamed",
            "The {entity_type} {old_name} was renamed to {new_name} \
             which impacts {count} direct {count|pluralize:consumer} \
             [{consumers|truncate:3|join}]",
        )
        .unwrap();

    // 6 consumers, truncated
    let mut ctx = Context::new();
    ctx.insert("entity_type", Value::String("class".into()));
    ctx.insert("old_name", Value::String("UserService".into()));
    ctx.insert("new_name", Value::String("AccountService".into()));
    ctx.insert("count", Value::Number(6));
    ctx.insert(
        "consumers",
        Value::List(vec![
            "ProfileComponent".into(),
            "SettingsComponent".into(),
            "AdminModule".into(),
            "AuthGuard".into(),
            "DashboardWidget".into(),
            "NotificationService".into(),
        ]),
    );
    show("Rename (6 consumers)", &engine.render("renamed", &ctx).unwrap());

    // 1 consumer
    ctx.insert("entity_type", Value::String("method".into()));
    ctx.insert("old_name", Value::String("getData".into()));
    ctx.insert("new_name", Value::String("fetchData".into()));
    ctx.insert("count", Value::Number(1));
    ctx.insert("consumers", Value::List(vec!["DashboardComponent".into()]));
    show("Rename (1 consumer)", &engine.render("renamed", &ctx).unwrap());

    // 2 consumers (no truncation, Oxford comma not needed)
    ctx.insert("entity_type", Value::String("interface".into()));
    ctx.insert("old_name", Value::String("ILogger".into()));
    ctx.insert("new_name", Value::String("Logger".into()));
    ctx.insert("count", Value::Number(2));
    ctx.insert(
        "consumers",
        Value::List(vec!["AppModule".into(), "TestHarness".into()]),
    );
    show("Rename (2 consumers)", &engine.render("renamed", &ctx).unwrap());

    // Inline template
    let mut ctx = Context::new();
    ctx.insert("n", Value::Number(42));
    ctx.insert("thing", Value::String("error".into()));
    show(
        "Inline template",
        &engine
            .render_inline(
                "Found {n} {n|pluralize:occurrence} of {thing|article} in the codebase",
                &ctx,
            )
            .unwrap(),
    );

    // Pipes: ordinal, words
    let mut ctx = Context::new();
    ctx.insert("n", Value::Number(3));
    ctx.insert("total", Value::Number(1_234));
    show(
        "Ordinal + words",
        &engine
            .render_inline(
                "This is the {n|ordinal} time this has happened, \
                 totalling {total|words} critical issue reports",
                &ctx,
            )
            .unwrap(),
    );

    // Capitalize — useful when a value might start a sentence
    let mut ctx = Context::new();
    ctx.insert("event", Value::String("authentication failure".into()));
    ctx.insert("count", Value::Number(7));
    show(
        "Capitalize (sentence start)",
        &engine
            .render_inline(
                "{event|capitalize} detected {count} {count|pluralize:time} today",
                &ctx,
            )
            .unwrap(),
    );
}

// ── Builder API ──────────────────────────────────────────────────────────

fn builder_api_demos() {
    header("Builder API");

    let engine = Engine::new(English::new());

    // Full sentence with clause (passive voice — default)
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
    show("Passive rename", &result);

    // Same sentence in active voice
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
    show("Active rename", &result);

    // Simple deletion
    let result = Sentence::new()
        .subject(entity("method", "processPayment"))
        .verb("remove", Tense::Past)
        .clause(
            Clause::with_intro("from")
                .noun("OrderService"),
        )
        .render(&engine)
        .unwrap();
    show("Deletion", &result);

    // Future tense (passive)
    let result = Sentence::new()
        .subject(entity("interface", "UserProfile"))
        .verb("deprecate", Tense::Future)
        .render(&engine)
        .unwrap();
    show("Future passive", &result);

    // Future tense (active)
    let result = Sentence::new()
        .subject(entity("interface", "UserProfile"))
        .verb("break", Tense::Future)
        .voice(Voice::Active)
        .clause(
            Clause::with_intro("in")
                .amount(5)
                .noun("downstream module"),
        )
        .render(&engine)
        .unwrap();
    show("Future active", &result);

    // Named subject (no entity type)
    let result = Sentence::new()
        .subject(named("AuthGuard"))
        .verb("modify", Tense::Past)
        .clause(
            Clause::which("affects")
                .amount(3)
                .noun("route")
                .list(&["Dashboard", "Settings", "Admin"]),
        )
        .render(&engine)
        .unwrap();
    show("Named subject", &result);

    // Custom preposition
    let result = Sentence::new()
        .subject(entity("class", "OldParser"))
        .verb("replace", Tense::Past)
        .preposition("with")
        .object("NewParser")
        .render(&engine)
        .unwrap();
    show("Custom preposition", &result);

    // Active present tense
    let result = Sentence::new()
        .subject(entity("module", "SharedModule"))
        .verb("export", Tense::Present)
        .voice(Voice::Active)
        .clause(
            Clause::with_intro("")
                .amount(12)
                .noun("component"),
        )
        .render(&engine)
        .unwrap();
    show("Present tense (3rd person)", &result);
}

// ── Vocab Code module ────────────────────────────────────────────────────

fn vocab_code_demos() {
    header("Code Vocabulary Module");

    let mut engine = Engine::new(English::new())
        .strictness(Strictness::Strict)
        .variation(Variation::Fixed);
    nlg_vocab_code::register(&mut engine).unwrap();

    // code.renamed
    let mut ctx = Context::new();
    ctx.insert("entity_type", Value::String("class".into()));
    ctx.insert("old_name", Value::String("HttpClient".into()));
    ctx.insert("new_name", Value::String("ApiClient".into()));
    ctx.insert("consumer_count", Value::Number(8));
    ctx.insert(
        "consumers",
        Value::List(vec![
            "UserService".into(), "OrderService".into(), "AuthService".into(),
            "PaymentService".into(), "NotificationService".into(),
            "ReportService".into(), "CacheService".into(), "LogService".into(),
        ]),
    );
    show("code.renamed", &engine.render("code.renamed", &ctx).unwrap());

    // code.deleted
    let mut ctx = Context::new();
    ctx.insert("entity_type", Value::String("interface".into()));
    ctx.insert("name", Value::String("LegacyUserProfile".into()));
    ctx.insert("consumer_count", Value::Number(3));
    ctx.insert(
        "consumers",
        Value::List(vec![
            "ProfilePage".into(), "SettingsPage".into(), "AdminPanel".into(),
        ]),
    );
    show("code.deleted", &engine.render("code.deleted", &ctx).unwrap());

    // code.added
    let mut ctx = Context::new();
    ctx.insert("entity_type", Value::String("service".into()));
    ctx.insert("name", Value::String("TelemetryService".into()));
    ctx.insert("location", Value::String("src/services/telemetry.service.ts".into()));
    show("code.added", &engine.render("code.added", &ctx).unwrap());

    // code.modified
    let mut ctx = Context::new();
    ctx.insert("entity_type", Value::String("method".into()));
    ctx.insert("name", Value::String("calculateTotal".into()));
    ctx.insert("consumer_count", Value::Number(7));
    ctx.insert(
        "consumers",
        Value::List(vec![
            "CartComponent".into(), "CheckoutComponent".into(),
            "InvoiceService".into(), "ReportGenerator".into(),
            "PricingEngine".into(), "TaxCalculator".into(),
            "DiscountService".into(),
        ]),
    );
    show("code.modified", &engine.render("code.modified", &ctx).unwrap());

    // code.moved
    let mut ctx = Context::new();
    ctx.insert("entity_type", Value::String("class".into()));
    ctx.insert("name", Value::String("DateUtils".into()));
    ctx.insert("old_location", Value::String("src/utils/date-utils.ts".into()));
    ctx.insert("new_location", Value::String("src/core/date-utils.ts".into()));
    ctx.insert("consumer_count", Value::Number(15));
    ctx.insert(
        "consumers",
        Value::List(vec![
            "AppModule".into(), "SchedulerService".into(),
            "CalendarComponent".into(), "ReportService".into(),
        ]),
    );
    show("code.moved", &engine.render("code.moved", &ctx).unwrap());

    // code.signature_changed
    let mut ctx = Context::new();
    ctx.insert("entity_type", Value::String("method".into()));
    ctx.insert("name", Value::String("getUser".into()));
    ctx.insert("consumer_count", Value::Number(1));
    ctx.insert("consumers", Value::List(vec!["ProfileController".into()]));
    show(
        "code.signature_changed (singular)",
        &engine.render("code.signature_changed", &ctx).unwrap(),
    );
}

// ── Variation ────────────────────────────────────────────────────────────

fn variation_demos() {
    header("Variation Strategies");

    let ctx = {
        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("class".into()));
        ctx.insert("old_name", Value::String("Foo".into()));
        ctx.insert("new_name", Value::String("Bar".into()));
        ctx.insert("consumer_count", Value::Number(2));
        ctx.insert("consumers", Value::List(vec!["A".into(), "B".into()]));
        ctx
    };

    // Fixed
    let mut engine = Engine::new(English::new()).variation(Variation::Fixed);
    nlg_vocab_code::register(&mut engine).unwrap();
    show("Fixed", &engine.render("code.renamed", &ctx).unwrap());

    // Seeded (different seeds)
    for seed in [1, 42, 999] {
        let mut engine = Engine::new(English::new()).variation(Variation::Seeded(seed));
        nlg_vocab_code::register(&mut engine).unwrap();
        show(
            &format!("Seeded({seed})"),
            &engine.render("code.renamed", &ctx).unwrap(),
        );
    }
}

// ── Strictness ───────────────────────────────────────────────────────────

fn strictness_demos() {
    header("Strictness Modes");

    let template = "The {entity_type} {name} was modified by {author}";
    let mut ctx = Context::new();
    ctx.insert("entity_type", Value::String("class".into()));
    ctx.insert("name", Value::String("Foo".into()));
    // "author" intentionally missing

    // Strict
    let mut engine = Engine::new(English::new()).strictness(Strictness::Strict);
    engine.register_template("t", template).unwrap();
    match engine.render("t", &ctx) {
        Ok(s) => show("Strict", &s),
        Err(e) => show("Strict (error)", &e.to_string()),
    }

    // Lenient
    let mut engine = Engine::new(English::new()).strictness(Strictness::Lenient);
    engine.register_template("t", template).unwrap();
    show("Lenient", &engine.render("t", &ctx).unwrap());

    // Silent
    let mut engine = Engine::new(English::new()).strictness(Strictness::Silent);
    engine.register_template("t", template).unwrap();
    show("Silent", &engine.render("t", &ctx).unwrap());
}

// ── Derive macro ─────────────────────────────────────────────────────────

#[derive(IntoContext)]
struct DeployEvent {
    entity_type: String,
    name: String,
    environment: String,
    consumer_count: i64,
    consumers: Vec<String>,
}

fn derive_macro_demo() {
    header("Derive Macro");

    let mut engine = Engine::new(English::new());
    engine
        .register_template(
            "deploy",
            "The {entity_type} {name} was deployed to {environment}, \
             affecting {consumer_count} {consumer_count|pluralize:service} \
             [{consumers|join}]",
        )
        .unwrap();

    let event = DeployEvent {
        entity_type: "microservice".into(),
        name: "PaymentGateway".into(),
        environment: "production".into(),
        consumer_count: 3,
        consumers: vec!["OrderService".into(), "BillingService".into(), "RefundService".into()],
    };

    show("From struct", &engine.render("deploy", event).unwrap());
}

// ── Grammar showcase ─────────────────────────────────────────────────────

fn grammar_showcase() {
    header("Grammar Showcase");

    let engine = Engine::new(English::new());

    // Pluralization edge cases
    let cases = [
        ("child", 1), ("child", 3),
        ("person", 1), ("person", 5),
        ("analysis", 2),
        ("sheep", 10),
        ("city", 4),
        ("knife", 2),
        ("hero", 3),
        ("software", 99),
    ];

    println!("  Pluralization:");
    for (word, count) in cases {
        let mut ctx = Context::new();
        ctx.insert("n", Value::Number(count));
        let result = engine
            .render_inline(&format!("{{n}} {{n|pluralize:{word}}}"), &ctx)
            .unwrap();
        println!("    {result}");
    }
    println!();

    // Articles
    let words = ["apple", "banana", "hour", "university", "FBI", "XML", "elephant", "user"];
    println!("  Articles:");
    for word in words {
        let mut ctx = Context::new();
        ctx.insert("w", Value::String(word.into()));
        let result = engine.render_inline("{w|article}", &ctx).unwrap();
        println!("    {result}");
    }
    println!();

    // Numbers to words
    let numbers = [0, 1, 12, 42, 100, 999, 1_234, 1_000_000];
    println!("  Numbers to words:");
    for n in numbers {
        let mut ctx = Context::new();
        ctx.insert("n", Value::Number(n));
        let result = engine.render_inline("{n|words}", &ctx).unwrap();
        println!("    {n} -> {result}");
    }
    println!();

    // Ordinals
    let ordinals = [1, 2, 3, 4, 11, 12, 13, 21, 22, 23, 100, 101];
    println!("  Ordinals:");
    for n in ordinals {
        let mut ctx = Context::new();
        ctx.insert("n", Value::Number(n));
        let result = engine.render_inline("{n|ordinal}", &ctx).unwrap();
        print!("    {result}");
    }
    println!("\n");

    // List joining
    println!("  List joining:");
    for items in [
        vec![],
        vec!["alpha".to_string()],
        vec!["alpha".into(), "beta".into()],
        vec!["alpha".into(), "beta".into(), "gamma".into()],
        vec!["alpha".into(), "beta".into(), "gamma".into(), "delta".into(), "epsilon".into()],
    ] {
        let mut ctx = Context::new();
        ctx.insert("items", Value::List(items.clone()));
        let result = engine.render_inline("{items|join}", &ctx).unwrap();
        println!(
            "    {} item(s): \"{}\"",
            items.len(),
            if result.is_empty() { "(empty)" } else { &result }
        );
    }
    println!();
}
