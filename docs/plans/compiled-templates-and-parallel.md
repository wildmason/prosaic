# Plan: `prosaic_template_compiled!` + `parallel` Cargo feature

**Owner:** sonnet agent
**Scope:** `prosaic-derive/src/lib.rs` (new proc macro) + `prosaic-core/Cargo.toml` (new feature) + `prosaic-core/src/document.rs` (new `render_parallel` method)
**Estimated size:** ~300–400 LOC including tests
**Test gate:** 972 baseline tests still pass; ~10–15 new
**Branch discipline:** local only, one commit

---

## Why

Two features bundled for narrative coherence — both target throughput:

1. **`prosaic_template_compiled!` macro** — like `format!`, but parses a prosaic template at compile time and emits a specialized render function. Skips the engine's runtime parsing pipeline. Useful for tight loops with known templates. Scoped to **bare slots only** (no pipes) — pipes require the full engine.

2. **`parallel` Cargo feature** — adds `DocumentPlan::render_parallel`. Each paragraph gets its own cloned Session and renders in parallel via rayon. Loses temporal-anchor threading across paragraphs (documented tradeoff). Beneficial for large change reports where paragraphs are genuinely independent.

## Design

### `prosaic_template_compiled!` proc macro

New proc macro in `prosaic-derive/src/lib.rs`. Input:

```rust
let render = prosaic_template_compiled! {
    "The class {name} was modified"
};
// Generated: fn render(ctx: &prosaic_core::Context) -> String { ... }
```

Actually — returning a closure-like value is awkward. Better: emit a free function declaration, reference it by name:

```rust
prosaic_template_compiled! {
    pub fn render_renamed(ctx: &Context) -> String;
    template: "The class {name} was renamed to {new_name}";
}

// Usage:
let s = render_renamed(&ctx);
```

Hmm, even simpler: emit a closure that captures the template structure. Rust proc macros can't return closures, but they can return function items via `{ fn _inner... _inner }` block expressions. That gives us:

```rust
let render = prosaic_template_compiled!("The class {name} was modified");
// render is a fn(&Context) -> String

let s = render(&ctx);
```

Where the macro expands to:

```rust
{
    fn __prosaic_compiled_render(ctx: &::prosaic_core::Context) -> String {
        let mut out = String::with_capacity(64);
        out.push_str("The class ");
        if let Some(v) = ctx.get("name") {
            out.push_str(&v.as_display());
        }
        out.push_str(" was modified");
        out
    }
    __prosaic_compiled_render
}
```

Much cleaner. Let's go with this form.

### Parse logic

Re-use `prosaic_core::Template::parse`. Walk the parsed template:

- Text segments → `out.push_str("<text>");`
- Slot segments (bare `{key}`) → `if let Some(v) = ctx.get("<key>") { out.push_str(&v.as_display()); }`
- Slot with pipes → **compile error**: "compiled templates don't support pipes in v1"
- Conditional sections `{?key}...{/?}` → **compile error**: "compiled templates don't support conditional sections in v1"
- Partials `{>name}` → **compile error**: "compiled templates don't support partials in v1"

### Implementation sketch (in proc macro)

```rust
#[proc_macro]
pub fn prosaic_template_compiled(input: TokenStream) -> TokenStream {
    let template_lit = parse_macro_input!(input as LitStr);
    let template_str = template_lit.value();
    let span = template_lit.span();

    let parsed = match prosaic_core::Template::parse(&template_str) {
        Ok(t) => t,
        Err(e) => return syn::Error::new(span, format!("invalid template: {e}"))
            .to_compile_error().into(),
    };

    // Reject any feature beyond bare slots.
    for seg in parsed.segments() {
        match seg {
            TemplateSegment::Text(_) | TemplateSegment::Slot { pipes, .. } if pipes.is_empty() => {}
            TemplateSegment::Slot { .. } => {
                return syn::Error::new(span,
                    "prosaic_template_compiled!: templates with pipes are not supported; use the runtime engine"
                ).to_compile_error().into();
            }
            _ => {
                return syn::Error::new(span,
                    "prosaic_template_compiled!: conditional sections, partials, and advanced features are not supported"
                ).to_compile_error().into();
            }
        }
    }

    // Generate push_str calls.
    let mut body = quote! { let mut out = String::with_capacity(128); };
    for seg in parsed.segments() {
        match seg {
            TemplateSegment::Text(t) => {
                let lit = t.as_str();
                body = quote! { #body out.push_str(#lit); };
            }
            TemplateSegment::Slot { key, .. } => {
                let key_lit = key.as_str();
                body = quote! {
                    #body
                    if let Some(v) = ctx.get(#key_lit) {
                        out.push_str(&v.as_display());
                    }
                };
            }
            _ => unreachable!(), // rejected above
        }
    }

    let expanded = quote! {
        {
            fn __prosaic_compiled_render(ctx: &::prosaic_core::Context) -> String {
                #body
                out
            }
            __prosaic_compiled_render
        }
    };

    expanded.into()
}
```

