use prosaic_core::{Context, IntoContext, Value};
use prosaic_derive::IntoContext;

#[derive(IntoContext)]
struct RenameEvent {
    entity_type: String,
    old_name: String,
    new_name: String,
    consumer_count: i64,
    consumers: Vec<String>,
}

#[test]
fn derive_into_context_basic() {
    let event = RenameEvent {
        entity_type: "class".to_string(),
        old_name: "Foo".to_string(),
        new_name: "Foobar".to_string(),
        consumer_count: 6,
        consumers: vec!["Baz".to_string(), "Qux".to_string()],
    };

    let ctx: Context = event.into_context();

    assert_eq!(
        ctx.get("entity_type"),
        Some(&Value::String("class".to_string()))
    );
    assert_eq!(ctx.get("old_name"), Some(&Value::String("Foo".to_string())));
    assert_eq!(
        ctx.get("new_name"),
        Some(&Value::String("Foobar".to_string()))
    );
    assert_eq!(ctx.get("consumer_count"), Some(&Value::Number(6)));
    assert_eq!(
        ctx.get("consumers"),
        Some(&Value::List(vec!["Baz".to_string(), "Qux".to_string()]))
    );
}

#[derive(IntoContext)]
struct OptionalEvent {
    name: String,
    description: Option<String>,
    count: Option<i64>,
}

#[test]
fn derive_with_option_some() {
    let event = OptionalEvent {
        name: "test".to_string(),
        description: Some("a description".to_string()),
        count: Some(42),
    };

    let ctx = event.into_context();

    assert_eq!(ctx.get("name"), Some(&Value::String("test".to_string())));
    assert_eq!(
        ctx.get("description"),
        Some(&Value::String("a description".to_string()))
    );
    assert_eq!(ctx.get("count"), Some(&Value::Number(42)));
}

#[test]
fn derive_with_option_none() {
    let event = OptionalEvent {
        name: "test".to_string(),
        description: None,
        count: None,
    };

    let ctx = event.into_context();

    assert_eq!(ctx.get("name"), Some(&Value::String("test".to_string())));
    assert_eq!(ctx.get("description"), None);
    assert_eq!(ctx.get("count"), None);
}

#[derive(IntoContext)]
struct NumericTypes {
    a: u32,
    b: usize,
    c: i32,
}

#[test]
fn derive_numeric_types() {
    let event = NumericTypes {
        a: 10,
        b: 20,
        c: -5,
    };
    let ctx = event.into_context();

    assert_eq!(ctx.get("a"), Some(&Value::Number(10)));
    assert_eq!(ctx.get("b"), Some(&Value::Number(20)));
    assert_eq!(ctx.get("c"), Some(&Value::Number(-5)));
}
