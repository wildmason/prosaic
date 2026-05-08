//! Before/after prose evidence for the cross-paragraph list-style fix.
//!
//! Run with `cargo run -p prosaic-core --example list_style_continuity_demo`.

use prosaic_core::{
    Context, DocumentPlan, Engine, Paragraph, Salience, Session, Strictness, Value, Variation,
};
use prosaic_grammar_en::English;

fn list_paragraph(template_key: &str, items: &[&str]) -> Paragraph {
    let mut p = Paragraph::new();
    let mut ctx = Context::new();
    ctx.insert(
        "items",
        Value::List(items.iter().map(|s| (*s).to_string()).collect()),
    );
    p.push(template_key.to_string(), ctx, Salience::Medium);
    p
}

fn build_engine() -> Engine {
    let mut engine = Engine::new(English::new())
        .strictness(Strictness::Strict)
        .variation(Variation::Fixed);
    for key in ["p1", "p2", "p3", "p4"] {
        engine
            .register_template(key, "Touched {items|truncate:2|join}.")
            .unwrap();
    }
    engine
}

fn build_plan() -> DocumentPlan {
    let mut plan = DocumentPlan::new();
    plan.paragraphs
        .push(list_paragraph("p1", &["Alpha", "Beta", "Gamma", "Delta"]));
    plan.paragraphs
        .push(list_paragraph("p2", &["Echo", "Foxtrot", "Golf", "Hotel"]));
    plan.paragraphs
        .push(list_paragraph("p3", &["India", "Juliet", "Kilo", "Lima"]));
    plan.paragraphs
        .push(list_paragraph("p4", &["Mike", "November", "Oscar", "Papa"]));
    plan
}

fn render_simulating_old_full_reset_per_paragraph(
    engine: &Engine,
    plan: &DocumentPlan,
) -> String {
    // Simulates the old behavior: a hard `Session::reset` between paragraphs,
    // wiping the list-style cycle. Renders each paragraph in isolation.
    let mut session = Session::new();
    let mut out = Vec::new();
    for (idx, p) in plan.paragraphs.iter().enumerate() {
        if idx > 0 {
            session.reset();
        }
        let mut sub = DocumentPlan::new();
        sub.paragraphs.push(p.clone());
        out.push(sub.render(engine, &mut session).unwrap());
    }
    out.join("\n\n")
}

fn main() {
    let engine = build_engine();
    let plan = build_plan();

    println!("=== BEFORE FIX (full reset per paragraph) ===\n");
    println!("{}", render_simulating_old_full_reset_per_paragraph(&engine, &plan));

    println!("\n=== AFTER FIX (paragraph-scoped reset preserves cycle) ===\n");
    let mut session = Session::new();
    println!("{}", plan.render(&engine, &mut session).unwrap());
}
