use prosaic_derive::{IntoContext, prosaic_template};

#[derive(IntoContext)]
#[allow(dead_code)]
struct Ctx {
    count: i64,
    items: Vec<String>,
}

fn main() {
    let _tpl = prosaic_template! {
        template: "{count|pluralize:item}: {items|truncate:3|join}",
        slots: [count, items],
        context: Ctx,
    };
}
