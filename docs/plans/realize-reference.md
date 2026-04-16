# Plan: Split `ReferenceForm` into Policy + `Language::realize_reference`

**Owner:** sonnet agent
**Scope:** `prosaic-core/src/discourse.rs` (ReferenceForm extension) + `prosaic-core/src/language.rs` (trait method) + `prosaic-core/src/engine.rs` (pipe_refer refactor)
**Estimated size:** ~200–300 LOC including tests
**Test gate:** all 598 existing tests pass; new tests add ~10–15; zero warnings
**Branch discipline:** local only, commit at each sub-phase

---

## Why

Today `pipe_refer_single` in `engine.rs` has English-specific pronoun logic baked in:

```rust
ReferenceForm::Pronoun => {
    if self.session.discourse.focus_is_plural() { "they".to_string() }
    else { "it".to_string() }
}
```

That's fine for English but breaks the moment a non-English grammar needs:
- **Spanish:** "él" / "ella" / "ellos" / "ellas" — gender- and number-aware pronouns
- **German:** "er" / "sie" / "es" / "sie" — gender and number (with case declension beyond v1.5)
- **Japanese:** zero-pronoun (empty realization) — pronouns are typically dropped when recoverable from context

The split:
- **Policy (unchanged):** `DiscourseState::reference_form(name)` decides Full/Short/Pronoun/Demonstrative/Zero based on language-agnostic rules (distance, ambiguity, Cb tracking for Centering Rule 1).
- **Realization (new):** `Language::realize_reference(form, features)` produces the language-specific surface string for Pronoun/Demonstrative/Zero.

Full and ShortName continue to route through REG (D&R, graph-based) at the engine layer — those are entity-attribute concerns, not language-realization concerns.

## Design (locked)

### `ReferenceForm` extension

Add two new variants to the existing enum in `discourse.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ReferenceForm {
    /// Full form: "The class UserService" (possibly with REG attributes)
    Full,
    /// Name only: "UserService"
    ShortName,
    /// Pronoun: "it" / "they" / (lang-specific)
    Pronoun,
    /// Demonstrative determiner + type: "this class" / (lang-specific).
    /// Reserved slot for future discourse rules; not currently emitted by
    /// `DiscourseState::reference_form`.
    Demonstrative,
    /// Zero realization: surface is empty. Used by pro-drop languages
    /// (Japanese, colloquial Spanish/Italian) where the pronoun is
    /// recoverable from context and the slot emits nothing.
    /// Not currently emitted by the default `DiscourseState::reference_form`;
    /// language-specific discourse extensions may choose this form.
    Zero,
}
```

Both new variants are additive. Existing tests asserting Full/ShortName/Pronoun continue to pass.

### `Language::realize_reference` trait method

Add to the `Language` trait in `language.rs` with a default implementation that handles English:

```rust
/// Realize a reference form as surface text for this language.
///
/// The discourse policy layer chooses the [`ReferenceForm`] (Full,
/// ShortName, Pronoun, Demonstrative, or Zero) based on
/// language-agnostic rules. This method converts that choice into the
/// language-specific surface string.
///
/// Only `Pronoun`, `Demonstrative`, and `Zero` are meaningfully handled
/// here. `Full` and `ShortName` route through the engine's REG layer
/// (Dale & Reiter, graph-based) because they involve entity-attribute
/// logic that's not the language's concern.
///
/// Returns:
/// - `Some(text)` for a realized form (e.g., `"it"`, `"they"`, `"this"`).
/// - `None` for `Zero` (pro-drop) or for `Full`/`ShortName` — the caller
///   handles those via REG.
///
/// The default implementation encodes English:
/// - `Pronoun`: `"they"` when `features.number` is `Plural` or `Dual`,
///   `"it"` otherwise.
/// - `Demonstrative`: `"this"`.
/// - `Zero`: `None` (English doesn't drop pronouns).
/// - `Full` / `ShortName`: `None` (engine handles).
fn realize_reference(
    &self,
    form: ReferenceForm,
    features: &AgreementFeatures,
) -> Option<String> {
    match form {
        ReferenceForm::Pronoun => {
            Some(match features.number {
                GrammaticalNumber::Plural | GrammaticalNumber::Dual => "they".to_string(),
                _ => "it".to_string(),
            })
        }
        ReferenceForm::Demonstrative => Some("this".to_string()),
        ReferenceForm::Zero => None,
        ReferenceForm::Full | ReferenceForm::ShortName => None,
    }
}
```

Imports needed in `language.rs`:
- `use crate::discourse::ReferenceForm;`
- `use crate::agreement::{AgreementFeatures, GrammaticalNumber};`

### `pipe_refer_single` refactor

