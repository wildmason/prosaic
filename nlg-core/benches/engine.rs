//! Engine benchmarks. Run with:
//!
//! ```text
//! cargo bench --package nlg-core --bench engine
//! ```
//!
//! These are intentionally small, deterministic benchmarks that cover
//! the hot paths: single render, batch with aggregation, REG with a
//! large registry, and document-plan rendering. Use as a baseline
//! against which to compare future perf changes.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use nlg_core::{
    Context, DocumentPlan, Engine, EntityDescriptor, GroupingStrategy, Session, Strictness, Value,
    Variation,
};
use nlg_grammar_en::English;

fn build_engine() -> Engine {
    let mut engine = Engine::new(English::new())
        .strictness(Strictness::Strict)
        .variation(Variation::Fixed);
    engine
        .register_template(
            "code.renamed",
            "{old_name|refer} was renamed to {new_name}{?consumer_count}, \
             which impacts {consumer_count} direct \
             {consumer_count|pluralize:consumer}{?consumers} \
             {consumers|truncate:3|join}{/?}{/?}",
        )
        .unwrap();
    engine
        .register_template(
            "code.modified",
            "{name|refer} was modified{?consumer_count}, which may affect \
             {consumer_count} {consumer_count|pluralize:consumer}{/?}",
        )
        .unwrap();
    engine
        .register_template(
            "code.moved",
            "{name|refer} was moved from {old_location} to {new_location}",
        )
        .unwrap();
    engine
}

fn sample_rename_ctx() -> Context {
    let mut ctx = Context::new();
    ctx.insert("entity_type", Value::String("class".into()));
    ctx.insert("old_name", Value::String("UserService".into()));
    ctx.insert("new_name", Value::String("AccountService".into()));
    ctx.insert("consumer_count", Value::Number(6));
    ctx.insert(
        "consumers",
        Value::List(vec![
            "ProfileComponent".into(),
            "SettingsComponent".into(),
            "AdminModule".into(),
            "Dashboard".into(),
            "Reports".into(),
            "Export".into(),
        ]),
    );
    ctx
}

fn bench_single_render(c: &mut Criterion) {
    let engine = build_engine();
    let ctx = sample_rename_ctx();
    c.bench_function("render_single_rename_medium", |b| {
        b.iter(|| {
            let mut session = Session::new();
            let out = engine.render(&mut session, "code.renamed", black_box(&ctx)).unwrap();
            black_box(out);
        });
    });
}

fn bench_batch_with_aggregation(c: &mut Criterion) {
    let engine = build_engine();
    // Three renames of different classes — triggers subject aggregation.
    let make_rename = |old: &str, new: &str| {
        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("class".into()));
        ctx.insert("old_name", Value::String(old.into()));
        ctx.insert("new_name", Value::String(new.into()));
        ctx.insert("consumer_count", Value::Number(2));
        ctx.insert(
            "consumers",
            Value::List(vec!["Foo".into(), "Bar".into()]),
        );
        ctx
    };
    let events: Vec<(&str, Context)> = vec![
        ("code.renamed", make_rename("UserService", "AccountService")),
        ("code.renamed", make_rename("AuthService", "IdentityService")),
        ("code.renamed", make_rename("DataService", "StorageService")),
    ];

    c.bench_function("render_batch_same_action_aggregation", |b| {
        b.iter(|| {
            let mut session = Session::new();
            let out = engine.render_batch(&mut session, black_box(&events)).unwrap();
            black_box(out);
        });
    });
}

fn bench_reg_with_many_distractors(c: &mut Criterion) {
    let mut engine = Engine::new(English::new())
        .strictness(Strictness::Strict)
        .variation(Variation::Fixed)
        .attribute_preference(vec!["layer".to_string()]);

    // 50 same-type entities, each with a different `layer` attribute —
    // REG must walk distractors until the target's layer is unique.
    for i in 0..50 {
        engine.register_entity(
            EntityDescriptor::new(format!("Service{i}"), "class")
                .with_attribute("layer", format!("layer_{i}")),
        );
    }
    engine
        .register_template("t", "{name|refer} was modified")
        .unwrap();

    let mut ctx = Context::new();
    ctx.insert("entity_type", Value::String("class".into()));
    ctx.insert("name", Value::String("Service25".into()));

    c.bench_function("reg_fifty_distractors_single_render", |b| {
        b.iter(|| {
            let mut session = Session::new();
            let out = engine.render(&mut session, "t", black_box(&ctx)).unwrap();
            black_box(out);
        });
    });
}

fn bench_document_plan_render(c: &mut Criterion) {
    let engine = build_engine();
    let make_ctx = |name: &str, count: i64| {
        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("class".into()));
        ctx.insert("name", Value::String(name.into()));
        ctx.insert("old_name", Value::String(name.into()));
        ctx.insert("new_name", Value::String(format!("{name}V2")));
        ctx.insert("consumer_count", Value::Number(count));
        ctx.insert(
            "consumers",
            Value::List(vec!["X".into(), "Y".into(), "Z".into()]),
        );
        ctx.insert("old_location", Value::String("old/".into()));
        ctx.insert("new_location", Value::String("new/".into()));
        ctx
    };

    let events: Vec<(&str, Context)> = vec![
        ("code.renamed", make_ctx("Alpha", 5)),
        ("code.modified", make_ctx("Alpha", 5)),
        ("code.modified", make_ctx("Alpha", 5)),
        ("code.moved", make_ctx("Beta", 3)),
        ("code.renamed", make_ctx("Gamma", 8)),
    ];

    c.bench_function("document_plan_by_entity", |b| {
        b.iter(|| {
            let plan = DocumentPlan::from_events_grouped(
                black_box(&events),
                &engine,
                GroupingStrategy::ByEntity,
            );
            let mut session = Session::new();
            let out = plan.render(&engine, &mut session).unwrap();
            black_box(out);
        });
    });
}

fn bench_clause_reduction(c: &mut Criterion) {
    let mut engine = Engine::new(English::new())
        .strictness(Strictness::Strict)
        .variation(Variation::Fixed);
    engine
        .register_template("renamed", "{name|refer} was renamed")
        .unwrap();
    engine
        .register_template("modified", "{name|refer} was modified")
        .unwrap();
    engine
        .register_template("moved", "{name|refer} was moved")
        .unwrap();

    let mut ctx = Context::new();
    ctx.insert("entity_type", Value::String("class".into()));
    ctx.insert("name", Value::String("UserService".into()));
    let events: Vec<(&str, Context)> = vec![
        ("renamed", ctx.clone()),
        ("modified", ctx.clone()),
        ("moved", ctx.clone()),
    ];

    c.bench_function("clause_reduction_three_same_entity", |b| {
        b.iter(|| {
            let mut session = Session::new();
            let out = engine.render_batch(&mut session, black_box(&events)).unwrap();
            black_box(out);
        });
    });
}

criterion_group!(
    benches,
    bench_single_render,
    bench_batch_with_aggregation,
    bench_reg_with_many_distractors,
    bench_document_plan_render,
    bench_clause_reduction,
);
criterion_main!(benches);
