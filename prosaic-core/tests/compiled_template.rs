//! Integration tests for the `prosaic_template_compiled!` proc macro.
//!
//! This macro emits a `fn(&Context) -> String` block expression that renders
//! a bare-slot template without the runtime parsing pipeline.

use prosaic_core::{Context, Value};
use prosaic_derive::prosaic_template_compiled;

// ── Core rendering ────────────────────────────────────────────────────────────

#[test]
fn compiled_template_renders_bare_slot() {
    let render = prosaic_template_compiled!("Hello {name}!");
    let mut ctx = Context::new();
    ctx.insert("name", Value::String("World".into()));
    assert_eq!(render(&ctx), "Hello World!");
}

#[test]
fn compiled_template_fills_missing_slot_with_empty() {
    let render = prosaic_template_compiled!("Hello {name}!");
    let ctx = Context::new();
    // No "name" slot — slot contributes nothing.
    assert_eq!(render(&ctx), "Hello !");
}

#[test]
fn compiled_template_handles_multiple_slots() {
    let render = prosaic_template_compiled!("{greeting}, {name}!");
    let mut ctx = Context::new();
    ctx.insert("greeting", Value::String("Hi".into()));
    ctx.insert("name", Value::String("Alice".into()));
    assert_eq!(render(&ctx), "Hi, Alice!");
}

#[test]
fn compiled_template_literal_only() {
    let render = prosaic_template_compiled!("All systems nominal.");
    let ctx = Context::new();
    assert_eq!(render(&ctx), "All systems nominal.");
}

#[test]
fn compiled_template_slot_only() {
    let render = prosaic_template_compiled!("{msg}");
    let mut ctx = Context::new();
    ctx.insert("msg", Value::String("test output".into()));
    assert_eq!(render(&ctx), "test output");
}

#[test]
fn compiled_template_number_slot() {
    let render = prosaic_template_compiled!("Count: {n}");
    let mut ctx = Context::new();
    ctx.insert("n", Value::Number(42));
    assert_eq!(render(&ctx), "Count: 42");
}

#[test]
fn compiled_template_multiple_missing_slots_produce_empty_regions() {
    let render = prosaic_template_compiled!("{a} and {b}");
    let ctx = Context::new();
    // Both missing → just the literal " and "
    assert_eq!(render(&ctx), " and ");
}

#[test]
fn compiled_template_returns_fn_item_usable_multiple_times() {
    let render = prosaic_template_compiled!("Hello {name}!");

    let mut ctx1 = Context::new();
    ctx1.insert("name", Value::String("Alice".into()));

    let mut ctx2 = Context::new();
    ctx2.insert("name", Value::String("Bob".into()));

    assert_eq!(render(&ctx1), "Hello Alice!");
    assert_eq!(render(&ctx2), "Hello Bob!");
}

// ── Type compatibility ────────────────────────────────────────────────────────

#[test]
fn compiled_template_renders_list_value_via_display() {
    let render = prosaic_template_compiled!("Items: {items}");
    let mut ctx = Context::new();
    ctx.insert(
        "items",
        Value::List(vec!["alpha".to_string(), "beta".to_string()]),
    );
    let out = render(&ctx);
    // Value::List display is implementation-defined; just check it renders
    // non-empty and contains the slot output (not a slot placeholder).
    assert!(!out.is_empty());
    assert!(!out.contains("{items}"));
}
