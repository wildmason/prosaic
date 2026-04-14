# NLG Crate Design Spec

**Date:** 2026-04-14
**Status:** Approved

## Overview

A Rust crate that transforms structured data into grammatically correct English sentences. Domain-agnostic core with pluggable vocabulary modules. Designed for deterministic, testable output with opt-in variation for batch rendering.

## Architecture

### Workspace Structure

```
nlg/
├── Cargo.toml          (workspace root)
├── nlg-core/           (the main crate — engine, templates, builder, traits)
├── nlg-grammar-en/     (English grammar: pluralization, articles, conjugation, list formatting)
├── nlg-derive/         (proc macro: #[derive(IntoContext)])
└── nlg-vocab-code/     (optional: code-analysis vocabulary module)
```

### Layer Diagram

```
┌─────────────────────────────────────┐
│         Vocabulary Modules          │  nlg-vocab-code, nlg-vocab-infra, ...
│   (domain templates & mappings)     │
├─────────────────────────────────────┤
│            Core Engine              │  nlg-core
│  (templates, builder, rendering)    │
├─────────────────────────────────────┤
│          Grammar Layer              │  nlg-grammar-en (implements Language trait)
│  (plurals, articles, conjugation)   │
└─────────────────────────────────────┘
```

1. **Grammar layer** (`nlg-grammar-en`) — Pluralization, article selection, verb conjugation, list formatting. Implements a `Language` trait defined in core. English-only for now, but the trait boundary exists for future languages.

2. **Core engine** (`nlg-core`) — Template registry, slot filling, sentence builder API, variation strategy, strictness config, rendering. This is the crate consumers depend on. It depends on a `Language` implementation but doesn't hardcode English.

3. **Vocabulary modules** (`nlg-vocab-code`, etc.) — Optional crates that register domain-specific templates, entity types, and verb mappings into the core engine.

4. **Derive macro** (`nlg-derive`) — `#[derive(IntoContext)]` for automatic struct-to-context conversion.

## Core API

### Language Trait

Defined in `nlg-core`, implemented by grammar crates:

```rust
pub enum Tense { Past, Present, Future }
pub enum Person { First, Second, Third }
pub enum Conjunction { And, Or }

pub trait Language: Send + Sync {
    fn pluralize(&self, word: &str, count: usize) -> String;
    fn singularize(&self, word: &str) -> String;
    fn article(&self, word: &str) -> &str;  // "a" or "an"
    fn conjugate(&self, verb: &str, tense: Tense, person: Person) -> String;
    fn join_list(&self, items: &[&str], conjunction: Conjunction) -> String;
    fn ordinal(&self, n: usize) -> String;
    fn number_to_words(&self, n: usize) -> String;
}
```

### Engine Configuration

```rust
let engine = Engine::new(English::new())
    .strictness(Strictness::Strict)
    .variation(Variation::Seeded(42));
```

### Template API

```rust
engine.register_template(
    "entity.renamed",
    "The {entity_type} {old_name} was renamed to {new_name} \
     which impacts {consumer_count} direct {consumer_count|pluralize:consumer} \
     [{consumers|join}]"
)?;

let sentence = engine.render("entity.renamed", &context)?;
```

### Template Syntax

| Syntax | Meaning |
|---|---|
| `{name}` | Substitute value from context |
| `{name\|pluralize:word}` | Pluralize `word` based on numeric value of `name` |
| `{name\|article}` | Prefix with "a" or "an" |
| `{name\|join}` | Join list with Oxford comma and "and" |
| `{name\|join:or}` | Join list with "or" |
| `{name\|ordinal}` | Render as ordinal ("1st") |
| `{name\|words}` | Render number as words ("forty-two") |
| `{name\|truncate:N}` | Show first N items, then "and N more" |
| `{name\|capitalize}` | Capitalize first letter |

Pipes chain left to right: `{consumers|truncate:3|join}`.

### Builder API

