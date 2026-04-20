use prosaic_derive::{IntoContext, prosaic_template};

#[derive(IntoContext)]
#[allow(dead_code)]
struct Ctx {
    count: String, // wrong — pluralize needs Number
}

fn main() {
    let _tpl = prosaic_template! {
        template: "{count|pluralize:item}",
        slots: [count],
        context: Ctx,
    };
}
