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
use nlg_core::{Engine, Context, Value, Variation, Strictness};
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

let sentence = engine.render("entity.renamed", &ctx)?;
// "The class Foo was renamed to Foobar, which impacts 6 direct consumers
//  including Baz, Qux, and Quux among others."
```

## Crate Structure

| Crate | Purpose |
|---|---|
| `nlg-core` | Engine, templates, discourse, salience, document planning, builder API, `Language` trait |
| `nlg-grammar-en` | English grammar: pluralization, articles, conjugation, list formatting, ordinals, number-to-words, past participles |
| `nlg-derive` | `#[derive(IntoContext)]` for automatic struct-to-context conversion |
| `nlg-vocab-code` | Code-analysis vocabulary templates (renamed, deleted, added, modified, moved, signature changed) at all three salience levels |

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

### Discourse Connectives

When consecutive renders share an entity or action type, the engine inserts natural connectives:

| Relationship | Connectives (rotates to avoid repetition) |
|---|---|
| Same entity, different action | "Additionally,", "Furthermore,", "It also" |
| Different entity, same action | "Similarly,", "Likewise," |
| Contrasting actions (add vs delete) | "Meanwhile,", "However,", "On the other hand," |

### Natural List Formatting

The `join` pipe auto-cycles through four styles to avoid repetitive list formatting across renders:

| Style | Example |
|---|---|
| Including | `"including A, B, and C among others"` |
| Such as | `"such as A, B, and C"` |
| Dash | `"— notably A, B, and C, plus 2 others"` |
| Bracketed | `"[A, B, C, and 2 more]"` |

Force a specific style with `{items|truncate:3|join:bracketed}`.

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

### Discourse Reset

Between unrelated rendering contexts, call `engine.reset()` to clear discourse state (entity registry, template history, connective budget, word frequency, list style cycle):

```rust
engine.render("code.renamed", &event1)?;
engine.render("code.modified", &event2)?;
// ...generates output linking these two events...

engine.reset();

engine.render("code.added", &event3)?;
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

Tense handling supports `Past`, `Present`, and `Future`. Passive voice uses past participles for irregular verbs ("was renamed", "was broken", "was chosen").

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
let sentence = engine.render("code.renamed", event)?;
```

Supported field types: `String`, integer types (`i8`…`i64`, `u8`…`u64`, `usize`, `isize`), `Vec<String>`, and `Option<T>` wrappers (skipped when `None`).

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
| `Variation::Fixed` | First alternative on first render; discourse system still varies subsequent renders. |
| `Variation::Seeded(n)` | Deterministic hash-based selection. Same seed + discourse state = same output. |
| `Variation::RoundRobin` | Cycles through alternatives in registration order. |
| `Variation::Random` | Non-deterministic selection (not for tests). |

All strategies are enhanced by the engine's **template anti-repeat** (immediately repeated variants are avoided) and **choosebest scoring** (candidates are scored against recent word history, the least-repetitive is selected).

## Strictness Modes

| Mode | Missing slot behavior |
|---|---|
| `Strictness::Strict` (default) | Returns `Err(NlgError::MissingSlot)` |
| `Strictness::Lenient` | Renders as `[missing: slot_name]` |
| `Strictness::Silent` | Renders as empty string |

## Adding a Language

Implement the `Language` trait from `nlg-core`:

```rust
pub trait Language: Send + Sync {
    fn pluralize(&self, word: &str, count: usize) -> String;
    fn singularize(&self, word: &str) -> String;
    fn article(&self, word: &str) -> &str;
    fn conjugate(&self, verb: &str, tense: Tense, person: Person) -> String;
    fn past_participle(&self, verb: &str) -> String;
    fn join_list(&self, items: &[&str], conjunction: Conjunction) -> String;
    fn ordinal(&self, n: usize) -> String;
    fn number_to_words(&self, n: usize) -> String;
}
```

The trait boundary is language-agnostic — the engine, discourse system, and template renderer don't hardcode English. Drop in any language implementation and the full naturalness pipeline works.

## Running the Demo

A complete end-to-end demo exercising every feature:

```bash
cargo run --example demo
```

The demo covers referring expressions, salience, document planning, discourse-aware sequential rendering, batch rendering with aggregation, templates, the builder API, the vocab module, variation strategies, strictness modes, the derive macro, and grammar edge cases.

## Design Philosophy

Deterministic, rule-based NLG — no LLM dependencies, no non-deterministic behavior by default. The goal is **natural-sounding output that is fully reproducible and testable**. Research informed by Reiter's NLG pipeline (content planning → microplanning → realisation), RosaeNLG's choosebest and referring expression systems, SimpleNLG's aggregation patterns, and Dale & Reiter's REG work.

## License

MIT OR Apache-2.0
