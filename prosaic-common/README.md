# prosaic-common

Shared type metadata for the Prosaic template engine.

This crate is the dependency-free, `no_std` layer shared by `prosaic-core` and
`prosaic-derive`. It owns the public pipe type registry used by runtime
template rendering and compile-time template validation.

## Install

```toml
[dependencies]
prosaic-common = "1.0.1"
```

Most applications should depend on `prosaic-core` instead. Use this crate
directly when you are writing tooling, macros, validators, or integrations that
need to reason about Prosaic template slot and pipe types without pulling in the
full engine.

## What It Provides

- `ValueType`: the slot-level type model shared by the engine and derive macros.
- `PipeSpec` and `PIPE_SPECS`: the canonical registry of built-in pipe input and
  output types.
- `pipe_spec`: const-friendly pipe lookup by name.
- `types_compatible`: const-friendly compatibility checks for schema validation.
- `schema_lookup`: const-friendly lookup for hand-authored schema tables.

## Example

```rust
use prosaic_common::{pipe_spec, types_compatible, ValueType};

let pipe = pipe_spec("pluralize").expect("pluralize is a built-in pipe");
assert_eq!(pipe.input, ValueType::Number);
assert_eq!(pipe.output, ValueType::String);

assert!(types_compatible(ValueType::Any, ValueType::String));
assert!(!types_compatible(ValueType::Number, ValueType::List));
```

## License

MIT OR Apache-2.0
