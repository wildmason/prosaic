# Plan: `nlg-tracing` — Tracing-to-Prose Bridge

**Owner:** sonnet agent
**Scope:** new workspace member crate `nlg-tracing/`
**Estimated size:** ~300–400 LOC including tests
**Test gate:** all 526 existing tests pass; new tests add ~10–15; zero warnings
**Branch discipline:** local only, 1 commit

---

## Why

The `tracing` crate is Rust's dominant structured observability framework — used by tokio, axum, tonic, and virtually every async backend. Spans and events carry typed fields (key-value pairs) that are currently consumed by log formatters (tracing-subscriber, tracing-bunyan-formatter) into human-readable but mechanical log lines.

`nlg-tracing` bridges these structured fields into the NLG engine, producing prose narratives from tracing events. The value proposition:

1. **Alert narratives:** `tracing::warn!(user_id = 42, action = "login_failed", reason = "invalid_token", attempts = 5)` → *"User 42 failed to log in after 5 attempts due to an invalid token."*
2. **Incident summaries:** Collect a span's child events into a `DocumentPlan` → *"The request to /api/users experienced 3 retries before timing out. The auth check passed but the database connection was refused."*
3. **Engineering digests:** Aggregate hourly tracing output into readable summaries.

No existing crate fills this niche. First-mover advantage in the tracing ecosystem.

## Design (locked)

### Architecture

`nlg-tracing` does NOT replace tracing-subscriber. It is a **Layer** (implements `tracing_subscriber::Layer<S>`) that intercepts events and spans, converts their fields into `(template_key, Context)` pairs, and feeds them to an `Engine`. Output goes to a configurable sink (writer).

### Dependencies

```toml
[dependencies]
nlg-core = { path = "../nlg-core" }
nlg-grammar-en = { path = "../nlg-grammar-en" }
tracing = "0.1"
tracing-subscriber = { version = "0.3", default-features = false, features = ["registry"] }
```

Dev-deps: `tracing-subscriber` with `fmt` feature for test setup.

### Core type: `NlgLayer`

```rust
pub struct NlgLayer<W: Write + Send + 'static> {
    engine: Engine,
    session: Mutex<Session>,
    writer: Mutex<W>,
    key_mapper: Box<dyn Fn(&tracing::Metadata<'_>) -> String + Send + Sync>,
}
```

The `Mutex<Session>` is necessary because `Layer::on_event` is called from potentially concurrent threads. The session mutex is the ONLY interior mutability; the engine is immutable (`Send + Sync`).

### Key mapper

The `key_mapper` converts a tracing event's metadata into a template key. Default implementation: `"{target}.{name}"` → e.g., `"auth.login_failed"`, `"db.query_slow"`.

Users can override with a custom closure: `NlgLayer::new(engine, writer).key_mapper(|meta| format!("app.{}", meta.name()))`.

### Field-to-Context conversion

Tracing fields are visited via `tracing::field::Visit`. The visitor maps:
- `record_str(field, value)` → `ctx.insert(field.name(), Value::String(value.into()))`
- `record_i64(field, value)` → `ctx.insert(field.name(), Value::Number(value))`
- `record_u64(field, value)` → `ctx.insert(field.name(), Value::Number(value as i64))`
- `record_bool(field, value)` → `ctx.insert(field.name(), Value::Number(if value { 1 } else { 0 }))`
- `record_debug(field, value)` → `ctx.insert(field.name(), Value::String(format!("{value:?}")))`

This mirrors the existing `context_from_slots` in `nlg-cli/src/main.rs`.

### Layer implementation

```rust
impl<S, W> tracing_subscriber::Layer<S> for NlgLayer<W>
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

        // Check if the engine has a template for this key. If not, skip.
        // (Quiet fallthrough — not every tracing event needs prose.)
        if !self.engine.has_template(&key) {
            return;
        }

        let mut nlg_ctx = nlg_core::Context::new();
        let mut visitor = FieldVisitor { ctx: &mut nlg_ctx };
        event.record(&mut visitor);

        let mut session = self.session.lock().unwrap();
        match self.engine.render(&mut session, &key, &nlg_ctx) {
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
```

### `Engine::has_template` helper

Needs a quick `pub fn has_template(&self, key: &str) -> bool` on `Engine`. Pure read, no mutation. Add to `nlg-core/src/engine.rs`.

### Builder API

```rust
impl<W: Write + Send + 'static> NlgLayer<W> {
    pub fn new(engine: Engine, writer: W) -> Self { ... }
    pub fn key_mapper(mut self, f: impl Fn(&tracing::Metadata<'_>) -> String + Send + Sync + 'static) -> Self { ... }
}
```

### Usage example (for docs/tests)