```rust
let sentence = Sentence::new()
    .subject(entity("class", "Foo"))
    .verb("rename", Tense::Past)
    .object("Foobar")
    .clause(
        Clause::which("impacts")
            .amount(6)
            .noun("direct consumer")
            .list(&["Baz", "Qux", "Quux", "Corge", "Grault", "Garply"])
            .truncate(3)
    )
    .render(&engine)?;
```

The builder composes the same grammar primitives that templates use.

### Context

```rust
let mut ctx = Context::new();
ctx.insert("entity_type", Value::String("class".into()));
ctx.insert("old_name", Value::String("Foo".into()));
ctx.insert("new_name", Value::String("Foobar".into()));
ctx.insert("consumer_count", Value::Number(6));
ctx.insert("consumers", Value::List(vec!["Baz".into(), "Qux".into()]));
```

Also supports `#[derive(IntoContext)]` via the `nlg-derive` crate.

### Variation

```rust
pub enum Variation {
    Fixed,              // always picks first registered template
    Seeded(u64),        // deterministic, varies across alternatives
    RoundRobin,         // cycles through alternatives in order
    Random,             // non-deterministic
}
```

Multiple templates registered for the same key are alternatives. `Variation` controls selection. `Fixed` and `Seeded` produce deterministic output for testing.

### Strictness

```rust
pub enum Strictness {
    Strict,     // missing slot -> Result::Err
    Lenient,    // missing slot -> "[missing: slot_name]"
    Silent,     // missing slot -> empty string, clause omitted
}
```

Default: `Strict`.

### Error Types

```rust
pub enum NlgError {
    MissingSlot { template: String, slot: String },
    UnknownTemplate(String),
    InvalidPipe { pipe: String, reason: String },
    GrammarError(String),
    TemplateParseError { template: String, position: usize, reason: String },
}
```

## Vocabulary Modules

A vocabulary module is a function that registers templates into an engine:

```rust
// nlg-vocab-code/src/lib.rs
pub fn register(engine: &mut Engine) -> Result<(), NlgError> {
    engine.register_template("entity.renamed",
        "The {entity_type} {old_name} was renamed to {new_name} \
         which impacts {consumer_count} direct {consumer_count|pluralize:consumer} \
         [{consumers|truncate:3|join}]")?;

    // second variant for variation
    engine.register_template("entity.renamed",
        "{old_name} ({entity_type}) -> {new_name}, \
         affecting {consumer_count} {consumer_count|pluralize:consumer}")?;

    engine.register_template("entity.deleted",
        "The {entity_type} {name} was removed, \
         impacting {consumer_count} {consumer_count|pluralize:dependent}")?;

    Ok(())
}
```

Consumers call `nlg_vocab_code::register(&mut engine)` at setup.

## Dependency Strategy

- `nlg-core`: minimal deps — `thiserror` for error types
- `nlg-grammar-en`: zero external deps — hand-rolled pluralization, article, and conjugation rules
- `nlg-derive`: `syn`, `quote`, `proc-macro2`
- `nlg-vocab-code`: depends on `nlg-core`

## Testing Strategy

- **Grammar layer**: Exhaustive unit tests for pluralization (regular, irregular, uncountable), articles (vowel sounds, acronyms, silent-h), conjugation, list formatting edge cases (0, 1, 2, 3+ items).
- **Template engine**: Slot filling, pipe chaining, pipe ordering, missing slots under each strictness mode, malformed template syntax, empty values.
- **Builder API**: Sentence construction, clause composition, truncation, empty lists, single-item lists.
- **Variation**: `Fixed` and `Seeded` produce deterministic output across runs. `RoundRobin` cycles correctly. Multiple alternatives registered and selected.
- **Integration**: End-to-end tests using `nlg-vocab-code` rendering realistic scenarios.
- **Snapshot tests**: Golden-file tests for rendered output to catch unintended phrasing regressions.

## Out of Scope for v1

- Languages other than English (trait boundary exists, no second implementation)
- Context-aware pronoun resolution
- Discourse-level planning (paragraph structure, sentence ordering)
- Async rendering
- Serde-based context deserialization
- CLI or binary — library only
