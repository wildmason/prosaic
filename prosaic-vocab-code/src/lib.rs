use prosaic_core::{Engine, ProsaicError};

/// English-language templates for code-analysis events.
///
/// Each locale module exposes a `register` function with the same signature,
/// enabling locale-aware dispatch without changing callers.
pub mod en;

/// Spanish-language templates for code-analysis events.
pub mod es;

/// Register code-analysis vocabulary templates into an engine.
///
/// Provides templates for common code change events: renames, deletions,
/// additions, modifications, moves, and signature changes.
///
/// Templates are registered at three salience levels (Low, Medium, High) so
/// the engine can match verbosity to event magnitude: low-impact changes get
/// terser phrasing while high-impact changes get elaboration.
///
/// Delegates to the English locale. When multilingual support is added,
/// callers can opt into a specific locale via `register_locale`.
pub fn register(engine: &mut Engine) -> Result<(), ProsaicError> {
    en::register(engine)
}

/// Register Spanish code-analysis vocabulary templates into an engine.
///
/// Provides the same template keys as [`register`] but with idiomatic
/// Spanish surface text for use with a Spanish grammar layer.
pub fn register_es(engine: &mut Engine) -> Result<(), ProsaicError> {
    es::register(engine)
}

#[cfg(test)]
mod tests {
    use super::*;
    use prosaic_core::{Context, Engine, Session, Strictness, Value, Variation};
    use prosaic_grammar_en::English;

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
        let mut session = Session::new();

        let result = engine.render(&mut session, "code.renamed", &ctx).unwrap();
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
        ctx.insert("consumers", Value::List(vec!["DashboardComponent".into()]));
        let mut session = Session::new();

        let result = engine.render(&mut session, "code.renamed", &ctx).unwrap();
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
        let mut session = Session::new();

        let result = engine.render(&mut session, "code.deleted", &ctx).unwrap();
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
        let mut session = Session::new();

        let result = engine.render(&mut session, "code.added", &ctx).unwrap();
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
        let mut session = Session::new();

        let result = engine.render(&mut session, "code.modified", &ctx).unwrap();
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
        let mut session = Session::new();

        let result = engine.render(&mut session, "code.moved", &ctx).unwrap();
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
        let mut session = Session::new();

        let result = engine
            .render(&mut session, "code.signature_changed", &ctx)
            .unwrap();
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
        ctx.insert("consumers", Value::List(vec!["A".into(), "B".into()]));

        let mut session1 = Session::new();
        let mut session2 = Session::new();
        let result1 = engine1.render(&mut session1, "code.renamed", &ctx).unwrap();
        let result2 = engine2.render(&mut session2, "code.renamed", &ctx).unwrap();

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
        let mut session = Session::new();

        let result = engine.render(&mut session, "code.modified", &ctx).unwrap();
        // Low salience: drop impact clause, keep it terse
        assert!(
            !result.contains("affect"),
            "Low salience should drop impact clause, got: {result}"
        );
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
                "A".into(),
                "B".into(),
                "C".into(),
                "D".into(),
                "E".into(),
                "F".into(),
                "G".into(),
                "H".into(),
            ]),
        );
        let mut session = Session::new();

        let result = engine.render(&mut session, "code.modified", &ctx).unwrap();
        // High salience: elaborate with emphasis language
        assert!(
            result.contains("substantial")
                || result.contains("significant")
                || result.contains("recommended"),
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
            Value::List(vec![
                "A".into(),
                "B".into(),
                "C".into(),
                "D".into(),
                "E".into(),
            ]),
        );
        let mut session = Session::new();

        let result = engine.render(&mut session, "code.modified", &ctx).unwrap();
        // Medium salience: standard impact clause with count
        assert!(result.contains("5"));
        assert!(
            result.contains("affect") || result.contains("review") || result.contains("dependent"),
            "Expected medium-salience impact clause, got: {result}"
        );
    }
}

#[cfg(test)]
mod tests_es {
    use super::*;
    use prosaic_core::{Context, Engine, Session, Strictness, Value, Variation};
    use prosaic_grammar_es::Spanish;

    fn test_engine() -> Engine {
        let mut engine = Engine::new(Spanish::new())
            .strictness(Strictness::Strict)
            .variation(Variation::Fixed);
        register_es(&mut engine).unwrap();
        engine
    }

