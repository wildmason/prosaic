# Plan: `prosaic-grammar-es` — Minimal Spanish Grammar Crate

**Owner:** sonnet agent
**Scope:** new workspace member `prosaic-grammar-es/`
**Estimated size:** ~600–900 LOC including tests
**Test gate:** all 607 existing tests pass; new tests add ~30–40; zero warnings
**Branch discipline:** local only, 1 commit

---

## Why

The final v1.5 milestone. Ships the first non-English grammar crate to prove the multilingual trait pipeline works end-to-end. The `AgreementFeatures`, `Value::Entity`, `plural_category`, `pluralize_with_category`, and `realize_reference` hooks added over the past several plans were all designed around Spanish as the validation target.

**Scope is deliberately minimal.** A SimpleNLG-ES-class grammar would be ~3.5k LOC. For v1.5, the goal is *functional Spanish that proves the architecture*, not production-complete Spanish. Expansion lives in follow-up crates.

**No icu4x.** Spanish plural rules are simple enough to implement in pure Rust. The `locale` Cargo feature gate introduced by the swarm is deferred to when a language actually needs CLDR data (Polish, Arabic — v2+).

## Design (locked)

### Spanish language features covered

| Feature | In scope (v1.5) | Out of scope |
|---|---|---|
| Gender-aware articles (el/la/los/las) | ✓ | del/al contractions (future) |
| Regular noun pluralization | ✓ | All irregular noun forms |
| Common irregular noun plurals (país/países, luz/luces, rey/reyes) | ✓ (~10 entries) | Full irregular noun tables |
| Regular verb conjugation (-ar/-er/-ir) | ✓ | Stem-changing verbs |
| Simple past tense (pretérito) | ✓ | Other past tenses (imperfecto, etc.) |
| Past participle (for passives) | ✓ | Full compound tense system |
| Common irregular verbs (ser, estar, haber, tener) | ✓ (~10 entries) | Full irregular verb tables |
| Gender agreement on adjectives | ✓ (through past participles) | Full adjective position rules |
| Subject-verb agreement (person/number) | ✓ | Vosotros vs. ustedes distinction |
| Gendered pronouns (él/ella/ellos/ellas) | ✓ | Object/reflexive pronouns |
| List joining with "y" (not "and") | ✓ | "e" before i-/hi- (→ v2) |
| Gender inference from noun ending | ✓ (basic -o/-a heuristic) | Full exception list |
| Number formatting (120 → "ciento veinte") | Partial (basic) | Full number words |

### Crate layout

```
prosaic-grammar-es/
├── Cargo.toml
└── src/
    ├── lib.rs          (pub struct Spanish; impl Language for Spanish)
    ├── pluralize.rs    (pluralize + singularize rules + irregulars)
    ├── conjugate.rs    (regular -ar/-er/-ir + irregulars)
    ├── articles.rs     (el/la/los/las selection)
    ├── gender.rs       (gender inference from noun ending)
    └── numbers.rs      (basic number-to-words)
```

### Cargo.toml

```toml
[package]
name = "prosaic-grammar-es"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "Spanish grammar layer for the prosaic NLG engine"

[dependencies]
prosaic-core = { path = "../prosaic-core", default-features = false }
```

Add to workspace `[workspace.members]`.

### `Spanish` struct + `Language` impl

