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
