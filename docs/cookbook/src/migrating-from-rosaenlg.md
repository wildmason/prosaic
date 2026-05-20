# Migrating from RosaeNLG

RosaeNLG is a JavaScript NLG library built on Pug templates. Prosaic covers
the same conceptual ground — template-based NLG with synonym variation,
referring expressions, and verb conjugation — but as a native Rust library
with no Node.js runtime requirement.

This chapter has two parts:

1. **General concept mapping** — translating Pug-based RosaeNLG into Prosaic.
2. **Spanish-specific recipe** — side-by-side translation for `prosaic-grammar-es`.

## Concept mapping

| RosaeNLG | Prosaic | Notes |
|----------|---------|-------|
| Pug mixin (`mixin myEvent(data)`) | `engine.register_template("key", "…")` | Templates are strings, not Pug AST nodes |
| `#[+value(data.name)]` | `{name}` | Direct slot substitution |
| `#[+value(data.name) \| upper]` | `{name\|capitalize}` | Pipe chains replace Pug filters |
| `synz` block | Multiple templates registered under the same key | Engine's `Variation` selects among them |
| `#[+verb(subject, {verb: "run", tense: "PAST"})]` | `{verb\|verb:past}` or `Language::conjugate` | Tense is a pipe argument |
| `#[+subjectVerb(subject, verb)]` | `Sentence` / `Clause` builder API | Programmatic sentence assembly |
| `choosebest` (anti-repeat) | `Variation::RoundRobin` + `Session` discourse state | Discourse state tracked per-session |
| `forEach` aggregation | `engine.render_batch(&mut session, &events)` | Clause reduction happens automatically |
| Referring expression (`#[+ref(entity)]`) | `{name\|refer}` pipe | REG algorithm is Dale-Reiter or graph-based |
| Multilingual `.pug` files | Separate grammar crates (`prosaic-grammar-es`, `prosaic-grammar-de`) + `register_es` / `register_de` vocab functions | One `Language` implementor per language |
| Gender & number agreement via `data.gender` | `Value::Entity { features: AgreementFeatures }` | Carried alongside the entity name, consumed by `Language::plural_description` / `realize_reference` |
| Possessives (`#[+value(data, {det: "POSSESSIVE"})]`) | Template literal + `Value::Entity` with features | v1 doesn't have a dedicated possessive pipe — use template text |

## Key differences

**No Node.js runtime.** Prosaic is a Rust library. Templates are parsed at
`register_template` time and rendering allocates no heap beyond the output
string. There's no V8 startup, no npm install, no WASM bridge (unless you
specifically target WASM — see `prosaic-wasm`).

**Deterministic by default.** `Variation::Fixed` always picks the first
registered alternative. `Variation::Seeded(u64)` gives reproducibility with
a single seed. RosaeNLG's synonym selection is non-deterministic without
explicit seeding.

**PARENT faithfulness built in.** Prosaic ships a reference-free faithfulness
scorer with `score_faithfulness` and `assert_faithful!`. RosaeNLG leaves
hallucination detection to the caller.

**Strictness is explicit.** `Strict` errors on missing slots, `Lenient` emits
a placeholder, `Silent` emits nothing. RosaeNLG renders `null` silently.

**Compile-time validation available.** The `prosaic_template!` macro validates
slot keys and pipe names at compile time. The `prosaic_template_compiled!` macro
generates a monomorphized render function for bare-slot templates. No equivalent
in RosaeNLG.

**Discourse theory is first-class.** `Session` carries Centering Theory state
(Cb, Cf, transitions) and a temporal anchor that persists across paragraph
breaks. RosaeNLG has simpler reference tracking (`<ref>` macro only).

## Migrating templates

A RosaeNLG mixin like this:

```pug
mixin codeRenamed(data)
  | #{data.oldName}
  if data.consumerCount
    |  was renamed to #{data.newName},
    |  affecting #{data.consumerCount}
    if data.consumerCount == 1
      |  consumer
    else
      |  consumers
  else
    |  was renamed to #{data.newName}
```

Becomes this Prosaic template:

```rust
engine.register_template(
    "code.renamed",
    "{old_name} was renamed to {new_name}\
     {?consumer_count}, affecting {consumer_count} \
     {consumer_count|pluralize:consumer}{/?}",
).unwrap();
```

