//! Integration tests for the `prosaic_template!` macro, including the
//! `context:` argument added in the type-aware validation plan.

use prosaic_derive::{IntoContext, prosaic_template};

#[derive(IntoContext)]
#[allow(dead_code)]
struct SimpleCtx {
    name: String,
    count: i64,
}

#[test]
fn macro_accepts_context_argument_when_types_align() {
    let tpl = prosaic_template! {
        template: "{name} has {count|pluralize:item}",
        slots: [name, count],
        context: SimpleCtx,
    };
    assert!(tpl.contains("{name}"));
    assert!(tpl.contains("{count|pluralize:item}"));
}

#[test]
fn macro_without_context_still_works() {
    let tpl = prosaic_template! {
        template: "{name} has {count|pluralize:item}",
        slots: [name, count],
    };
    assert!(tpl.contains("{count|pluralize:item}"));
}

#[test]
fn macro_with_context_and_matching_types_compiles_and_runs() {
    #[derive(IntoContext)]
    #[allow(dead_code)]
    struct Ok1 {
        count: i64,
        items: Vec<String>,
    }

    let _tpl = prosaic_template! {
        template: "{count|pluralize:item}: {items|truncate:3|join}",
        slots: [count, items],
        context: Ok1,
    };
}

#[test]
fn macro_with_context_unifies_any_and_concrete() {
    // Slot is used once bare and once with a pipe — should unify to Number.
    #[derive(IntoContext)]
    #[allow(dead_code)]
    struct Ok2 {
        x: i64,
    }

    let _tpl = prosaic_template! {
        template: "{x} items ({x|pluralize:item})",
        slots: [x],
        context: Ok2,
    };
}
