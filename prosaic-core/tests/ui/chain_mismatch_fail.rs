use prosaic_derive::prosaic_template;

fn main() {
    let _tpl = prosaic_template! {
        template: "{x|capitalize|pluralize}",
        slots: [x],
    };
}
