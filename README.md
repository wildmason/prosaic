# nlg

General-purpose natural language generation from structured data, in Rust.

Takes structured events and produces **natural-sounding** English text, not just grammatically correct output. The engine tracks discourse state across calls, so multiple renders flow together like human-written prose — using pronouns, varying phrasing, matching verbosity to impact, and structuring multi-paragraph narratives.

## What Makes It Natural

Many NLG libraries produce grammatical-but-robotic output like *"The class UserService was modified. The class UserService was renamed. The class UserService was moved."* This crate's engine is **discourse-aware** — it remembers what it just said and adapts subsequent output:

```text
The class UserService was renamed to AccountService, which impacts 6 direct consumers
  including ProfileComponent, SettingsComponent, and AdminModule among others.
Additionally, changes to it affect 3 dependents
  ProfilePage, SettingsPage, and AuthModule.
It has been updated (3 consumers may need review: ProfilePage, SettingsPage, AuthModule).
```

Notice: **pronouns** on second and third mentions, a **discourse connective** ("Additionally") linking related events, **list style variation** ("including … among others" vs bracketed), and **different template variants** chosen each time.

## Quick Start

```rust
use nlg_core::{Engine, Context, Session, Value, Variation, Strictness};
use nlg_grammar_en::English;

let mut engine = Engine::new(English::new())
    .strictness(Strictness::Strict)
    .variation(Variation::Fixed);

engine.register_template(
    "entity.renamed",
    "{old_name|refer} was renamed to {new_name}{?consumer_count}, \
     which impacts {consumer_count} direct {consumer_count|pluralize:consumer}{?consumers} \
     {consumers|truncate:3|join}{/?}{/?}",
)?;

let mut ctx = Context::new();
ctx.insert("entity_type", Value::String("class".into()));
ctx.insert("old_name", Value::String("Foo".into()));
ctx.insert("new_name", Value::String("Foobar".into()));
ctx.insert("consumer_count", Value::Number(6));
ctx.insert("consumers", Value::List(vec![
    "Baz".into(), "Qux".into(), "Quux".into(),
    "Corge".into(), "Grault".into(), "Garply".into(),
]));

// Session holds all discourse state (focus, word history, list-style cycle).
// Create one per logical document — reset it or drop it to start a new narrative.
let mut session = engine.new_session();
let sentence = engine.render(&mut session, "entity.renamed", &ctx)?;
// "The class Foo was renamed to Foobar, which impacts 6 direct consumers
//  including Baz, Qux, and Quux among others."
```

## Crate Structure

| Crate | Purpose |
|---|---|
| `nlg-core` | Engine, templates, discourse, salience, document planning, builder API, referring expression generation, `Language` trait |
| `nlg-grammar-en` | English grammar: pluralization, articles, conjugation, list formatting, ordinals, number-to-words, past participles |
| `nlg-derive` | `#[derive(IntoContext)]` for automatic struct-to-context conversion |
| `nlg-vocab-code` | Code-analysis vocabulary templates (renamed, deleted, added, modified, moved, signature changed) at all three salience levels |
| `nlg-vocab-git` | Git/VCS activity templates (commits, PRs, issues, reviews, releases) |
| `nlg-cli` | `nlg` binary: reads JSON-lines events on stdin, writes rendered prose on stdout |

## Core Concepts

### Templates and Pipes

Templates use `{slot}` for substitution and `{slot|pipe}` for transforms. Pipes chain left-to-right: `{items|truncate:3|join}`.

