# Plan: `nlg_template!` Compile-Time Template Validator

**Owner:** sonnet agent
**Scope:** `nlg-derive` (new proc macro) + `nlg-core` tests exercising it
**Estimated size:** ~250–350 LOC including tests
**Test gate:** all 444 tests pass after every sub-phase; zero warnings
**Branch discipline:** local only, commit at each sub-phase

---

## Why

The ecosystem agent ranked this the #2 highest-ROI DX win (after `ctx!`), the performance agent signed off on "validator-only in v1, monomorphized codegen in v2." Askama's compile-time-checked templates pattern is proven in the Rust template-engine space. Today all template errors surface at *runtime* — the `Engine::register_template` call returns `Err` for bad slot refs or unknown pipes. Moving that to compile time catches the whole class of typo bugs at `cargo check`.

**Scope: validator only.** Parses the template string, verifies every slot reference is in a declared slot list, verifies every pipe name is known to the engine. Emits `compile_error!` with useful messages on mismatch. Does NOT do codegen, does NOT change the runtime path. The validated template string is returned unchanged at runtime.

## Design (locked — do not deviate)

### User-facing syntax

```rust
use nlg_derive::nlg_template;

// Function-like macro. Returns the template literal unchanged on success;
// emits a compile_error! on slot or pipe mismatch.
let tpl: &'static str = nlg_template! {
    template: "The {entity_type} {name|refer} was renamed to {new_name}",
    slots: [entity_type, name, new_name],
};
```

The slot list is `[ident, ident, ...]` — bare identifiers, matching how `ctx!` accepts keys.

### Pipe whitelist

Hardcoded in the macro. Derived from `engine.rs::apply_pipe` dispatch:

```
pluralize, article, join, ordinal, words, truncate, capitalize, refer,
verb, syn, relative, quantify, hedge, negated
```

Plus the `|choose` pipe is explicitly **rejected** (not shipped yet — the unified-choice pipe is a future Tier-1 item). If a user tries `{slot|choose}` the macro errors with "unknown pipe `choose` — did you mean one of: ..." and suggests nearest matches.

### What the macro validates

1. **Template parses successfully** via `nlg_core::Template::parse`. If `Err`, surface the parse error at compile time.
2. **Every slot key used in the template is in the declared `slots: [...]` list.** Unknown slot → compile error.
3. **Every pipe name on every slot is in the hardcoded pipe whitelist.** Unknown pipe → compile error.
4. **Partials (`{>name}`) are allowed without being in slots.** Partials are declared separately via `engine.register_partial`; validating cross-partial consistency is out of scope for v1.
5. **Conditional sections' condition keys must also be declared slots.** `{?count}...{/?}` requires `count` in `slots`.

### What it does NOT validate (out of scope)

