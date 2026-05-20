# prosaic-grammar-es

Spanish grammar layer for the Prosaic NLG engine.

This crate implements `prosaic_core::Language` for Spanish, including the
agreement hooks Prosaic uses for articles, references, proportions, and
discourse markers.

## Install

```toml
[dependencies]
prosaic-core = "1.0.1"
prosaic-grammar-es = "1.0.1"
```

## What It Provides

- Spanish pluralization and singularization.
- Gender-aware definite and indefinite article helpers.
- Regular verb conjugation plus common irregular handling.
- Gendered pronouns, possessives, and demonstratives for references.
- Spanish list joining, ordinals, number words, proportions, and RST markers.

## Example

```rust
use prosaic_core::{Language, Person, Tense};
use prosaic_grammar_es::Spanish;

let es = Spanish::new();

assert_eq!(es.article("clase"), "la");
assert_eq!(es.pluralize("servicio", 2), "servicios");
assert_eq!(es.conjugate("hablar", Tense::Present, Person::First), "hablo");
```

The grammar is intentionally pure Rust and ships without CLDR data. It covers
the common deterministic surface forms needed by Prosaic templates; full
dialectal variation and exhaustive irregular tables are out of scope for 1.x.

## License

MIT OR Apache-2.0