In `engine.rs`, replace the English-hardcoded pronoun match with a delegation to the language:

```rust
let form = self.session.discourse.reference_form(&name);

let rendered = match form {
    ReferenceForm::Full => self.engine.render_full_reference(&name, &entity_type),
    ReferenceForm::ShortName => name,
    ReferenceForm::Pronoun | ReferenceForm::Demonstrative | ReferenceForm::Zero => {
        // Synthesize features from discourse state. Today this is just
        // the plural flag; v1.5+ multilingual grammars can thread richer
        // features through via Value::Entity at the call site.
        let features = AgreementFeatures {
            number: if self.session.discourse.focus_is_plural() {
                GrammaticalNumber::Plural
            } else {
                GrammaticalNumber::Singular
            },
            ..AgreementFeatures::default()
        };
        self.engine.language
            .realize_reference(form, &features)
            .unwrap_or_default()
    }
};
```

For `Zero` forms: `realize_reference` returns `None`; `.unwrap_or_default()` produces an empty string. The rendering pipeline's silent-mode cleanup will handle any stray whitespace around the empty slot.

### Out of scope for this plan

- **Do not** thread `Value::Entity` features into the REG path. That's a deeper refactor — `pipe_refer_single` would need to resolve the slot value as an Entity and extract its features. Future v1.5 work.
- **Do not** extend `DiscourseState::reference_form` to emit `Zero` or `Demonstrative`. Those are reserved for future language-specific extensions.
- **Do not** override `realize_reference` in `prosaic-grammar-en`. Use the default.
- **Do not** change any public API — `ReferenceForm` gains variants but existing variants are unchanged; `Language::realize_reference` has a default impl so existing impls keep compiling.

---

## Phase 0 — Baseline

```bash
cargo test --all-features
cargo clippy --all-features -- -D warnings
```

Expected: **598 tests passing**, 0 warnings.

**No commit.**

---

## Phase 1 — Add `Zero` and `Demonstrative` variants to `ReferenceForm`

**File:** `prosaic-core/src/discourse.rs`

1. Add the two variants to the enum per the design above.
2. Update every `match ReferenceForm::*` in the codebase — grep for `ReferenceForm::` patterns. Most sites will need `Zero => ...` and `Demonstrative => ...` arms.

Likely touch points:
- `engine.rs::pipe_refer_single` (handled in Phase 2)
- `engine.rs::render_explained` (if it maps `ReferenceForm` for diagnostics — add passthrough)
- Test helpers that pattern-match (unlikely but possible)

### Tests

```rust
#[test]
fn reference_form_all_variants_distinct() {
    // Sanity: ensure the new variants are distinguishable.
    assert_ne!(ReferenceForm::Full, ReferenceForm::Zero);
    assert_ne!(ReferenceForm::Pronoun, ReferenceForm::Demonstrative);
    assert_ne!(ReferenceForm::Zero, ReferenceForm::Demonstrative);
}
```

Verify: `cargo test --all-features && cargo clippy --all-features -- -D warnings`.

**Commit:** `Add Zero and Demonstrative variants to ReferenceForm`

---

## Phase 2 — Add `Language::realize_reference` trait method

**File:** `prosaic-core/src/language.rs`

1. Import `ReferenceForm` and agreement types.
2. Add the trait method with default implementation per the design.

### Tests

In `language.rs` `#[cfg(test)]`:

```rust
#[test]
fn realize_reference_pronoun_singular() {
    let lang = MiniLang; // existing helper
    let f = AgreementFeatures::default(); // number=Unknown → falls through to "it"
    assert_eq!(
        lang.realize_reference(ReferenceForm::Pronoun, &f),
        Some("it".to_string())
    );
}

#[test]
fn realize_reference_pronoun_plural() {
    let lang = MiniLang;
    let f = AgreementFeatures::default().with_number(GrammaticalNumber::Plural);
    assert_eq!(
        lang.realize_reference(ReferenceForm::Pronoun, &f),
        Some("they".to_string())
    );
}

#[test]
fn realize_reference_pronoun_dual() {
    let lang = MiniLang;
    let f = AgreementFeatures::default().with_number(GrammaticalNumber::Dual);
    assert_eq!(
        lang.realize_reference(ReferenceForm::Pronoun, &f),
        Some("they".to_string())
    );
}

#[test]
fn realize_reference_demonstrative() {
    let lang = MiniLang;
    let f = AgreementFeatures::default();
    assert_eq!(
        lang.realize_reference(ReferenceForm::Demonstrative, &f),
        Some("this".to_string())
    );
}

#[test]
fn realize_reference_zero_is_none() {
    let lang = MiniLang;
    let f = AgreementFeatures::default();
    assert_eq!(lang.realize_reference(ReferenceForm::Zero, &f), None);
}

#[test]
fn realize_reference_full_is_none() {
    // Full form is handled by engine REG, not the language layer.
    let lang = MiniLang;
    let f = AgreementFeatures::default();
    assert_eq!(lang.realize_reference(ReferenceForm::Full, &f), None);
}
```

