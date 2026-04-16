# Plan: `AgreementFeatures` Struct + `Value::Entity` Variant

**Owner:** sonnet agent
**Scope:** `prosaic-core` only (new module, new Value variant, new context helper)
**Estimated size:** ~400–500 LOC including tests
**Test gate:** all 534 existing tests pass; new tests add ~15–20; zero warnings
**Branch discipline:** local only, commit at each sub-phase

---

## Why

The keystone of v1.5 multilingual. Gives the engine a typed channel for grammatical features (gender, number, case, definiteness, animacy, person) that non-English grammars will consult. English ignores all of it — zero runtime cost on the existing path. With this in place, `prosaic-grammar-es` / `-de` / future-languages can drop in without core engine changes.

Today's `Value::String(name)` loses agreement info even if the caller knows it. After this plan, the caller can instead pass `Value::Entity { name, features }` via `entity("UserService").fem().sing().defined()`.

## Design (locked)

### New module `prosaic-core/src/agreement.rs`

```rust
//! Grammatical agreement features for multilingual rendering.
//!
//! Carries gender / number / case / definiteness / animacy / person
//! metadata on entity-typed context values. The English grammar layer
//! ignores these features entirely; non-English grammars (`-es`, `-de`,
//! etc.) consult them to produce correctly-agreeing articles, adjectives,
//! pronouns, and verb forms.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Gender {
    #[default]
    Unknown,
    Masc,
    Fem,
    Neut,
    /// Dutch, Scandinavian 2-gender ("common" + "neuter") systems.
    Common,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Number {
    #[default]
    Unknown,
    Singular,
    Plural,
    /// Arabic, Slovenian, Biblical Hebrew dual number.
    Dual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Case {
    #[default]
    Unknown,
    Nominative,
    Accusative,
    Dative,
    Genitive,
    // Additional cases (instrumental, locative, ablative, etc.)
    // reserved for v2 when Finnish/Russian/etc. become targets.
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Definiteness {
    #[default]
    Unknown,
    Definite,
    Indefinite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Animacy {
    #[default]
    Unknown,
    Animate,
    Inanimate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Person {
    #[default]
    Third,
    First,
    Second,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AgreementFeatures {
    pub gender: Gender,
    pub number: Number,
    pub case: Case,
    pub definiteness: Definiteness,
    pub animacy: Animacy,
    pub person: Person,
}

impl AgreementFeatures {
    pub fn new() -> Self { Self::default() }
    pub fn with_gender(mut self, g: Gender) -> Self { self.gender = g; self }
    pub fn with_number(mut self, n: Number) -> Self { self.number = n; self }
    pub fn with_case(mut self, c: Case) -> Self { self.case = c; self }
    pub fn with_definiteness(mut self, d: Definiteness) -> Self { self.definiteness = d; self }
    pub fn with_animacy(mut self, a: Animacy) -> Self { self.animacy = a; self }
    pub fn with_person(mut self, p: Person) -> Self { self.person = p; self }
}
```

### `Value::Entity` variant

Extend `prosaic-core/src/context.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Value {
    String(String),
    Number(i64),
    List(Vec<String>),
    /// A named entity carrying agreement features for multilingual rendering.
    /// In English rendering this behaves identically to `Value::String(name)`.
    Entity {
        name: String,
        #[cfg_attr(feature = "serde", serde(default))]
        features: AgreementFeatures,
    },
}
```

`#[serde(default)]` on `features` so older serialized payloads without features round-trip cleanly.

### `Value::as_display` behaviour

`Value::Entity { name, .. }` → `Cow::Borrowed(name)`. Identical to how `Value::String` renders.

### `EntityValue` builder + `entity()` helper

In `context.rs`:

