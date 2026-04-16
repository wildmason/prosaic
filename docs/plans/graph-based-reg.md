# Plan: Graph-Based REG (Krahmer 2003) — v1 Greedy Backend

**Owner:** sonnet agent
**Scope:** `nlg-core/src/reg.rs`, `nlg-core/src/engine.rs` (pipe integration + builder), `nlg-core/src/lib.rs` (re-exports)
**Estimated size:** ~500–700 LOC including tests
**Test gate:** all 483 tests pass after every sub-phase; zero warnings
**Branch discipline:** local only, commit at each sub-phase

---

## Why

Dale & Reiter 1995 (our current REG) disambiguates same-type entities using unary attributes only: "the domain class UserService" vs "the infra class AuthService". It cannot express relational identity: "the handler that calls AuthService", "the test for UserService", "the file in the auth module".

Krahmer, van Erk & Verleg 2003 (Computational Linguistics 29(1)) generalizes to graphs: entities are nodes, relations are labeled directed edges, and the algorithm finds a minimal distinguishing subgraph. The paper's own greedy implementation is tractable and well-validated — Krahmer explicitly recommends it as the default for production use over Branch & Bound.

Linguistics agent's finding (upgraded to Tier 1 post-synthesis):
> "Should be shipped as a second REG backend behind the `reg` feature, selectable via a builder option, not a replacement. Greedy implementation is a superset of Dale & Reiter."

**Value today:** modest — our current vocabularies (`nlg-vocab-code`, `nlg-vocab-git`) don't expose relations. **Value tomorrow:** large — any future vocab crate that models "test-for", "calls", "contained-in", "depends-on" gets relational REG automatically.

## Design (locked)

### Relations on `EntityDescriptor`

Extend the struct:

```rust
pub struct EntityDescriptor {
    pub name: String,
    pub entity_type: String,
    pub attributes: Vec<(String, String)>,
    /// Labeled directed edges from this entity to other named entities.
    /// Each `(relation_label, target_name)` pair.
    pub relations: Vec<(String, String)>,
}
```

Builder method:

```rust
impl EntityDescriptor {
    pub fn with_relation(mut self, label: impl Into<String>, target: impl Into<String>) -> Self {
        self.relations.push((label.into(), target.into()));
        self
    }
}
```

**Relation label surface form:** the label IS the surface-form fragment. `with_relation("that calls", "AuthService")` renders as `"that calls AuthService"`. Users choose labels that read naturally when inserted after the head noun. This keeps the algorithm out of the surface-realization business and is consistent with how attribute values work today.

**Relation target:** the second argument is the target entity's **name**. For v1, the target renders as a bare name in the referring expression (no recursive REG on the target). That keeps the render cheap and avoids cycle handling. If a target is itself ambiguous, too bad — v1 is deliberately shallow.

### Algorithm selection

New enum and builder option:

```rust
// nlg-core/src/engine.rs
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RegAlgorithm {
    /// Dale & Reiter 1995 Incremental Algorithm. Unary attributes only.
    /// Fast, well-understood, default.
    #[default]
    DaleReiter,
    /// Krahmer et al. 2003 graph-based greedy. Handles both unary
    /// attributes AND binary relations. Falls back to D&R behaviour
    /// when no relations are registered.
    GraphBased,
}

impl Engine {
    pub fn reg_algorithm(mut self, algo: RegAlgorithm) -> Self {
        self.reg_algorithm = algo;
        self
    }
}
```

### Greedy graph-based distinguishing-subgraph algorithm

