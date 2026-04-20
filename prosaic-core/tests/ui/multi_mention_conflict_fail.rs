use prosaic_derive::prosaic_template;

fn main() {
    let _tpl = prosaic_template! {
        template: "{x|pluralize:item} and {x|join}",
        slots: [x],
    };
}
