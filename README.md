# nlg

General-purpose natural language generation from structured data, in Rust.

Takes structured events and produces grammatically correct English sentences. Domain-agnostic core with pluggable vocabulary modules.

## Quick Start

```rust
use nlg_core::{Engine, Context, Value, Variation, Strictness};
use nlg_grammar_en::English;

let mut engine = Engine::new(English::new())
    .strictness(Strictness::Strict)
    .variation(Variation::Fixed);

engine.register_template(
    "entity.renamed",
    "The {entity_type} {old_name} was renamed to {new_name} \
     which impacts {count} direct {count|pluralize:consumer} \
     [{consumers|truncate:3|join}]",
)?;

let mut ctx = Context::new();
ctx.insert("entity_type", Value::String("class".into()));
ctx.insert("old_name", Value::String("Foo".into()));
ctx.insert("new_name", Value::String("Foobar".into()));
ctx.insert("count", Value::Number(6));
ctx.insert("consumers", Value::List(vec![
    "Baz".into(), "Qux".into(), "Quux".into(),
    "Corge".into(), "Grault".into(), "Garply".into(),
]));

let sentence = engine.render("entity.renamed", &ctx)?;
// "The class Foo was renamed to Foobar which impacts 6 direct consumers
//  [Baz, Qux, Quux, and 3 more]"
```

## Crate Structure

| Crate | Purpose |
|---|---|
| `nlg-core` | Engine, templates, builder API, `Language` trait |
| `nlg-grammar-en` | English grammar: pluralization, articles, conjugation, list formatting, ordinals, number-to-words |
| `nlg-derive` | `#[derive(IntoContext)]` for automatic struct-to-context conversion |
| `nlg-vocab-code` | Code-analysis vocabulary templates (renamed, deleted, added, modified, moved, signature changed) |

## Template Syntax

Templates use `{slot}` for substitution and `{slot|pipe}` for transforms:

| Pipe | Example | Output |
|---|---|---|
| `pluralize:word` | `{count\|pluralize:item}` | "item" or "items" based on count |
| `article` | `{thing\|article}` | "an apple" or "a banana" |
| `join` | `{items\|join}` | "a, b, and c" (Oxford comma) |
| `join:or` | `{items\|join:or}` | "a, b, or c" |
| `truncate:N` | `{items\|truncate:3}` | First 3 items + "N more" |
| `ordinal` | `{n\|ordinal}` | "1st", "2nd", "3rd" |
| `words` | `{n\|words}` | "forty-two" |
| `capitalize` | `{word\|capitalize}` | "Hello" |

Pipes chain: `{items|truncate:3|join}`.

## Builder API

For complex sentences where templates get unwieldy:

```rust
use nlg_core::{Sentence, Clause, entity, Tense};

let sentence = Sentence::new()
    .subject(entity("class", "Foo"))
    .verb("rename", Tense::Past)
    .object("Foobar")
    .clause(
        Clause::which("impacts")
            .amount(6)
            .noun("direct consumer")
            .list(&["Baz", "Qux", "Quux", "Corge", "Grault", "Garply"])
            .truncate(3),
    )
    .render(&engine)?;
```

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
let sentence = engine.render("entity.renamed", event)?;
```

## Vocabulary Modules

Pre-built domain vocabularies register templates into the engine:

```rust
use nlg_vocab_code;

let mut engine = Engine::new(English::new());
nlg_vocab_code::register(&mut engine)?;

// Now you can render: code.renamed, code.deleted, code.added,
// code.modified, code.moved, code.signature_changed
```

Each template key has multiple variants for use with `Variation::RoundRobin` or `Variation::Seeded` to avoid repetitive output in batch rendering.

## Variation Strategies

| Strategy | Behavior | Testing |
|---|---|---|
| `Variation::Fixed` | Always picks first template | Fully deterministic |
| `Variation::Seeded(n)` | Hash-based selection, same seed = same output | Deterministic |
| `Variation::RoundRobin` | Cycles through alternatives | Predictable sequence |
| `Variation::Random` | Non-deterministic selection | Not for tests |

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
    fn join_list(&self, items: &[&str], conjunction: Conjunction) -> String;
    fn ordinal(&self, n: usize) -> String;
    fn number_to_words(&self, n: usize) -> String;
}
```

## License

MIT OR Apache-2.0