```rust
// prosaic-grammar-es/src/lib.rs
//! Spanish grammar layer for the Prosaic NLG engine.
//!
//! Minimal v1.5 scope: gender-aware articles, regular+common-irregular
//! pluralization, regular verb conjugation (present/simple-past/past-participle),
//! gender agreement on passive participles, gendered pronouns.
//!
//! Uses only pure Rust — no CLDR data. Expansion (subjunctive,
//! imperfecto, full irregular tables, dialectal variation) is planned
//! for follow-up crates.

use prosaic_core::{
    AgreementFeatures, Conjunction, Gender, GrammaticalNumber, Language,
    PluralCategory, ReferenceForm, Tense, AgreementPerson,
};

pub struct Spanish;

impl Spanish {
    pub fn new() -> Self { Self }
}

impl Default for Spanish {
    fn default() -> Self { Self::new() }
}

impl Language for Spanish {
    fn pluralize(&self, word: &str, count: usize) -> String {
        if count == 1 {
            word.to_string()
        } else {
            pluralize::pluralize_es(word)
        }
    }

    fn singularize(&self, word: &str) -> String {
        pluralize::singularize_es(word)
    }

    fn article(&self, word: &str) -> &str {
        // Default-singular-masculine when no agreement features available.
        // Callers with AgreementFeatures should use article_with_features via
        // realize_reference or a dedicated pipe; the bare article() is English-shaped
        // and returns "el" as a pragmatic default.
        //
        // Gender inference from the word ending: -a → "la", else → "el".
        articles::basic_article(word)
    }

    fn conjugate(&self, verb: &str, tense: Tense, person: AgreementPerson) -> String {
        conjugate::conjugate_es(verb, tense, person)
    }

    fn past_participle(&self, verb: &str) -> String {
        conjugate::past_participle_es(verb)
    }

    fn present_participle(&self, verb: &str) -> String {
        conjugate::present_participle_es(verb)
    }

    fn join_list(&self, items: &[&str], conjunction: Conjunction) -> String {
        let conj = match conjunction {
            Conjunction::And => "y",
            Conjunction::Or => "o",
        };
        join_list_es(items, conj)
    }

    fn ordinal(&self, n: usize) -> String {
        // Simple fallback — Spanish ordinals beyond 10 are verbose ("decimoprimero", etc.)
        // and rarely used. For v1.5 return the numeric form for n >= 11.
        match n {
            1 => "primero".to_string(), 2 => "segundo".to_string(),
            3 => "tercero".to_string(), 4 => "cuarto".to_string(),
            5 => "quinto".to_string(), 6 => "sexto".to_string(),
            7 => "séptimo".to_string(), 8 => "octavo".to_string(),
            9 => "noveno".to_string(), 10 => "décimo".to_string(),
            _ => format!("{n}º"),
        }
    }

    fn number_to_words(&self, n: usize) -> String {
        numbers::number_to_words_es(n)
    }

    // ── Category-aware pluralization ──
    fn plural_category(&self, n: i64) -> PluralCategory {
        // Spanish plural rules are one/other — matches English default.
        match n { 1 => PluralCategory::One, _ => PluralCategory::Other }
    }

    // Default pluralize_with_category inherited — routes One → pluralize(_,1), Other → pluralize(_,2).
    // Works correctly via our pluralize impl above.

    // ── Reference realization ──
    fn realize_reference(
        &self,
        form: ReferenceForm,
        features: &AgreementFeatures,
    ) -> Option<String> {
        match form {
            ReferenceForm::Pronoun => {
                Some(spanish_pronoun(features))
            }
            ReferenceForm::Demonstrative => {
                Some(spanish_demonstrative(features))
            }
            ReferenceForm::Zero => None,
            ReferenceForm::Full | ReferenceForm::ShortName => None,
        }
    }

    // ── Plural description ──
    fn plural_description(
        &self,
        entity_type: &str,
        count: usize,
        features: &AgreementFeatures,
    ) -> String {
        // Gender-aware: "las 3 clases" (fem), "los 3 servicios" (masc).
        let article = plural_article(features);
        let noun = self.pluralize(entity_type, count);
        match count {
            0 => String::new(),
            1 => format!("{} {}", singular_article(features), entity_type),
            _ => format!("{article} {count} {noun}"),
        }
    }
}

fn spanish_pronoun(features: &AgreementFeatures) -> String {
    match (features.gender, features.number) {
        (Gender::Fem, GrammaticalNumber::Plural) | (Gender::Fem, GrammaticalNumber::Dual) => "ellas".to_string(),
        (Gender::Fem, _) => "ella".to_string(),
        (_, GrammaticalNumber::Plural) | (_, GrammaticalNumber::Dual) => "ellos".to_string(),
        _ => "él".to_string(),
    }
}

fn spanish_demonstrative(features: &AgreementFeatures) -> String {
    match (features.gender, features.number) {
        (Gender::Fem, GrammaticalNumber::Plural) => "estas".to_string(),
        (Gender::Fem, _) => "esta".to_string(),
        (_, GrammaticalNumber::Plural) => "estos".to_string(),
        _ => "este".to_string(),
    }
}

fn singular_article(features: &AgreementFeatures) -> &'static str {
    match features.gender {
        Gender::Fem => "la",
        _ => "el",
    }
}

fn plural_article(features: &AgreementFeatures) -> &'static str {
    match features.gender {
        Gender::Fem => "las",
        _ => "los",
    }
}

fn join_list_es(items: &[&str], conjunction: &str) -> String {
    match items.len() {
        0 => String::new(),
        1 => items[0].to_string(),
        2 => format!("{} {} {}", items[0], conjunction, items[1]),
        _ => {
            let (last, rest) = items.split_last().unwrap();
            format!("{}, {} {}", rest.join(", "), conjunction, last)
        }
    }
}

mod pluralize;
mod conjugate;
mod articles;
mod gender;
mod numbers;
```