| Pipe | Example | Output |
|---|---|---|
| `pluralize:word` | `{count\|pluralize:item}` | "item" or "items" based on count |
| `article` | `{thing\|article}` | "an apple" / "a banana" / "an hour" / "a user" |
| `join` | `{items\|join}` | Auto-selects natural list style (see below) |
| `join:or` | `{items\|join:or}` | "a, b, or c" with Oxford comma |
| `join:bracketed` | `{items\|join:bracketed}` | Forces bracket style: `[a, b, and c]` |
| `truncate:N` | `{items\|truncate:3}` | First 3 items + "N more" tail |
| `ordinal` | `{n\|ordinal}` | "1st", "2nd", "3rd" |
| `words` | `{n\|words}` | 42 → "forty-two" |
| `capitalize` | `{word\|capitalize}` | "Hello" |
| `refer` | `{name\|refer}` | Discourse-aware entity reference (see below) |
| `verb:form` | `{rename\|verb:present_perfect}` | "has been renamed" — full tense/aspect phrase (see below) |
| `syn` | `{class\|syn}` | Pick a registered synonym, least recently used (see *Elegant Variation*) |
| `relative` | `{ts\|relative}` | Unix timestamp → "yesterday" / "3 weeks ago" / "in 2 months" |
| `quantify` | `{n\|quantify}` | 0 → "no", 1 → "a single", 47 → "47", 300 → "hundreds of"; `:exact` / `:hedged` flavours available |
| `hedge` | `{conf\|hedge}` | 0..=100 confidence → "certainly" / "likely" / "probably" / "possibly" / "perhaps"; `:modal` / `:prefix` flavours |
| `negated` | `{phrase\|negated}` | Emit a negated verb phrase — registered positive antonym if available, else inserts "not" after the aux |
| `demonstrative` | `{change\|demonstrative}` | "this change" (continuation) / "the change" (fresh discourse) |

### Conditional Sections

Wrap optional content in `{?key}...{/?}` to render it only when the condition key is truthy (non-zero number, non-empty list/string):

```rust
engine.register_template(
    "deleted",
    "{name|refer} was removed{?consumer_count}, \
     impacting {consumer_count} {consumer_count|pluralize:dependent}{/?}",
)?;
```

When `consumer_count` is 0, output is simply `"The class Foo was removed."` — no awkward "impacting 0 dependents."

### Referring Expressions

The `{name|refer}` pipe tracks entity mentions and adapts the reference form:

| Situation | Output |
|---|---|
| First mention | `"The class UserService"` (full form with article + type) |
| Recent mention, same focus, unambiguous | `"it"` (pronoun) |
| Recent mention, non-focus or ambiguous | `"UserService"` (short name) |
| Distant mention (3+ renders ago) | Re-introduces with full form |

Capitalization is handled automatically based on sentence position.

### Referring Expression Generation (REG)

When multiple entities of the same type appear in a narrative, the bare form "the class UserService" and "the class AuthService" is grammatical but says nothing to distinguish them. Register entities with distinguishing **attributes** and the engine runs the Dale & Reiter Incremental Algorithm — the standard REG approach — to pick the shortest attribute set that uniquely identifies each:

```rust
use nlg_core::EntityDescriptor;

let mut engine = Engine::new(English::new())
    .attribute_preference(vec!["layer".into()]);

engine.register_entity(
    EntityDescriptor::new("UserService", "class").with_attribute("layer", "domain"),
);
engine.register_entity(
    EntityDescriptor::new("AuthService", "class").with_attribute("layer", "infra"),
);

engine.register_template("t", "{name|refer} was modified")?;
let mut session = engine.new_session();
engine.render(&mut session, "t", &user_ctx)?;  // "The domain class UserService was modified."
engine.render(&mut session, "t", &auth_ctx)?;  // "Similarly, the infra class AuthService was modified."
```

**Algorithm**: For each target entity, start with all registered same-type entities as distractors. Walk the preferred attribute order; for each attribute, if including its value rules out at least one distractor, add it. Stop as soon as no distractors remain.

This means REG is pay-as-you-go — unambiguous entities render with just "the class Foo", but as soon as a same-type rival is registered, the minimal distinguisher appears:

| Scenario | Output |
|---|---|
| Only `UserService` registered | `the class UserService` |
| `UserService` + `AuthService` (different layer) | `the domain class UserService` |
| Three widgets sharing color OR size | `the red small widget Alpha` (both attrs needed) |
| Same name, different type (`UserService` class vs `UserService` trait) | `the class UserService` (head noun alone disambiguates) |

REG only applies to the *Full form* path of `{name|refer}`. Pronouns and short names (used after an entity is already in focus) stay terse.

### Discourse Connectives

When consecutive renders share an entity or action type, the engine inserts natural connectives:

| Relationship | Connectives (rotates to avoid repetition) |
|---|---|
| Same entity, different action | "Additionally,", "Furthermore,", "It also" |
| Different entity, same action | "Similarly,", "Likewise," |
| Contrasting actions (add vs delete) | "Meanwhile,", "However,", "On the other hand," |

### Anaphora: Plural Pronouns and Demonstratives

The discourse system produces appropriate pronoun forms for compound subjects:

```text
"UserService and AuthService were renamed."          (aggregated compound subject)
"They were deployed to production."                  (follow-up uses "they", not "it")
```

The `{noun|demonstrative}` pipe emits context-aware determiners. Useful for continuation templates that refer to a prior action rather than an entity:

```rust
engine.register_template("t", "{change|demonstrative} affects {n} consumers")?;
// First render (fresh discourse): "the change affects 6 consumers"
// Follow-up (prior render in scope): "this change affects 6 consumers"
```

### Hedging / Confidence Language

Turn a 0–100 confidence score into a hedge word that matches its certainty. Three flavours:

```rust
engine.register_template("adv",    "The change {c|hedge} broke tests")?;        // adverb (default)
engine.register_template("modal",  "The change {c|hedge:modal} break tests")?;  // modal verb
engine.register_template("prefix", "{c|hedge:prefix} the change broke tests")?; // prefix clause
```

| Score | Adverb | Modal | Prefix |
|---|---|---|---|
| 95 | certainly | must | it is certain that |
| 80 | likely | should | it is likely that |
| 60 | probably | may | probably |
| 40 | possibly | might | possibly |
| 10 | perhaps | could | perhaps |

### Negation Naturalization

Register positive-framing antonyms for common negative verb phrases; the `{phrase|negated}` pipe prefers the antonym when available, falling back to a grammatical "not"-insertion when it isn't:

```rust
engine.register_antonym("was modified", "remained unchanged");
engine.register_antonym("was broken", "still works");

engine.register_template("t", "The class {name} {state|negated}")?;
// state="was modified"  → "The class Foo remained unchanged."
// state="was renamed"   → "The class Foo was not renamed."   (fallback)
// state="has been moved" → "The class Foo has not been moved." (fallback, "not" after first aux)
```

The fallback correctly places "not" after the first auxiliary so "has been renamed" → "has not been renamed" (not the incorrect "has been not renamed").

### Sentence-Length Budgeting

Set a character budget; sentences exceeding it get split at natural boundaries with a lightweight grammatical fix-up on the continuation:

```rust
let engine = Engine::new(English::new()).max_sentence_length(80);
```

| Input | Output (with budget) |
|---|---|
| `"The class X was renamed to Y, which impacts 6 consumers"` | `"The class X was renamed to Y. This impacts 6 consumers."` |
| `"… affecting 3 routes …"` | `"… This affects 3 routes …"` |
| `"… including Foo, Bar, Baz"` (long tail) | `"… Including Foo, Bar, Baz."` |
| Single long word with no natural boundary | Passes through unchanged (never chops mid-word) |

Split points (in priority order): subordinate clauses (", which", ", affecting", ", impacting", ", requiring"), list prefixes (" including"), em-dashes (" — "), and explicit sentence breaks. Splitting is recursive — if the tail is still over budget, it splits again.

### Elegant Variation (Synonyms)

Repeating the same noun across several sentences reads robotically, even when the templates themselves vary. Register synonym groups and use the `{word|syn}` pipe — the engine tracks recent output and picks whichever synonym has been used the least:

```rust
engine.register_synonyms(&["consumer", "dependent", "caller"]);

// Template: "{count} {word|syn} may need review"
// Three consecutive renders with word="consumer" now rotate through
// "consumer", "dependent", "caller" instead of repeating "consumer" each time.
```

Ties break toward registration order for deterministic output. Unregistered words pass through unchanged. Input capitalization is preserved.

### Time-Aware Framing

The `{timestamp|relative}` pipe converts a Unix-epoch integer into a natural relative phrase:

| Input (seconds from now) | Output |
|---|---|
| `0` (now) | just now |
| `-30` (30 s ago) | just now |
| `-3600` | an hour ago |
| `-90_000` | yesterday |
| `-3 * 86_400` | 3 days ago |
| `-10 * 86_400` | last week |
| `-3 * 30 * 86_400` | 3 months ago |
| `+86_400 * 2` | in 2 days |
| `+86_400 * 10` | next week |

The engine uses `SystemTime::now()` as "now" by default; override with `engine.reference_time(unix_secs)` for deterministic tests or "as of" reports.

### Quantifier Naturalization

The `{count|quantify}` pipe replaces awkward raw numbers with natural phrasing — essential for "0 consumers" / "1 consumer" edge cases:

| Count | Natural (default) | Exact | Hedged |
|---|---|---|---|
| `0` | no | no | no |
| `1` | a single | a single | a single |
| `3` | three | 3 | a few |
| `12` | twelve | 12 | a handful of |
| `47` | 47 | 47 | dozens of |
| `55` | about 60 | 55 | scores of |
| `150` | over a hundred | 150 | scores of |
| `473` | hundreds of | 473 | hundreds of |
| `5000` | thousands of | 5000 | thousands of |

Use `{n|quantify:exact}` when you want precise numbers unconditionally, or `{n|quantify:hedged}` when counts come from noisy sources and even small numbers should be hedged.

### Clause Aggregation (Conjunction Reduction)

When a batch or document plan renders a run of same-entity events whose voice and tense match, `render_batch` fuses their predicates into one sentence:

```text
input events (rendered individually):
  "The class UserService was renamed."
  "It was modified."
  "It was moved from src/ to lib/."

reduced output:
  "The class UserService was renamed, modified, and moved from src/ to lib/."
```

The reducer declines when the result would be lossy:
- predicates with embedded subordinate clauses (", which affects …", ", requiring …") stay separate
- mixed auxiliaries (`was …` vs `has been …`) stay separate
- sentences where the entity is reintroduced by full form (not via "it") stay separate

Discourse connectives ("Additionally,", "Similarly,", …) that the discourse system would otherwise prepend to continuation sentences are stripped first — the final conjunction ("and") takes over the linking role.

### Natural List Formatting

The `join` pipe auto-cycles through four styles to avoid repetitive list formatting across renders:

| Style | Example |
|---|---|
| Including | `"including A, B, and C among others"` |
| Such as | `"such as A, B, and C"` |
| Dash | `"— notably A, B, and C, plus 2 others"` |
| Bracketed | `"[A, B, C, and 2 more]"` |

Force a specific style with `{items|truncate:3|join:bracketed}`.

### Tense and Aspect

The `verb` pipe composes a full verb phrase — **tense × aspect × voice × mood** — from any base verb. This lets templates vary verbal nuance without hand-writing every form.

```rust
// Same base verb, six natural framings
engine.register_template("past",       "{action|verb:past}")?;              // "was renamed"
engine.register_template("perfect",    "{action|verb:present_perfect}")?;   // "has been renamed"
engine.register_template("ongoing",    "{action|verb:present_progressive}")?; // "is being renamed"
engine.register_template("future",     "{action|verb:future}")?;            // "will be renamed"
engine.register_template("hypothetical","{action|verb:conditional}")?;      // "would be renamed"
engine.register_template("active_past","{action|verb:active_past}")?;       // "renamed"
```