The conditional `{?consumer_count}...{/?}` handles both the zero and non-zero
case. The `pluralize` pipe handles the singular/plural split.

## Migrating synonym variation

RosaeNLG `synz` blocks list alternatives inline. Prosaic registers multiple
templates under the same key:

```rust
// Prosaic — register multiple templates under the same key
engine.register_template("code.renamed", "{old_name} was renamed to {new_name}").unwrap();
engine.register_template("code.renamed", "{old_name} is now called {new_name}").unwrap();
engine.register_template("code.renamed", "{old_name} has been renamed to {new_name}").unwrap();
```

The engine's `Variation` mode selects between them. `Seeded` gives stable
output; `Random` or `RoundRobin` gives natural variety. Engine-level
anti-repetition scoring picks the alternative with the lowest recent-word
overlap against previous renders.

## Phase 2: Spanish migration recipe

Prosaic shipped `prosaic-grammar-es` in v1.5. This section is the concrete
side-by-side translation guide for RosaeNLG Spanish users.

### Setup

```toml
[dependencies]
prosaic-core = "1.0.1"
prosaic-grammar-es = "1.0.1"
prosaic-vocab-code = "1.0.1"  # Optional — pre-built vocab with es.rs sibling
```

```rust
use prosaic_core::{Engine, Session, Context, Value, Strictness, Variation};
use prosaic_grammar_es::Spanish;

let mut engine = Engine::new(Spanish::new())
    .strictness(Strictness::Strict)
    .variation(Variation::Fixed);

// Register templates in Spanish, or use a vocab crate's es sibling:
prosaic_vocab_code::register_es(&mut engine)?;
```

### Gender & number agreement

RosaeNLG Spanish infers gender from a `data.gender` field and agrees articles,
pronouns, and past participles:

```pug
//- RosaeNLG
| #[+value(class, {det: "DEFINITE"})] fue renombrada
//- with class = { name: "Clase", gender: "F" }
//- renders: "La Clase fue renombrada"
```

Prosaic uses `Value::Entity { name, features }`:

```rust
use prosaic_core::{entity, AgreementFeatures, Gender};

let mut ctx = Context::new();
ctx.insert("class", entity("MiClase").fem().sing().defined());

engine.register_template(
    "renamed",
    "{class|refer} fue renombrada",  // "refer" resolves via Spanish::realize_reference
).unwrap();
```

The `Spanish` grammar's `plural_description` and `realize_reference` consume
the `AgreementFeatures` to produce correctly-agreeing output.

### Pronouns (él / ella / ellos / ellas)

RosaeNLG selects pronouns by gender + number:

```pug
//- RosaeNLG
| #[+pronoun(class)] fue importante
//- with class = { gender: "F", number: "P" } → "Ellas fue importante"
```

Prosaic: `{name|refer}` on a second mention of the same entity delegates to
`Spanish::realize_reference`, which returns the gender-and-number-appropriate
Spanish pronoun:

```rust
// First mention: Full form ("La clase MiClase")
// Second mention: Pronoun ("ella" for Fem Sing; "ellas" for Fem Plur)
engine.register_template("step1", "{class|refer} fue modificada.").unwrap();
engine.register_template("step2", "{class|refer} es ahora pública.").unwrap();

let ctx1 = {
    let mut c = Context::new();
    c.insert("class", entity("MiClase").fem().sing().defined());
    c
};
let out1 = engine.render(&mut session, "step1", &ctx1)?;
// "La clase MiClase fue modificada."

let ctx2 = {
    let mut c = Context::new();
    c.insert("class", entity("MiClase").fem().sing().defined());
    c
};
let out2 = engine.render(&mut session, "step2", &ctx2)?;
// "Ella es ahora pública." — pronoun via discourse state + realize_reference
```

### Verb conjugation

RosaeNLG:

```pug
//- #[+verb(getSubject(), {verb: "hablar", tense: "PASADO_COMPUESTO"})]
//- renders "ha hablado"
```

Prosaic uses `Language::conjugate` directly or the `verb` pipe:

```rust
engine.register_template(
    "spoke",
    "{subject} {_verb|verb:past}",
).unwrap();

let mut ctx = Context::new();
ctx.insert("subject", Value::String("Alicia".into()));
ctx.insert("_verb", Value::String("hablar".into()));
let out = engine.render(&mut session, "spoke", &ctx)?;
// "Alicia habló"
```

Supported tenses in `prosaic-grammar-es`: `Present`, `Past` (pretérito),
`Future`, plus `past_participle` ("hablado") and `present_participle`
("hablando"). Imperfecto and subjunctive are deferred to a future crate.

### Gender-aware past participles

RosaeNLG agrees past participles:

```pug
//- #[+verb(class, {verb: "renombrar", tense: "PASSIVE_PRESENT"})]
//- class = { gender: "F" } → "es renombrada"
```

Prosaic: grammar-es's `past_participle` returns masculine by default.
Gender-aware participle agreement requires template-level branching via
conditional sections — a convenience helper is on the roadmap:

```rust
engine.register_template(
    "renamed_fem",
    "{class|refer} fue renombrada",  // masculine: "renombrado"
).unwrap();
```

For the common case (classes, methods, services) the masculine default is
correct in Spanish technical prose. Explicit fem-agreement templates cover
the rest.

### Pluralization

RosaeNLG:

```pug
//- #[+value("clase", {number: "P"})] → "clases"
//- #[+value("función", {number: "P"})] → "funciones"
```

Prosaic's `Language::pluralize` on `Spanish`:

```rust
let es = Spanish::new();
assert_eq!(es.pluralize("clase", 3), "clases");
assert_eq!(es.pluralize("función", 3), "funciones");  // -ón → -ones rule
```

Or via the `pluralize` pipe:

```rust
engine.register_template(
    "count",
    "{count} {count|pluralize:clase}",
).unwrap();
// count=1 → "1 clase"; count=5 → "5 clases"
```

### Lists with "y" / "o"

RosaeNLG:

```pug
//- #[+synonym([A, B, C], "and")] → "A, B y C"
```

Prosaic:

```rust
engine.register_template(
    "list",
    "{items|join}",
).unwrap();

let mut ctx = Context::new();
ctx.insert("items", Value::List(vec!["A".into(), "B".into(), "C".into()]));
let out = engine.render(&mut session, "list", &ctx)?;
// "A, B y C" — Spanish::join_list omits the Oxford comma
```

### Numbers

RosaeNLG:

```pug
//- numberToText(123) → "ciento veintitrés"
```

Prosaic:

```rust
let es = Spanish::new();
assert_eq!(es.number_to_words(123), "ciento veintitrés");
```

Or via the `words` pipe: `{count|words}` → "ciento veintitrés".

### Ordinals

```rust
let es = Spanish::new();
assert_eq!(es.ordinal(1), "primero");
assert_eq!(es.ordinal(5), "quinto");
assert_eq!(es.ordinal(11), "11º");  // Beyond 10: numeric form
```

## What's still missing compared to RosaeNLG

- **Subjunctive (`subjuntivo presente`, `imperfecto de subjuntivo`)** — not yet
  supported. The `prosaic-grammar-es` verb table covers indicative only.
- **Imperfecto** (e.g. "hablaba", "comía") — not yet in the verb table.
- **Arbitrary JavaScript in templates** — deliberate non-goal. Prosaic
  templates are sandboxed; they can only substitute, apply pipes, and branch
  on truthiness. This is safer for untrusted template sources.
- **Deep Pug template inheritance** — Prosaic has partials (`{>name}`) for
  fragment reuse, but not deep hierarchies. Most NLG templates are shallow
  in practice.

## Languages available today

| Language | Crate | Status |
|----------|-------|--------|
| English | `prosaic-grammar-en` | Full (regular + 30+ irregular verbs, REG, Centering, ELLEIPO) |
| Spanish | `prosaic-grammar-es` | Regular conjugation + 10 irregulars; gender-aware articles, pluralization, pronouns; RST markers |
| German | `prosaic-grammar-de` | Case declension (Nom/Acc/Dat/Gen × Masc/Fem/Neut × Sg/Pl); regular weak verbs + ~10 irregulars; RST markers |

Additional languages can be added by implementing the `Language` trait in a
new crate — no core changes required.