Note: `Template` segments need to be accessible from outside `prosaic-core`. If they're private, we need to add a `pub fn segments(&self) -> impl Iterator<Item = Segment>` accessor or similar. Check `template.rs` — if segments are already accessible, great; if not, we need to expose them. The existing `prosaic_template!` macro in prosaic-derive already uses `parsed.slot_keys()` and `parsed.pipe_names()` — so there's some accessor surface. Extend it as needed.

Actually if `Template::parse` returns internal AST that's hard to expose, the fallback is to parse again in the macro with a simpler regex-based parser. For "bare slot templates" only, a one-regex parser is trivial — but that duplicates logic.

**Preferred path:** add `pub fn text_segments_and_slots(&self) -> impl Iterator<...>` to `prosaic-core::Template`, returning text spans and bare slot keys. Restrict to `prosaic-derive` internal use if needed.

Simpler still: add `Template::render_bare_slots(&self, ctx: &Context) -> Option<String>` that renders the template if and only if it has no pipes / conditionals / partials, and `Template::is_bare_slot_only(&self) -> bool`. Then the macro's codegen is trivial: parse at compile time, validate it's bare-slot-only, emit `Template::parse(LIT).unwrap().render_bare_slots(ctx).unwrap()`. But that doesn't actually skip the runtime parse — just lets us know it's safe.

For actual compile-time codegen: we need to walk the parsed AST at macro-expansion time. Let me mandate adding accessors for that.

### `Template` accessor additions

In `prosaic-core/src/template.rs`, add public accessors:

```rust
impl Template {
    /// Iterator over (text, slot-keys) in parse order. Yields:
    /// - (Some(text), None) for literal text segments
    /// - (None, Some(key)) for bare slot references
    /// Used by the `prosaic_template_compiled!` macro for compile-time codegen.
    ///
    /// Returns `None` the iteration if the template contains pipes, conditionals,
    /// or partials — those require the runtime engine.
    pub fn as_bare_slots(&self) -> Option<Vec<BareSegment<'_>>> {
        // ... walk self.segments, return None on non-bare feature ...
    }
}

pub enum BareSegment<'a> {
    Text(&'a str),
    Slot(&'a str),
}
```

The proc macro uses this accessor to drive its codegen.

### `parallel` Cargo feature

New feature on `prosaic-core`:

```toml
[features]
parallel = ["dep:rayon"]

[dependencies]
rayon = { version = "1", optional = true }
```

In `prosaic-core/src/document.rs`:

```rust
impl DocumentPlan {
    /// Render paragraphs in parallel. Each paragraph gets its own cloned
    /// Session. **Trade-off:** temporal-anchor threading across paragraphs
    /// is lost — each paragraph anchors independently. For temporally
    /// coherent narratives use `render` instead.
    ///
    /// Requires the `parallel` feature.
    #[cfg(feature = "parallel")]
    pub fn render_parallel(
        &self,
        engine: &Engine,
        initial_session: &Session,
    ) -> Result<String, ProsaicError>
    where
        Engine: Sync,
        Session: Send,
    {
        use rayon::prelude::*;

        let rendered: Result<Vec<String>, ProsaicError> = self
            .paragraphs
            .par_iter()
            .map(|p| {
                let mut session = initial_session.clone();
                // Force a fresh discourse state per paragraph (mirrors
                // the sequential render's session.reset() between paragraphs).
                session.reset();

                // Render the paragraph's events via the engine, with or
                // without relations per the paragraph's relations vec.
                if p.relations.iter().any(|r| r.is_some()) {
                    let triples: Vec<_> = p
                        .events
                        .iter()
                        .zip(p.relations.iter())
                        .map(|((k, c), r)| (k.as_str(), c.clone(), *r))
                        .collect();
                    engine.render_batch_with_relations(&mut session, &triples)
                } else {
                    let events: Vec<_> = p
                        .events
                        .iter()
                        .map(|(k, c)| (k.as_str(), c.clone()))
                        .collect();
                    engine.render_batch(&mut session, &events)
                }
            })
            .filter(|r| !matches!(r, Ok(s) if s.is_empty()))
            .collect();

        Ok(rendered?.join("\n\n"))
    }
}
```

`Engine: Sync` is already satisfied — there's an existing `Engine: Send + Sync` assertion in tests.

`Session: Send + Clone` — we need to verify `Session` is `Send`. It contains `AtomicUsize` (which is Send) and `HashMap` / `HashSet` (Send if K/V are Send). Should be fine.

### Out of scope

- **Compiled templates with pipes** — the pipe dispatch is too intertwined with Engine state (Variation, Strictness, REG, etc.). Would require monomorphizing the entire render path; that's a v2 project.
- **Parallel with temporal anchor threading** — requires cross-paragraph dependency, which is inherently sequential. Callers who need both temporal coherence AND parallelism can split their document into independent sub-documents themselves.
- **Macro-generated entire Engine setup** — `prosaic_template_compiled!` only produces the render function; users still wire up the engine separately.

---

## Phase 0 — Baseline