| Form spec | Passive (default) | Active (prefix `active_`) |
|---|---|---|
| `past` | was renamed | renamed |
| `present` | is renamed | renames |
| `future` | will be renamed | will rename |
| `present_perfect` | has been renamed | has renamed |
| `past_perfect` | had been renamed | had renamed |
| `future_perfect` | will have been renamed | will have renamed |
| `present_progressive` | is being renamed | is renaming |
| `past_progressive` | was being renamed | was renaming |
| `conditional` | would be renamed | would rename |
| `conditional_perfect` | would have been renamed | would have renamed |

The same composition is available on the builder:

```rust
Sentence::new()
    .subject(entity("class", "OrderProcessor"))
    .verb_word("break")
    .form(VerbForm::PresentPerfect)
    .render(&engine)?;
// "The class OrderProcessor has been broken"
```

English handles irregular verbs throughout the pipeline — `break` ↔ `broken`, `write` ↔ `written`, `lie` ↔ `lying`, plus regular rules for consonant doubling (`stop` → `stopping`) and silent-e drop (`rename` → `renaming`). Add a new language by implementing `Language::past_participle` and `Language::present_participle`; the default `verb_phrase` implementation composes everything else.

### Importance-Aware Verbosity (Salience)

Register templates at specific salience levels, and the engine picks verbosity that matches event magnitude:

```rust
use nlg_core::Salience;

// Low: terse — used for 0-1 consumers
engine.register_template_at(
    "code.modified",
    "{name|refer} was modified",
    Salience::Low,
)?;

// Medium (default): standard — used for 2-19 consumers
engine.register_template(
    "code.modified",
    "{name|refer} was modified{?consumer_count}, affecting {consumer_count} \
     {consumer_count|pluralize:consumer}{/?}",
)?;

// High: elaborative — used for 20+ consumers
engine.register_template_at(
    "code.modified",
    "{name|refer} has been substantially modified, with downstream impact across \
     {consumer_count} {consumer_count|pluralize:consumer}{?consumers} including \
     {consumers|truncate:5|join:bracketed}{/?}. Thorough review is recommended.",
    Salience::High,
)?;
```

The engine derives salience from:
1. An explicit `salience` context key (`"low"` / `"medium"` / `"high"`)
2. The `consumer_count` value mapped through `SalienceThresholds`
3. Default: `Medium`

Customize thresholds:

```rust
use nlg_core::SalienceThresholds;

let engine = Engine::new(English::new())
    .salience_thresholds(SalienceThresholds {
        low_max: 3,     // 0, 1, 2 → Low
        high_min: 50,   // 50+ → High; 3-49 → Medium
    });
```

Fallback chain: if no template is registered at the target salience, the engine falls back to Medium, then to any available template.

### Document Planning

For multi-paragraph narratives, `DocumentPlan` takes a flat event list and organizes it:

```rust
use nlg_core::DocumentPlan;

let events: Vec<(&str, Context)> = vec![
    ("code.added", minor_add_ctx),          // Low-impact trivia
    ("code.modified", repo_mod_ctx),        // Medium, same entity
    ("code.modified", repo_mod_ctx2),       // Medium, same entity
    ("code.renamed", critical_rename_ctx),  // High-impact
];

let plan = DocumentPlan::from_events(&events, &engine);
let narrative = plan.render(&engine)?;
```

Produces a multi-paragraph narrative where:
- Biggest changes lead (ordered by highest salience first)
- Consecutive events sharing an entity are grouped into the same paragraph
- Discourse state resets between paragraphs so entities reintroduce cleanly
- Within a paragraph, pronouns and connectives flow naturally

#### Rhetorical grouping

For release-note-style summaries, switch to action-category grouping — removals, additions, and modifications become their own sections in that canonical order:

