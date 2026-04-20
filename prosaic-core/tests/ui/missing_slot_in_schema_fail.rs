use prosaic_derive::{IntoContext, prosaic_template};

#[derive(IntoContext)]
#[allow(dead_code)]
struct Ctx {
    name: String,
}

fn main() {
    let _tpl = prosaic_template! {
        template: "{count|pluralize:item}",
        slots: [count],
        context: Ctx,
    };
}