### `pluralize.rs` — Spanish plural rules

```rust
//! Spanish pluralization rules.
//!
//! 1. Ends in unstressed vowel (a/e/i/o/u) → add "s"
//! 2. Ends in stressed vowel (á/é/í/ó/ú) → add "es"
//! 3. Ends in "z" → change "z" to "c" and add "es"
//! 4. Ends in consonant → add "es"
//! 5. Ends in -s AND has more than 1 syllable stressed on last → add "es"
//!    (mes → meses, país → países)
//! 6. Ends in -s AND unstressed last syllable → unchanged (lunes → lunes)

const IRREGULAR_PLURALS: &[(&str, &str)] = &[
    ("país", "países"),
    ("rey", "reyes"),
    ("ley", "leyes"),
    ("luz", "luces"),
    ("vez", "veces"),
    ("voz", "voces"),
    ("ciudad", "ciudades"),
    ("clase", "clases"),  // regular but noted
    ("servicio", "servicios"),
];

pub fn pluralize_es(word: &str) -> String { /* … */ }
pub fn singularize_es(word: &str) -> String { /* … */ }
```

Full implementation: ~150 LOC with test coverage.

### `conjugate.rs` — Spanish verb conjugation

Scope: regular `-ar`, `-er`, `-ir` verbs in:
- Present tense (all 6 persons)
- Simple past / pretérito (all 6 persons)
- Past participle (gender-neutral)
- Present participle (gerundio)

Plus ~10 common irregulars: ser, estar, haber, tener, ir, ver, hacer, decir, poder, querer.

Example:
```rust
pub fn conjugate_es(verb: &str, tense: Tense, person: AgreementPerson) -> String {
    if let Some(form) = irregular_lookup(verb, tense, person) {
        return form.to_string();
    }
    match tense {
        Tense::Present => conjugate_present(verb, person),
        Tense::Past => conjugate_preterite(verb, person),
        Tense::Future => conjugate_future(verb, person),
    }
}

pub fn past_participle_es(verb: &str) -> String {
    // -ar → -ado; -er/-ir → -ido
    // A few irregulars: hacer → hecho, decir → dicho, ver → visto
    // ...
}
```

Full implementation: ~250 LOC.

### `articles.rs`, `gender.rs`

`articles::basic_article(word)` — returns "el" or "la" based on noun ending heuristic.

`gender::infer_gender(word) -> Gender` — applies simple rules:
- -a, -ad, -ción, -sión, -tad → Fem
- -o, -or, -aje → Masc
- Default → Masc (most common)

~100 LOC combined.

### `numbers.rs`

Basic number-to-words for 0-100. Beyond 100, return numeric form.

```rust
pub fn number_to_words_es(n: usize) -> String {
    match n {
        0 => "cero".to_string(),
        1 => "uno".to_string(),
        2 => "dos".to_string(),
        // ... 1-29 spelled
        n if n < 100 => format!("{n}"), // punt on compound numbers for v1.5
        _ => n.to_string(),
    }
}
```

~50 LOC. Deliberately minimal; most template use is `|plural` and article selection, not spelling numbers.

### Integration test

In `prosaic-grammar-es/tests/integration.rs`:

```rust
use prosaic_core::{ctx, entity, Engine, Session, Strictness, Value, Variation};
use prosaic_grammar_es::Spanish;

#[test]
fn renders_simple_spanish_sentence() {
    let mut engine = Engine::new(Spanish::new())
        .strictness(Strictness::Silent)
        .variation(Variation::Fixed);
    engine.register_template("t", "{user} abrió {count} {count|plural:tarea}").unwrap();

    let mut session = Session::new();
    let ctx = ctx! {
        user: "Alice",
        count: 3,
    };

    let out = engine.render(&mut session, "t", &ctx).unwrap();
    assert!(out.contains("Alice abrió 3 tareas"), "got: {out}");
}

#[test]
fn gender_aware_pronoun_for_feminine_entity() {
    let mut engine = Engine::new(Spanish::new())
        .strictness(Strictness::Silent)
        .variation(Variation::Fixed);
    engine.register_template("t1", "{name|refer} fue modificada").unwrap();
    engine.register_template("t2", "{name|refer} fue desplegada").unwrap();

    let mut session = Session::new();
    let c = ctx! {
        entity_type: "clase",
        name: entity("UserService").fem().sing(),
    };

    let r1 = engine.render(&mut session, "t1", &c).unwrap();
    let r2 = engine.render(&mut session, "t2", &c).unwrap();

    // r2 should use feminine pronoun "ella" not "él"
    assert!(r2.to_lowercase().contains("ella"), "got: {r2}");
}

#[test]
fn plural_description_is_gender_aware() {
    let mut engine = Engine::new(Spanish::new())
        .strictness(Strictness::Silent)
        .variation(Variation::Fixed);
    engine.register_template("t", "{names|refer} fueron modificadas").unwrap();

    let mut session = Session::new();
    let c = ctx! {
        entity_type: "clase",
        names: vec!["UserService".to_string(), "AuthService".to_string(), "ProfileService".to_string()],
    };
    // Plural description path doesn't currently get AgreementFeatures from list —
    // defaults to masculine. Test the default behaviour.
    let out = engine.render(&mut session, "t", &c).unwrap();
    // Should contain "los 3 clases" or "las 3 clases" depending on default
    assert!(out.contains("3 clases"), "got: {out}");
}
```

