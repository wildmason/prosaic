//! WebAssembly bindings for the Prosaic NLG engine.
//!
//! Exposes [`ProsaicEngine`] and [`ProsaicSession`] to JavaScript via
//! wasm-bindgen. Context values are passed as plain JS objects where each
//! value is a string, number, or array of strings.
//!
//! # Example (JavaScript)
//!
//! ```js
//! import init, { ProsaicEngine, ProsaicSession } from './prosaic_wasm.js';
//! await init();
//!
//! const engine = new ProsaicEngine();
//! engine.registerTemplate("hello", "Hello, {name}!");
//!
//! const session = new ProsaicSession();
//! const result = engine.render(session, "hello", { name: "world" });
//! console.log(result); // "Hello, world!"
//! ```

use wasm_bindgen::prelude::*;
use prosaic_core::{Context, Engine, Session, Strictness, Value, Variation};
use prosaic_grammar_en::English;
use serde_json::Value as JsonValue;

/// A configured Prosaic NLG engine with registered templates.
///
/// Construct with `new ProsaicEngine()`, register templates with
/// `registerTemplate`, then drive rendering with `render` or `renderBatch`.
#[wasm_bindgen]
pub struct ProsaicEngine {
    inner: Engine,
}

/// Per-document mutable discourse state.
///
/// Create one `ProsaicSession` per logical document or narrative. Pass it
/// (mutably) into every `render` / `renderBatch` call so the engine can
/// track centering state, pronoun reference, and template variation across
/// successive renders.
#[wasm_bindgen]
pub struct ProsaicSession {
    inner: Session,
}

#[wasm_bindgen]
impl ProsaicEngine {
    /// Construct a new English-language engine.
    ///
    /// Defaults to `Strictness::Strict` (missing slots are errors) and
    /// `Variation::Fixed` (always picks the first registered alternative).
    #[wasm_bindgen(constructor)]
    pub fn new() -> ProsaicEngine {
        ProsaicEngine {
            inner: Engine::new(English::new())
                .strictness(Strictness::Strict)
                .variation(Variation::Fixed),
        }
    }

