# Plan: `Language::plural_category` + `pluralize_with_category` + `|plural` Pipe

**Owner:** sonnet agent
**Scope:** `prosaic-core` trait additions + `prosaic-grammar-en` English default + new `|plural` pipe + whitelist update in `prosaic-derive`
**Estimated size:** ~300–400 LOC including tests
**Test gate:** all 579 existing tests pass; new tests add ~15–20; zero warnings
**Branch discipline:** local only, commit at each sub-phase

---

## Why

CLDR defines six plural categories — `Zero`, `One`, `Two`, `Few`, `Many`, `Other` — which different languages use in different combinations. English uses `One` / `Other`. Polish uses `One` / `Few` / `Many` / `Other`. Arabic uses all six. A `pluralize(word, count) -> String` method that returns one of two forms cannot represent these distinctions.

This plan introduces the trait surface. Non-English grammars will override when they ship. English's two-arm default handles the trivial case without ceremony.

**No icu4x dependency introduced here.** The `locale` Cargo feature that pulls in `icu_plurals::PluralRules` is introduced by `prosaic-grammar-es` (next plan) — for v1.5 English-only users, this plan adds zero deps.

## Design (locked)

### `PluralCategory` enum

```rust
// prosaic-core/src/language.rs
/// CLDR plural categories. Any language's `plural_category` implementation
/// maps an integer count into one of these six buckets. The subset a
/// language actually uses depends on its grammar:
/// - English: One | Other
/// - Spanish: One | Other
/// - Polish: One | Few | Many | Other
/// - Arabic: Zero | One | Two | Few | Many | Other
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PluralCategory {
    Zero,
    One,
    Two,
    Few,
    Many,
    #[default]
    Other,
}
```

### Trait methods

Add to `Language` trait with default implementations that encode English semantics (so grammar crates only override when their language differs):

```rust
/// Classify an integer count into a CLDR plural category.
///
/// Default implementation uses English rules: `n == 1` → `One`,
/// anything else → `Other`. Non-English grammars must override this
/// method to return correct categories for their language.
fn plural_category(&self, n: i64) -> PluralCategory {
    match n {
        1 => PluralCategory::One,
        _ => PluralCategory::Other,
    }
}

/// Produce the form of `word` appropriate for the given plural category.
///
/// Default implementation uses English rules: `One` returns the singular
/// form; anything else returns the plural form via `self.pluralize`.
/// Non-English grammars override for richer category sets (e.g., Polish
/// "one/few/many/other") and for agreement with gender/case.
fn pluralize_with_category(&self, word: &str, category: PluralCategory) -> String {
    match category {
        PluralCategory::One => word.to_string(),
        _ => self.pluralize(word, 2), // 2 picks the plural form in the legacy API
    }
}
```

### `|plural` pipe

New pipe in `engine.rs`:

```
{count|plural:service}   → "service" (count=1) or "services" (count≠1) under English
{count|plural:persona}   → "persona" / "personas" under Spanish (when -es ships)
{count|plural:ребенок}   → "ребенок" / "ребенка" / "детей" under Russian (future)
```

Pipe logic:
1. Read the slot as an integer (require `Value::Number`).
2. Require a string argument (the singular noun).
3. Compute `category = self.engine.language.plural_category(n)`.
4. Return `self.engine.language.pluralize_with_category(noun, category)` as `Value::String`.

Error cases:
- Non-numeric slot → `InvalidPipe { pipe: "plural", reason: "requires a numeric value" }`
- Missing argument → `InvalidPipe { pipe: "plural", reason: "requires a singular noun argument" }`

### Relationship to existing `|pluralize`

`|pluralize:word` already exists. It takes a count and a singular noun, returns the right form. Under English this is equivalent to `|plural:word`. **Keep both pipes.**

- **`|pluralize`** — the legacy pipe. Internally uses `Language::pluralize(word, count)`. Boolean singular-vs-plural.
- **`|plural`** — the new pipe. Internally uses `plural_category` + `pluralize_with_category`. CLDR-aware, ready for non-English grammars.

Rationale: don't break existing templates; users opt in to the new pipe when they need category-aware plurals. English output is identical between the two today; non-English output diverges.

### Out of scope for this plan

