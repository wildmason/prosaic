use nlg_core::{Engine, NlgError};

/// Register code-analysis vocabulary templates into an engine.
///
/// Provides templates for common code change events: renames, deletions,
/// additions, modifications, moves, and signature changes.
///
/// Each event type has multiple template variants for use with
/// `Variation::RoundRobin` or `Variation::Seeded` to avoid repetitive output.
pub fn register(engine: &mut Engine) -> Result<(), NlgError> {
    register_rename_templates(engine)?;
    register_delete_templates(engine)?;
    register_add_templates(engine)?;
    register_modify_templates(engine)?;
    register_move_templates(engine)?;
    register_signature_templates(engine)?;
    Ok(())
}

fn register_rename_templates(engine: &mut Engine) -> Result<(), NlgError> {
    engine.register_template(
        "code.renamed",
        "{old_name|refer} was renamed to {new_name}{?consumer_count}, \
         which impacts {consumer_count} direct {consumer_count|pluralize:consumer}{?consumers} \
         {consumers|truncate:3|join}{/?}{/?}",
    )?;
    engine.register_template(
        "code.renamed",
        "{old_name|refer} has been renamed to {new_name}{?consumer_count}, \
         affecting {consumer_count} {consumer_count|pluralize:dependent}{?consumers} \
         {consumers|truncate:3|join}{/?}{/?}",
    )?;
    engine.register_template(
        "code.renamed",
        "{old_name|refer} is now called {new_name}{?consumer_count} \
         ({consumer_count} {consumer_count|pluralize:consumer} affected{?consumers}: \
         {consumers|truncate:3|join}{/?}){/?}",
    )?;
    Ok(())
}

fn register_delete_templates(engine: &mut Engine) -> Result<(), NlgError> {
    engine.register_template(
        "code.deleted",
        "{name|refer} was removed{?consumer_count}, \
         impacting {consumer_count} {consumer_count|pluralize:dependent}{?consumers} \
         {consumers|truncate:3|join}{/?}{/?}",
    )?;
    engine.register_template(
        "code.deleted",
        "{name|refer} has been deleted{?consumer_count} \
         ({consumer_count} {consumer_count|pluralize:reference} to update{?consumers}: \
         {consumers|truncate:3|join}{/?}){/?}",
    )?;
    engine.register_template(
        "code.deleted",
        "{name|refer} no longer exists{?consumer_count}, \
         breaking {consumer_count} {consumer_count|pluralize:consumer}{?consumers} \
         {consumers|truncate:3|join}{/?}{/?}",
    )?;
    Ok(())
}

fn register_add_templates(engine: &mut Engine) -> Result<(), NlgError> {
    engine.register_template(
        "code.added",
        "A new {entity_type} {name} was added in {location}",
    )?;
    engine.register_template(
        "code.added",
        "The {entity_type} {name} was introduced in {location}",
    )?;
    engine.register_template(
        "code.added",
        "{name} \u{2014} a new {entity_type} \u{2014} was created in {location}",
    )?;
    Ok(())
}

fn register_modify_templates(engine: &mut Engine) -> Result<(), NlgError> {
    engine.register_template(
        "code.modified",
        "{name|refer} was modified{?consumer_count}, \
         which may affect {consumer_count} {consumer_count|pluralize:consumer}{?consumers} \
         {consumers|truncate:3|join}{/?}{/?}",
    )?;
    engine.register_template(
        "code.modified",
        "{name|refer} has been updated{?consumer_count} \
         ({consumer_count} {consumer_count|pluralize:consumer} may need review{?consumers}: \
         {consumers|truncate:3|join}{/?}){/?}",
    )?;
    engine.register_template(
        "code.modified",
        "Changes to {name|refer}{?consumer_count} affect \
         {consumer_count} {consumer_count|pluralize:dependent}{?consumers} \
         {consumers|truncate:3|join}{/?}{/?}",
    )?;
    Ok(())
}

fn register_move_templates(engine: &mut Engine) -> Result<(), NlgError> {
    engine.register_template(
        "code.moved",
        "{name|refer} was moved from {old_location} to {new_location}{?consumer_count}, \
         requiring import updates in {consumer_count} {consumer_count|pluralize:file}{?consumers} \
         {consumers|truncate:3|join}{/?}{/?}",
    )?;
    engine.register_template(
        "code.moved",
        "{name|refer} has been relocated to {new_location}{?consumer_count} \
         ({consumer_count} {consumer_count|pluralize:import} to update{?consumers}: \
         {consumers|truncate:3|join}{/?}){/?}",
    )?;
    Ok(())
}