Verify: `cargo test --all-features && cargo clippy --all-features -- -D warnings`.

**Commit:** `Add Language::realize_reference trait method for surface-form realization`

---

## Phase 3 — Refactor `pipe_refer_single` to call `realize_reference`

**File:** `prosaic-core/src/engine.rs`

Replace the existing `ReferenceForm::Pronoun` match arm with a delegated call per the design. Covers `Pronoun | Demonstrative | Zero` in one arm that calls `realize_reference`.

### Tests

Existing pronoun tests (`refer_second_mention_uses_pronoun`, etc.) must still pass — the default `realize_reference` impl produces byte-identical output to the old inline logic for English.

Add one new test to prove the indirection works:

```rust
#[test]
fn pronoun_realization_routes_through_language_trait() {
    // This test exists purely to assert that the refactor preserved
    // the English pronoun output. If it fails, the trait default impl
    // doesn't match the old inline logic.
    let mut engine = test_engine();
    engine.register_template("t", "{name|refer} was modified").unwrap();
    let mut session = Session::new();
    let mut ctx = Context::new();
    ctx.insert("entity_type", Value::String("class".into()));
    ctx.insert("name", Value::String("Foo".into()));

    // First render: Full form
    let r1 = engine.render(&mut session, "t", &ctx).unwrap();
    assert!(r1.contains("The class Foo"), "got: {r1}");

    // Second render: Pronoun, should be "It"
    let r2 = engine.render(&mut session, "t", &ctx).unwrap();
    assert!(r2.to_lowercase().contains("it "), "got: {r2}");
}
```

Verify:

```bash
cargo test --all-features
cargo test --no-default-features
cargo test --features serde
cargo clippy --all-features -- -D warnings
cargo doc --all-features --no-deps
```

**Commit:** `Refactor pipe_refer to delegate pronoun realization to Language trait`

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

Test count up by ~10–15 → **~613 total**.

**Report:** 3 commit hashes, test count delta, whether any existing pronoun tests changed output.

---

## Risk register

| Risk | Mitigation |
|---|---|
| Adding variants to a public enum breaks exhaustive matches | Let the compiler find them. Existing variants still exist; new ones need arms added (often as passthrough). |
| Refactored `pipe_refer_single` produces different pronoun output than before | Default `realize_reference` impl mirrors the old inline logic exactly. Run existing pronoun tests to verify. |
| `GrammaticalNumber::Unknown` in features → default impl falls through to "it" | Correct — preserves existing behavior. Discourse's `focus_is_plural` flag maps `true → Plural`, `false → Singular` (not Unknown). Verify the synthesis in the refactor. |
| `Demonstrative` is reserved but never emitted by discourse | Acknowledged. Future work can make discourse choose Demonstrative for specific contexts (e.g., "Consider [the class Foo]... This class..."). |
| `Zero` variant goes through the "unwrap_or_default" path producing an empty string — cleanup may not trim adjacent whitespace | Existing silent-mode cleanup strips double spaces. Verify with a test if concerned. For v1.5 with no language returning Zero yet, it's aspirational. |
| `serde` round-trip for new ReferenceForm variants | Auto-derive produces `"Zero"` / `"Demonstrative"` names. Add a quick round-trip test for each. |
| Pattern matches in tests that use `ReferenceForm` exhaustively | Grep for test sites; most will be the narrow engine/discourse tests that already name specific variants. |

## What NOT to do

- **Do not** extend `DiscourseState::reference_form` to emit Zero or Demonstrative.
- **Do not** override `realize_reference` in `prosaic-grammar-en`.
- **Do not** refactor the `render_full_reference` path (REG handles Full/ShortName).
- **Do not** thread Value::Entity features into `pipe_refer` for Pronoun realization. Future work.
- **Do not** amend commits.

## Definition of done

- [ ] `ReferenceForm` has `Zero` and `Demonstrative` variants
- [ ] `Language::realize_reference` trait method with default impl
- [ ] `pipe_refer_single` delegates Pronoun/Demonstrative/Zero to `realize_reference`
- [ ] All 598 existing tests pass + ~10-15 new
- [ ] Existing pronoun output is byte-identical
- [ ] `cargo clippy --all-features -- -D warnings` clean
- [ ] `Engine: Send + Sync` assert still compiles
- [ ] 3 commits with specified subject lines