- Pipe argument types. The pipes that take arguments (`truncate:3`, `pluralize:consumer`, `verb:past`, `hedge:modal`, `quantify:hedged`, `join:bracketed`) have argument forms — validate the pipe name, not the argument content.
- Runtime slot values (can't).
- Whether the declared slot list is minimal (OK to declare slots not used in the template — just a minor waste, not a correctness issue).
- Cross-partial slot visibility.
- `Value` variant compatibility (that's runtime territory).

### Return value

The macro expands to a `&'static str` — specifically the original template literal verbatim. No codegen. Users pass it to `engine.register_template(key, nlg_template!(...))` or store it in a const.

### Error style

Use `syn::Error` + `to_compile_error()` per proc-macro conventions. Error spans should point at the problematic slot or pipe inside the template string when feasible — `proc_macro2::Span` allows a span per token, but pointing into a string literal's interior requires `subspan` on the literal. If subspan accuracy is awkward, point at the whole `template:` literal with a descriptive message naming the offending slot/pipe. Clarity over span precision for v1.

---

## Phase 0 — Baseline

```bash
cargo test --all-features
cargo clippy --all-features -- -D warnings
```

Expected: **444 tests passing**, 0 warnings.

**No commit.**

---

## Phase 1 — Scaffold `nlg_template!` proc macro

**File:** `nlg-derive/src/lib.rs`, `nlg-derive/Cargo.toml`

### 1.1 Add `nlg-core` as a regular dependency of `nlg-derive`

Edit `nlg-derive/Cargo.toml`:

```toml
[dependencies]
# ... existing ...
nlg-core = { path = "../nlg-core", default-features = false }
```

Why `default-features = false`: the proc-macro doesn't need `time` / `polish` / `reg` features — it only uses `Template::parse`, which is in the default always-on surface. Dropping optional features keeps the proc-macro compile time small.

**Cycle check:** `nlg-core`'s dev-dependencies include `nlg-derive` (for integration tests). `nlg-derive` now depends on `nlg-core` as a non-dev dep. Dev-deps don't create cycles — cargo resolves dev-deps only when building tests/examples. This is fine. Verify `cargo build -p nlg-derive` succeeds.

### 1.2 Add the proc macro skeleton

In `nlg-derive/src/lib.rs`:

```rust
use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    Ident, LitStr, Token,
};

/// Compile-time-validated template string.
///
/// Parses the template, checks every slot reference against the declared
/// `slots` list, and checks every pipe against the engine's known-pipe
/// set. On success, expands to the original template string literal. On
/// mismatch, emits a compile error pointing at the offending input.
///
/// # Example
///
/// ```
/// use nlg_derive::nlg_template;
/// let tpl = nlg_template! {
///     template: "The {entity_type} {name|refer} was renamed to {new_name}",
///     slots: [entity_type, name, new_name],
/// };
/// assert!(tpl.contains("{name|refer}"));
/// ```
#[proc_macro]
pub fn nlg_template(input: TokenStream) -> TokenStream {
    let parsed = parse_macro_input!(input as NlgTemplateInput);

    match validate(&parsed) {
        Ok(()) => {
            let lit = &parsed.template;
            quote! { #lit }.into()
        }
        Err(e) => e.to_compile_error().into(),
    }
}

struct NlgTemplateInput {
    template: LitStr,
    slots: Vec<Ident>,
}

impl Parse for NlgTemplateInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Expect: template: "...", slots: [a, b, c]
        let mut template: Option<LitStr> = None;
        let mut slots: Option<Vec<Ident>> = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![:]>()?;
            match key.to_string().as_str() {
                "template" => {
                    template = Some(input.parse::<LitStr>()?);
                }
                "slots" => {
                    let content;
                    syn::bracketed!(content in input);
                    let parsed: Punctuated<Ident, Token![,]> =
                        Punctuated::parse_terminated(&content)?;
                    slots = Some(parsed.into_iter().collect());
                }
                other => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!("unknown key `{other}` — expected `template` or `slots`"),
                    ));
                }
            }
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }

        let template = template.ok_or_else(|| {
            syn::Error::new(input.span(), "missing `template: \"...\"` argument")
        })?;
        let slots = slots.unwrap_or_default();

        Ok(NlgTemplateInput { template, slots })
    }
}

fn validate(input: &NlgTemplateInput) -> syn::Result<()> {
    // Stub — fills in across Phase 2 and Phase 3.
    let _ = input;
    Ok(())
}
```

### 1.3 Tests for the skeleton

Create `nlg-core/tests/nlg_template_macro.rs` (yes, live in nlg-core's tests so we can exercise the macro without nlg-derive needing a test binary — the macro is re-exported or directly used from `nlg-derive`):

Wait — the test lives where the macro is *used*. Since `nlg-core`'s dev-deps include `nlg-derive`, the test crate there can use it. Create:

`nlg-core/tests/nlg_template_macro.rs`:

```rust
//! Integration tests for the `nlg_template!` proc macro from `nlg-derive`.

use nlg_derive::nlg_template;

#[test]
fn passes_through_valid_template() {
    let tpl = nlg_template! {
        template: "The {type} {name} was renamed",
        slots: [type, name],
    };
    assert_eq!(tpl, "The {type} {name} was renamed");
}

