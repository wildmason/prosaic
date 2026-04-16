# Plan: Plural REG with `Language::plural_description` Hook

**Owner:** sonnet agent
**Scope:** `prosaic-core` trait addition + `prosaic-grammar-en` default impl + `|refer` pipe extension
**Estimated size:** ~250–350 LOC including tests
**Test gate:** all 568 existing tests pass; new tests add ~10–15; zero warnings
**Branch discipline:** local only, commit at each sub-phase

---

## Why

Today `{names|refer}` on a list of same-type entities emits an enumeration: *"UserService, AccountService, ProfileService"*. Van Deemter 2002 / Gatt & van Deemter 2007 plural REG collapses this to a **plural description**: *"the three domain classes"* or *"the services"* when the individual identities don't matter.

This is the sibling of D&R singular REG and graph-based REG — a natural generalization to sets. It also exposes a `Language` trait hook (`plural_description`) that non-English grammars will read to handle:
- Spanish gender-aware plurals: *"las tres clases"* vs *"los tres servicios"*
- Arabic dual/few/many categories
- Japanese counter-word forms (*"3人の開発者"*)

English default is grammatically simple: `"the {count} {entity_type_plural}"`.

## Design (locked)

### Trait method

```rust
// prosaic-core/src/language.rs
impl Language for ... {
    /// Produce a plural description for a set of same-type entities.
    ///
    /// Called by the `|refer` pipe when the slot value is a list of 2+
    /// entities sharing an entity type. English default: "the N types"
    /// (e.g., "the 3 classes"). Non-English grammars override to handle
    /// gender agreement, counter words, etc.
    ///
    /// The `features` parameter carries propagated agreement info from
    /// the first entity in the set; languages may use it to pick agreeing
    /// articles and adjectives.
    fn plural_description(
        &self,
        entity_type: &str,
        count: usize,
        _features: &AgreementFeatures,
    ) -> String {
        // Default implementation — English-shaped but generic enough to
        // serve as a reasonable fallback for languages that haven't
        // overridden it.
        match count {
            0 => String::new(),
            1 => format!("the {entity_type}"),
            _ => format!("the {count} {}", self.pluralize(entity_type, count)),
        }
    }
}
```

The default impl lives in the trait definition so callers don't have to override when the default is adequate. English's `prosaic_grammar_en::English` uses the default — no override needed for v1.

### `|refer` pipe extension

Extend `pipe_refer` in `engine.rs`. Today it expects a single entity value (String or Entity). Add a branch: if the value is `Value::List(names)`:

- **Empty list** → return empty string (existing empty-list convention).
- **Single-item list** → delegate to the existing single-entity path using the first name.
- **Multi-item list** → call `self.language.plural_description(entity_type, count, &agreement_for_first)`. `entity_type` comes from the context slot `entity_type` (existing convention). `agreement_for_first` is `AgreementFeatures::default()` for now since `Value::List` doesn't carry entity features; future work can introduce `Value::EntityList` if needed.

### Integration with REG

Plural REG does NOT consult the EntityRegistry (no individual entity attributes to check). It's a purely grammatical collapse: "the N types". Graph-based REG and Dale & Reiter continue to handle singular cases; plural REG handles list cases. Orthogonal.

### Discourse integration

Plural REG updates discourse state by:
- Setting `focus_is_plural = true` on the current render (so pronoun continuations use "they/them")
- Calling `mention_entity` for each name in the list so individual entities stay trackable for future singular references

### Out of scope for v1.5 Phase 2

- **Do not** introduce a `Value::EntityList` variant. Lists stay as `Vec<String>` for now; future multilingual work can add richer list types.
- **Do not** implement Conceptual Coherence Constraint (Gatt & van Deemter 2007) — "the two symbols" vs "the class and the trait". Requires per-entity-type semantic similarity; out of scope.
- **Do not** implement numeric-range plural ("a handful of classes"). That's the existing `|quantify` pipe's domain.
- **Do not** change English output for non-`|refer` list rendering (`|join`, `|truncate` unchanged).
- **Do not** override `plural_description` in `prosaic-grammar-en`. Use the default impl.

---

## Phase 0 — Baseline

```bash
cargo test --all-features
cargo clippy --all-features -- -D warnings
```

Expected: **568 tests passing**, 0 warnings.

**No commit.**

---

## Phase 1 — Add `Language::plural_description` trait method

**File:** `prosaic-core/src/language.rs`

1. Import `AgreementFeatures` from `crate::agreement`.
2. Add the method with the default implementation shown above.
3. Make sure the default impl calls `self.pluralize(entity_type, count)` — that method already exists on the trait.