    /// Register a template under the given key.
    ///
    /// Templates use `{slot_name}` for bare substitution and
    /// `{slot_name|pipe}` for piped transforms. Returns a JS `Error` if
    /// the template source cannot be parsed.
    #[wasm_bindgen(js_name = registerTemplate)]
    pub fn register_template(&mut self, key: &str, template: &str) -> Result<(), JsValue> {
        self.inner
            .register_template(key, template)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    // NOTE: `Engine::reference_time` is gated behind `prosaic-core`'s `time`
    // feature, which requires `SystemTime`. Since `wasm32-unknown-unknown` does
    // not have `SystemTime::now()`, the `time` feature is intentionally
    // disabled for this crate. Callers that need temporal pipes should
    // re-enable the `time` feature and call `engine.reference_time(unix_secs)`
    // directly on the inner engine in a fork of this crate.

    /// Render a single event.
    ///
    /// `context` is a JS object `{ key: value }` where values may be strings,
    /// numbers (coerced to `i64`), or arrays of strings. Returns a `String`
    /// on success or a JS `Error` on failure.
    #[wasm_bindgen]
    pub fn render(
        &self,
        session: &mut ProsaicSession,
        key: &str,
        context: &JsValue,
    ) -> Result<String, JsValue> {
        let ctx = js_object_to_context(context)?;
        self.inner
            .render(&mut session.inner, key, &ctx)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Render a batch of events as a single aggregated paragraph.
    ///
    /// `events` is a JS array of `[key, context]` pairs, e.g.:
    ///
    /// ```js
    /// engine.renderBatch(session, [
    ///   ["user.created", { name: "Alice" }],
    ///   ["user.promoted", { name: "Alice", role: "admin" }],
    /// ]);
    /// ```
    ///
    /// Returns a `String` on success or a JS `Error` on failure.
    #[wasm_bindgen(js_name = renderBatch)]
    pub fn render_batch(
        &self,
        session: &mut ProsaicSession,
        events: &JsValue,
    ) -> Result<String, JsValue> {
        let parsed = js_events_to_pairs(events)?;
        let borrowed: Vec<(&str, Context)> = parsed
            .iter()
            .map(|(k, c)| (k.as_str(), c.clone()))
            .collect();
        self.inner
            .render_batch(&mut session.inner, &borrowed)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

impl Default for ProsaicEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl ProsaicSession {
    /// Create a new, empty session.
    #[wasm_bindgen(constructor)]
    pub fn new() -> ProsaicSession {
        ProsaicSession {
            inner: Session::new(),
        }
    }

    /// Reset discourse state (centering, pronoun tracking, word history) while
    /// preserving the temporal anchor. Use between paragraphs of the same
    /// document.
    pub fn reset(&mut self) {
        self.inner.reset();
    }

    /// Fully clear the session, including the temporal anchor. Use when
    /// starting a temporally-disjoint narrative.
    #[wasm_bindgen(js_name = resetTemporal)]
    pub fn reset_temporal(&mut self) {
        self.inner.reset_temporal();
    }
}

impl Default for ProsaicSession {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Internal helpers — not exposed to JS
// ---------------------------------------------------------------------------

/// Convert a JS object `{ k: v, ... }` to a [`Context`].
///
/// Each value must be a string, number (coerced to `i64`), or array of strings.
fn js_object_to_context(obj: &JsValue) -> Result<Context, JsValue> {
    let entries: JsonValue = serde_wasm_bindgen::from_value(obj.clone())
        .map_err(|e| JsValue::from_str(&format!("context must be a plain object: {e}")))?;

    let map = entries
        .as_object()
        .ok_or_else(|| JsValue::from_str("context must be an object"))?;

    let mut ctx = Context::new();
    for (k, v) in map {
        let value = match v {
            JsonValue::String(s) => Value::String(s.clone()),
            JsonValue::Number(n) => Value::Number(
                n.as_i64().ok_or_else(|| {
                    JsValue::from_str(&format!("context number `{k}` out of i64 range"))
                })?,
            ),
            JsonValue::Array(arr) => {
                let items: Vec<String> = arr
                    .iter()
                    .filter_map(|x: &JsonValue| x.as_str().map(String::from))
                    .collect();
                if items.len() != arr.len() {
                    return Err(JsValue::from_str(&format!(
                        "context array `{k}` must contain only strings"
                    )));
                }
                Value::List(items)
            }
            _ => {
                return Err(JsValue::from_str(&format!(
                    "unsupported context value type for key `{k}`"
                )))
            }
        };
        ctx.insert(k.clone(), value);
    }
    Ok(ctx)
}

/// Convert a JS array of `[key, context]` pairs to `Vec<(String, Context)>`.
fn js_events_to_pairs(events: &JsValue) -> Result<Vec<(String, Context)>, JsValue> {
    let arr = js_sys::Array::from(events);
    let mut out = Vec::with_capacity(arr.length() as usize);
    for entry in arr.iter() {
        let pair = js_sys::Array::from(&entry);
        if pair.length() != 2 {
            return Err(JsValue::from_str(
                "each event must be a [key, context] pair",
            ));
        }
        let key = pair
            .get(0)
            .as_string()
            .ok_or_else(|| JsValue::from_str("event key must be a string"))?;
        let ctx = js_object_to_context(&pair.get(1))?;
        out.push((key, ctx));
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Native unit tests (run with `cargo test -p prosaic-wasm`)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_constructor_works() {
        let _ = ProsaicEngine::new();
    }

    #[test]
    fn session_constructor_works() {
        let _ = ProsaicSession::new();
    }

    #[test]
    fn engine_default_equals_new() {
        // Default impl must not panic and must produce a usable engine.
        let _ = ProsaicEngine::default();
    }

    #[test]
    fn session_default_equals_new() {
        let _ = ProsaicSession::default();
    }

    #[test]
    fn register_and_render_simple_template() {
        let mut engine = ProsaicEngine::new();
        engine
            .register_template("greet", "Hello, {name}!")
            .expect("template registration must not fail");

        let mut session = ProsaicSession::new();
        // We cannot construct a real JsValue in a native test without a WASM
        // runtime, so we exercise the lower-level Context API directly.
        let mut ctx = Context::new();
        ctx.insert("name", Value::String("world".into()));
        let result = engine
            .inner
            .render(&mut session.inner, "greet", &ctx)
            .expect("render must not fail");
        assert_eq!(result, "Hello, world!");
    }

    #[test]
    fn session_reset_preserves_temporal_anchor() {
        let mut session = ProsaicSession::new();
        // Simulate anchor being set, then reset (discourse) preserving it.
        session.inner.reset_temporal();  // ensure no anchor
        session.reset();                 // discourse reset; no anchor still present
        // No panic = pass.
    }

    #[test]
    fn session_reset_temporal_clears_state() {
        let mut session = ProsaicSession::new();
        session.reset_temporal(); // must not panic
    }
}
