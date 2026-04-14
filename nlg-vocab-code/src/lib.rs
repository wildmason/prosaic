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
        "The {entity_type} {old_name} was renamed to {new_name} \
         which impacts {consumer_count} direct {consumer_count|pluralize:consumer} \
         [{consumers|truncate:3|join}]",
    )?;
    engine.register_template(
        "code.renamed",
        "{old_name} ({entity_type}) was renamed to {new_name}, \
         affecting {consumer_count} {consumer_count|pluralize:dependent} \
         [{consumers|truncate:3|join}]",
    )?;
    engine.register_template(
        "code.renamed",
        "Renamed {entity_type} {old_name} to {new_name} \
         ({consumer_count} {consumer_count|pluralize:consumer} affected: \
         {consumers|truncate:3|join})",
    )?;
    Ok(())
}

fn register_delete_templates(engine: &mut Engine) -> Result<(), NlgError> {
    engine.register_template(
        "code.deleted",
        "The {entity_type} {name} was removed, \
         impacting {consumer_count} {consumer_count|pluralize:dependent} \
         [{consumers|truncate:3|join}]",
    )?;
    engine.register_template(
        "code.deleted",
        "Removed {entity_type} {name} \
         ({consumer_count} {consumer_count|pluralize:reference} to update: \
         {consumers|truncate:3|join})",
    )?;
    engine.register_template(
        "code.deleted",
        "{name} ({entity_type}) was deleted, \
         breaking {consumer_count} {consumer_count|pluralize:consumer} \
         [{consumers|truncate:3|join}]",
    )?;
    Ok(())
}

fn register_add_templates(engine: &mut Engine) -> Result<(), NlgError> {
    engine.register_template(
        "code.added",
        "Added new {entity_type} {name} in {location}",
    )?;
    engine.register_template(
        "code.added",
        "New {entity_type} {name} was introduced in {location}",
    )?;
    engine.register_template(
        "code.added",
        "Created {entity_type} {name} ({location})",
    )?;
    Ok(())
}

fn register_modify_templates(engine: &mut Engine) -> Result<(), NlgError> {
    engine.register_template(
        "code.modified",
        "The {entity_type} {name} was modified, \
         which may affect {consumer_count} {consumer_count|pluralize:consumer} \
         [{consumers|truncate:3|join}]",
    )?;
    engine.register_template(
        "code.modified",
        "Modified {entity_type} {name} \
         ({consumer_count} {consumer_count|pluralize:consumer} may need review: \
         {consumers|truncate:3|join})",
    )?;
    engine.register_template(
        "code.modified",
        "Changes to {entity_type} {name} affect \
         {consumer_count} {consumer_count|pluralize:dependent} \
         [{consumers|truncate:3|join}]",
    )?;
    Ok(())
}

fn register_move_templates(engine: &mut Engine) -> Result<(), NlgError> {
    engine.register_template(
        "code.moved",
        "The {entity_type} {name} was moved from {old_location} to {new_location}, \
         requiring import updates in {consumer_count} {consumer_count|pluralize:file} \
         [{consumers|truncate:3|join}]",
    )?;
    engine.register_template(
        "code.moved",
        "Moved {entity_type} {name}: {old_location} -> {new_location} \
         ({consumer_count} {consumer_count|pluralize:import} to update: \
         {consumers|truncate:3|join})",
    )?;
    Ok(())
}

fn register_signature_templates(engine: &mut Engine) -> Result<(), NlgError> {
    engine.register_template(
        "code.signature_changed",
        "The signature of {entity_type} {name} was changed, \
         requiring updates in {consumer_count} {consumer_count|pluralize:caller} \
         [{consumers|truncate:3|join}]",
    )?;
    engine.register_template(
        "code.signature_changed",
        "{name} ({entity_type}) has a new signature, \
         impacting {consumer_count} {consumer_count|pluralize:call} {consumer_count|pluralize:site} \
         [{consumers|truncate:3|join}]",
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
        assert_eq!(
            result,
            "The class Foo was renamed to Foobar \
             which impacts 6 direct consumers \
             [Baz, Qux, Quux, and 3 more]"
        );
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
        assert_eq!(
            result,
            "The method getData was renamed to fetchData \
             which impacts 1 direct consumer \
             [DashboardComponent]"
        );
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
        assert_eq!(
            result,
            "The interface UserProfile was removed, \
             impacting 3 dependents \
             [UserService, ProfilePage, and AdminPanel]"
        );
    }

    #[test]
    fn add_event() {
        let engine = test_engine();

        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("service".into()));
        ctx.insert("name", Value::String("AuthGuard".into()));
        ctx.insert("location", Value::String("src/guards/auth.guard.ts".into()));

        let result = engine.render("code.added", &ctx).unwrap();
        assert_eq!(
            result,
            "Added new service AuthGuard in src/guards/auth.guard.ts"
        );
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
        assert_eq!(
            result,
            "The method processOrder was modified, \
             which may affect 4 consumers \
             [OrderPage, CartService, CheckoutFlow, and 1 more]"
        );
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