```rust
use nlg_core::{Engine, Strictness};
use nlg_grammar_en::English;
use nlg_tracing::NlgLayer;
use tracing_subscriber::prelude::*;

let mut engine = Engine::new(English::new()).strictness(Strictness::Silent);
engine.register_template(
    "auth.login_failed",
    "User {user_id} failed to log in{?reason} due to {reason}{/?}{?attempts} after {attempts} {attempts|pluralize:attempt}{/?}",
).unwrap();

let buf = std::sync::Arc::new(std::sync::Mutex::new(Vec::<u8>::new()));
let layer = NlgLayer::new(engine, SharedWriter(buf.clone()));
tracing_subscriber::registry().with(layer).init();

tracing::warn!(
    target: "auth",
    name: "login_failed",
    user_id = 42,
    reason = "invalid_token",
    attempts = 5,
);

let output = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
assert!(output.contains("User 42"));
assert!(output.contains("invalid_token"));
```

### Out of scope for v1

- **Span aggregation.** v1 handles events only. Collecting a span's child events into a DocumentPlan (incident summaries) is v2.
- **Log-level-to-salience mapping.** Could map `WARN → Medium`, `ERROR → High`, `INFO → Low` — nice but not v1.
- **Pre-built tracing vocabulary crate.** Users register their own templates. A future `nlg-vocab-tracing` could ship common patterns.
- **Async writer.** Synchronous writes only — tracing events arrive on the calling thread's sync path.
- **`no_std` support.** `tracing` itself requires `std`.

---

## Phase 0 — Baseline

```bash
cargo test --all-features
cargo clippy --all-features -- -D warnings
```

Expected: **526 tests passing**, 0 warnings.

**No commit.**

---

## Phase 1 — Add `Engine::has_template` helper to `nlg-core`

**File:** `nlg-core/src/engine.rs`

```rust
/// Check whether a template is registered under the given key.
/// Pure read, no mutation. Useful for callers that want to skip
/// rendering when no template matches (e.g. tracing bridges where
/// not every event type has a registered template).
pub fn has_template(&self, key: &str) -> bool {
    self.templates.contains_key(key)
}
```

Add a small test:

```rust
#[test]
fn has_template_returns_true_for_registered() {
    let mut engine = test_engine();
    engine.register_template("t", "hello").unwrap();
    assert!(engine.has_template("t"));
    assert!(!engine.has_template("nope"));
}
```

Verify: `cargo test --all-features && cargo clippy --all-features -- -D warnings`

This is a tiny addition to nlg-core that MUST land before the nlg-tracing crate can compile. Include it in the same commit as the crate.

---

## Phase 2 — Scaffold `nlg-tracing` crate

### 2.1 Crate files

```
nlg-tracing/
├── Cargo.toml
└── src/
    └── lib.rs
```

`nlg-tracing/Cargo.toml`:

```toml
[package]
name = "nlg-tracing"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "Bridge tracing events to natural language prose via the nlg engine"

[dependencies]
nlg-core = { path = "../nlg-core" }
nlg-grammar-en = { path = "../nlg-grammar-en" }
tracing = "0.1"
tracing-subscriber = { version = "0.3", default-features = false, features = ["registry"] }

[dev-dependencies]
tracing-subscriber = { version = "0.3", features = ["fmt", "registry"] }
```

### 2.2 Workspace Cargo.toml

Add `"nlg-tracing"` to `[workspace.members]`.

### 2.3 Implement `lib.rs`

Full implementation:

- `NlgLayer<W>` struct
- `FieldVisitor` that implements `tracing::field::Visit`
- `Layer<S>` impl with `on_event`
- Builder with `key_mapper`
- Default key mapper: `format!("{}.{}", meta.target(), meta.name())`