```rust
// nlg-core/src/reg.rs
pub struct SubgraphDescription {
    /// Attribute values to include as premodifiers, in preference order.
    pub attributes: Vec<String>,
    /// Distinguishing relation, if any: (label, target_name).
    pub relation: Option<(String, String)>,
}

pub fn distinguishing_subgraph(
    target: &EntityDescriptor,
    registry: &EntityRegistry,
    preference_order: &[String],
) -> SubgraphDescription {
    // 1. Run Dale & Reiter first — same logic as today.
    let attrs = distinguishing_attributes(target, registry, preference_order);

    // 2. Check if attrs alone disambiguate. Compute remaining distractors.
    let mut distractors: Vec<&EntityDescriptor> = registry
        .iter()
        .filter(|d| d.name != target.name && d.entity_type == target.entity_type)
        .collect();
    distractors.retain(|d| {
        // A distractor survives iff it shares every selected attribute value.
        attrs.iter().all(|chosen_val| {
            target
                .attributes
                .iter()
                .find(|(_, v)| v == chosen_val)
                .and_then(|(k, _)| d.attribute(k))
                == Some(chosen_val.as_str())
        })
    });

    if distractors.is_empty() {
        return SubgraphDescription { attributes: attrs, relation: None };
    }

    // 3. Attributes alone didn't disambiguate. Try adding one relation.
    //    Walk target's relations in order. Pick the first relation that
    //    no surviving distractor also has (same label, same target).
    for (label, target_name) in &target.relations {
        let still_matching = distractors.iter().any(|d| {
            d.relations
                .iter()
                .any(|(l, t)| l == label && t == target_name)
        });
        if !still_matching {
            return SubgraphDescription {
                attributes: attrs,
                relation: Some((label.clone(), target_name.clone())),
            };
        }
    }

    // 4. Exhausted — return best-effort (may still be ambiguous).
    SubgraphDescription { attributes: attrs, relation: None }
}
```

Complexity: O(|distractors| × (|attributes| + |relations|)) — linear in registry size. No backtracking, no B&B. Krahmer 2003 § 5 explicitly validates greedy as near-optimal when the distractor set is small, which is our case.

### Surface realization

When graph-based REG selects a relation, the Full form becomes:

```
"The [attrs joined by space] [entity_type] [name] [relation_label] [target_name]"
```

Example: `"The domain class UserService that calls AuthService"`.

When no relation is needed, output is identical to D&R — the existing path.

Integration point: `pipe_refer` in `engine.rs`. Where today it calls `distinguishing_attributes`, dispatch on `self.reg_algorithm`:

```rust
let (attrs, relation) = match self.reg_algorithm {
    RegAlgorithm::DaleReiter => (
        distinguishing_attributes(target, &self.entity_registry, &self.reg_preference),
        None,
    ),
    RegAlgorithm::GraphBased => {
        let desc = distinguishing_subgraph(target, &self.entity_registry, &self.reg_preference);
        (desc.attributes, desc.relation)
    }
};
```

Then in the Full-form emission, if `relation.is_some()`, append ` {label} {target}` after the name.

### Out of scope for v1 (do NOT implement)

- Branch & Bound search. Greedy only.
- Multi-hop relations (transitive: "the handler whose module depends on auth"). One hop only.
- Recursive REG on relation targets. Targets are bare names.
- User-configurable cost functions. Fixed cost = 1/attr, 1/relation, attributes preferred.
- Cycle detection (we don't recurse on targets, so cycles can't arise).
- Multiple relations in one expression ("the file in the auth module that imports AuthService"). One relation max.
- Changes to D&R. It stays exactly as-is.

---

## Phase 0 — Baseline

```bash
cargo test --all-features
cargo clippy --all-features -- -D warnings
```

Expected: **483 tests passing**, 0 warnings.

**No commit.**

---

## Phase 1 — Extend `EntityDescriptor` with relations

**File:** `nlg-core/src/reg.rs`

### 1.1 Add the field

```rust
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EntityDescriptor {
    pub name: String,
    pub entity_type: String,
    pub attributes: Vec<(String, String)>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub relations: Vec<(String, String)>,
}
```

`Default` + `#[serde(default)]` so existing serialized `EntityDescriptor`s without a `relations` field round-trip cleanly.

### 1.2 Builder method

```rust
impl EntityDescriptor {
    pub fn with_relation(mut self, label: impl Into<String>, target: impl Into<String>) -> Self {
        self.relations.push((label.into(), target.into()));
        self
    }

    /// Look up a relation's target by label. Returns `None` if the
    /// entity has no such relation.
    pub fn relation(&self, label: &str) -> Option<&str> {
        self.relations.iter().find(|(l, _)| l == label).map(|(_, t)| t.as_str())
    }
}
```

