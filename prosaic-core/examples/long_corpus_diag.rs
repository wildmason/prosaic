use prosaic_core::*;
use prosaic_grammar_en::English;
fn main() {
    let mut e = Engine::new(English::new())
        .strictness(Strictness::Strict)
        .variation(Variation::Fixed);
    e.register_template("evt.modified", "{name|refer} was modified")
        .unwrap();
    e.register_template("evt.touched", "{name|refer} was touched")
        .unwrap();
    let mut events: Vec<(&str, Context)> = Vec::new();
    let names = [
        "Alpha", "Bravo", "Charlie", "Delta", "Echo", "Foxtrot", "Golf", "Hotel", "India",
        "Juliet", "Kilo", "Lima",
    ];
    for n in names {
        let mut c = Context::new();
        c.insert("name", Value::String(n.into()));
        c.insert("entity_type", Value::String("class".into()));
        events.push(("evt.modified", c.clone()));
        events.push(("evt.touched", c));
    }
    let plan = DocumentPlan::from_events(&events, &e);
    let doc = plan.render_structured(&e, &mut Session::new()).unwrap();
    println!("{}", doc.text);
    println!("---");
    let mut counts = std::collections::BTreeMap::<String, usize>::new();
    for c in &doc.connectives_used {
        *counts.entry(c.connective.clone()).or_insert(0) += 1;
    }
    for (k, v) in &counts {
        println!("{k}: {v}");
    }
}
