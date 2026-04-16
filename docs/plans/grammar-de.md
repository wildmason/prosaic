# Plan: `prosaic-grammar-de` — German grammar crate

**Owner:** sonnet agent
**Scope:** New workspace member `prosaic-grammar-de/` mirroring `prosaic-grammar-es/` plus case declension axis
**Estimated size:** ~1,500–1,800 LOC including tests
**Test gate:** all existing tests pass; new crate adds ~60–80 tests; zero warnings
**Branch discipline:** local only, one commit at completion

---

## Why

German is the canonical Western European test case for case declension. Articles, adjectives, and pronouns inflect across four cases (nominative / accusative / dative / genitive), three genders (der/die/das), and two numbers (sg/pl). This exercises the `Case` axis of `AgreementFeatures`, which Spanish left untouched.

Scope is deliberately narrow: cover the most common forms correctly; defer full Duden-level accuracy (ablaut verb tables, strong/weak noun declension tables, adjective agreement in attributive vs predicative position) to future work.

## Design

### Crate layout

```
prosaic-grammar-de/
├── Cargo.toml
└── src/
    ├── lib.rs         # German struct, Language impl, module glue
    ├── articles.rs    # der/die/das across 4 cases × 2 numbers
    ├── pluralize.rs   # noun pluralization (-e, -en, -er, -s, umlaut)
    ├── conjugate.rs   # regular + ~10 common irregular verbs
    ├── gender.rs      # gender inference from noun endings
    └── numbers.rs     # cardinal number spelling
```

### Article declension table

German definite articles decline across:
- gender: masc / fem / neut / plural (plural collapses gender)
- case: Nominative / Accusative / Dative / Genitive

```
           Masc    Fem    Neut    Plural
Nom        der     die    das     die
Acc        den     die    das     die
Dat        dem     der    dem     den
Gen        des     der    des     der
```

Indefinite article ("ein/eine") has a similar table but no plural form:

```
           Masc    Fem    Neut
Nom        ein     eine   ein
Acc        einen   eine   ein
Dat        einem   einer  einem
Gen        eines   einer  eines
```

### Noun pluralization (minimal correct set)

Five plural classes, selected by ending:
1. `-e` suffix: Tisch → Tische (most masculine monosyllables)
2. `-en` / `-n`: Frau → Frauen (most feminine nouns)
3. `-er` + umlaut: Buch → Bücher (some neuter)
4. `-s`: Auto → Autos (loanwords)
5. Zero + umlaut: Vater → Väter (some masc/neut in -er/-el/-en)

Use a decision tree by ending:
- ends in `-ung`, `-heit`, `-keit`, `-schaft`, `-ion` → `-en`
- ends in `-chen`, `-lein` → unchanged (diminutives)
- ends in `-e` (feminine) → `-n`
- ends in `-er`, `-el`, `-en` (masc/neut) → zero or umlaut (apply umlaut to `a`/`o`/`u` → `ä`/`ö`/`ü`)
- ends in consonant + loanword ending (`-o`, `-y`) → `-s`
- default: `-e`

Include ~20 common irregulars in a lookup table: Mann → Männer, Kind → Kinder, Haus → Häuser, Wort → Wörter, Mensch → Menschen, etc.

### Verb conjugation

Regular weak verb present: stem + -e / -st / -t / -en / -t / -en
Regular weak preterite: stem + -te / -test / -te / -ten / -tet / -ten
Past participle: ge- + stem + -t (e.g. gemacht)
Present participle: stem + -end (e.g. machend)

Map `Person::{First, Second, Third}` → indices 0, 1, 2 (ich/du/er-sie-es).

Include ~10 strong irregulars in a lookup table (forms for Present/Past/Future):
- sein (to be): ich bin, du bist, er ist / ich war / ich werde sein
- haben (to have): ich habe, du hast, er hat / ich hatte / ich werde haben
- werden (to become): ich werde, du wirst, er wird / ich wurde / ich werde werden
- können (can): ich kann, du kannst, er kann / ich konnte / ich werde können
- müssen (must): ich muss, du musst, er muss / ich musste
- sollen (should)
- wollen (want): ich will, du willst, er will / ich wollte
- gehen (to go): ich gehe / ich ging / ich werde gehen
- kommen (to come): ich komme / ich kam / ich werde kommen
- machen (to make — regular, include only if a showcase irregular is wanted; otherwise fall through)
- sagen (to say — regular; similar)
- sehen (to see): ich sehe / ich sah / ich werde sehen