- **Do not** add icu4x or any other dep. That ships with `prosaic-grammar-es`.
- **Do not** add the `locale` Cargo feature flag to `prosaic-core`. The flag's first consumer is the Spanish grammar crate.
- **Do not** deprecate or remove `|pluralize`. Both pipes coexist.
- **Do not** override `plural_category` in `prosaic-grammar-en`. Use the default.
- **Do not** integrate `PluralCategory` into `AgreementFeatures`. They're related but independent — `PluralCategory` is per-count; `Number` in agreement features is a type-level entity property.

---

## Phase 0 — Baseline

```bash
cargo test --all-features
cargo clippy --all-features -- -D warnings
```

Expected: **579 tests passing**, 0 warnings.

**No commit.**

---

## Phase 1 — Add `PluralCategory` + trait methods

**File:** `prosaic-core/src/language.rs`

1. Add `PluralCategory` enum per the design.
2. Add `plural_category` and `pluralize_with_category` trait methods with the default implementations.
3. Re-export `PluralCategory` from `prosaic-core/src/lib.rs`.

### Tests

```rust
#[test]
fn default_plural_category_one() {
    let lang = MiniLang; // existing test helper
    assert_eq!(lang.plural_category(1), PluralCategory::One);
}

#[test]
fn default_plural_category_other_for_zero() {
    let lang = MiniLang;
    assert_eq!(lang.plural_category(0), PluralCategory::Other);
}

#[test]
fn default_plural_category_other_for_plurals() {
    let lang = MiniLang;
    assert_eq!(lang.plural_category(2), PluralCategory::Other);
    assert_eq!(lang.plural_category(17), PluralCategory::Other);
}

#[test]
fn default_plural_category_other_for_negatives() {
    let lang = MiniLang;
    assert_eq!(lang.plural_category(-5), PluralCategory::Other);
}

#[test]
fn default_pluralize_with_category_one_is_singular() {
    let lang = MiniLang;
    assert_eq!(lang.pluralize_with_category("service", PluralCategory::One), "service");
}

#[test]
fn default_pluralize_with_category_other_is_plural() {
    let lang = MiniLang;
    assert_eq!(lang.pluralize_with_category("service", PluralCategory::Other), "services");
}

#[test]
fn default_pluralize_with_category_few_falls_to_plural() {
    // English collapses Few/Many to Other; the default impl returns plural.
    let lang = MiniLang;
    assert_eq!(lang.pluralize_with_category("service", PluralCategory::Few), "services");
}
```

Check the existing `MiniLang` test helper in `language.rs` — its `pluralize(word, count)` must route `count=2` to the plural form (or adjust the helper).

**Commit:** `Add PluralCategory enum and category-aware trait methods to Language`

---

## Phase 2 — Add `|plural` pipe

**File:** `prosaic-core/src/engine.rs`

### 2.1 Add pipe dispatch

In `apply_pipe`, add a new arm:

```rust
"plural" => self.pipe_plural(pipe, value),
```

### 2.2 Add pipe implementation

```rust
fn pipe_plural(
    &self,
    pipe: &Pipe,
    value: &Value,
) -> Result<Value, NlgError> {
    let noun = match &pipe.arg {
        Some(PipeArg::String(s)) => s.as_str(),
        _ => return Err(NlgError::InvalidPipe {
            pipe: "plural".to_string(),
            reason: "requires a singular noun argument, e.g., {count|plural:service}".to_string(),
        }),
    };

    let count = match value {
        Value::Number(n) => *n,
        _ => return Err(NlgError::InvalidPipe {
            pipe: "plural".to_string(),
            reason: "requires a numeric slot value".to_string(),
        }),
    };

    let category = self.engine.language.plural_category(count);
    Ok(Value::String(
        self.engine.language.pluralize_with_category(noun, category),
    ))
}
```

### 2.3 Update whitelist in `prosaic-derive`

Add `"plural"` to `VALID_PIPES` in `prosaic-derive/src/lib.rs`.

### 2.4 Tests

In `engine.rs` `#[cfg(test)]`:

```rust
#[test]
fn plural_pipe_singular_for_one() {
    let engine = test_engine();
    let mut ctx = Context::new();
    ctx.insert("count", Value::Number(1));
    let mut session = Session::new();
    let out = engine
        .render_inline(&mut session, "{count|plural:service}", &ctx)
        .unwrap();
    assert!(out.trim_end_matches('.').contains("service"));
    assert!(!out.trim_end_matches('.').contains("services"));
}

#[test]
fn plural_pipe_plural_for_many() {
    let engine = test_engine();
    let mut ctx = Context::new();
    ctx.insert("count", Value::Number(5));
    let mut session = Session::new();
    let out = engine
        .render_inline(&mut session, "{count|plural:service}", &ctx)
        .unwrap();
    assert!(out.contains("services"));
}

#[test]
fn plural_pipe_requires_noun_arg() {
    let mut engine = Engine::new(English::new()).strictness(Strictness::Strict);
    engine.register_template("t", "{count|plural}").unwrap();
    let mut ctx = Context::new();
    ctx.insert("count", Value::Number(3));
    let mut session = Session::new();
    let err = engine.render(&mut session, "t", &ctx).unwrap_err();
    assert!(matches!(err, NlgError::InvalidPipe { .. }));
}

#[test]
fn plural_pipe_requires_numeric_value() {
    let mut engine = Engine::new(English::new()).strictness(Strictness::Strict);
    engine.register_template("t", "{word|plural:service}").unwrap();
    let mut ctx = Context::new();
    ctx.insert("word", Value::String("hello".into()));
    let mut session = Session::new();
    let err = engine.render(&mut session, "t", &ctx).unwrap_err();
    assert!(matches!(err, NlgError::InvalidPipe { .. }));
}

#[test]
fn plural_pipe_works_with_template_macro() {
    // Compile-time validation — this must compile.
    use prosaic_derive::prosaic_template;
    let tpl = prosaic_template! {
        template: "{count|plural:service} affected",
        slots: [count],
    };
    assert!(tpl.contains("|plural"));
}
```

### 2.5 Verify

```bash
cargo test --all-features
cargo test --no-default-features
cargo test --features serde
cargo clippy --all-features -- -D warnings
```

**Commit:** `Add |plural pipe using category-aware Language trait methods`

---

## Phase 3 — Final verification

```bash
cargo test --all-features
cargo test --no-default-features
cargo test --features serde
cargo clippy --all-features -- -D warnings
cargo doc --all-features --no-deps
cargo bench --bench engine -- --test
```

Test count up by ~15–20 → **~595 total**.

**Report:** 2 commit hashes, test count delta, whether the default `pluralize_with_category` impl correctly handles all categories without an English override.

---

## Risk register

| Risk | Mitigation |
|---|---|
| `MiniLang` test helper in `language.rs` may not handle the `count=2` call in the default `pluralize_with_category` | Verify and adjust the helper if it's a simple stub. |
| Default impl maps every category except `One` to plural — fine for English but misleading for future Polish/Arabic impls | Document that non-English grammars MUST override `pluralize_with_category` when they support richer categories. |
| The `|plural` pipe duplicates `|pluralize` semantically under English | Accept — future non-English users will use `|plural`; English users can continue using either. |
| `prosaic-grammar-en` might need to override the default for irregulars (e.g. "person" → "people") | The default routes to `self.pluralize(word, 2)`, which in `prosaic-grammar-en` already handles irregulars via its tables. Transitive dispatch works. |
| Existing `MiniLang` impls in test files might not satisfy the new trait methods | Default impls on the trait mean existing test helpers keep compiling without changes. |
| `PluralCategory` serde format | Auto-derive produces `"Zero"`, `"One"`, etc. Matches the convention used for other enums (`Salience::High`). |

## What NOT to do

- **Do not** add icu4x.
- **Do not** add a `locale` Cargo feature.
- **Do not** override `plural_category` in `prosaic-grammar-en`.
- **Do not** deprecate `|pluralize`.
- **Do not** merge `PluralCategory` into `AgreementFeatures::Number` — separate concerns.
- **Do not** amend commits.

## Definition of done

- [ ] `PluralCategory` enum in `prosaic-core/src/language.rs` with six CLDR variants
- [ ] `Language::plural_category` trait method with English default impl
- [ ] `Language::pluralize_with_category` trait method with English default impl
- [ ] `PluralCategory` re-exported from `prosaic-core/src/lib.rs`
- [ ] `|plural` pipe dispatched in `apply_pipe` calling `pipe_plural`
- [ ] `|plural` added to `VALID_PIPES` in `prosaic-derive`
- [ ] All 579 existing tests pass + ~15-20 new
- [ ] `cargo clippy --all-features -- -D warnings` clean
- [ ] `Engine: Send + Sync` assert still compiles
- [ ] 2 commits with specified subject lines