```bash
cargo test --all-features
cargo clippy --all-features -- -D warnings
```

Expected: 972 tests passing.

---

## Phase 1 — `Template::as_bare_slots` accessor

1. In `prosaic-core/src/template.rs`, add `BareSegment` enum and `as_bare_slots` method per design.
2. Re-export `BareSegment` from `lib.rs` so `prosaic-derive` can use it.

### Tests

```rust
#[test]
fn as_bare_slots_accepts_bare_template() {
    let t = Template::parse("Hello {name} world").unwrap();
    let segs = t.as_bare_slots().unwrap();
    // 3 segments: "Hello ", slot name, " world"
    assert_eq!(segs.len(), 3);
}

#[test]
fn as_bare_slots_rejects_piped_template() {
    let t = Template::parse("Hello {name|capitalize}").unwrap();
    assert!(t.as_bare_slots().is_none());
}

#[test]
fn as_bare_slots_rejects_conditional_template() {
    let t = Template::parse("Hello{?greet} friend{/?}").unwrap();
    assert!(t.as_bare_slots().is_none());
}
```

---

## Phase 2 — `prosaic_template_compiled!` proc macro

1. In `prosaic-derive/src/lib.rs`, add the new `#[proc_macro]` function.
2. Validate template at compile time via `Template::parse + as_bare_slots`.
3. Emit codegen per design.

### Tests (in prosaic-derive or a separate integration test crate)

Integration tests from `prosaic-core` (since derive is an opaque dependency):

```rust
use prosaic_core::{Context, Value};
use prosaic_derive::prosaic_template_compiled;

#[test]
fn compiled_template_renders_bare_slots() {
    let render = prosaic_template_compiled!("Hello {name}!");
    let mut ctx = Context::new();
    ctx.insert("name", Value::String("World".into()));
    assert_eq!(render(&ctx), "Hello World!");
}

#[test]
fn compiled_template_fills_missing_slot_with_empty() {
    let render = prosaic_template_compiled!("Hello {name}!");
    let ctx = Context::new();
    // No "name" slot — slot contributes nothing.
    assert_eq!(render(&ctx), "Hello !");
}

#[test]
fn compiled_template_handles_multiple_slots() {
    let render = prosaic_template_compiled!("{greeting}, {name}!");
    let mut ctx = Context::new();
    ctx.insert("greeting", Value::String("Hi".into()));
    ctx.insert("name", Value::String("Alice".into()));
    assert_eq!(render(&ctx), "Hi, Alice!");
}
```

`trybuild` compile-fail tests for pipes/conditionals/partials are out of scope for v1 (same as existing `prosaic_template!`).

---

## Phase 3 — `parallel` feature + `DocumentPlan::render_parallel`

1. Add `parallel` feature + `rayon` optional dep to `prosaic-core/Cargo.toml`.
2. Implement `render_parallel` per design.

### Tests

```rust
#[test]
#[cfg(feature = "parallel")]
fn render_parallel_produces_same_output_for_independent_paragraphs() {
    // When paragraphs don't need temporal threading, parallel and sequential
    // must produce byte-identical output.
    let mut engine = test_engine();
    engine.register_template("t", "{name} was modified").unwrap();

    let events = vec![
        ("t", ctx_with_entity("Alpha", 1)),
        ("t", ctx_with_entity("Beta", 1)),
    ];
    let plan = DocumentPlan::from_events(&events, &engine);

    let mut s1 = Session::new();
    let seq = plan.render(&engine, &mut s1).unwrap();

    let s2 = Session::new();
    let par = plan.render_parallel(&engine, &s2).unwrap();

    assert_eq!(seq, par);
}
```

### Optional: Send+Sync assertions

```rust
#[test]
fn engine_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Engine>();
    // Session is Send (it has no Rc/RefCell) — verify.
    assert_send_sync::<Session>();
}
```

(The Engine assertion likely exists; add a Session one if missing.)

---

## Phase 4 — Final verification

```bash
cargo test --all-features
cargo test --no-default-features
cargo test --features serde
cargo test --features parallel
cargo clippy --all-features -- -D warnings
cargo doc --all-features --no-deps
```

Test count up by ~10–15.

**Commit:** `Add prosaic_template_compiled macro and parallel DocumentPlan rendering`

---

## Definition of done

- [ ] `Template::as_bare_slots()` accessor + `BareSegment` enum
- [ ] `prosaic_template_compiled!` proc macro emitting a `fn(&Context) -> String`
- [ ] Macro rejects pipes / conditionals / partials at compile time with helpful error
- [ ] `parallel` Cargo feature on `prosaic-core` with rayon dep
- [ ] `DocumentPlan::render_parallel(engine, initial_session)` produces equivalent output for independent paragraphs
- [ ] Documentation note that temporal anchor threading is lost in parallel mode
- [ ] All 972 existing tests pass + 10-15 new
- [ ] `cargo clippy --all-features -- -D warnings` clean
- [ ] One commit: `Add prosaic_template_compiled macro and parallel DocumentPlan rendering`