#[test]
fn empty_slots_list_allowed_for_literal_only_template() {
    let tpl = nlg_template! {
        template: "All systems nominal.",
        slots: [],
    };
    assert_eq!(tpl, "All systems nominal.");
}
```

These should compile cleanly — no validation yet. Confirms the scaffolding works.

### 1.4 Verify

```bash
cargo check --all-features
cargo build -p nlg-derive
cargo test --all-features
cargo clippy --all-features -- -D warnings
```

Check that the cycle-check passes (`cargo build -p nlg-derive` succeeds).

**Commit:** `Scaffold nlg_template! proc macro`

---

## Phase 2 — Implement slot validation

**File:** `nlg-derive/src/lib.rs`

### 2.1 Parse the template via nlg-core

Update `validate`:

```rust
fn validate(input: &NlgTemplateInput) -> syn::Result<()> {
    let template_str = input.template.value();
    let parsed = nlg_core::Template::parse(&template_str).map_err(|e| {
        syn::Error::new(
            input.template.span(),
            format!("invalid template: {e}"),
        )
    })?;

    let declared_slots: std::collections::HashSet<String> =
        input.slots.iter().map(|i| i.to_string()).collect();

    validate_slots(&parsed, &declared_slots, input.template.span())?;
    Ok(())
}

fn validate_slots(
    template: &nlg_core::Template,
    declared: &std::collections::HashSet<String>,
    span: proc_macro2::Span,
) -> syn::Result<()> {
    let used = collect_slot_keys(&template.segments);
    let mut undeclared: Vec<String> = used
        .into_iter()
        .filter(|k| !declared.contains(k))
        .collect();
    undeclared.sort();
    undeclared.dedup();

    if !undeclared.is_empty() {
        let list = undeclared.join(", ");
        let declared_list = {
            let mut v: Vec<_> = declared.iter().cloned().collect();
            v.sort();
            v.join(", ")
        };
        return Err(syn::Error::new(
            span,
            format!(
                "template uses slot(s) not declared in `slots: [...]`: {list}\n  declared: [{declared_list}]",
            ),
        ));
    }
    Ok(())
}

fn collect_slot_keys(segments: &[nlg_core::template::Segment]) -> Vec<String> {
    // NOTE: nlg_core::template::Segment may or may not be publicly
    // accessible. If not, add `pub use` in nlg-core/src/lib.rs or rely
    // on a helper method on Template.
    let mut out = Vec::new();
    walk_segments(segments, &mut out);
    out
}

fn walk_segments(segments: &[nlg_core::template::Segment], out: &mut Vec<String>) {
    use nlg_core::template::Segment;
    for seg in segments {
        match seg {
            Segment::Literal(_) => {}
            Segment::Slot { key, .. } => out.push(key.clone()),
            Segment::Conditional { condition_key, inner } => {
                out.push(condition_key.clone());
                walk_segments(inner, out);
            }
            // Partials resolve at engine registration time; skip for v1.
            _ => {}
        }
    }
}
```

**Dependency check:** `nlg_core::template::Segment` needs to be reachable from the proc-macro. Options:

1. Add `pub mod template;` visibility in `nlg-core/src/lib.rs`, OR
2. Add a `pub fn slot_keys(&self) -> Vec<String>` method on `Template` that does the walk internally, AND optionally `pub fn pipe_names(&self) -> Vec<String>`, avoiding exposing `Segment` publicly.

**Preferred: option 2.** Don't expose the internal `Segment` enum publicly. Add two helper methods:

```rust
// nlg-core/src/template.rs
impl Template {
    /// Every slot key referenced by this template, including condition keys.
    pub fn slot_keys(&self) -> Vec<String> {
        let mut out = Vec::new();
        Self::walk(&self.segments, |seg| match seg {
            Segment::Slot { key, .. } => out.push(key.clone()),
            Segment::Conditional { condition_key, .. } => out.push(condition_key.clone()),
            _ => {}
        }, |_| {});
        // Simpler: inline the recursion.
        out
    }

