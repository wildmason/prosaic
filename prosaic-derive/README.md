# prosaic-derive

Derive macros for the Prosaic natural language generation engine.

This crate provides compile-time helpers for turning Rust data structures into
Prosaic template contexts and for validating template strings before they ship.

## Install

```toml
[dependencies]
prosaic-core = "1.0.1"
prosaic-derive = "1.0.1"
```

## What It Provides

- `#[derive(IntoContext)]`: converts supported struct fields into
  `prosaic_core::Context` values.
- `prosaic_template!`: validates template slot names and pipe names at compile
  time.
- `prosaic_template_compiled!`: emits a specialized renderer for simple
  bare-slot templates.

## Example

```rust
use prosaic_derive::{prosaic_template, IntoContext};

#[derive(IntoContext)]
struct RenameEvent {
    old_name: String,
    new_name: String,
    consumer_count: i64,
}

let template = prosaic_template! {
    template: "{old_name} was renamed to {new_name}, affecting {consumer_count|pluralize:consumer}",
    slots: [old_name, new_name, consumer_count],
    context: RenameEvent,
};

assert!(template.contains("{consumer_count|pluralize:consumer}"));
```

Supported derive field types include `String`, `&str`, integer types,
`Vec<String>`, and `Option<T>` for those same value shapes. Unsupported fields
produce compile errors rather than silently disappearing from the context.

## License

MIT OR Apache-2.0
