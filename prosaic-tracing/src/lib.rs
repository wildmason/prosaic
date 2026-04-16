//! Bridge `tracing` events to natural language prose via the prosaic engine.
//!
//! `prosaic-tracing` provides [`ProsaicLayer`], a [`tracing_subscriber::Layer`] that
//! intercepts tracing events, converts their fields into an [`prosaic_core::Context`],
//! and renders prose via a registered [`prosaic_core::Engine`].
//!
//! # Example
//!
//! ```rust
//! use prosaic_core::{Engine, Strictness};
//! use prosaic_grammar_en::English;
//! use prosaic_tracing::ProsaicLayer;
//! use tracing_subscriber::prelude::*;
//!
//! let mut engine = Engine::new(English::new()).strictness(Strictness::Silent);
//! engine
//!     .register_template(
//!         "auth.login_failed",
//!         "User {user_id} failed to log in after {attempts} attempts.",
//!     )
//!     .unwrap();
//!
//! let buf: Vec<u8> = Vec::new();
//! let layer = ProsaicLayer::new(engine, buf);
//! let subscriber = tracing_subscriber::registry().with(layer);
//! tracing::subscriber::with_default(subscriber, || {
//!     tracing::warn!(
//!         name: "login_failed",
//!         target: "auth",
//!         user_id = 42,
//!         attempts = 5,
//!     );
//! });
//! ```

use std::io::Write;
use std::sync::Mutex;

use prosaic_core::{Context, Engine, Session, Value};

/// A [`tracing_subscriber::Layer`] that renders matching tracing events as
/// natural language prose via an [`Engine`].
///
/// Events are matched to registered templates using a *key mapper* — a
/// function from [`tracing::Metadata`] to `String`. The default mapper
/// produces `"{target}.{name}"` (e.g. `"auth.login_failed"`).
///
/// If the engine has no template for the derived key the event is silently
/// skipped — not every tracing event needs prose output.
///
/// Rendered prose is written to the configured writer, one line per event.
pub struct ProsaicLayer<W: Write + Send + 'static> {
    engine: Engine,
    session: Mutex<Session>,
    writer: Mutex<W>,
    key_mapper: Box<dyn Fn(&tracing::Metadata<'_>) -> String + Send + Sync>,
}

impl<W: Write + Send + 'static> ProsaicLayer<W> {
    /// Create a new [`ProsaicLayer`] with the given engine and writer.
    ///
    /// The default key mapper derives the template key as
    /// `"{target}.{name}"` from the event metadata.
    pub fn new(engine: Engine, writer: W) -> Self {
        Self {
            engine,
            session: Mutex::new(Session::new()),
            writer: Mutex::new(writer),
            key_mapper: Box::new(|meta| format!("{}.{}", meta.target(), meta.name())),
        }
    }

    /// Override the key mapper used to derive the template key from event
    /// metadata.
    ///
    /// The closure receives a reference to the event's [`tracing::Metadata`]
    /// and returns the template key string. Use this when your registered
    /// template keys don't follow the default `"{target}.{name}"` convention.
    pub fn key_mapper(
        mut self,
        f: impl Fn(&tracing::Metadata<'_>) -> String + Send + Sync + 'static,
    ) -> Self {
        self.key_mapper = Box::new(f);
        self
    }
}

/// Visits tracing event fields and populates an [`prosaic_core::Context`].
struct FieldVisitor<'a> {
    ctx: &'a mut Context,
}

impl tracing::field::Visit for FieldVisitor<'_> {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.ctx
            .insert(field.name(), Value::String(value.to_owned()));
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.ctx.insert(field.name(), Value::Number(value));
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        // Saturating cast: values above i64::MAX are clamped rather than
        // wrapping. In practice tracing numeric fields are small counters.
        self.ctx
            .insert(field.name(), Value::Number(value.min(i64::MAX as u64) as i64));
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.ctx
            .insert(field.name(), Value::Number(if value { 1 } else { 0 }));
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.ctx
            .insert(field.name(), Value::String(format!("{value:?}")));
    }
}

