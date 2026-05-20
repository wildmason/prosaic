# prosaic-grammar-de

German grammar layer for the Prosaic NLG engine.

This crate implements `prosaic_core::Language` for German, including the
agreement hooks Prosaic uses for articles, references, proportions, and
discourse markers.

## Install

```toml
[dependencies]
prosaic-core = "1.0.1"
prosaic-grammar-de = "1.0.1"
```

## What It Provides

- German article selection across nominative, accusative, dative, and genitive.
- Regular and common-irregular noun pluralization.
- Regular weak verb conjugation plus common strong irregulars.
- Past and present participles.
- German number words, list joining, pronouns, demonstratives, and RST markers.

## Example

```rust
use prosaic_core::{Language, Person, Tense};
use prosaic_grammar_de::German;

let de = German::new();

assert_eq!(de.article("Tisch"), "der");
assert_eq!(de.pluralize("Kind", 2), "Kinder");
assert_eq!(de.conjugate("machen", Tense::Past, Person::Third), "machte");
```

The 1.x scope deliberately does not include attributive adjective declension,
Konjunktiv, perfect compound tenses, or exhaustive strong-verb tables.

## License

MIT OR Apache-2.0