Future tense: Conceptually `werden` + infinitive. For simplicity in this scope, return "wird <verb>" for third person, "werde <verb>" for first, "wirst <verb>" for second. This is simplified — real German splits auxiliary and infinitive across the clause — but acceptable for the single-word `conjugate` API.

### Gender inference

Rough heuristic by ending (same pattern as Spanish):
- `-ung`, `-heit`, `-keit`, `-schaft`, `-ion`, `-ei`, `-e` → Fem
- `-chen`, `-lein`, `-um`, `-ment` → Neut
- `-er`, `-ling`, `-ismus`, `-ant`, `-ist` → Masc
- default: Masc (most common default)

Include ~30 common exceptions (Mädchen is neut, Tisch is masc, etc.) via lookup.

### `realize_reference` (pronouns)

German personal pronouns depend on gender + number + case. Restrict to nominative case for v1 — that covers subject pronouns, the most common context:

```
            Masc    Fem    Neut    Plural
Nom         er      sie    es      sie
```

Demonstratives ("dieser/diese/dieses/diese") follow the article table — emit the nominative form for the given gender/number.

### `plural_description`

"die 3 Klassen" (fem), "die 3 Häuser" (neut w/ umlaut), "die 3 Tische" (masc).
Format: `{article_for_gender_number_case_nominative} {count} {pluralized(entity_type)}`.

### `Language` trait impls (summary)

- `pluralize(word, count)`: count==1 → unchanged, else `pluralize_de(word)`
- `singularize(word)`: `singularize_de(word)` (strip plural suffixes, un-umlaut when unambiguous)
- `article(word)`: nominative, singular, gender inferred from ending
- `conjugate(verb, tense, person)`: as per above
- `past_participle(verb)`: `ge-` + stem + `-t` (simplified)
- `present_participle(verb)`: stem + `-end`
- `join_list(items, conj)`: German uses `und`/`oder`, Oxford-style comma is unusual — use `X, Y und Z` without comma before `und`
- `ordinal(n)`: `erste`, `zweite`, `dritte`, ..., `zehnte`; default `{n}.` for n ≥ 11
- `number_to_words(n)`: one-word German cardinals (eins, zwei, drei, ..., einundzwanzig, hundertdreiundzwanzig)
- `plural_category(n)`: `1 → One`, `_ → Other` (matches CLDR `de`)

### Out of scope

- **Attributive adjective declension** — weak/mixed/strong classes. Requires a full adjective-ending table; defer to v2.
- **Subjunctive** (Konjunktiv I, II) — deferred.
- **Perfekt compound** — trait surface is single-word; would need a tense-builder API.
- **Dative/genitive noun endings** — e.g. "dem Kinde" — archaic; ignore.
- **Capitalization enforcement** — all German nouns are capitalized; we trust the caller to pass already-capitalized forms. Do NOT auto-capitalize in `pluralize_de` — preserve input casing.

---

## Phase 0 — Baseline

```bash
cargo test --all-features
cargo clippy --all-features -- -D warnings
```

Expected: 713 tests passing, 0 warnings.

**No commit.**

---

## Phase 1 — Crate skeleton

1. Create `prosaic-grammar-de/Cargo.toml` modeled on `prosaic-grammar-es/Cargo.toml`:
   ```toml
   [package]
   name = "prosaic-grammar-de"
   version.workspace = true
   edition.workspace = true
   license.workspace = true
   description = "German grammar layer for the Prosaic NLG engine"

   [dependencies]
   prosaic-core = { path = "../prosaic-core", default-features = false }

   [dev-dependencies]
   prosaic-grammar-de = { path = "." }
   ```

2. Add `prosaic-grammar-de` to `Cargo.toml` workspace members (alphabetical).

3. Create empty module files:
   - `src/lib.rs` with `pub mod gender; pub(crate) mod pluralize; pub(crate) mod articles; pub(crate) mod conjugate; pub(crate) mod numbers;` and a stub `German` struct.

Verify: `cargo build --all-features` compiles.

---

## Phase 2 — gender.rs

Mirror `prosaic-grammar-es/src/gender.rs`:

```rust
use prosaic_core::Gender;

pub fn infer_gender(word: &str) -> Gender {
    // Normalize: trust input casing but match lowercase for suffixes
    let lower = word.to_lowercase();

    // Exception lookup
    if let Some(g) = exception_lookup(lower.as_str()) {
        return g;
    }

    // Feminine endings
    if lower.ends_with("ung") || lower.ends_with("heit") || lower.ends_with("keit")
        || lower.ends_with("schaft") || lower.ends_with("ion") || lower.ends_with("ei") {
        return Gender::Fem;
    }
    if lower.ends_with('e') && !lower.ends_with("chen") && !lower.ends_with("lein") {
        // Most -e nouns are feminine (Klasse, Katze, Flasche)
        return Gender::Fem;
    }

    // Neuter endings
    if lower.ends_with("chen") || lower.ends_with("lein")
        || lower.ends_with("um") || lower.ends_with("ment") {
        return Gender::Neut;
    }

    // Masculine (default + explicit endings)
    if lower.ends_with("er") || lower.ends_with("ling")
        || lower.ends_with("ismus") || lower.ends_with("ant") || lower.ends_with("ist") {
        return Gender::Masc;
    }

    Gender::Masc
}

fn exception_lookup(lower: &str) -> Option<Gender> {
    // ~20-30 common exceptions.
    match lower {
        "mädchen" | "fräulein" => Some(Gender::Neut),
        "tisch" | "stuhl" | "mann" | "tag" | "monat" | "herbst" => Some(Gender::Masc),
        "frau" | "nacht" | "stadt" | "hand" => Some(Gender::Fem),
        "haus" | "buch" | "kind" | "auto" | "wort" => Some(Gender::Neut),
        _ => None,
    }
}
```

### Tests
- 3× feminine ending tests (Meinung, Freiheit, Nation)
- 3× neuter ending tests (Mädchen, Buch→via exception, Ministerium)
- 3× masculine default tests (Tisch via exception, Lehrer via -er, plain default)
- 2× exception override tests (Mädchen is neut despite -chen default neut, Frau is fem despite no -e)

---

## Phase 3 — pluralize.rs

Implement `pluralize_de` with the decision tree above, plus an irregulars table of ~20 common words.

```rust
pub fn pluralize_de(word: &str) -> String {
    if let Some(pl) = irregular_plural(word) {
        return pl.to_string();
    }
    let lower = word.to_lowercase();

    // Suffix-driven rules
    if has_fem_suffix(&lower) { return add_en(word); }
    if lower.ends_with("chen") || lower.ends_with("lein") { return word.to_string(); }
    if lower.ends_with('e') { return format!("{word}n"); }
    if lower.ends_with("er") || lower.ends_with("el") || lower.ends_with("en") {
        return umlaut_or_same(word);
    }
    if lower.ends_with('o') || lower.ends_with('y') { return format!("{word}s"); }

    // Default: -e with possible umlaut on short vowel
    apply_umlaut_and_suffix(word, "e")
}

pub fn singularize_de(word: &str) -> String {
    // Strip common plural markers; best-effort only.
    if let Some(sg) = irregular_singular(word) { return sg.to_string(); }

    // -en / -n
    if word.ends_with("en") && word.len() > 3 { return word[..word.len()-2].to_string(); }
    if word.ends_with('n') && word.len() > 2 { return word[..word.len()-1].to_string(); }
    // -er + umlaut: Bücher → Buch (un-umlaut) — lossy; only attempt when safe
    // -e
    if word.ends_with('e') && word.len() > 2 { return word[..word.len()-1].to_string(); }
    // -s
    if word.ends_with('s') && word.len() > 2 { return word[..word.len()-1].to_string(); }
    word.to_string()
}
```

Irregulars table (incomplete; aim for ~20):
```
("Mann", "Männer"), ("Kind", "Kinder"), ("Haus", "Häuser"),
("Wort", "Wörter"), ("Buch", "Bücher"), ("Mensch", "Menschen"),
("Frau", "Frauen"), ("Apfel", "Äpfel"), ("Vater", "Väter"),
("Mutter", "Mütter"), ("Bruder", "Brüder"), ("Land", "Länder"),
("Stadt", "Städte"), ("Nacht", "Nächte"), ("Hand", "Hände"),
("Auto", "Autos"), ("Tag", "Tage"), ("Jahr", "Jahre"),
("Zeit", "Zeiten"), ("Klasse", "Klassen"),
```

