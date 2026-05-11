use prosaic_core::{
    Context, Engine, PIPE_SPECS, RenderExplanation, Salience, Session, Strictness, Template, Value,
    ValueType, Variation,
};
use prosaic_grammar_en::English;

#[test]
fn builtin_pipe_registry_is_stable_for_one_x() {
    let contract: Vec<(&str, ValueType, ValueType)> = PIPE_SPECS
        .iter()
        .map(|spec| (spec.name, spec.input, spec.output))
        .collect();

    assert_eq!(
        contract,
        vec![
            ("plural", ValueType::Number, ValueType::String),
            ("pluralize", ValueType::Number, ValueType::String),
            ("article", ValueType::Any, ValueType::String),
            ("join", ValueType::List, ValueType::String),
            ("ordinal", ValueType::Number, ValueType::String),
            ("words", ValueType::Number, ValueType::String),
            ("truncate", ValueType::List, ValueType::List),
            ("capitalize", ValueType::Any, ValueType::String),
            ("refer", ValueType::Any, ValueType::String),
            ("possessive", ValueType::Any, ValueType::String),
            ("verb", ValueType::Any, ValueType::String),
            ("syn", ValueType::Any, ValueType::String),
            ("relative", ValueType::Number, ValueType::String),
            ("since_last", ValueType::Number, ValueType::String),
            ("quantify", ValueType::Number, ValueType::String),
            ("proportion", ValueType::Number, ValueType::String),
            ("hedge", ValueType::Number, ValueType::String),
            ("negated", ValueType::Any, ValueType::String),
            ("choose", ValueType::Any, ValueType::String),
            ("demonstrative", ValueType::Any, ValueType::String),
        ]
    );
}

#[test]
fn template_syntax_contract_covers_slots_pipes_conditionals_and_partials() {
    let template = Template::parse(
        "{old_name|refer} became {new_name}{?consumer_count}, affecting \
         {consumer_count} {consumer_count|pluralize:consumer}: {>consumer_tail}{/?}",
    )
    .unwrap();

    let mut keys = template.slot_keys();
    keys.sort();
    assert_eq!(
        keys,
        vec![
            "consumer_count",
            "consumer_count",
            "consumer_count",
            "new_name",
            "old_name",
        ]
    );

    assert_eq!(template.pipe_names(), vec!["refer", "pluralize"]);
    assert_eq!(template.partial_names(), vec!["consumer_tail"]);

    let mut inferred = template.infer_types().unwrap();
    inferred.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(
        inferred,
        vec![
            ("consumer_count".to_string(), ValueType::Number),
            ("new_name".to_string(), ValueType::Any),
            ("old_name".to_string(), ValueType::Any),
        ]
    );
}

#[test]
fn core_rendering_contract_for_code_rename_is_stable() {
    let mut engine = Engine::new(English::new())
        .strictness(Strictness::Strict)
        .variation(Variation::Fixed);
    prosaic_vocab_code::register(&mut engine).unwrap();

    let mut ctx = Context::new();
    ctx.insert("entity_type", Value::String("class".into()));
    ctx.insert("old_name", Value::String("Foo".into()));
    ctx.insert("new_name", Value::String("Bar".into()));
    ctx.insert("consumer_count", Value::Number(3));

    let mut session = Session::new();
    let explanation: RenderExplanation = engine
        .render_explained(&mut session, "code.renamed", &ctx)
        .unwrap();

    assert_eq!(
        explanation.output,
        "The class Foo was renamed to Bar, which impacts 3 direct consumers."
    );
    assert_eq!(explanation.template_key, "code.renamed");
    assert_eq!(explanation.variant_index, 0);
    assert_eq!(explanation.salience, Salience::Medium);
    assert_eq!(format!("{:?}", explanation.reference_form), "Some(Full)");
    assert_eq!(explanation.connective, None);
    assert_eq!(format!("{:?}", explanation.centering_transition), "NoCb");
}