```rust
use nlg_core::GroupingStrategy;

let plan = DocumentPlan::from_events_grouped(
    &events, &engine, GroupingStrategy::ByAction,
);
let narrative = plan.render(&engine)?;
```

The built-in classifier maps well-known template keys (`code.added`, `code.deleted`, `code.modified`, `code.renamed`, `code.moved`, `code.signature_changed`, etc.) into `RhetoricalCategory::{Removal, Addition, Modification, Other}`. For domain-specific keys, pass a custom classifier via `DocumentPlan::from_events_classified(events, engine, |key| …)`. Within each section, events sharing an entity stay clustered so co-reference still works.

### Sessions, Discourse State, and Long-Lived Services

`Engine` is stateless after construction — all template registrations, grammar rules, and configuration are immutable. Every `render()` call takes a `&mut Session` that carries the discourse state (focus stack, word-frequency log, list-style cycle, round-robin counters) for one logical narrative.

Pick the lifecycle that matches your workload:

- **Single narrative / batch** — create one `Session`, share it across the whole batch. The engine links outputs together through it.
- **Multi-tenant / long-lived service** — create one `Session` per request. Drop it when the request ends, or call `session.reset()` to reuse the allocation. The `Engine` itself is `Send + Sync` and can safely be shared across threads (e.g. inside an `Arc`).
- **Isolated renders** — create a fresh `Session::new()` for each call. State never leaks between calls.
- **Failed renders are safe** — `render()` is transactional: if a render fails (missing slot in Strict mode, unknown pipe, etc.), the session state is rolled back to what it was before the call.

Per-request session:

```rust
// Engine is shared; sessions are per-request
let mut session = engine.new_session();   // or Session::new()

engine.render(&mut session, "code.renamed", &event1)?;
engine.render(&mut session, "code.modified", &event2)?;
// ...generates output linking these two events...

session.reset();  // start a new narrative in the same session

engine.render(&mut session, "code.added", &event3)?;
// ...starts fresh, no pronouns or connectives referencing prior events
```

## Builder API

For complex programmatic sentences where templates get unwieldy:

```rust
use nlg_core::{Sentence, Clause, Voice, entity, Tense};

let sentence = Sentence::new()
    .subject(entity("class", "Foo"))
    .verb("rename", Tense::Past)       // default: passive voice
    .object("Foobar")
    .clause(
        Clause::which("impacts")
            .amount(6)
            .noun("direct consumer")
            .list(&["Baz", "Qux", "Quux", "Corge", "Grault", "Garply"])
            .truncate(3),
    )
    .render(&engine)?;
// "The class Foo was renamed to Foobar which impacts 6 direct consumers..."

// Active voice
let active = Sentence::new()
    .subject(entity("team", "Backend"))
    .verb("deploy", Tense::Past)
    .voice(Voice::Active)
    .object("the service")
    .render(&engine)?;
// "The team Backend deployed the service"

// Custom preposition
let replaced = Sentence::new()
    .subject(entity("class", "OldParser"))
    .verb("replace", Tense::Past)
    .preposition("with")
    .object("NewParser")
    .render(&engine)?;
// "The class OldParser was replaced with NewParser"
```

`Tense` handles `Past`, `Present`, and `Future` for the simple case; for richer constructions (present perfect, progressive, conditional) use `.form(VerbForm::…)` — see **Tense and Aspect** above. Passive voice uses past participles for irregular verbs ("was renamed", "was broken", "was chosen").

## Derive Macro

Convert structs to template contexts automatically:

```rust
use nlg_derive::IntoContext;

#[derive(IntoContext)]
struct RenameEvent {
    entity_type: String,
    old_name: String,
    new_name: String,
    consumer_count: i64,
    consumers: Vec<String>,
}

let event = RenameEvent { /* ... */ };
let mut session = engine.new_session();
let sentence = engine.render(&mut session, "code.renamed", event)?;
```