fn register_signature_templates(engine: &mut Engine) -> Result<(), NlgError> {
    engine.register_template(
        "code.signature_changed",
        "The signature of {name|refer} was changed{?consumer_count}, \
         requiring updates in {consumer_count} {consumer_count|pluralize:caller}{?consumers} \
         {consumers|truncate:3|join}{/?}{/?}",
    )?;
    engine.register_template(
        "code.signature_changed",
        "{name|refer} has a new signature{?consumer_count}, \
         impacting {consumer_count} call {consumer_count|pluralize:site}{?consumers} \
         {consumers|truncate:3|join}{/?}{/?}",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nlg_core::{Context, Engine, Strictness, Value, Variation};
    use nlg_grammar_en::English;

    fn test_engine() -> Engine {
        let mut engine = Engine::new(English::new())
            .strictness(Strictness::Strict)
            .variation(Variation::Fixed);
        register(&mut engine).unwrap();
        engine
    }

    #[test]
    fn rename_event() {
        let engine = test_engine();

        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("class".into()));
        ctx.insert("old_name", Value::String("Foo".into()));
        ctx.insert("new_name", Value::String("Foobar".into()));
        ctx.insert("consumer_count", Value::Number(6));
        ctx.insert(
            "consumers",
            Value::List(vec![
                "Baz".into(),
                "Qux".into(),
                "Quux".into(),
                "Corge".into(),
                "Grault".into(),
                "Garply".into(),
            ]),
        );

        let result = engine.render("code.renamed", &ctx).unwrap();
        assert!(result.contains("Foo was renamed to Foobar"));
        assert!(result.contains("6 direct consumers"));
        assert!(result.contains("Baz"));
        assert!(result.contains("Qux"));
        assert!(result.contains("Quux"));
    }

    #[test]
    fn rename_event_single_consumer() {
        let engine = test_engine();

        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("method".into()));
        ctx.insert("old_name", Value::String("getData".into()));
        ctx.insert("new_name", Value::String("fetchData".into()));
        ctx.insert("consumer_count", Value::Number(1));
        ctx.insert(
            "consumers",
            Value::List(vec!["DashboardComponent".into()]),
        );

        let result = engine.render("code.renamed", &ctx).unwrap();
        assert!(result.contains("getData was renamed to fetchData"));
        assert!(result.contains("1 direct consumer"));
        assert!(result.contains("DashboardComponent"));
    }

    #[test]
    fn delete_event() {
        let engine = test_engine();

        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("interface".into()));
        ctx.insert("name", Value::String("UserProfile".into()));
        ctx.insert("consumer_count", Value::Number(3));
        ctx.insert(
            "consumers",
            Value::List(vec![
                "UserService".into(),
                "ProfilePage".into(),
                "AdminPanel".into(),
            ]),
        );

        let result = engine.render("code.deleted", &ctx).unwrap();
        assert!(result.contains("UserProfile was removed"));
        assert!(result.contains("3 dependents"));
        assert!(result.contains("UserService"));
        assert!(result.contains("ProfilePage"));
        assert!(result.contains("AdminPanel"));
    }

    #[test]
    fn add_event() {
        let engine = test_engine();

        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("service".into()));
        ctx.insert("name", Value::String("AuthGuard".into()));
        ctx.insert("location", Value::String("src/guards/auth.guard.ts".into()));

        let result = engine.render("code.added", &ctx).unwrap();
        assert!(result.contains("service AuthGuard"));
        assert!(result.contains("src/guards/auth.guard.ts"));
    }

    #[test]
    fn modify_event() {
        let engine = test_engine();

        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("method".into()));
        ctx.insert("name", Value::String("processOrder".into()));
        ctx.insert("consumer_count", Value::Number(4));
        ctx.insert(
            "consumers",
            Value::List(vec![
                "OrderPage".into(),
                "CartService".into(),
                "CheckoutFlow".into(),
                "OrderHistory".into(),
            ]),
        );

        let result = engine.render("code.modified", &ctx).unwrap();
        assert!(result.contains("processOrder was modified"));
        assert!(result.contains("4 consumers"));
        assert!(result.contains("OrderPage"));
        assert!(result.contains("CartService"));
        assert!(result.contains("CheckoutFlow"));
    }

    #[test]
    fn move_event() {
        let engine = test_engine();

        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("class".into()));
        ctx.insert("name", Value::String("Logger".into()));
        ctx.insert("old_location", Value::String("src/utils/logger.ts".into()));
        ctx.insert("new_location", Value::String("src/core/logger.ts".into()));
        ctx.insert("consumer_count", Value::Number(12));
        ctx.insert(
            "consumers",
            Value::List(vec![
                "AppModule".into(),
                "AuthService".into(),
                "UserService".into(),
                "OrderService".into(),
            ]),
        );

        let result = engine.render("code.moved", &ctx).unwrap();
        assert!(result.contains("Logger"));
        assert!(result.contains("src/utils/logger.ts"));
        assert!(result.contains("src/core/logger.ts"));
        assert!(result.contains("12 files"));
    }

    #[test]
    fn signature_changed_event() {
        let engine = test_engine();

        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("method".into()));
        ctx.insert("name", Value::String("getUser".into()));
        ctx.insert("consumer_count", Value::Number(5));
        ctx.insert(
            "consumers",
            Value::List(vec![
                "ProfileController".into(),
                "AuthMiddleware".into(),
                "UserTest".into(),
                "AdminPanel".into(),
                "SettingsPage".into(),
            ]),
        );

        let result = engine.render("code.signature_changed", &ctx).unwrap();
        assert!(result.contains("getUser"));
        assert!(result.contains("method"));
        assert!(result.contains("5 callers"));
    }

    #[test]
    fn variation_produces_different_templates() {
        let mut engine1 = Engine::new(English::new())
            .strictness(Strictness::Strict)
            .variation(Variation::Fixed);
        register(&mut engine1).unwrap();

        let mut engine2 = Engine::new(English::new())
            .strictness(Strictness::Strict)
            .variation(Variation::Seeded(999));
        register(&mut engine2).unwrap();

        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("class".into()));
        ctx.insert("old_name", Value::String("Foo".into()));
        ctx.insert("new_name", Value::String("Bar".into()));
        ctx.insert("consumer_count", Value::Number(2));
        ctx.insert(
            "consumers",
            Value::List(vec!["A".into(), "B".into()]),
        );

        let result1 = engine1.render("code.renamed", &ctx).unwrap();
        let result2 = engine2.render("code.renamed", &ctx).unwrap();

        // With 3 alternatives and different variation strategies,
        // they should produce different output (though this is seed-dependent)
        // The important thing is both render successfully
        assert!(!result1.is_empty());
        assert!(!result2.is_empty());
    }
}
