# prosaic-core

General-purpose natural language generation from structured data.

`prosaic-core` turns structured events into deterministic prose. It tracks
discourse state across renders, so consecutive sentences can use pronouns,
connectives, varied list styles, salience-aware template variants, and
document-level aggregation without depending on an LLM.

## Install

```toml
[dependencies]
prosaic-core = "1.0.1"
prosaic-grammar-en = "1.0.1"
```

Add `prosaic-derive` for compile-time context and template checks, and add a
vocabulary crate such as `prosaic-vocab-code` when you want built-in templates.

## Quick Start

```rust
use prosaic_core::{Context, Engine, Session, Strictness, Value, Variation};
use prosaic_grammar_en::English;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut engine = Engine::new(English::new())
        .strictness(Strictness::Strict)
        .variation(Variation::Fixed);

    engine.register_template(
        "entity.renamed",
        "{old_name|refer} was renamed to {new_name}",
    )?;

    let mut ctx = Context::new();
    ctx.insert("entity_type", Value::String("class".into()));
    ctx.insert("old_name", Value::String("Foo".into()));
    ctx.insert("new_name", Value::String("Foobar".into()));

    let mut session = Session::new();
    let sentence = engine.render(&mut session, "entity.renamed", &ctx)?;
    assert_eq!(sentence, "The class Foo was renamed to Foobar.");
    Ok(())
}
```

## Highlights

- Template syntax with slots, pipes, conditionals, partials, and variant tiers.
- Discourse-aware references through `{name|refer}` and `{name|possessive}`.
- Salience-aware variant selection and deterministic variation strategies.
- Document planning and batch rendering for multi-event narratives.
- English, Spanish, and German support through sibling grammar crates.
- Optional serde, parallel rendering, polish, relative-time, and REG features.
- `no_std + alloc` support with `default-features = false`.

## Cargo Features

- `std` (default): standard-library error support and system-time fallbacks.
- `time` (default): `{ts|relative}` and `{ts|since_last}` pipes.
- `polish` (default): smart quotes and sentence-length splitting.
- `reg` (default): referring-expression generation.
- `serde`: serialization for core public data types.
- `parallel`: rayon-backed document paragraph rendering.

## License

MIT OR Apache-2.0