### Umlaut helper

```rust
fn apply_umlaut_and_suffix(word: &str, suffix: &str) -> String {
    // Preserves input casing. Applies umlaut to the LAST a/o/u (not au → äu unless followed)
    // For v1 simplicity: only apply umlaut on explicit irregulars via the table, NOT via heuristic.
    // Regular rule: just append the suffix.
    format!("{word}{suffix}")
}
```

Keep umlaut application explicit via the irregulars table — do NOT try to apply umlaut heuristically, since it requires lexical knowledge of stem vowel stress patterns.

### Tests
- 5× regular suffix cases (each rule branch)
- 5× irregular lookup cases (Mann, Kind, Haus, Auto, Klasse)
- 3× case-preservation tests (input capitalized, output capitalized)
- Singularize round-trip for 5 common words

---

## Phase 4 — articles.rs

```rust
use prosaic_core::{AgreementFeatures, Case, Gender, GrammaticalNumber};

pub fn basic_article(word: &str) -> &'static str {
    match crate::gender::infer_gender(word) {
        Gender::Fem => "die",
        Gender::Neut => "das",
        _ => "der",
    }
}

pub fn article_with_features(features: &AgreementFeatures) -> &'static str {
    let plural = matches!(features.number, GrammaticalNumber::Plural | GrammaticalNumber::Dual);

    if plural {
        return match features.case {
            Case::Dative => "den",
            Case::Genitive => "der",
            _ => "die",
        };
    }

    match (features.gender, features.case) {
        // Masc
        (Gender::Masc, Case::Nominative) | (Gender::Masc, Case::Unknown) => "der",
        (Gender::Masc, Case::Accusative) => "den",
        (Gender::Masc, Case::Dative)     => "dem",
        (Gender::Masc, Case::Genitive)   => "des",
        // Fem
        (Gender::Fem, Case::Dative)   => "der",
        (Gender::Fem, Case::Genitive) => "der",
        (Gender::Fem, _) => "die",
        // Neut
        (Gender::Neut, Case::Dative)   => "dem",
        (Gender::Neut, Case::Genitive) => "des",
        (Gender::Neut, _) => "das",
        // Common / Unknown gender → default Masc
        (_, Case::Accusative) => "den",
        (_, Case::Dative)     => "dem",
        (_, Case::Genitive)   => "des",
        _ => "der",
    }
}

pub fn indefinite_article(features: &AgreementFeatures) -> &'static str {
    // No plural indefinite form in German (bare noun).
    if matches!(features.number, GrammaticalNumber::Plural | GrammaticalNumber::Dual) {
        return "";
    }
    match (features.gender, features.case) {
        (Gender::Masc, Case::Accusative) => "einen",
        (Gender::Masc, Case::Dative)     => "einem",
        (Gender::Masc, Case::Genitive)   => "eines",
        (Gender::Fem, Case::Dative)      => "einer",
        (Gender::Fem, Case::Genitive)    => "einer",
        (Gender::Fem, _) => "eine",
        (Gender::Neut, Case::Dative)     => "einem",
        (Gender::Neut, Case::Genitive)   => "eines",
        _ => "ein",
    }
}
```

### Tests (a LOT — case declension is the whole point)
- 4 cases × 4 gender-number combos = 16 definite article tests
- 3 cases × 3 genders + plural = ~10 indefinite article tests
- `basic_article` inference: masc/fem/neut via endings

---

## Phase 5 — conjugate.rs

Regular weak conjugation + ~10 irregulars. Mirror the Spanish module's structure but with German endings.