```rust
/// Fluent builder for entity-typed context values with agreement features.
/// Produced by [`entity()`]; consumed into a [`Value::Entity`] via
/// [`IntoValue::into_value`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityValue {
    name: String,
    features: AgreementFeatures,
}

impl EntityValue {
    // Gender shortcuts
    pub fn masc(mut self) -> Self { self.features.gender = Gender::Masc; self }
    pub fn fem(mut self) -> Self { self.features.gender = Gender::Fem; self }
    pub fn neut(mut self) -> Self { self.features.gender = Gender::Neut; self }
    pub fn common(mut self) -> Self { self.features.gender = Gender::Common; self }

    // Number shortcuts
    pub fn sing(mut self) -> Self { self.features.number = Number::Singular; self }
    pub fn plur(mut self) -> Self { self.features.number = Number::Plural; self }
    pub fn dual(mut self) -> Self { self.features.number = Number::Dual; self }

    // Definiteness shortcuts
    pub fn defined(mut self) -> Self { self.features.definiteness = Definiteness::Definite; self }
    pub fn indef(mut self) -> Self { self.features.definiteness = Definiteness::Indefinite; self }

    // Animacy shortcuts
    pub fn animate(mut self) -> Self { self.features.animacy = Animacy::Animate; self }
    pub fn inanimate(mut self) -> Self { self.features.animacy = Animacy::Inanimate; self }

    // Case — no shortcut methods; use .with_case() via features() or a case() builder
    pub fn case(mut self, c: Case) -> Self { self.features.case = c; self }

    // Person — same pattern
    pub fn person(mut self, p: Person) -> Self { self.features.person = p; self }

    // Direct feature override for callers who want full control
    pub fn with_features(mut self, f: AgreementFeatures) -> Self { self.features = f; self }

    /// Consume into a [`Value::Entity`].
    pub fn build(self) -> Value {
        Value::Entity { name: self.name, features: self.features }
    }
}

/// Create an [`EntityValue`] for a named entity with default (unknown)
/// agreement features. Chain builder methods to set gender/number/etc.
///
/// ```
/// use prosaic_core::{ctx, entity};
/// let c = ctx! {
///     user: entity("Alice").fem().sing().defined(),
///     service: entity("UserService"),  // features stay Unknown — English default
/// };
/// ```
pub fn entity(name: impl Into<String>) -> EntityValue {
    EntityValue {
        name: name.into(),
        features: AgreementFeatures::default(),
    }
}

impl IntoValue for EntityValue {
    fn into_value(self) -> Value {
        self.build()
    }
}
```

### Re-exports in `lib.rs`

Add:
- `pub mod agreement;`
- `pub use agreement::{AgreementFeatures, Gender, Number, Case, Definiteness, Animacy, Person};`
- `pub use context::{entity, EntityValue};`

### Engine integration

Where `Value::Entity` needs handling:
- **`as_display`:** already in the plan — `Cow::Borrowed(name)`
- **Template slot rendering (`render_slot`):** after resolving to `Value`, just call `as_display()`. Entity renders as its name. No special-casing.
- **Pipes that read lists (`|join`, `|truncate`):** `Value::Entity` is not a list; these pipes should error with the existing "value must be a list" message. No change needed — existing `as_list()` returns None for non-List variants.
- **Pipes that read strings (`|capitalize`, `|article`, `|pluralize`, `|refer`, etc.):** they call `as_display()` already; Entity works. No change.
- **`|refer` pipe specifically:** today it looks up the entity name in the `EntityRegistry` (REG). With `Value::Entity`, the name is the same lookup key. `features` are currently unused by `|refer` — that's OK for v1.5 Phase 1. Future work (v1.5 Phase 2+) can route features into the language layer.

### Out of scope for this plan

- **Do not** change `Language` trait methods yet. Features exist but nothing reads them.
- **Do not** implement `realize_reference`. Separate plan.
- **Do not** implement `plural_category`. Separate plan.
- **Do not** add Spanish grammar. Separate plan.
- **Do not** thread features into the REG subgraph result.
- **Do not** rename `EntityDescriptor` (REG registry) or its methods. It's distinct from `EntityValue` (context carrier).

---

## Phase 0 — Baseline

```bash
cargo test --all-features
cargo clippy --all-features -- -D warnings
```

Expected: **534 tests passing**, 0 warnings.

**No commit.**

---

## Phase 1 — Add `agreement` module

Create `prosaic-core/src/agreement.rs` with the full content from the design above. Add `pub mod agreement;` to `lib.rs`. Add re-exports.

Tests (in `agreement.rs`):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_all_unknown() {
        let f = AgreementFeatures::default();
        assert_eq!(f.gender, Gender::Unknown);
        assert_eq!(f.number, Number::Unknown);
        assert_eq!(f.case, Case::Unknown);
        assert_eq!(f.definiteness, Definiteness::Unknown);
        assert_eq!(f.animacy, Animacy::Unknown);
        assert_eq!(f.person, Person::Third);
    }

    #[test]
    fn builder_with_methods_set_fields() {
        let f = AgreementFeatures::new()
            .with_gender(Gender::Fem)
            .with_number(Number::Singular)
            .with_case(Case::Accusative)
            .with_definiteness(Definiteness::Definite)
            .with_animacy(Animacy::Animate)
            .with_person(Person::First);
        assert_eq!(f.gender, Gender::Fem);
        assert_eq!(f.number, Number::Singular);
        assert_eq!(f.case, Case::Accusative);
        assert_eq!(f.definiteness, Definiteness::Definite);
        assert_eq!(f.animacy, Animacy::Animate);
        assert_eq!(f.person, Person::First);
    }

    #[test]
    fn features_are_copy() {
        fn takes_copy<T: Copy>(_: T) {}
        takes_copy(AgreementFeatures::default());
        takes_copy(Gender::Fem);
        takes_copy(Number::Plural);
    }
}
```