### Out of scope for this plan

- **Do not** add icu4x or any external dep.
- **Do not** ship `nlg-vocab-code-es` or any other Spanish vocab. Separate plan.
- **Do not** override `article` to accept features in the trait — that would require a trait change affecting English. Defer.
- **Do not** handle contractions (del, al).
- **Do not** handle "e" before i-/hi- in list joining.
- **Do not** provide full ordinal spelling beyond 10.
- **Do not** provide full irregular noun or verb tables.
- **Do not** add imperfecto, subjuntivo, or compound tenses beyond basic past participle.

---

## Phase 0 — Baseline

```bash
cargo test --all-features
cargo clippy --all-features -- -D warnings
```

Expected: **607 tests passing**, 0 warnings.

**No commit.**

---

## Phase 1 — Create crate scaffold

### 1.1 Directory structure + Cargo.toml

Create `prosaic-grammar-es/Cargo.toml` per the design. Create empty `src/lib.rs`, `src/pluralize.rs`, `src/conjugate.rs`, `src/articles.rs`, `src/gender.rs`, `src/numbers.rs`.

### 1.2 Workspace wiring

Add `"prosaic-grammar-es"` to workspace `Cargo.toml` `[workspace.members]`.

### 1.3 Minimal `lib.rs` skeleton

Just enough for `cargo check` to pass:

```rust
pub struct Spanish;

impl Spanish {
    pub fn new() -> Self { Self }
}

impl Default for Spanish {
    fn default() -> Self { Self::new() }
}

// TODO Phase 2+: implement Language trait
```

### 1.4 Verify

```bash
cargo check -p prosaic-grammar-es
cargo test --all-features
```

**Commit:** `Scaffold prosaic-grammar-es crate`

---

## Phase 2 — Pluralization + articles + gender inference

Implement `pluralize.rs`, `articles.rs`, `gender.rs` per the design. Pure functions, no trait impl yet.

### Tests (per module)

```rust
// pluralize.rs
#[test] fn pluralize_vowel_ending_adds_s() {
    assert_eq!(pluralize_es("casa"), "casas");
    assert_eq!(pluralize_es("libro"), "libros");
}

#[test] fn pluralize_consonant_ending_adds_es() {
    assert_eq!(pluralize_es("color"), "colores");
}

#[test] fn pluralize_z_becomes_ces() {
    assert_eq!(pluralize_es("luz"), "luces");
    assert_eq!(pluralize_es("vez"), "veces");
}

#[test] fn pluralize_irregulars_use_table() {
    assert_eq!(pluralize_es("país"), "países");
    assert_eq!(pluralize_es("rey"), "reyes");
}

// gender.rs
#[test] fn gender_a_ending_is_fem() {
    assert_eq!(infer_gender("casa"), Gender::Fem);
    assert_eq!(infer_gender("clase"), Gender::Fem);
}

#[test] fn gender_o_ending_is_masc() {
    assert_eq!(infer_gender("servicio"), Gender::Masc);
    assert_eq!(infer_gender("libro"), Gender::Masc);
}

// articles.rs
#[test] fn article_masculine_noun() {
    assert_eq!(basic_article("servicio"), "el");
}

#[test] fn article_feminine_noun() {
    assert_eq!(basic_article("clase"), "la");
}
```

~15 tests total.

**Commit:** `Add Spanish pluralization, gender inference, and article selection`

---

## Phase 3 — Verb conjugation

Implement `conjugate.rs`:
- Regular -ar present/preterite/future
- Regular -er present/preterite/future
- Regular -ir present/preterite/future
- Past participle (`-ar → -ado`, `-er/-ir → -ido`, irregulars)
- Present participle (`-ar → -ando`, `-er/-ir → -iendo`)
- Irregular table for ~10 common verbs

### Tests