### 1.3 Initialize `relations` in `EntityDescriptor::new`

```rust
pub fn new(name: impl Into<String>, entity_type: impl Into<String>) -> Self {
    Self {
        name: name.into(),
        entity_type: entity_type.into(),
        attributes: Vec::new(),
        relations: Vec::new(),
    }
}
```

### 1.4 Tests

Add to `#[cfg(test)]`:

```rust
#[test]
fn with_relation_adds_edge() {
    let e = EntityDescriptor::new("Handler", "function")
        .with_relation("calls", "AuthService");
    assert_eq!(e.relations, vec![("calls".to_string(), "AuthService".to_string())]);
}

#[test]
fn relation_lookup_by_label() {
    let e = EntityDescriptor::new("Handler", "function")
        .with_relation("calls", "AuthService")
        .with_relation("tests", "HandlerTests");
    assert_eq!(e.relation("calls"), Some("AuthService"));
    assert_eq!(e.relation("tests"), Some("HandlerTests"));
    assert_eq!(e.relation("unknown"), None);
}

#[test]
fn default_has_empty_relations() {
    let e = EntityDescriptor::default();
    assert!(e.relations.is_empty());
}
```

### 1.5 Verify

```bash
cargo test --all-features
cargo test --features serde
cargo clippy --all-features -- -D warnings
```

The serde test must still pass — confirm that adding `relations` with `#[serde(default)]` doesn't break the existing `entity_descriptor_roundtrips` test.

**Commit:** `Add relations field to EntityDescriptor`

---

## Phase 2 — Add `RegAlgorithm` enum and `Engine::reg_algorithm` builder

**Files:** `nlg-core/src/engine.rs`, `nlg-core/src/lib.rs`

### 2.1 Define the enum

Place in `engine.rs` near other config enums (or in `reg.rs` if it reads better there — but keeping it with the builder options is more discoverable). Use `engine.rs`:

```rust
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RegAlgorithm {
    #[default]
    DaleReiter,
    GraphBased,
}
```

### 2.2 Add engine field + builder

```rust
pub struct Engine {
    // ... existing fields ...
    #[cfg(feature = "reg")]
    reg_algorithm: RegAlgorithm,
    // ...
}

impl Engine {
    pub fn new(language: impl Language + 'static) -> Self {
        Self {
            // ... existing inits ...
            #[cfg(feature = "reg")]
            reg_algorithm: RegAlgorithm::default(),
            // ...
        }
    }

    #[cfg(feature = "reg")]
    pub fn reg_algorithm(mut self, algo: RegAlgorithm) -> Self {
        self.reg_algorithm = algo;
        self
    }
}
```

### 2.3 Re-export

In `nlg-core/src/lib.rs`, add `RegAlgorithm` to the `#[cfg(feature = "reg")]` re-export block next to `EntityDescriptor` and `EntityRegistry`.

### 2.4 Serde round-trip test

If `tests/serde.rs` covers the engine config enums (it tests `Salience::High`, `GroupingStrategy::ByAction`, etc.), add:

```rust
assert_eq!(
    serde_json::to_string(&RegAlgorithm::GraphBased).unwrap(),
    "\"GraphBased\""
);
```

### 2.5 Verify

```bash
cargo test --all-features
cargo test --features serde
cargo clippy --all-features -- -D warnings
```

No behaviour changes yet — just plumbing. D&R is still the default and only active path.

**Commit:** `Add RegAlgorithm enum and Engine::reg_algorithm builder option`

---

## Phase 3 — Implement `distinguishing_subgraph`

**File:** `nlg-core/src/reg.rs`

### 3.1 Add the `SubgraphDescription` struct

```rust
/// Output of the graph-based REG algorithm: attributes to premodify with,
/// and an optional relation to append as a postmodifier.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SubgraphDescription {
    pub attributes: Vec<String>,
    pub relation: Option<(String, String)>,
}
```