Verify: `cargo test --all-features && cargo clippy --all-features -- -D warnings`

**Commit:** `Add AgreementFeatures struct with gender/number/case/definiteness/animacy/person axes`

---

## Phase 2 — Add `Value::Entity` variant

Extend `Value` in `context.rs`. This may require updating match arms elsewhere in the codebase — search for `match ... value` or `Value::String | Value::Number | Value::List` patterns and add `Value::Entity { name, .. } => ...` arms, typically mirroring the `Value::String` arm with the name field.

Likely touch points:
- `Value::as_display` — add Entity arm
- `Value::as_list` — already returns None for non-List; keep
- `Context` serialization — serde auto-derives, but verify with a test
- `as_display_plain` or similar helpers if they exist
- The CLI crate's `context_from_slots` may need a new arm if it pattern-matches, but since it only builds Values (never matches them), likely not affected

Any place that does exhaustive `match Value::*` will fail to compile — let the compiler find them all.

### Tests

In `context.rs`:

```rust
#[test]
fn entity_display_is_name() {
    let v = Value::Entity {
        name: "UserService".into(),
        features: AgreementFeatures::default(),
    };
    assert_eq!(v.as_display(), "UserService");
}

#[test]
fn entity_as_list_is_none() {
    let v = Value::Entity {
        name: "X".into(),
        features: AgreementFeatures::default(),
    };
    assert!(v.as_list().is_none());
}
```

Verify all existing tests still pass.

**Commit:** `Add Value::Entity variant carrying AgreementFeatures`

---

## Phase 3 — `entity()` helper + `EntityValue` builder + `IntoValue`

Add to `context.rs` per the design. Re-export from `lib.rs`.

### Tests

```rust
#[test]
fn entity_helper_default_features() {
    let ev = entity("UserService");
    let v = ev.into_value();
    match v {
        Value::Entity { name, features } => {
            assert_eq!(name, "UserService");
            assert_eq!(features, AgreementFeatures::default());
        }
        _ => panic!("expected Value::Entity"),
    }
}

#[test]
fn entity_builder_chain_sets_features() {
    let v = entity("Alice").fem().sing().defined().animate().into_value();
    match v {
        Value::Entity { name, features } => {
            assert_eq!(name, "Alice");
            assert_eq!(features.gender, Gender::Fem);
            assert_eq!(features.number, Number::Singular);
            assert_eq!(features.definiteness, Definiteness::Definite);
            assert_eq!(features.animacy, Animacy::Animate);
        }
        _ => panic!("expected Value::Entity"),
    }
}

#[test]
fn ctx_macro_accepts_entity_value() {
    let c = ctx! {
        user: entity("Alice").fem().sing(),
        count: 3,
    };
    match c.get("user").unwrap() {
        Value::Entity { name, features } => {
            assert_eq!(name, "Alice");
            assert_eq!(features.gender, Gender::Fem);
        }
        _ => panic!("expected Value::Entity"),
    }
    assert_eq!(c.get("count"), Some(&Value::Number(3)));
}

#[test]
fn entity_in_template_renders_as_name() {
    use crate::Engine;
    use prosaic_grammar_en::English;
    let mut engine = Engine::new(English::new()).strictness(crate::Strictness::Silent);
    engine.register_template("t", "Welcome, {user}!").unwrap();
    let mut session = crate::Session::new();
    let c = ctx! { user: entity("Alice").fem().sing() };
    let out = engine.render(&mut session, "t", &c).unwrap();
    assert!(out.contains("Welcome, Alice"), "got: {out}");
}
```

Note: the rendering test requires `prosaic-grammar-en` which is a dev-dep of `prosaic-core` already. Confirm before writing it; if not, move the render test to an integration test file.

**Commit:** `Add entity() helper and EntityValue builder for fluent agreement-feature construction`

---

## Phase 4 — Serde round-trip + integration verification

### Serde test

Add to `prosaic-core/tests/serde.rs`:

```rust
#[test]
fn value_entity_roundtrips() {
    let v = Value::Entity {
        name: "UserService".into(),
        features: AgreementFeatures::default()
            .with_gender(Gender::Fem)
            .with_number(Number::Singular)
            .with_definiteness(Definiteness::Definite),
    };
    let json = serde_json::to_string(&v).unwrap();
    let back: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v, back);
}

#[test]
fn value_entity_without_features_deserializes() {
    // Legacy-style payload without the features field.
    let json = r#"{"Entity":{"name":"X"}}"#;
    let v: Value = serde_json::from_str(json).unwrap();
    match v {
        Value::Entity { name, features } => {
            assert_eq!(name, "X");
            assert_eq!(features, AgreementFeatures::default());
        }
        _ => panic!("expected Value::Entity"),
    }
}
```

### `nlg_template!` / `prosaic_template!` compatibility

The compile-time template validator in `prosaic-derive` doesn't inspect Value types — it only validates slot names and pipe names. Entity-valued slots pass through unchanged. No changes needed to `prosaic-derive`. Add a test to `prosaic-core/tests/prosaic_template_macro.rs` confirming a template with an entity-valued slot compiles and renders:

```rust
#[test]
fn prosaic_template_macro_accepts_entity_slot() {
    let tpl = prosaic_template! {
        template: "Welcome, {user}!",
        slots: [user],
    };
    assert_eq!(tpl, "Welcome, {user}!");
}
```

### Full verify

```bash
cargo test --all-features
cargo test --no-default-features
cargo test --features serde
cargo clippy --all-features -- -D warnings
cargo doc --all-features --no-deps
cargo bench --bench engine -- --test
```

All clean. Test count up by ~15–20 to **~550**.

**Commit:** `Add serde round-trip and template-macro compatibility for Value::Entity`

---

## Risk register

| Risk | Mitigation |
|---|---|
| Adding a `Value` variant is a minor breaking change for exhaustive matchers | The compiler will find every site. This is in-development code with no external users; all call sites live in the workspace. |
| `Value::Entity` serialization format for serde | Default serde derive produces `{"Entity":{"name":"...","features":{...}}}`. With `#[serde(default)]` on `features`, legacy `{"Entity":{"name":"..."}}` still deserializes. Test both. |
| `PartialEq/Eq` on `Value` with the new variant | Auto-derived. Since AgreementFeatures derives PartialEq/Eq, it works. |
| `Value::Entity` in the CLI's `context_from_slots` — JSON has no native "entity" type | Leave unsupported for v1.5 Phase 1. Users building entities via CLI pass plain strings; the CLI doesn't expose entity features today. Future CLI extension could accept a JSON object like `{"name":"X","features":{"gender":"Fem"}}` — documented as future work in the cookbook. |
| Case enum has only 4 variants — insufficient for Finnish/Russian | v2+ adds the rest. For v1.5 targeting Spanish (no case declension) and German (4 cases), current set is sufficient. Document the reserve note. |
| `AgreementFeatures` getting too big to be `Copy` in the future | Six `u8`-sized enums = 6 bytes. Easily Copy. If later expanded beyond ~24 bytes, drop Copy and callers adjust. |
| `EntityValue::build` vs `into_value` — two ways to convert | Both exist; `into_value` is for the `IntoValue` trait dispatch, `build` is sugar for callers who don't go through the trait. Consistent with many builder patterns. |
| Tests relying on `Value` being `String|Number|List` exhaustively | Expected; let compiler catch. Add the Entity arm to each match. |
| `Context::iter` already exists (per PARENT plan) and yields `(&str, &Value)` | No change needed. Entity values pass through. |

## What NOT to do

- Do not modify the `Language` trait.
- Do not touch `prosaic-grammar-en`.
- Do not change render behaviour of existing Value variants.
- Do not add Spanish or German anywhere.
- Do not rename `EntityDescriptor` (REG).
- Do not amend commits.

## Definition of done

- [ ] `agreement` module with 7 types (6 enums + AgreementFeatures struct)
- [ ] `Value::Entity { name, features }` variant
- [ ] `Value::as_display` handles Entity (returns name)
- [ ] `entity(name)` helper + `EntityValue` builder with gender/number/case/definiteness/animacy/person methods
- [ ] `IntoValue for EntityValue` impl
- [ ] All 534 existing tests still pass + ~15–20 new
- [ ] Serde round-trip verified
- [ ] `prosaic_template!` macro unchanged (entity slots compile)
- [ ] `cargo clippy --all-features -- -D warnings` clean
- [ ] `Engine: Send + Sync` assert still passes
- [ ] 4 commits with specified subject lines