Supported field types: `String`, `&str` (cloned into the context), integer types (`i8`…`i64`, `u8`…`u64`, `usize`, `isize`), `Vec<String>`, and `Option<T>` wrapping any of those (skipped when `None`). Unsupported field types produce a compile-time error — no silent drops — so template slots can't disappear from a struct without being noticed.

## Vocabulary Modules

Pre-built domain vocabularies register a family of templates in one call:

```rust
use nlg_vocab_code;

let mut engine = Engine::new(English::new());
nlg_vocab_code::register(&mut engine)?;

// Available keys:
//   code.renamed        code.deleted          code.added
//   code.modified       code.moved            code.signature_changed
```

Each key has multiple template variants at Low/Medium/High salience levels, covering terse summaries through to elaborative descriptions for high-impact events.

## Variation Strategies

| Strategy | Behavior |
|---|---|
| `Variation::Fixed` | Always picks the first-registered alternative. Literal and predictable. |
| `Variation::RoundRobin` | Strictly cycles through alternatives in registration order. |
| `Variation::Seeded(n)` | Deterministic hash-based selection, layered with choose-best scoring. Same seed + same discourse state produces the same output. |
| `Variation::Random` | Non-deterministic, layered with choose-best scoring. Not for tests. |

`Seeded` and `Random` are additionally refined by the engine's **choose-best scoring** — on the second and subsequent renders, the engine scores candidate alternatives against recent word history and emits the least-repetitive one. `Fixed` and `RoundRobin` deliberately skip choose-best so their contracts stay literal.

Cross-render naturalness (pronouns, connectives, list-style cycling, sentence termination) is produced by the discourse system and applies to every variation strategy — even `Fixed`.

## Strictness Modes

| Mode | Missing slot behavior |
|---|---|
| `Strictness::Strict` (default) | Returns `Err(NlgError::MissingSlot)` |
| `Strictness::Lenient` | Renders as `[missing: slot_name]` |
| `Strictness::Silent` | Renders as empty string, plus cleanup: dangling prepositions and conjunctions left by omitted slots (`"was modified by "`) get stripped so output reads naturally. |

## Adding a Language

Implement the `Language` trait from `nlg-core`:

```rust
pub trait Language: Send + Sync {
    fn pluralize(&self, word: &str, count: usize) -> String;
    fn singularize(&self, word: &str) -> String;
    fn article(&self, word: &str) -> &str;
    fn conjugate(&self, verb: &str, tense: Tense, person: Person) -> String;
    fn past_participle(&self, verb: &str) -> String;
    fn present_participle(&self, verb: &str) -> String;
    fn join_list(&self, items: &[&str], conjunction: Conjunction) -> String;
    fn ordinal(&self, n: usize) -> String;
    fn number_to_words(&self, n: usize) -> String;

    // Default impl composes `conjugate` + participles into a full verb
    // phrase covering tense × aspect × voice × mood. Override only if
    // the language's auxiliary structure differs from English's.
    fn verb_phrase(
        &self, verb: &str, form: VerbForm, voice: Voice, person: Person,
    ) -> String { english_verb_phrase(self, verb, form, voice, person) }
}
```

The trait boundary is language-agnostic — the engine, discourse system, and template renderer don't hardcode English. Drop in any language implementation and the full naturalness pipeline works.

## Running the Demo

A complete end-to-end demo exercising every feature:

```bash
cargo run --example demo
```

The demo covers referring expressions, salience, document planning, discourse-aware sequential rendering, batch rendering with aggregation, templates, the builder API, the vocab module, variation strategies, strictness modes, the derive macro, and grammar edge cases.

## Developer Tools

### Template Partials

Share fragments across templates with `{>name}`:

```rust
engine.register_partial(
    "impact_tail",
    "{?consumer_count}, affecting {consumer_count} \
     {consumer_count|pluralize:consumer}{/?}",
)?;
engine.register_template("code.modified", "{name|refer} was modified{>impact_tail}")?;
engine.register_template("code.renamed",  "{name|refer} was renamed{>impact_tail}")?;
```