    /// Every pipe name referenced by any slot in this template.
    pub fn pipe_names(&self) -> Vec<String> {
        let mut out = Vec::new();
        Self::walk_pipes(&self.segments, &mut out);
        out
    }
}
```

Actually simplest: inline two small recursive helpers inside `Template::slot_keys` and `Template::pipe_names` methods, keeping `Segment` private-ish. The existing `Template::literal_tokens()` precedent (from the PARENT plan) shows how — follow the same pattern.

The proc macro then calls `parsed.slot_keys()` and `parsed.pipe_names()`. Clean.

### 2.2 Test cases

Update `nlg-core/tests/nlg_template_macro.rs`:

```rust
#[test]
fn rejects_undeclared_slot() {
    // Compile-fail scenarios can't be asserted in regular #[test] blocks.
    // For v1, verify via a separate trybuild harness (deferred) OR
    // manually verify by attempting to compile a deliberately-broken
    // macro invocation.
    //
    // This test just documents the expectation. The macro itself is
    // exercised in compile-pass tests above. A compile-fail
    // harness can be added in a follow-up with `trybuild`.
}
```

Compile-fail tests are hard without trybuild. For v1 **skip them** and document:

> **Note:** Compile-fail behaviour (undeclared slot / unknown pipe emits a clean compile error) is exercised by manual verification during development and by downstream user bug reports. Adding a `trybuild` harness is a follow-up.

That's honest and avoids over-engineering v1.

### 2.3 Verify

```bash
cargo test --all-features
cargo clippy --all-features -- -D warnings
```

**Commit:** `Validate slot references in nlg_template! macro`

---

## Phase 3 — Implement pipe validation

**File:** `nlg-derive/src/lib.rs`

### 3.1 Hardcoded valid-pipe list

```rust
const VALID_PIPES: &[&str] = &[
    "pluralize", "article", "join", "ordinal", "words", "truncate",
    "capitalize", "refer", "verb", "syn", "relative", "quantify",
    "hedge", "negated",
];
```

### 3.2 Validation step

```rust
fn validate(input: &NlgTemplateInput) -> syn::Result<()> {
    // ... existing parsing + slot validation

    validate_pipes(&parsed, input.template.span())?;
    Ok(())
}

fn validate_pipes(
    template: &nlg_core::Template,
    span: proc_macro2::Span,
) -> syn::Result<()> {
    let used = template.pipe_names();
    let mut unknown: Vec<String> = used
        .into_iter()
        .filter(|p| !VALID_PIPES.contains(&p.as_str()))
        .collect();
    unknown.sort();
    unknown.dedup();

    if !unknown.is_empty() {
        let list = unknown
            .iter()
            .map(|p| {
                let suggestion = nearest_pipe(p);
                match suggestion {
                    Some(s) => format!("`{p}` (did you mean `{s}`?)"),
                    None => format!("`{p}`"),
                }
            })
            .collect::<Vec<_>>()
            .join(", ");
        return Err(syn::Error::new(
            span,
            format!(
                "template uses unknown pipe(s): {list}\n  known pipes: [{}]",
                VALID_PIPES.join(", ")
            ),
        ));
    }
    Ok(())
}

fn nearest_pipe(unknown: &str) -> Option<&'static str> {
    // Hamming-distance-light nearest match via prefix matching — good
    // enough for typo catching without pulling in a distance crate.
    for &valid in VALID_PIPES {
        if valid.starts_with(unknown) || unknown.starts_with(valid) {
            return Some(valid);
        }
    }
    // Fallback: any pipe sharing first 3 chars
    let prefix: String = unknown.chars().take(3).collect();
    for &valid in VALID_PIPES {
        if valid.starts_with(&prefix) {
            return Some(valid);
        }
    }
    None
}
```

### 3.3 Test cases

Update `nlg-core/tests/nlg_template_macro.rs`:

```rust
#[test]
fn valid_pipes_pass() {
    // Each invocation must compile.
    let _ = nlg_template!(
        template: "{name|refer} is {count|pluralize:item}",
        slots: [name, count],
    );
    let _ = nlg_template!(
        template: "{items|truncate:3|join:bracketed}",
        slots: [items],
    );
    let _ = nlg_template!(
        template: "{action|verb:past}",
        slots: [action],
    );
    let _ = nlg_template!(
        template: "{phrase|negated}",
        slots: [phrase],
    );
    let _ = nlg_template!(
        template: "{ts|relative}",
        slots: [ts],
    );
    let _ = nlg_template!(
        template: "{conf|hedge:modal}",
        slots: [conf],
    );
    let _ = nlg_template!(
        template: "{count|quantify:natural}",
        slots: [count],
    );
    let _ = nlg_template!(
        template: "{word|syn}",
        slots: [word],
    );
}

