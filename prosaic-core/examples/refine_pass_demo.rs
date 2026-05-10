//! Before/after evidence for the retrospective refine pass.
//!
//! Renders the same corpus twice — first with `RefineConfig::off()` (the
//! default), then with `RefineConfig::balanced()` — and prints both. The
//! same `Variation::Fixed` is used everywhere so the only difference
//! between the columns is whether the refine loop fires.
//!
//! The corpus is constructed to deliberately trigger
//! `ParagraphOpenerMonotony` (≥3 paragraphs sharing an opener) so the
//! refine pass has something to bite on.
//!
//! Run with `cargo run -p prosaic-core --example refine_pass_demo`.

use prosaic_core::{
    Context, DocumentPlan, Engine, RefineConfig, Session, Strictness, Value, Variation,
};
use prosaic_grammar_en::English;

fn main() {
    let events = corpus();

    println!("=== refine: off (baseline) ===");
    let off = build_engine(false);
    let off_plan = DocumentPlan::from_events(&events, &off);
    let off_text = off_plan.render_structured(&off, &mut Session::new()).unwrap();
    println!("{}", off_text.text);

    println!();
    println!("=== refine: balanced (max_iterations=3) ===");
    let on = build_engine(true);
    let on_plan = DocumentPlan::from_events(&events, &on);
    let outcome = on_plan.render_refined(&on, &mut Session::new()).unwrap();
    println!("{}", outcome.text);

    println!();
    println!(
        "Iterations run: {} (cap = 3)\nFinal score: {:.3}\nConverged clean: {}",
        outcome.iterations_run, outcome.final_score, outcome.converged_clean,
    );

    let off_additionally = off_text.text.matches("Additionally,").count();
    let on_additionally = outcome.text.matches("Additionally,").count();
    println!(
        "\n'Additionally,' opener count: baseline = {off_additionally}, refined = {on_additionally}"
    );
}

fn build_engine(refine_on: bool) -> Engine {
    let mut e = Engine::new(English::new())
        .strictness(Strictness::Strict)
        .variation(Variation::Fixed);
    if refine_on {
        e = e.refine(RefineConfig::balanced().with_max_iterations(3));
    }
    e.register_template("evt.modified", "{name|refer} was modified")
        .unwrap();
    e.register_template("evt.touched", "{name|refer} was touched")
        .unwrap();
    e
}

fn corpus() -> Vec<(&'static str, Context)> {
    let entities = ["Alpha", "Bravo", "Charlie", "Delta", "Echo"];
    let mut out: Vec<(&str, Context)> = Vec::new();
    for name in entities {
        out.push(("evt.modified", ctx_named(name)));
        out.push(("evt.touched", ctx_named(name)));
    }
    out
}

fn ctx_named(name: &str) -> Context {
    let mut c = Context::new();
    c.insert("name", Value::String(name.into()));
    c.insert("entity_type", Value::String("class".into()));
    c
}