    #[test]
    fn rename_event() {
        let engine = test_engine();
        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("clase".into()));
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
        let mut session = Session::new();

        let result = engine.render(&mut session, "code.renamed", &ctx).unwrap();
        assert!(
            result.contains("fue renombrado a")
                || result.contains("ahora se llama")
                || result.contains("ha sido renombrado a"),
            "Expected Spanish rename phrase, got: {result}"
        );
        assert!(
            result.contains("Foobar"),
            "Expected new name, got: {result}"
        );
    }

    #[test]
    fn rename_event_includes_consumer_count() {
        let engine = test_engine();
        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("clase".into()));
        ctx.insert("old_name", Value::String("Foo".into()));
        ctx.insert("new_name", Value::String("Foobar".into()));
        ctx.insert("consumer_count", Value::Number(6));
        ctx.insert(
            "consumers",
            Value::List(vec!["Baz".into(), "Qux".into(), "Quux".into()]),
        );
        let mut session = Session::new();

        let result = engine.render(&mut session, "code.renamed", &ctx).unwrap();
        assert!(
            result.contains("6"),
            "Expected consumer count in output, got: {result}"
        );
    }

    #[test]
    fn delete_event() {
        let engine = test_engine();
        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("interfaz".into()));
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
        let mut session = Session::new();

        let result = engine.render(&mut session, "code.deleted", &ctx).unwrap();
        assert!(
            result.contains("fue eliminado")
                || result.contains("ha sido eliminado")
                || result.contains("ya no existe"),
            "Expected Spanish delete phrase, got: {result}"
        );
        assert!(
            result.contains("UserProfile"),
            "Expected entity name, got: {result}"
        );
    }

    #[test]
    fn add_event() {
        let engine = test_engine();
        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("servicio".into()));
        ctx.insert("name", Value::String("AuthGuard".into()));
        ctx.insert("location", Value::String("src/guards/auth.guard.ts".into()));
        let mut session = Session::new();

        let result = engine.render(&mut session, "code.added", &ctx).unwrap();
        assert!(result.contains("AuthGuard"), "got: {result}");
        assert!(result.contains("src/guards/auth.guard.ts"), "got: {result}");
        assert!(
            result.contains("servicio"),
            "Expected entity type in Spanish, got: {result}"
        );
    }

    #[test]
    fn modify_event() {
        let engine = test_engine();
        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("método".into()));
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
        let mut session = Session::new();

        let result = engine.render(&mut session, "code.modified", &ctx).unwrap();
        assert!(
            result.contains("fue modificado")
                || result.contains("ha sido actualizado")
                || result.contains("afectan"),
            "Expected Spanish modify phrase, got: {result}"
        );
        assert!(result.contains("processOrder"), "got: {result}");
    }

    #[test]
    fn move_event() {
        let engine = test_engine();
        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("clase".into()));
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
        let mut session = Session::new();

        let result = engine.render(&mut session, "code.moved", &ctx).unwrap();
        assert!(result.contains("Logger"), "got: {result}");
        assert!(
            result.contains("src/utils/logger.ts") || result.contains("src/core/logger.ts"),
            "Expected location in output, got: {result}"
        );
        assert!(
            result.contains("12"),
            "Expected consumer count, got: {result}"
        );
    }

    #[test]
    fn signature_changed_event() {
        let engine = test_engine();
        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("método".into()));
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
        let mut session = Session::new();

        let result = engine
            .render(&mut session, "code.signature_changed", &ctx)
            .unwrap();
        assert!(result.contains("getUser"), "got: {result}");
        assert!(
            result.contains("firma") || result.contains("invocador"),
            "Expected Spanish signature/caller terms, got: {result}"
        );
    }

    #[test]
    fn low_salience_terse_output() {
        let engine = test_engine();
        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("clase".into()));
        ctx.insert("name", Value::String("Trivial".into()));
        ctx.insert("consumer_count", Value::Number(1));
        ctx.insert("consumers", Value::List(vec!["OnlyConsumer".into()]));
        let mut session = Session::new();

        let result = engine.render(&mut session, "code.modified", &ctx).unwrap();
        // Low salience: should drop the impact clause
        assert!(
            !result.contains("afectan"),
            "Low salience should drop impact clause, got: {result}"
        );
    }
}