Partials use the same syntax as top-level templates — slots, pipes, conditionals, and nested partials all work.

### A/B Variant Scoring

`engine.score_variants(&mut session, key, ctx)` returns every variant that would be considered, along with the choose-best score the engine would assign and a flag marking which one would be selected right now:

```rust
for v in engine.score_variants(&mut session, "code.renamed", &ctx)? {
    println!("[{:?}] score={:.2} {}{}",
        v.salience, v.score,
        if v.selected { "← selected " } else { "" },
        v.rendered);
}
```

Does not mutate discourse state.

### Explain Output

`engine.render_explained(&mut session, key, ctx)` returns a `RenderExplanation` with the output plus the engine's decisions — variant index and source, salience bucket, candidate scores, reference form, connective, plural focus, and whether the length-budget split fired. Useful for debugging vocab modules.

### Streaming Render

`engine.render_iter(&mut session, events)` returns an iterator over `Result<String, NlgError>`, yielding one sentence per aggregated run. Each `.next()` produces output as soon as the next batch unit is ready — useful for long code-review narratives where time-to-first-sentence matters.

### Punctuation Polish

Opt-in typographic quotes:

```rust
let engine = Engine::new(English::new()).smart_quotes(true);
// "foo" → "foo",  'bar' → ‘bar’,  it's → it’s
```

## Cargo Features

All three feature flags are enabled by default. Disable for leaner builds (embedded, WASM, stripped-down services).

| Feature | Default | What it gates |
|---|---|---|
| `time` | on | `{ts\|relative}` pipe, `engine.reference_time()`, `SystemTime::now()` fallback. Disable for WASM targets where system time panics. |
| `polish` | on | `engine.smart_quotes()`, `engine.max_sentence_length()` and their post-processing passes. |
| `reg` | on | Dale & Reiter REG algorithm, `EntityDescriptor`, `EntityRegistry`, `engine.register_entity()`, `engine.attribute_preference()`. `{name\|refer}` still produces Full/Short/Pronoun forms from discourse state without this feature. |
| `serde` | off | `Serialize`/`Deserialize` on `Context`, `Value`, `EntityDescriptor`, `RenderExplanation`, `VariantScore`, and all configuration enums. |

Build examples:

```bash
# Minimal build (no optional pipes, no post-processing)
cargo build --package nlg-core --no-default-features

# WASM-safe build (time feature off so SystemTime::now is never referenced)
cargo build --package nlg-core --target wasm32-unknown-unknown --no-default-features --features "reg,polish"

# With serde for JSON payloads
cargo build --package nlg-core --features serde
```

Even with the `time` feature enabled, the crate compiles for `wasm32-unknown-unknown`. The runtime fallback to `SystemTime::now()` is guarded with `cfg(target_arch)` — on wasm32 the engine returns a clear error if you use the `relative` pipe without calling `engine.reference_time()` first.

## Command-Line Usage

The `nlg` binary turns the engine into a pipe-friendly tool:

```bash
echo '{"key":"code.renamed","entity_type":"class","old_name":"Foo","new_name":"Bar","consumer_count":3}' \
  | nlg --strategy sequential
```

produces

```text
The class Foo was renamed to Bar, which impacts 3 direct consumers.
```

Flags: `--vocab code|git|both|none`, `--strategy sequential|by-entity|by-action`, `--smart-quotes`, `--max-length <N>`, `--explain` (emit JSON `RenderExplanation` per event), `--strict|--lenient|--silent`.

## Design Philosophy

Deterministic, rule-based NLG — no LLM dependencies, no non-deterministic behavior by default. The goal is **natural-sounding output that is fully reproducible and testable**. Research informed by Reiter's NLG pipeline (content planning → microplanning → realisation), RosaeNLG's choosebest and referring expression systems, SimpleNLG's aggregation patterns, and Dale & Reiter's REG work.

## License

MIT OR Apache-2.0