```rust
#[test] fn regular_ar_present_yo() {
    assert_eq!(conjugate_es("hablar", Tense::Present, AgreementPerson::First), "hablo");
}

#[test] fn regular_er_preterite_el() {
    assert_eq!(conjugate_es("comer", Tense::Past, AgreementPerson::Third), "comió");
}

#[test] fn past_participle_regular() {
    assert_eq!(past_participle_es("hablar"), "hablado");
    assert_eq!(past_participle_es("comer"), "comido");
}

#[test] fn past_participle_irregulars() {
    assert_eq!(past_participle_es("hacer"), "hecho");
    assert_eq!(past_participle_es("decir"), "dicho");
    assert_eq!(past_participle_es("ver"), "visto");
}

#[test] fn irregular_ser_present() {
    assert_eq!(conjugate_es("ser", Tense::Present, AgreementPerson::First), "soy");
    assert_eq!(conjugate_es("ser", Tense::Present, AgreementPerson::Third), "es");
}
```

~15-20 tests.

**Commit:** `Add Spanish verb conjugation for regular and common irregular verbs`

---

## Phase 4 — Language trait impl + integration tests

Wire up the `Language` trait impl in `lib.rs` per the design. Implement `numbers.rs` with the minimal 0-29 table + numeric fallback.

Add `prosaic-grammar-es` as a dev-dep of `prosaic-grammar-es/tests/integration.rs` (self-reference for the test crate). Add the integration tests per the design.

### Verify

```bash
cargo test --all-features
cargo test --no-default-features
cargo test --features serde
cargo clippy --all-features -- -D warnings
cargo doc --all-features --no-deps
cargo bench --bench engine -- --test
```

**Commit:** `Implement Language trait for Spanish with realize_reference and plural_description overrides`

---

## Phase 5 — Final verification

Test count up by ~30-40 → **~640 total**.

**Report:** 4 commit hashes, test count delta per feature variant, any Spanish grammar edge cases that surfaced (e.g. stressed-vowel endings in plural rules, gender inference errors).

---

## Risk register

| Risk | Mitigation |
|---|---|
| Spanish plural rules have stress-sensitive edge cases (país/países, mes/meses) that pure-Rust heuristics can miss | Handle the common 10 irregulars via a lookup table. Accept edge-case misses as v1.5 scope — users can override in custom `Language` impls. |
| Gender inference from endings is ~90% correct for common nouns but fails on some domain terms (el problema, el idioma, la mano) | Accept v1.5 misses. Document the extension path: users can wrap `Spanish` in a newtype that overrides `article` with their own table. |
| `Conjunction::Or` maps to "o" but Spanish uses "u" before o-/ho- (siete u ocho) | Punt — minor aesthetic. Document. |
| Dev-dep self-reference in tests/ | Standard Rust pattern: `[dev-dependencies] prosaic-grammar-es = { path = "." }`. Works. |
| `prosaic-core` default-features-off — if disabling features breaks the trait surface, the crate won't compile | Current `prosaic-core` default features are `time`, `polish`, `reg`. None of those are in the Language trait surface. Safe. |
| Workspace build time grows | One more crate; negligible. |
| `realize_reference` for Demonstrative is reserved but untested via the discourse path | Add a direct test of `Spanish::realize_reference(Demonstrative, ...)` to cover the method; end-to-end discourse-triggered Demonstrative is future work. |

## What NOT to do

- **Do not** add icu4x or any external dep.
- **Do not** ship Spanish vocabulary crates in this plan.
- **Do not** implement imperfecto, subjunctive, or compound tenses.
- **Do not** exhaustively cover Spanish irregulars — ~10 of each is enough.
- **Do not** change `prosaic-core`.
- **Do not** amend commits.

## Definition of done

- [ ] `prosaic-grammar-es` crate exists in workspace members
- [ ] `Spanish` struct implementing `Language` trait
- [ ] Pluralization with vowel/consonant/z rules + ~10 irregulars
- [ ] Gender inference with -a/-o heuristic
- [ ] Articles: el/la/los/las based on features or ending
- [ ] Conjugation: regular -ar/-er/-ir in present/past/future + ~10 irregulars
- [ ] Past and present participles (regular + irregulars)
- [ ] Override `realize_reference` for gendered pronouns
- [ ] Override `plural_description` for gender-aware "las N clases" / "los N servicios"
- [ ] Override `join_list` with "y"/"o"
- [ ] Integration test rendering a Spanish sentence end-to-end
- [ ] All 607 existing tests still pass + ~30-40 new
- [ ] `cargo clippy --all-features -- -D warnings` clean
- [ ] `Engine: Send + Sync` assert still compiles
- [ ] 4 commits with specified subject lines