### 3.2 Implement the greedy search

Per the design section above. The function calls `distinguishing_attributes` first, then checks whether any distractors survived the attribute filter. If so, it walks target relations to find one that no surviving distractor shares.

**Important detail:** the helper that computes "surviving distractors after attributes are chosen" needs to match the actual semantics of `distinguishing_attributes`. Re-read that function carefully and mirror its distractor-survival logic. A clean approach is to refactor `distinguishing_attributes` internally so it returns BOTH the chosen attributes AND the surviving distractors, and have `distinguishing_subgraph` call that internal helper — avoiding semantic drift.

```rust
// Internal helper — used by both D&R and graph-based.
pub(crate) fn incremental_attributes_with_remaining<'a>(
    target: &EntityDescriptor,
    registry: &'a EntityRegistry,
    preference_order: &[String],
) -> (Vec<String>, Vec<&'a EntityDescriptor>) {
    // Existing `distinguishing_attributes` body, but also track the
    // post-attribute distractor set.
    // ...
}

pub fn distinguishing_attributes(
    target: &EntityDescriptor,
    registry: &EntityRegistry,
    preference_order: &[String],
) -> Vec<String> {
    let (attrs, _) = incremental_attributes_with_remaining(target, registry, preference_order);
    attrs
}

pub fn distinguishing_subgraph(
    target: &EntityDescriptor,
    registry: &EntityRegistry,
    preference_order: &[String],
) -> SubgraphDescription {
    let (attrs, remaining) = incremental_attributes_with_remaining(target, registry, preference_order);

    if remaining.is_empty() {
        return SubgraphDescription { attributes: attrs, relation: None };
    }

    // Walk target's relations in insertion order. Pick the first that
    // rules out every remaining distractor.
    for (label, target_name) in &target.relations {
        let any_shared = remaining.iter().any(|d| {
            d.relations.iter().any(|(l, t)| l == label && t == target_name)
        });
        if !any_shared {
            return SubgraphDescription {
                attributes: attrs,
                relation: Some((label.clone(), target_name.clone())),
            };
        }
    }

    SubgraphDescription { attributes: attrs, relation: None }
}
```

### 3.3 Tests

Add to `#[cfg(test)]` in `reg.rs`:

```rust
#[test]
fn graph_reg_no_distractors_returns_empty() {
    let target = EntityDescriptor::new("Foo", "class");
    let registry = reg_with(vec![target.clone()]);
    let desc = distinguishing_subgraph(&target, &registry, &[]);
    assert!(desc.attributes.is_empty());
    assert!(desc.relation.is_none());
}

#[test]
fn graph_reg_falls_back_to_dale_reiter_when_attributes_suffice() {
    let target = EntityDescriptor::new("UserService", "class")
        .with_attribute("layer", "domain");
    let other = EntityDescriptor::new("AuthService", "class")
        .with_attribute("layer", "infra");
    let registry = reg_with(vec![target.clone(), other]);
    let desc = distinguishing_subgraph(&target, &registry, &[]);
    assert_eq!(desc.attributes, vec!["domain".to_string()]);
    assert!(desc.relation.is_none());
}

#[test]
fn graph_reg_adds_relation_when_attributes_dont_disambiguate() {
    // Two handlers, both in the same layer, differentiated only by what they call.
    let target = EntityDescriptor::new("LoginHandler", "function")
        .with_attribute("layer", "api")
        .with_relation("calls", "AuthService");
    let other = EntityDescriptor::new("LogoutHandler", "function")
        .with_attribute("layer", "api")
        .with_relation("calls", "SessionService");
    let registry = reg_with(vec![target.clone(), other]);
    let desc = distinguishing_subgraph(&target, &registry, &[]);
    // Attributes alone don't distinguish (both are `api` layer), so the
    // relation must be included.
    assert_eq!(desc.relation, Some(("calls".to_string(), "AuthService".to_string())));
}

#[test]
fn graph_reg_skips_shared_relation_picks_next() {
    // Two handlers, both call the same logging service; but target has
    // an additional distinguishing relation.
    let target = EntityDescriptor::new("LoginHandler", "function")
        .with_relation("calls", "LogService")
        .with_relation("tests", "LoginTests");
    let other = EntityDescriptor::new("LogoutHandler", "function")
        .with_relation("calls", "LogService")
        .with_relation("tests", "LogoutTests");
    let registry = reg_with(vec![target.clone(), other]);
    let desc = distinguishing_subgraph(&target, &registry, &[]);
    assert_eq!(
        desc.relation,
        Some(("tests".to_string(), "LoginTests".to_string()))
    );
}

#[test]
fn graph_reg_gives_up_when_nothing_distinguishes() {
    // Two identical entities — no attributes, no distinguishing relations.
    let target = EntityDescriptor::new("Foo", "thing")
        .with_relation("calls", "X");
    let other = EntityDescriptor::new("Bar", "thing")
        .with_relation("calls", "X");
    let registry = reg_with(vec![target.clone(), other]);
    let desc = distinguishing_subgraph(&target, &registry, &[]);
    // Returns whatever attributes D&R could find (likely none) and no relation.
    assert!(desc.relation.is_none());
}

#[test]
fn graph_reg_combines_attributes_and_relation() {
    // Three handlers:
    // - Target: layer=api, calls=AuthService
    // - Distractor 1: layer=api, calls=SessionService (differs by relation)
    // - Distractor 2: layer=web, calls=AuthService (differs by attribute)
    //
    // D&R picks `layer=api` to exclude distractor 2, leaving distractor 1.
    // Graph-based adds `calls=AuthService` to exclude distractor 1.
    let target = EntityDescriptor::new("LoginHandler", "function")
        .with_attribute("layer", "api")
        .with_relation("calls", "AuthService");
    let d1 = EntityDescriptor::new("LogoutHandler", "function")
        .with_attribute("layer", "api")
        .with_relation("calls", "SessionService");
    let d2 = EntityDescriptor::new("ProfileHandler", "function")
        .with_attribute("layer", "web")
        .with_relation("calls", "AuthService");
    let registry = reg_with(vec![target.clone(), d1, d2]);
    let desc = distinguishing_subgraph(&target, &registry, &[]);
    assert_eq!(desc.attributes, vec!["api".to_string()]);
    assert_eq!(desc.relation, Some(("calls".to_string(), "AuthService".to_string())));
}
```

### 3.4 Re-export

In `nlg-core/src/lib.rs`:

```rust
#[cfg(feature = "reg")]
pub use reg::{
    distinguishing_attributes, distinguishing_subgraph,
    EntityDescriptor, EntityRegistry, SubgraphDescription,
};
```

### 3.5 Verify

```bash
cargo test --all-features
cargo test --features serde
cargo clippy --all-features -- -D warnings
```

**Commit:** `Implement graph-based REG distinguishing_subgraph`

---

## Phase 4 — Wire into `pipe_refer` and add integration tests

**File:** `nlg-core/src/engine.rs`, `nlg-core/tests/integration.rs` (or new `nlg-core/tests/graph_reg.rs`)

### 4.1 Dispatch in `pipe_refer`

Find where `pipe_refer` handles the Full form — it calls `distinguishing_attributes` to premodify the type + name. Replace with a dispatch on `self.reg_algorithm`:

```rust
#[cfg(feature = "reg")]
let (attrs, relation) = match self.reg_algorithm {
    RegAlgorithm::DaleReiter => (
        distinguishing_attributes(target, &self.entity_registry, &self.reg_preference),
        None,
    ),
    RegAlgorithm::GraphBased => {
        let desc = distinguishing_subgraph(target, &self.entity_registry, &self.reg_preference);
        (desc.attributes, desc.relation)
    }
};
```

Emit the Full form. Today it's something like:

```rust
format!("The {} {} {}", attrs.join(" "), entity_type, name)
```

With relation:

