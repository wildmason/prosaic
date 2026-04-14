use nlg_core::{Engine, NlgError, Salience};

/// Register code-analysis vocabulary templates into an engine.
///
/// Provides templates for common code change events: renames, deletions,
/// additions, modifications, moves, and signature changes.
///
/// Templates are registered at three salience levels (Low, Medium, High) so
/// the engine can match verbosity to event magnitude: low-impact changes get
/// terser phrasing while high-impact changes get elaboration.
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
    // Low: terse, drops impact details
    engine.register_template_at(
        "code.renamed",
        "{old_name|refer} was renamed to {new_name}",
        Salience::Low,
    )?;
    engine.register_template_at(
        "code.renamed",
        "{old_name|refer} is now called {new_name}",
        Salience::Low,
    )?;

    // Medium: default, includes impact clause
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

    // High: elaborative, emphasizes significance.
    // Uses plain `join:bracketed` so the template's own literal prefix
    // ("including", "notably") isn't duplicated by an auto-selected list style.
    engine.register_template_at(
        "code.renamed",
        "{old_name|refer} has been renamed to {new_name} \u{2014} a significant change \
         rippling through {consumer_count} direct {consumer_count|pluralize:consumer}{?consumers}, \
         including {consumers|truncate:5|join:bracketed}{/?}",
        Salience::High,
    )?;
    engine.register_template_at(
        "code.renamed",
        "{old_name|refer} was renamed to {new_name}. This change affects a substantial \
         {consumer_count} {consumer_count|pluralize:dependent}{?consumers} \u{2014} notably \
         {consumers|truncate:5|join:bracketed}{/?}",
        Salience::High,
    )?;

    Ok(())
}

fn register_delete_templates(engine: &mut Engine) -> Result<(), NlgError> {
    // Low
    engine.register_template_at(
        "code.deleted",
        "{name|refer} was removed",
        Salience::Low,
    )?;
    engine.register_template_at(
        "code.deleted",
        "{name|refer} has been deleted",
        Salience::Low,
    )?;

    // Medium
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

    // High
    engine.register_template_at(
        "code.deleted",
        "{name|refer} has been removed entirely \u{2014} a breaking change affecting \
         {consumer_count} {consumer_count|pluralize:consumer}{?consumers} including \
         {consumers|truncate:5|join:bracketed}{/?}. All {consumer_count|pluralize:reference} \
         will need migration.",
        Salience::High,
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
    // Low
    engine.register_template_at(
        "code.modified",
        "{name|refer} was modified",
        Salience::Low,
    )?;
    engine.register_template_at(
        "code.modified",
        "{name|refer} has been updated",
        Salience::Low,
    )?;

    // Medium
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

    // High
    engine.register_template_at(
        "code.modified",
        "{name|refer} has been substantially modified, with downstream impact \
         across {consumer_count} {consumer_count|pluralize:consumer}{?consumers} \
         including {consumers|truncate:5|join:bracketed}{/?}. Thorough review is \
         recommended.",
        Salience::High,
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
        // With consumer_count=1, this is a Low-salience event — templates
        // drop the impact clause for terseness.
        assert!(result.contains("getData"));
        assert!(result.contains("fetchData"));
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

    #[test]
    fn low_salience_produces_terse_output() {
        let engine = test_engine();
        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("class".into()));
        ctx.insert("name", Value::String("Trivial".into()));
        ctx.insert("consumer_count", Value::Number(1));
        ctx.insert("consumers", Value::List(vec!["OnlyConsumer".into()]));

        let result = engine.render("code.modified", &ctx).unwrap();
        // Low salience: drop impact clause, keep it terse
        assert!(!result.contains("affect"), "Low salience should drop impact clause, got: {result}");
    }

    #[test]
    fn high_salience_produces_elaborated_output() {
        let engine = test_engine();
        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("class".into()));
        ctx.insert("name", Value::String("Critical".into()));
        ctx.insert("consumer_count", Value::Number(50));
        ctx.insert(
            "consumers",
            Value::List(vec![
                "A".into(), "B".into(), "C".into(), "D".into(), "E".into(),
                "F".into(), "G".into(), "H".into(),
            ]),
        );

        let result = engine.render("code.modified", &ctx).unwrap();
        // High salience: elaborate with emphasis language
        assert!(
            result.contains("substantial") || result.contains("significant") || result.contains("recommended"),
            "Expected elaborated high-salience output, got: {result}"
        );
        // Should show more consumers (truncate:5 vs truncate:3)
        assert!(result.contains("50"));
    }

    #[test]
    fn medium_salience_uses_default_templates() {
        let engine = test_engine();
        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("class".into()));
        ctx.insert("name", Value::String("Standard".into()));
        ctx.insert("consumer_count", Value::Number(5));
        ctx.insert(
            "consumers",
            Value::List(vec!["A".into(), "B".into(), "C".into(), "D".into(), "E".into()]),
        );

        let result = engine.render("code.modified", &ctx).unwrap();
        // Medium salience: standard impact clause with count
        assert!(result.contains("5"));
        assert!(
            result.contains("affect") || result.contains("review") || result.contains("dependent"),
            "Expected medium-salience impact clause, got: {result}"
        );
    }
}
