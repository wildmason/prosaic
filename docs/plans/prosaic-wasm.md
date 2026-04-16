# Plan: `prosaic-wasm` member crate

**Owner:** sonnet agent
**Scope:** New workspace member `prosaic-wasm/` providing wasm-bindgen bindings
**Estimated size:** ~250–350 LOC including tests
**Test gate:** Existing 965 tests still pass; new crate compiles for `wasm32-unknown-unknown` and passes a handful of in-crate unit tests on native target
**Branch discipline:** local only, one commit

---

## Why

Prosaic's deterministic render pipeline is an excellent fit for client-side use: changelog rendering, live-preview pages, embeddable docs. A WASM binding lets JS/TS code drive the engine without a server round-trip.

We already laid the groundwork in task 7 (`no_std + alloc`) — the library compiles without `std`. WASM targets can compile either with or without `std`; we'll use `std` on wasm because `wasm32-unknown-unknown` supports it (just not SystemTime by default, which we've gated).

## Design

### Crate layout

```
prosaic-wasm/
├── Cargo.toml
└── src/
    └── lib.rs
```

### Cargo.toml

```toml
[package]
name = "prosaic-wasm"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "WebAssembly bindings for the Prosaic NLG engine"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
prosaic-core = { path = "../prosaic-core", default-features = false, features = ["std"] }
prosaic-grammar-en = { path = "../prosaic-grammar-en" }
wasm-bindgen = "0.2"
serde = { version = "1", features = ["derive"] }
serde-wasm-bindgen = "0.6"
js-sys = "0.3"

[dev-dependencies]
wasm-bindgen-test = "0.3"
```

Note: `prosaic-core/time` feature is off because `SystemTime::now()` doesn't work on vanilla `wasm32-unknown-unknown`. Callers who need the `relative` pipe pass `engine.reference_time(unix_secs)` explicitly. We keep `polish` and `reg` off for a lean WASM binary; callers who need them can fork.

Actually — let's keep polish and reg on for feature completeness. WASM size is secondary to feature parity.

```toml
prosaic-core = { path = "../prosaic-core", default-features = false, features = ["std", "polish", "reg"] }
```

### API surface

```rust
use wasm_bindgen::prelude::*;
use prosaic_core::{Engine, Session, Context, Value, Strictness, Variation};
use prosaic_grammar_en::English;

#[wasm_bindgen]
pub struct ProsaicEngine {
    inner: Engine,
}

#[wasm_bindgen]
pub struct ProsaicSession {
    inner: Session,
}

#[wasm_bindgen]
impl ProsaicEngine {
    /// Construct a new English-language engine.
    #[wasm_bindgen(constructor)]
    pub fn new() -> ProsaicEngine {
        ProsaicEngine {
            inner: Engine::new(English::new())
                .strictness(Strictness::Strict)
                .variation(Variation::Fixed),
        }
    }

    /// Register a template under the given key.
    #[wasm_bindgen(js_name = registerTemplate)]
    pub fn register_template(&mut self, key: &str, template: &str) -> Result<(), JsValue> {
        self.inner
            .register_template(key, template)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Set a reference time for `{ts|relative}` / `{ts|since_last}` pipes.
    #[wasm_bindgen(js_name = referenceTime)]
    pub fn set_reference_time(&mut self, unix_secs: i64) {
        // Consume-and-replace: Engine::reference_time takes self by value.
        let old = std::mem::replace(
            &mut self.inner,
            Engine::new(English::new())
                .strictness(Strictness::Strict)
                .variation(Variation::Fixed),
        );
        self.inner = old.reference_time(unix_secs);
    }

    /// Render a single event.
    ///
    /// `context` is a JS object `{ key: value }`; values may be strings,
    /// numbers, or arrays of strings.
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

    /// Render a batch of events as a single paragraph.
    ///
    /// `events` is a JS array of `[key, context]` pairs.
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

#[wasm_bindgen]
impl ProsaicSession {
    #[wasm_bindgen(constructor)]
    pub fn new() -> ProsaicSession {
        ProsaicSession { inner: Session::new() }
    }

    /// Reset discourse state (but preserve temporal anchor).
    pub fn reset(&mut self) {
        self.inner.reset();
    }

    /// Fully clear the session, including temporal anchor.
    #[wasm_bindgen(js_name = resetTemporal)]
    pub fn reset_temporal(&mut self) {
        self.inner.reset_temporal();
    }
}

fn js_object_to_context(obj: &JsValue) -> Result<Context, JsValue> {
    // Expect an object: { k1: v1, k2: v2, ... }.
    // v may be string, number, or string array.
    let entries: serde_json::Value = serde_wasm_bindgen::from_value(obj.clone())
        .map_err(|e| JsValue::from_str(&format!("context must be a plain object: {e}")))?;

    let map = entries.as_object().ok_or_else(|| JsValue::from_str("context must be an object"))?;

    let mut ctx = Context::new();
    for (k, v) in map {
        let value = match v {
            serde_json::Value::String(s) => Value::String(s.clone()),
            serde_json::Value::Number(n) => {
                Value::Number(n.as_i64().ok_or_else(|| JsValue::from_str(
                    &format!("context number `{k}` out of i64 range")
                ))?)
            }
            serde_json::Value::Array(arr) => {
                let items: Vec<String> = arr.iter().filter_map(|x| x.as_str().map(String::from)).collect();
                if items.len() != arr.len() {
                    return Err(JsValue::from_str(
                        &format!("context array `{k}` must be all strings"),
                    ));
                }
                Value::List(items)
            }
            _ => return Err(JsValue::from_str(
                &format!("unsupported context value type for `{k}`"),
            )),
        };
        ctx.insert(k.as_str(), value);
    }
    Ok(ctx)
}

fn js_events_to_pairs(events: &JsValue) -> Result<Vec<(String, Context)>, JsValue> {
    let arr = js_sys::Array::from(events);
    let mut out = Vec::with_capacity(arr.length() as usize);
    for entry in arr.iter() {
        let pair = js_sys::Array::from(&entry);
        if pair.length() != 2 {
            return Err(JsValue::from_str("each event must be a [key, context] pair"));
        }
        let key = pair.get(0).as_string().ok_or_else(|| {
            JsValue::from_str("event key must be a string")
        })?;
        let ctx = js_object_to_context(&pair.get(1))?;
        out.push((key, ctx));
    }
    Ok(out)
}
```