impl<S, W> tracing_subscriber::Layer<S> for ProsaicLayer<W>
where
    S: tracing::Subscriber,
    W: Write + Send + 'static,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let key = (self.key_mapper)(event.metadata());

        // Quiet fallthrough — not every tracing event needs prose.
        if !self.engine.has_template(&key) {
            return;
        }

        let mut prosaic_ctx = Context::new();
        let mut visitor = FieldVisitor { ctx: &mut prosaic_ctx };
        event.record(&mut visitor);

        let mut session = self.session.lock().unwrap();
        match self.engine.render(&mut session, &key, &prosaic_ctx) {
            Ok(prose) => {
                let mut w = self.writer.lock().unwrap();
                let _ = writeln!(w, "{prose}");
            }
            Err(_) => {
                // Silently skip render failures — tracing paths must not panic.
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prosaic_core::{Strictness, Variation};
    use prosaic_grammar_en::English;
    use std::sync::{Arc, Mutex};
    use tracing_subscriber::prelude::*;

    /// A clonable shared writer backed by a `Vec<u8>` for test assertions.
    #[derive(Clone)]
    struct TestWriter(Arc<Mutex<Vec<u8>>>);

    impl Write for TestWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().write(buf)
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl TestWriter {
        fn new() -> Self {
            Self(Arc::new(Mutex::new(Vec::new())))
        }

        fn output(&self) -> String {
            String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
        }
    }

    #[test]
    fn renders_event_with_matching_template() {
        let mut engine = Engine::new(English::new())
            .strictness(Strictness::Silent)
            .variation(Variation::Fixed);
        engine
            .register_template("test_target.test_event", "User {user_id} did {action}")
            .unwrap();

        let buf = TestWriter::new();
        let layer = ProsaicLayer::new(engine, buf.clone());

        let subscriber = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(name: "test_event", target: "test_target", user_id = 42, action = "login");
        });

        let out = buf.output();
        assert!(out.contains("User 42"), "got: {out}");
        assert!(out.contains("login"), "got: {out}");
    }

    #[test]
    fn silently_skips_events_without_matching_template() {
        let engine = Engine::new(English::new());
        let buf = TestWriter::new();
        let layer = ProsaicLayer::new(engine, buf.clone());

        let subscriber = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(name: "no_template", target: "unknown", foo = "bar");
        });

        let out = buf.output();
        assert!(out.is_empty(), "should skip unmatched events, got: {out}");
    }

    #[test]
    fn custom_key_mapper_overrides_default() {
        let mut engine = Engine::new(English::new())
            .strictness(Strictness::Silent)
            .variation(Variation::Fixed);
        engine
            .register_template("custom.key", "Mapped: {val}")
            .unwrap();

        let buf = TestWriter::new();
        let layer = ProsaicLayer::new(engine, buf.clone()).key_mapper(|_meta| "custom.key".to_string());

        let subscriber = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(name: "whatever", target: "anything", val = "hello");
        });

        let out = buf.output();
        assert!(out.contains("Mapped: hello"), "got: {out}");
    }

    #[test]
    fn maps_numeric_and_bool_fields() {
        let mut engine = Engine::new(English::new())
            .strictness(Strictness::Silent)
            .variation(Variation::Fixed);
        engine
            .register_template("t.e", "{count} {count|pluralize:item}, active: {active}")
            .unwrap();

        let buf = TestWriter::new();
        let layer =
            ProsaicLayer::new(engine, buf.clone()).key_mapper(|_| "t.e".to_string());

        let subscriber = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(name: "e", target: "t", count = 3i64, active = true);
        });

        let out = buf.output();
        assert!(out.contains("3 items"), "got: {out}");
    }

    #[test]
    fn u64_field_saturates_at_i64_max() {
        let mut engine = Engine::new(English::new())
            .strictness(Strictness::Silent)
            .variation(Variation::Fixed);
        engine
            .register_template("t.big", "value: {n}")
            .unwrap();

        let buf = TestWriter::new();
        let layer = ProsaicLayer::new(engine, buf.clone()).key_mapper(|_| "t.big".to_string());

        let subscriber = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(subscriber, || {
            // u64 value that exceeds i64::MAX — must not panic or wrap.
            tracing::info!(name: "big", target: "t", n = u64::MAX);
        });

        let out = buf.output();
        assert!(out.contains("value:"), "got: {out}");
    }
}