#[test]
fn conditional_section_key_counted_as_slot() {
    let tpl = nlg_template! {
        template: "Added {name}{?count}, impacting {count} consumers{/?}",
        slots: [name, count],
    };
    assert!(tpl.contains("{?count}"));
}
```

### 3.4 Verify

```bash
cargo test --all-features
cargo test --no-default-features
cargo test --features serde
cargo clippy --all-features -- -D warnings
cargo doc --all-features --no-deps
```

**Commit:** `Validate pipe names in nlg_template! macro`

---

## Phase 4 — Final verification

```bash
cargo test --all-features
cargo test --no-default-features
cargo test --features serde
cargo clippy --all-features -- -D warnings
cargo doc --all-features --no-deps
cargo bench --bench engine -- --test
```

Test count should increase by ~10–12. Target: **~455**, up from 444.

**Report:** 3 commit hashes, test count delta, any surprises.

---

## Risk register

| Risk | Mitigation |
|---|---|
| `nlg-derive → nlg-core` dep creates a cycle with `nlg-core → [dev] nlg-derive` | Cargo dev-deps don't count toward main dep graph. Verify explicitly: `cargo build -p nlg-derive` and `cargo build -p nlg-core` both succeed. If cargo complains about a cycle anyway, the fallback is to duplicate `Template::parse` into nlg-derive — undesirable, about 200 LOC. Hope cargo cooperates. |
| `Template::slot_keys` / `pipe_names` helper methods don't exist yet | Add them in Phase 2 as public API on `Template` (same file as `literal_tokens` added in the PARENT plan). Mirror that method's style. |
| Proc macro tests can't easily exercise compile-fail cases | Document as a known v1 limitation. trybuild harness is a follow-up. Manual verification during development is sufficient for this small macro. |
| Span precision inside the string literal is awkward | Point at the whole `template:` literal. Error message naming the specific slot/pipe is usually more useful than a sub-span anyway. |
| The macro should work with `default-features = false` on nlg-core | All of `Template::parse`, `Template::slot_keys`, `Template::pipe_names` are in the default-always-on surface. They don't touch `time`, `polish`, or `reg`. Confirm. |
| Nested conditional sections' inner slots not collected | The `walk_segments` recursion handles nested conditionals. Add a test with a two-level conditional to verify. |
| `nlg_template!` clashes with future `#[nlg_template]` attribute macro | Named `nlg_template` (function-like) today. If a v2 attribute macro is added later, pick a different name (e.g., `#[nlg_template_impl]`). Document in rustdoc so future contributors know the namespace is taken. |
| Pipe arg syntax (`pluralize:consumer`) — does `Template::parse` handle this and surface the pipe name as `pluralize` (not `pluralize:consumer`)? | Yes — `Pipe` struct has `name` and `args` fields separately. `pipe_names()` returns only `.name`. Confirm via the existing template tests. |

## What NOT to do

- **Do not** add compile-time codegen / monomorphization. v1 is validator only.
- **Do not** add trybuild as a dev-dependency for compile-fail tests. Deferred.
- **Do not** validate pipe arguments (`truncate:3`, `verb:past`, etc.). Names only.
- **Do not** validate cross-partial slot references.
- **Do not** change the runtime rendering path.
- **Do not** add a crate-level feature flag for the macro. It's always available.
- **Do not** amend commits.

## Definition of done

- [ ] Phase 0 baseline clean
- [ ] 3 commits with specified subject lines
- [ ] `nlg-derive` depends on `nlg-core` (default-features = false)
- [ ] `cargo build -p nlg-derive` succeeds
- [ ] `Template::slot_keys()` and `Template::pipe_names()` public methods added
- [ ] `nlg_template!` function-like proc macro exported from `nlg-derive`
- [ ] Valid templates compile and return the literal unchanged at runtime
- [ ] Invalid slot/pipe use emits a `compile_error!` (manually verified; trybuild deferred)
- [ ] All 444 → ~455 tests pass across feature variants
- [ ] `cargo clippy --all-features -- -D warnings` clean
- [ ] `Engine: Send + Sync` assert still compiles
- [ ] No new public API beyond `Template::slot_keys`, `Template::pipe_names`, and the proc macro itself