```rust
use prosaic_core::{Person, Tense};

fn irregular_lookup(verb: &str, tense: Tense, person: Person) -> Option<&'static str> {
    let forms: Option<[&'static str; 6]> = match (verb, tense) {
        ("sein", Tense::Present) => Some(["bin", "bist", "ist", "sind", "seid", "sind"]),
        ("sein", Tense::Past)    => Some(["war", "warst", "war", "waren", "wart", "waren"]),
        ("haben", Tense::Present) => Some(["habe", "hast", "hat", "haben", "habt", "haben"]),
        ("haben", Tense::Past)    => Some(["hatte", "hattest", "hatte", "hatten", "hattet", "hatten"]),
        ("werden", Tense::Present) => Some(["werde", "wirst", "wird", "werden", "werdet", "werden"]),
        ("werden", Tense::Past)    => Some(["wurde", "wurdest", "wurde", "wurden", "wurdet", "wurden"]),
        ("gehen", Tense::Past)     => Some(["ging", "gingst", "ging", "gingen", "gingt", "gingen"]),
        ("kommen", Tense::Past)    => Some(["kam", "kamst", "kam", "kamen", "kamt", "kamen"]),
        ("sehen", Tense::Past)     => Some(["sah", "sahst", "sah", "sahen", "saht", "sahen"]),
        ("können", Tense::Present) => Some(["kann", "kannst", "kann", "können", "könnt", "können"]),
        ("können", Tense::Past)    => Some(["konnte", "konntest", "konnte", "konnten", "konntet", "konnten"]),
        ("müssen", Tense::Present) => Some(["muss", "musst", "muss", "müssen", "müsst", "müssen"]),
        ("wollen", Tense::Present) => Some(["will", "willst", "will", "wollen", "wollt", "wollen"]),
        ("wollen", Tense::Past)    => Some(["wollte", "wolltest", "wollte", "wollten", "wolltet", "wollten"]),
        _ => None,
    };
    forms.map(|f| f[person_index(person)])
}

fn person_index(p: Person) -> usize {
    match p {
        Person::First  => 0, // ich
        Person::Second => 1, // du
        Person::Third  => 2, // er/sie/es
    }
}

pub fn conjugate_de(verb: &str, tense: Tense, person: Person) -> String {
    if let Some(form) = irregular_lookup(verb, tense, person) {
        return form.to_string();
    }

    // Strip -en infinitive
    let stem = verb.strip_suffix("en").unwrap_or(verb);

    match tense {
        Tense::Present => conjugate_present_regular(stem, person),
        Tense::Past    => conjugate_preterite_regular(stem, person),
        Tense::Future  => conjugate_future(verb, person),
    }
}

fn conjugate_present_regular(stem: &str, person: Person) -> String {
    let ending = match person {
        Person::First  => "e",
        Person::Second => "st",
        Person::Third  => "t",
    };
    format!("{stem}{ending}")
}

fn conjugate_preterite_regular(stem: &str, person: Person) -> String {
    let ending = match person {
        Person::First  => "te",
        Person::Second => "test",
        Person::Third  => "te",
    };
    format!("{stem}{ending}")
}

fn conjugate_future(verb: &str, person: Person) -> String {
    // Simplified: "werde <inf>" / "wirst <inf>" / "wird <inf>"
    let aux = match person {
        Person::First  => "werde",
        Person::Second => "wirst",
        Person::Third  => "wird",
    };
    format!("{aux} {verb}")
}

pub fn past_participle_de(verb: &str) -> String {
    // Simplified weak: ge- + stem + -t
    // Strong verbs omit ge- from prefixed forms; handle later.
    let stem = verb.strip_suffix("en").unwrap_or(verb);
    format!("ge{stem}t")
}

pub fn present_participle_de(verb: &str) -> String {
    // stem + -end (e.g. machend, laufend)
    let stem = verb.strip_suffix("en").unwrap_or(verb);
    format!("{stem}end")
}
```

### Tests
- Regular present for ich/du/er (machen)
- Regular preterite for ich/du/er (machen)
- Future for all 3 persons
- Past participle (gemacht, gesagt)
- Present participle (machend, laufend)
- Each of the ~5 irregulars across Present + Past

---

## Phase 6 — numbers.rs

German cardinals have an unusual "reversed" structure: 21 = einundzwanzig (one-and-twenty), 123 = hundertdreiundzwanzig.