### Workspace addition

Add `prosaic-wasm` to `Cargo.toml` members list in alphabetical order (after `prosaic-vocab-release`, before `prosaic-tracing`).

### Testing strategy

We cannot easily run wasm-bindgen-test inside CI without headless browser setup. For v1:

1. Add **unit tests** (non-wasm) that exercise `js_object_to_context` using a mocked `JsValue` — or skip, since `serde_wasm_bindgen` is already well-tested upstream.
2. Add **native unit tests** that construct ProsaicEngine and ProsaicSession directly (they wrap Engine/Session, which are testable).
3. Verify the crate compiles under `cargo check -p prosaic-wasm --target wasm32-unknown-unknown` IF the target is installed. Document this in README.

For minimal verification:

```rust
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
}
```

### Out of scope

- **DocumentPlan bindings** — defer; complicated by RST relation enum marshaling. v2.
- **Custom Language trait binding from JS** — defer; would require dynamic dispatch from JS back to Rust.
- **npm package publishing** — out of scope; user will handle with `wasm-pack build`.

---

## Phase 0 — Baseline

```bash
cargo test --all-features
cargo clippy --all-features -- -D warnings
```

Expected: 965 tests passing.

---

## Phase 1 — Create `prosaic-wasm` crate

1. Create directory `prosaic-wasm/` with Cargo.toml + src/lib.rs per design.
2. Add to workspace members in root Cargo.toml (alphabetically).
3. Implement ProsaicEngine + ProsaicSession + JsValue conversion helpers.
4. Add native unit tests for constructor sanity.

### Verify

```bash
cargo build -p prosaic-wasm
cargo test -p prosaic-wasm
cargo check -p prosaic-wasm --target wasm32-unknown-unknown  # if target installed; skip if not
```

If `wasm32-unknown-unknown` target is not installed, document this in the commit message: "wasm32 target check deferred to CI / user; native build and test pass."

---

## Phase 2 — Final verification

```bash
cargo test --all-features
cargo test --no-default-features
cargo test --features serde
cargo clippy --all-features -- -D warnings
cargo doc --all-features --no-deps
```

Test count up by ~2–5 (ProsaicEngine/Session constructor tests only).

**Commit:** `Add prosaic-wasm member crate with wasm-bindgen interop`

---

## Definition of done

- [ ] `prosaic-wasm/` is a workspace member with Cargo.toml + src/lib.rs
- [ ] ProsaicEngine struct with constructor, registerTemplate, referenceTime, render, renderBatch
- [ ] ProsaicSession struct with constructor, reset, resetTemporal
- [ ] Context conversion from JS objects (strings, numbers, string arrays)
- [ ] Native unit tests pass
- [ ] `cargo build -p prosaic-wasm` succeeds
- [ ] All 965 existing tests still pass
- [ ] `cargo clippy --all-features -- -D warnings` clean
- [ ] One commit: `Add prosaic-wasm member crate with wasm-bindgen interop`