```rust
let mut out = String::with_capacity(64);
out.push_str("The ");
if !attrs.is_empty() {
    out.push_str(&attrs.join(" "));
    out.push(' ');
}
out.push_str(entity_type);
out.push(' ');
out.push_str(name);
if let Some((label, target_name)) = relation {
    out.push(' ');
    out.push_str(&label);
    out.push(' ');
    out.push_str(&target_name);
}
```

The whitespace around `attrs` is important — when `attrs` is empty, don't emit a double space. Today's code already handles this; mirror it.

### 4.2 Integration tests

Add `nlg-core/tests/graph_reg.rs`:

```rust
#![cfg(feature = "reg")]

use nlg_core::{
    Context, Engine, EntityDescriptor, RegAlgorithm, Session, Strictness, Value, Variation,
};
use nlg_grammar_en::English;

fn engine_graph() -> Engine {
    Engine::new(English::new())
        .strictness(Strictness::Strict)
        .variation(Variation::Fixed)
        .reg_algorithm(RegAlgorithm::GraphBased)
}

#[test]
fn graph_based_relation_in_referring_expression() {
    let mut engine = engine_graph();
    engine.register_entity(
        EntityDescriptor::new("LoginHandler", "function")
            .with_attribute("layer", "api")
            .with_relation("that calls", "AuthService"),
    );
    engine.register_entity(
        EntityDescriptor::new("LogoutHandler", "function")
            .with_attribute("layer", "api")
            .with_relation("that calls", "SessionService"),
    );
    engine.register_template("t", "{name|refer} was modified").unwrap();

    let mut session = Session::new();
    let mut ctx = Context::new();
    ctx.insert("entity_type", Value::String("function".into()));
    ctx.insert("name", Value::String("LoginHandler".into()));

    let out = engine.render(&mut session, "t", &ctx).unwrap();
    assert!(
        out.contains("that calls AuthService"),
        "expected relation clause, got: {out}"
    );
}

#[test]
fn graph_based_falls_back_to_plain_when_attributes_suffice() {
    let mut engine = engine_graph();
    engine.register_entity(
        EntityDescriptor::new("UserService", "class")
            .with_attribute("layer", "domain"),
    );
    engine.register_entity(
        EntityDescriptor::new("AuthService", "class")
            .with_attribute("layer", "infra"),
    );
    engine.register_template("t", "{name|refer} was modified").unwrap();

    let mut session = Session::new();
    let mut ctx = Context::new();
    ctx.insert("entity_type", Value::String("class".into()));
    ctx.insert("name", Value::String("UserService".into()));

    let out = engine.render(&mut session, "t", &ctx).unwrap();
    // Attributes disambiguate; no relation clause emitted.
    assert!(out.contains("The domain class UserService"), "got: {out}");
    assert!(!out.contains("that"), "no relation clause expected: {out}");
}

#[test]
fn graph_based_default_dale_reiter_does_not_emit_relations() {
    // Same registry as first test but with default algorithm — no relation in output.
    let mut engine = Engine::new(English::new())
        .strictness(Strictness::Strict)
        .variation(Variation::Fixed);
    engine.register_entity(
        EntityDescriptor::new("LoginHandler", "function")
            .with_attribute("layer", "api")
            .with_relation("that calls", "AuthService"),
    );
    engine.register_entity(
        EntityDescriptor::new("LogoutHandler", "function")
            .with_attribute("layer", "api")
            .with_relation("that calls", "SessionService"),
    );
    engine.register_template("t", "{name|refer} was modified").unwrap();

    let mut session = Session::new();
    let mut ctx = Context::new();
    ctx.insert("entity_type", Value::String("function".into()));
    ctx.insert("name", Value::String("LoginHandler".into()));

    let out = engine.render(&mut session, "t", &ctx).unwrap();
    // D&R can't disambiguate (both api layer) — emits ambiguous output.
    // The test confirms D&R doesn't accidentally use relations.
    assert!(!out.contains("calls"), "D&R should not emit relation clause: {out}");
}
```

### 4.3 Verify

```bash
cargo test --all-features
cargo test --no-default-features
cargo test --features serde
cargo clippy --all-features -- -D warnings
cargo doc --all-features --no-deps
cargo bench --bench engine -- --test
```

