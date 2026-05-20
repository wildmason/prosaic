# prosaic-grammar-en

English grammar implementation for the Prosaic NLG engine.

This crate implements `prosaic_core::Language` for English. Pair it with
`prosaic-core` to render English prose from Prosaic templates.

## Install

```toml
[dependencies]
prosaic-core = "1.0.1"
prosaic-grammar-en = "1.0.1"
```

## What It Provides

- English pluralization and singularization.
- Indefinite article selection (`a` vs. `an`) with common edge cases.
- Verb conjugation, past participles, and present participles.
- Natural list joining with `and` / `or`.
- Ordinals and integer-to-word rendering.

## Example

```rust
use prosaic_core::{Language, Person, Tense};
use prosaic_grammar_en::English;

let en = English::new();

assert_eq!(en.article("hour"), "an");
assert_eq!(en.pluralize("consumer", 2), "consumers");
assert_eq!(en.conjugate("rename", Tense::Past, Person::Third), "renamed");
```

## License

MIT OR Apache-2.0