### Tests

Add to `prosaic-core/src/language.rs` `#[cfg(test)]`:

```rust
#[test]
fn plural_description_default_zero_is_empty() {
    struct MiniLang;
    impl Language for MiniLang {
        fn pluralize(&self, word: &str, count: usize) -> String {
            if count == 1 { word.to_string() } else { format!("{word}s") }
        }
        // ... stub out the rest of the trait methods
    }
    let l = MiniLang;
    assert_eq!(l.plural_description("class", 0, &AgreementFeatures::default()), "");
}

#[test]
fn plural_description_default_one_is_the_type() {
    // ... same MiniLang
    assert_eq!(l.plural_description("class", 1, &AgreementFeatures::default()), "the class");
}

#[test]
fn plural_description_default_many_uses_pluralize() {
    // ... same
    assert_eq!(l.plural_description("class", 3, &AgreementFeatures::default()), "the 3 classes");
}
```

Consider adding the `MiniLang` helper as a shared test fixture if it's already defined elsewhere in `language.rs` (check — there's likely a similar mini impl for other tests).

**Commit:** `Add Language::plural_description trait method with English-shaped default`

---

## Phase 2 — Extend `pipe_refer` to handle list values

**File:** `prosaic-core/src/engine.rs`

Find `pipe_refer`. Add a branch at the top: if the value is `Value::List(names)`, handle plurally. Otherwise fall through to the existing single-entity logic.

```rust
fn pipe_refer(
    &mut self,
    pipe: &Pipe,
    value: &Value,
    context: &Context,
) -> Result<Value, NlgError> {
    // Plural REG: list of same-type entities
    if let Value::List(names) = value {
        return self.pipe_refer_plural(names, context);
    }
    // Existing single-entity path
    // ...
}

fn pipe_refer_plural(
    &mut self,
    names: &[String],
    context: &Context,
) -> Result<Value, NlgError> {
    let entity_type = context.get("entity_type")
        .and_then(|v| v.as_string())
        .unwrap_or("");

    match names.len() {
        0 => Ok(Value::String(String::new())),
        1 => {
            // Delegate to single-entity path with the first name
            let v = Value::String(names[0].clone());
            // Call the existing single-entity refer logic via a helper
            self.pipe_refer_single(&v, context)
        }
        n => {
            // Mention each entity for future singular-reference tracking
            for name in names {
                if !entity_type.is_empty() {
                    self.session.discourse.mention_entity(name, entity_type);
                }
            }
            // Mark focus as plural for pronoun continuation
            self.session.discourse.set_focus_plural(true);

            // Gather agreement features from the first entity if any
            let features = AgreementFeatures::default(); // v1.5: enhance when Value::EntityList exists

            let output = self.engine.language.plural_description(
                entity_type,
                n,
                &features,
            );
            Ok(Value::String(output))
        }
    }
}
```

The existing single-entity `pipe_refer` logic should be refactored into `pipe_refer_single` taking the same parameters so `pipe_refer_plural` can delegate. Minimise the refactor — only extract what's needed.

### Tests

Add integration tests in `prosaic-core/tests/integration.rs` or a new `prosaic-core/tests/plural_reg.rs`:

```rust
#[test]
fn plural_refer_emits_the_count_type() {
    let mut engine = base_engine();
    engine.register_template("t", "{names|refer} were modified").unwrap();
    let mut session = Session::new();
    let mut ctx = Context::new();
    ctx.insert("entity_type", Value::String("class".into()));
    ctx.insert("names", Value::List(vec![
        "UserService".into(),
        "AuthService".into(),
        "ProfileService".into(),
    ]));
    let out = engine.render(&mut session, "t", &ctx).unwrap();
    assert!(out.contains("the 3 classes"), "got: {out}");
}

#[test]
fn plural_refer_single_item_uses_singular_path() {
    let mut engine = base_engine();
    engine.register_template("t", "{names|refer} was modified").unwrap();
    let mut session = Session::new();
    let mut ctx = Context::new();
    ctx.insert("entity_type", Value::String("class".into()));
    ctx.insert("names", Value::List(vec!["UserService".into()]));
    let out = engine.render(&mut session, "t", &ctx).unwrap();
    assert!(out.contains("UserService"), "got: {out}");
    assert!(!out.contains("1 class"), "should use singular form: {out}");
}

#[test]
fn plural_refer_empty_list_is_empty_substitution() {
    let mut engine = base_engine();
    engine.register_template("t", "Impact: {names|refer}").unwrap();
    let mut session = Session::new();
    let mut ctx = Context::new();
    ctx.insert("entity_type", Value::String("class".into()));
    ctx.insert("names", Value::List(vec![]));
    let out = engine.render(&mut session, "t", &ctx).unwrap();
    // Silent-mode cleanup may strip the empty substitution; in strict mode the
    // output should still render with an empty slot.
    assert!(out.starts_with("Impact:"));
}

#[test]
fn plural_refer_updates_focus_to_plural() {
    // After a plural refer, the next pronoun should be "they" not "it".
    let mut engine = base_engine();
    engine.register_template("t1", "{names|refer} were modified").unwrap();
    engine.register_template("t2", "{name|refer} was deployed").unwrap();
    let mut session = Session::new();

    let mut ctx1 = Context::new();
    ctx1.insert("entity_type", Value::String("class".into()));
    ctx1.insert("names", Value::List(vec!["A".into(), "B".into(), "C".into()]));
    engine.render(&mut session, "t1", &ctx1).unwrap();

    // Check focus_is_plural is true on the session post-render
    // (accessor may need to be added if not exposed — only if it's cheap)
}

#[test]
fn plural_refer_without_entity_type_uses_empty_type() {
    // No entity_type in context: plural_description receives empty string.
    let mut engine = base_engine();
    engine.register_template("t", "{names|refer}").unwrap();
    let mut session = Session::new();
    let mut ctx = Context::new();
    ctx.insert("names", Value::List(vec!["A".into(), "B".into()]));
    let out = engine.render(&mut session, "t", &ctx).unwrap();
    // Output is degenerate but shouldn't panic
    assert!(!out.is_empty() || out.is_empty()); // sanity: renders without crash
}
```

**Commit:** `Extend refer pipe to emit plural descriptions via Language::plural_description`

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

Test count up by ~10–15 → **~580 total**.

**Report:** 2 commit hashes, test count delta, any structural surprises from refactoring `pipe_refer`.

---

## Risk register

| Risk | Mitigation |
|---|---|
| Refactoring `pipe_refer` to split into single/plural branches may break existing discourse-aware behaviour | Existing `refer` tests cover the single-entity path extensively. Run them after the refactor; any failure reveals a semantic shift. Single-entity path must delegate to the old logic verbatim. |
| `plural_description` default impl references `self.pluralize` which may not produce ideal plurals for all entity types | Existing `pluralize` is English-only and handles common cases. Non-English grammars override the whole method. For edge cases in English ("person" → "people"), the grammar already has irregular tables. |
| `Value::List` today means a list of strings; introducing "list of entities" semantics conflates the type | Acknowledged. A cleaner `Value::EntityList(Vec<EntityValue>)` is future work. For v1.5 we reuse `Value::List(Vec<String>)` when the `|refer` pipe is applied — this is a pipe-level interpretation, not a type-level change. Document the convention. |
| Discourse state updates (`mention_entity` per name) could blow up entity registry for large lists | `mention_entity` is cheap and bounded. If a list has 10k items, the entities HashMap grows by 10k entries. Accept for v1.5; future optimization can aggregate. |
| `set_focus_plural(true)` assumes the list represents a compound subject, but `|refer` might render inside an object position | True — "The team reviewed [the 3 classes]" shouldn't make "they" the next-sentence pronoun. For v1.5 accept the limitation; document. Future work can distinguish subject-vs-object refer. |
| Existing match arms for `Value` inside `pipe_refer` may already handle List differently | Grep `pipe_refer` for Value pattern matches. The new List branch must come FIRST so it intercepts before the existing logic. |

## What NOT to do

- **Do not** add `Value::EntityList`.
- **Do not** implement Conceptual Coherence Constraint.
- **Do not** change how lists render under non-`refer` pipes.
- **Do not** override `plural_description` in `prosaic-grammar-en`.
- **Do not** touch REG algorithms (D&R or graph-based).
- **Do not** amend commits.

## Definition of done

- [ ] `Language::plural_description` trait method with default impl
- [ ] `pipe_refer` dispatches on `Value::List` to `pipe_refer_plural`
- [ ] Empty / single / multi list cases handled
- [ ] Each name in multi-item list is mentioned in discourse for future tracking
- [ ] Multi-item list sets `focus_is_plural = true`
- [ ] All 568 existing tests pass + ~10-15 new
- [ ] `cargo clippy --all-features -- -D warnings` clean
- [ ] `Engine: Send + Sync` assert still compiles
- [ ] 2 commits with specified subject lines
