# Deployment Matrix

Prosaic is a pure-Rust library with no runtime dependencies, which makes it
straightforward to target multiple environments. The main variables are which
features you enable and whether the `time` feature is compatible with your
target's `SystemTime` support.

## Targets

| Target | Feature flags | Approx binary contribution | Notes |
|--------|---------------|---------------------------|-------|
| Server (Linux / macOS / Windows) | `default` | N/A (library) | Full feature set; all pipes available |
| CLI tool | `default` + `serde` | ~2 MB release | `cargo install prosaic-cli` |
| WASM (browser) | `default` minus `time` | ~150–300 KB gzipped (est.) | `time` requires `SystemTime`; disable for WASM |
| WASM with locale | `default` + `locale` minus `time` | ~200–400 KB gzipped (est.) | icu4x data adds ~50–100 KB per locale |
| Embedded / no_std | not yet supported | — | Planned for v2 (`no_std + alloc` path) |

## Disabling `time` for WASM

The `relative` pipe (`{ts|relative}`) calls `SystemTime::now()`, which is not
available in browser WASM. Disable the feature:

```toml
[dependencies]
prosaic-core = { version = "0.1", default-features = false, features = ["reg", "polish"] }
prosaic-grammar-en = "0.1"
```

Any template that uses `{slot|relative}` will return an `InvalidPipe` error
at render time when compiled without the `time` feature. Guard against this
by only registering `relative` templates in builds where the feature is enabled,
or use `#[cfg(feature = "time")]` guards in your vocab crate.

## Features reference

| Feature | What it enables | When to disable |
|---------|-----------------|-----------------|
| `reg` | Referring expression generation (`{name\|refer}`, Dale-Reiter, graph-based) | If you never use `{name\|refer}` and want a smaller binary |
| `polish` | `max_sentence_length` splitting and `smart_quotes` substitution | Embedded or size-constrained targets |
| `time` | `{slot\|relative}` pipe via `SystemTime` | WASM, `no_std` |
| `serde` | `Serialize`/`Deserialize` on `Context`, `FaithfulnessScore`, `RenderExplanation`, etc. | Pure in-process use with no serialization |
| `locale` | icu4x-backed locale-aware number formatting | When only ASCII output is needed |

All features are additive and independent.

## Server deployment

No special configuration needed. Link `prosaic-core` and your vocab crates as
normal library dependencies. The engine is `Send + Sync` — create it once at
startup and share it across threads via `Arc<Engine>`. `Session` is not
`Send`; keep one per request or per thread.

```rust
use std::sync::Arc;
use prosaic_core::{Engine, Session};

// At startup
let engine: Arc<Engine> = Arc::new(build_engine());

// Per request
let engine = Arc::clone(&engine);
let mut session = Session::new();
let prose = engine.render(&mut session, "event.key", &ctx).unwrap();
```

## WASM build

```sh
cargo build --target wasm32-unknown-unknown \
  --no-default-features \
  --features reg,polish,serde
```

The `prosaic-cli` binary is not WASM-compatible (stdin/stdout I/O). Use the
`prosaic-core` API directly from your WASM module.

## CLI binary size

The release build of `prosaic-cli` is approximately 2 MB on Linux x86-64 with
`opt-level = "z"` and `lto = true`. The dominant contributors are the grammar
tables in `prosaic-grammar-en` and serde's code generation. Stripping the
binary reduces it further:

```sh
cargo build --release
strip target/release/prosaic
```