### 2.4 Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use nlg_core::{Engine, Session, Strictness, Value, Variation};
    use nlg_grammar_en::English;
    use std::sync::{Arc, Mutex};
    use tracing_subscriber::prelude::*;

    /// A shared writer for test assertions.
    #[derive(Clone)]
    struct TestWriter(Arc<Mutex<Vec<u8>>>);

    impl std::io::Write for TestWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().write(buf)
        }
        fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
    }

    fn test_output(writer: &TestWriter) -> String {
        String::from_utf8(writer.0.lock().unwrap().clone()).unwrap()
    }

    #[test]
    fn renders_event_with_matching_template() {
        let mut engine = Engine::new(English::new())
            .strictness(Strictness::Silent)
            .variation(Variation::Fixed);
        engine.register_template("test_target.test_event", "User {user_id} did {action}").unwrap();

        let buf = TestWriter(Arc::new(Mutex::new(Vec::new())));
        let layer = NlgLayer::new(engine, buf.clone());

        let subscriber = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(target: "test_target", name: "test_event", user_id = 42, action = "login");
        });

        let out = test_output(&buf);
        assert!(out.contains("User 42"), "got: {out}");
        assert!(out.contains("login"), "got: {out}");
    }

    #[test]
    fn silently_skips_events_without_matching_template() {
        let engine = Engine::new(English::new());
        let buf = TestWriter(Arc::new(Mutex::new(Vec::new())));
        let layer = NlgLayer::new(engine, buf.clone());

        let subscriber = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(target: "unknown", name: "no_template", foo = "bar");
        });

        let out = test_output(&buf);
        assert!(out.is_empty(), "should skip: {out}");
    }

    #[test]
    fn custom_key_mapper() {
        let mut engine = Engine::new(English::new())
            .strictness(Strictness::Silent)
            .variation(Variation::Fixed);
        engine.register_template("custom.key", "Mapped: {val}").unwrap();

        let buf = TestWriter(Arc::new(Mutex::new(Vec::new())));
        let layer = NlgLayer::new(engine, buf.clone())
            .key_mapper(|_meta| "custom.key".to_string());

        let subscriber = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(target: "anything", name: "whatever", val = "hello");
        });

        let out = test_output(&buf);
        assert!(out.contains("Mapped: hello"), "got: {out}");
    }

    #[test]
    fn maps_i64_and_bool_fields() {
        let mut engine = Engine::new(English::new())
            .strictness(Strictness::Silent)
            .variation(Variation::Fixed);
        engine.register_template("t.e", "{count} {count|pluralize:item}, active: {active}").unwrap();

        let buf = TestWriter(Arc::new(Mutex::new(Vec::new())));
        let layer = NlgLayer::new(engine, buf.clone())
            .key_mapper(|_| "t.e".to_string());

        let subscriber = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(target: "t", name: "e", count = 3, active = true);
        });

        let out = test_output(&buf);
        assert!(out.contains("3 items"), "got: {out}");
    }
}
```

**Note on tracing test isolation:** tracing's global subscriber can only be set once per process. Use `tracing::subscriber::with_default(subscriber, || { ... })` to scope each test to its own subscriber. This avoids test-ordering interference.

### 2.5 Verify

```bash
cargo test -p nlg-tracing
cargo test --all-features
cargo test --no-default-features
cargo test --features serde
cargo clippy --all-features -- -D warnings
cargo doc --all-features --no-deps
```

**Commit:** `Add nlg-tracing crate bridging tracing events to prose narratives`

(Include the `Engine::has_template` addition in this same commit — it's a prerequisite.)

---

## Risk register

| Risk | Mitigation |
|---|---|
| `Mutex<Session>` on the tracing hot path adds contention | v1 accepts this. The session lock is held only for the duration of one `engine.render` call (~16 µs per bench). For high-throughput tracing, users can scope the layer to specific targets via tracing-subscriber's `filter` combinators. |
| `tracing::Event` field visitation order is undefined | The visitor accumulates into a `Context` (HashMap-like); order doesn't matter for template rendering. |
| Default key mapper produces keys like `"auth.login_failed"` that don't match user-registered templates with different naming | Documented. Users must match their registered template keys to their tracing target+name convention, or supply a custom key mapper. |
| `tracing` 0.1 vs newer versions | `tracing` 0.1 is the current stable release (unchanged since 2019; intentionally stable). Safe dep. |
| `tracing_subscriber` brings in a dep tree | Gated to `default-features = false, features = ["registry"]` to minimize. The `registry` feature is the minimum needed for `Layer` support. |
| `on_event` silently swallows render errors | Correct — tracing paths must never panic or propagate errors. Errors are already logged at the `NlgError` level; the tracing consumer doesn't see them. Matches tracing-subscriber's own convention. |
| Test isolation — `tracing::subscriber::set_global_default` called multiple times | Use `with_default` (scoped, not global). Each test gets its own subscriber. |
| `Engine::has_template` is a new public API on `nlg-core` | Yes — minimal surface (one bool-returning method). No mutation, no new types. |

## What NOT to do

- **Do not** aggregate span events into DocumentPlans. v2.
- **Do not** map log level → salience. v2.
- **Do not** ship a pre-built vocab crate. Users register their own.
- **Do not** use async writers.
- **Do not** amend commits.

## Definition of done

- [ ] Phase 0 baseline clean
- [ ] 1 commit (including `Engine::has_template` in nlg-core + full nlg-tracing crate)
- [ ] `NlgLayer<W>` implements `tracing_subscriber::Layer<S>`
- [ ] `FieldVisitor` maps tracing fields to `Context` values
- [ ] Default key mapper: `{target}.{name}`
- [ ] Custom key mapper via builder
- [ ] `Engine::has_template` added to nlg-core
- [ ] ~4 tests covering: matching template, skip unmatched, custom mapper, field type mapping
- [ ] Workspace `Cargo.toml` includes `nlg-tracing`
- [ ] All existing 526 + ~5 new tests pass
- [ ] `cargo clippy --all-features -- -D warnings` clean
- [ ] No new `unsafe`