**Commit:** `Wire graph-based REG into the refer pipe with relation surface form`

---

## Phase 5 — Final verification

```bash
cargo test --all-features
cargo test --no-default-features
cargo test --features serde
cargo clippy --all-features -- -D warnings
cargo doc --all-features --no-deps
cargo bench --bench engine -- --test
```

Test count should increase by ~15–18 → **~498–501 total**.

**Report:** 4 commit hashes, test count delta per feature variant, any surprises around the internal refactor of `distinguishing_attributes` to share its distractor-survival logic with `distinguishing_subgraph`.

---

## Risk register

| Risk | Mitigation |
|---|---|
| Extending `EntityDescriptor` with a new field breaks serde round-trip | `#[serde(default)]` on `relations` + `Default` on Vec handles missing-field deserialization. Existing serialized payloads round-trip. Test with the existing `entity_descriptor_roundtrips` suite. |
| Refactoring `distinguishing_attributes` to share internals with `distinguishing_subgraph` introduces semantic drift | Extract the shared helper `incremental_attributes_with_remaining` and have both public functions call it. Existing `distinguishing_attributes` tests (all of `reg::tests::*`) must continue to pass unchanged. |
| The `remaining` distractor list in the helper must semantically match the final state inside `distinguishing_attributes` | The existing D&R loop already tracks distractors progressively. Return `distractors` at the end of the walk rather than implementing a separate "survival" pass. |
| Relation label contains characters that look weird in the rendered expression | Users choose labels. Document that labels are inserted verbatim. If they want "that calls", they write "that calls". Garbage in, garbage out is fine at the API level. |
| `pipe_refer` integration accidentally changes D&R behaviour | The `RegAlgorithm::DaleReiter` branch calls the SAME `distinguishing_attributes` function as today, with the SAME arguments. Existing D&R tests (`reg_disambiguates_two_same_type_entities_in_narrative`, `reg_unambiguous_single_entity_skips_attributes`, `reg_registered_entity_with_unregistered_distractor`) must pass unchanged. |
| Trailing whitespace when `attrs` is empty AND no relation — `"The  function LoginHandler"` | The emission code needs one if-guard around `attrs.is_empty()`. Covered by the existing D&R zero-attribute test. |
| Two relations with the same label but different targets — the scan picks the first, ignoring the rest | Acceptable for v1. Preference order matters; users order relations as they want them considered. |
| `#[cfg(feature = "reg")]` must gate the new `RegAlgorithm` enum, builder, and all integration | Mirror the existing `#[cfg(feature = "reg")]` gates on `EntityDescriptor` and `reg_preference`. The `--no-default-features` build must compile cleanly without `reg`. |
| `SubgraphDescription` without serde feature flag — does it need the derive? | Keep it `pub` without serde; add `#[cfg_attr(feature = "serde", derive(...))]` if users might want to serialize it. Follow the existing `EntityDescriptor` pattern. |

## What NOT to do

- **Do not** implement Branch & Bound. Greedy only.
- **Do not** recursively REG the relation target.
- **Do not** support multi-hop relations.
- **Do not** add user-configurable cost functions.
- **Do not** change D&R.
- **Do not** allow multiple relations in one referring expression.
- **Do not** amend commits.
- **Do not** introduce `unsafe`.

## Definition of done

- [ ] Phase 0 baseline clean
- [ ] 4 commits with specified subject lines
- [ ] `EntityDescriptor.relations: Vec<(String, String)>` field + `with_relation` + `relation` methods
- [ ] `RegAlgorithm` enum + `Engine::reg_algorithm` builder option, feature-gated on `reg`
- [ ] `distinguishing_subgraph` function returning `SubgraphDescription`
- [ ] `pipe_refer` dispatches on algorithm choice and emits relation clause when present
- [ ] All 483 → ~500 tests pass on all feature variants
- [ ] `cargo clippy --all-features -- -D warnings` clean
- [ ] `Engine: Send + Sync` assert still compiles
- [ ] No public API breakage on the existing D&R path
- [ ] No `unsafe`