```rust
pub fn number_to_words_de(n: usize) -> String {
    match n {
        0  => "null".into(),
        1  => "eins".into(),
        2  => "zwei".into(),
        3  => "drei".into(),
        4  => "vier".into(),
        5  => "fünf".into(),
        6  => "sechs".into(),
        7  => "sieben".into(),
        8  => "acht".into(),
        9  => "neun".into(),
        10 => "zehn".into(),
        11 => "elf".into(),
        12 => "zwölf".into(),
        13 => "dreizehn".into(),
        14 => "vierzehn".into(),
        15 => "fünfzehn".into(),
        16 => "sechzehn".into(), // note: sechs→sech
        17 => "siebzehn".into(), // note: sieben→sieb
        18 => "achtzehn".into(),
        19 => "neunzehn".into(),
        20..=99 => compound_tens(n),
        100..=999 => compound_hundreds(n),
        1000..=999_999 => compound_thousands(n),
        _ => n.to_string(),
    }
}

fn tens_word(t: usize) -> &'static str {
    match t {
        2 => "zwanzig", 3 => "dreißig", 4 => "vierzig", 5 => "fünfzig",
        6 => "sechzig", 7 => "siebzig", 8 => "achtzig", 9 => "neunzig",
        _ => "",
    }
}

fn compound_tens(n: usize) -> String {
    let tens = n / 10;
    let ones = n % 10;
    if ones == 0 {
        tens_word(tens).into()
    } else {
        let ones_word = number_to_words_de(ones);
        // eins → ein in compounds
        let ones_in_compound = if ones == 1 { "ein".to_string() } else { ones_word };
        format!("{}und{}", ones_in_compound, tens_word(tens))
    }
}

fn compound_hundreds(n: usize) -> String {
    let h = n / 100;
    let rest = n % 100;
    let h_prefix = if h == 1 { "einhundert".into() }
                   else { format!("{}hundert", number_to_words_de(h)) };
    if rest == 0 { h_prefix }
    else         { format!("{}{}", h_prefix, number_to_words_de(rest)) }
}

fn compound_thousands(n: usize) -> String {
    let k = n / 1000;
    let rest = n % 1000;
    let k_prefix = if k == 1 { "eintausend".into() }
                   else { format!("{}tausend", number_to_words_de(k)) };
    if rest == 0 { k_prefix }
    else         { format!("{}{}", k_prefix, number_to_words_de(rest)) }
}
```

### Tests
- 0, 1, 7, 10, 11, 12 (unit words)
- 13, 16, 17, 19 (teen irregulars — sechzehn, siebzehn stems)
- 20, 21 (einundzwanzig), 30 (dreißig spelling), 99 (neunundneunzig)
- 100, 101 (einhunderteins), 123 (einhundertdreiundzwanzig)
- 1000, 2001, 1_234_567

---

## Phase 7 — lib.rs wiring

Mirror `prosaic-grammar-es/src/lib.rs`:

```rust
//! German grammar layer for the Prosaic NLG engine.

pub mod gender;
pub(crate) mod articles;
pub(crate) mod conjugate;
pub(crate) mod numbers;
pub(crate) mod pluralize;

use prosaic_core::{
    AgreementFeatures, Case, Conjunction, Gender, GrammaticalNumber, Language, Person,
    PluralCategory, ReferenceForm, Tense,
};

use articles::{article_with_features, basic_article};
pub use articles::indefinite_article;
use pluralize::{pluralize_de, singularize_de};
use conjugate::{conjugate_de, past_participle_de, present_participle_de};
use numbers::number_to_words_de;

#[derive(Debug, Clone, Default)]
pub struct German;

impl German {
    pub fn new() -> Self { Self }
}

impl Language for German {
    fn pluralize(&self, word: &str, count: usize) -> String {
        if count == 1 { word.to_string() } else { pluralize_de(word) }
    }

    fn singularize(&self, word: &str) -> String { singularize_de(word) }

    fn article(&self, word: &str) -> &str { basic_article(word) }

    fn conjugate(&self, verb: &str, tense: Tense, person: Person) -> String {
        conjugate_de(verb, tense, person)
    }

    fn past_participle(&self, verb: &str) -> String { past_participle_de(verb) }
    fn present_participle(&self, verb: &str) -> String { present_participle_de(verb) }

    fn join_list(&self, items: &[&str], conjunction: Conjunction) -> String {
        let conj = match conjunction {
            Conjunction::And => "und",
            Conjunction::Or  => "oder",
        };
        join_list_de(items, conj)
    }

    fn ordinal(&self, n: usize) -> String {
        match n {
            1  => "erste".into(),
            2  => "zweite".into(),
            3  => "dritte".into(),
            4  => "vierte".into(),
            5  => "fünfte".into(),
            6  => "sechste".into(),
            7  => "siebte".into(),
            8  => "achte".into(),
            9  => "neunte".into(),
            10 => "zehnte".into(),
            _  => format!("{n}."),
        }
    }

    fn number_to_words(&self, n: usize) -> String { number_to_words_de(n) }

    fn plural_category(&self, n: i64) -> PluralCategory {
        match n {
            1 => PluralCategory::One,
            _ => PluralCategory::Other,
        }
    }

    fn realize_reference(
        &self,
        form: ReferenceForm,
        features: &AgreementFeatures,
    ) -> Option<String> {
        match form {
            ReferenceForm::Pronoun       => Some(german_pronoun(features)),
            ReferenceForm::Demonstrative => Some(german_demonstrative(features)),
            ReferenceForm::Zero          => None,
            ReferenceForm::Full | ReferenceForm::ShortName => None,
        }
    }

    fn plural_description(
        &self,
        entity_type: &str,
        count: usize,
        features: &AgreementFeatures,
    ) -> String {
        match count {
            0 => String::new(),
            1 => format!("{} {}", article_with_features(features), entity_type),
            _ => {
                let plural_features = AgreementFeatures::default()
                    .with_gender(features.gender)
                    .with_number(GrammaticalNumber::Plural);
                format!(
                    "{} {count} {}",
                    article_with_features(&plural_features),
                    self.pluralize(entity_type, count)
                )
            }
        }
    }
}

fn german_pronoun(features: &AgreementFeatures) -> String {
    let plural = matches!(features.number, GrammaticalNumber::Plural | GrammaticalNumber::Dual);
    if plural { return "sie".into(); }
    match features.gender {
        Gender::Fem => "sie".into(),
        Gender::Neut => "es".into(),
        _ => "er".into(),
    }
}

fn german_demonstrative(features: &AgreementFeatures) -> String {
    let plural = matches!(features.number, GrammaticalNumber::Plural | GrammaticalNumber::Dual);
    if plural { return "diese".into(); }
    match features.gender {
        Gender::Fem => "diese".into(),
        Gender::Neut => "dieses".into(),
        _ => "dieser".into(),
    }
}

fn join_list_de(items: &[&str], conj: &str) -> String {
    match items.len() {
        0 => String::new(),
        1 => items[0].into(),
        2 => format!("{} {} {}", items[0], conj, items[1]),
        _ => {
            let (last, rest) = items.split_last().unwrap();
            // German: "X, Y und Z" (no Oxford comma)
            format!("{} {} {}", rest.join(", "), conj, last)
        }
    }
}
```

### Tests for lib.rs
Full surface coverage mirroring `prosaic-grammar-es/src/lib.rs::tests`:
- pluralize (count=1 unchanged, count=0 plural, count=2 plural)
- singularize (basic)
- article (masc/fem/neut inference)
- conjugate (present 1st/3rd, past 1st/3rd)
- past_participle / present_participle
- join_list (0/1/2/3 items, And/Or)
- ordinal (1..=10, 11+ numeric)
- number_to_words (3, 21 spot-checks)
- plural_category (1, 0, 5)
- realize_reference: Pronoun × 4 (masc sg, fem sg, neut sg, plural) + Demonstrative × 4 + Zero → None + Full → None
- plural_description: 0/1/3 × masc/fem/neut
- **Case-aware plural_description test:** build AgreementFeatures with `.with_case(Case::Dative)` and verify article picks "den" (plural dative) or "dem" (masc sg dative)
- Send + Sync assertion

---

## Phase 8 — Final verification

```bash
cargo test --all-features
cargo test --no-default-features
cargo test --features serde
cargo clippy --all-features -- -D warnings
cargo doc --all-features --no-deps
```

Total tests should increase by ~60–80.

**Commit:** `Add prosaic-grammar-de German grammar crate with case declension`

---

## Definition of done

- [ ] `prosaic-grammar-de` is a workspace member
- [ ] `German` struct implements `Language` trait
- [ ] Articles correctly decline across Nom/Acc/Dat/Gen × Masc/Fem/Neut × Sg/Pl
- [ ] `pluralize` covers ~5 regular rules + ~20 irregulars
- [ ] `conjugate` covers regular weak verbs + ~5 strong irregulars across Present/Past/Future
- [ ] `number_to_words` handles 0 through ~1_000_000 with German compound-word spelling
- [ ] All existing tests still pass (713 baseline)
- [ ] New crate adds ~60–80 tests, all passing
- [ ] `cargo clippy --all-features -- -D warnings` clean
- [ ] `German: Send + Sync` assertion compiles
- [ ] One commit with subject: `Add prosaic-grammar-de German grammar crate with case declension`
