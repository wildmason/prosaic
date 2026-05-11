# Deployment Matrix

Prosaic is a pure-Rust library with no required runtime dependencies. The
deployment story is a product of four axes: which **features** you enable,
which **target** you compile for, which **capabilities** you need, and what
**use case** you're building. This chapter maps them.

## Features

Every feature is independent and additive. Defaults are `std + time + polish + reg`.

| Feature | Default | What it enables | Cost of enabling | When to disable |
|---------|---------|-----------------|------------------|-----------------|
| `std` | yes | `std::error::Error` impl, `SystemTime::now()` fallback, `thiserror/std` | Requires the `std` crate | Embedded, `no_std + alloc` targets |
| `time` | yes | `{ts\|relative}` and `{ts\|since_last}` pipes | Depends on `std` | `no_std`, WASM without `SystemTime` |
| `polish` | yes | `engine.max_sentence_length`, `engine.smart_quotes` | Two small post-processors | Size-constrained builds |
| `reg` | yes | REG (Dale-Reiter + graph-based) for `{name\|refer}` | Entity registry + distractor walk | If you never use `{name\|refer}` with attributes |
| `serde` | no | `Serialize`/`Deserialize` on `Context`, `Value`, `RenderExplanation`, `FaithfulnessScore`, all config enums | +serde + codegen | Pure in-process usage, no persistence |
| `parallel` | no | `DocumentPlan::render_parallel` via rayon | +rayon dependency | Sequential-only, single-threaded, WASM |

## Targets

| Target | Feature flags | Crate | Notes |
|--------|---------------|-------|-------|
| Server (Linux / macOS / Windows) | `default` (+ `serde` if persisting) | `prosaic-core` | Engine is `Send + Sync`; share via `Arc<Engine>` |
| Multi-threaded batch rendering | `default` + `parallel` | `prosaic-core` | Use `DocumentPlan::render_parallel` for independent paragraphs |
| CLI tool | `default` + `serde` | `prosaic` | `cargo install prosaic` |
| Browser WASM | no default | `prosaic-wasm` | `cdylib + rlib`; wraps Engine/Session for JS |
| Node.js / serverless WASM | no default | `prosaic-wasm` | Same crate — runtime chooses how to import |
| Embedded / `no_std + alloc` | `default-features = false` (optionally `+ polish + reg`) | `prosaic-core` | `Variation::Random` degrades to `Variation::Fixed`; `{ts\|relative}` requires explicit `engine.reference_time()` |
| Tracing observability pipeline | `default` | `prosaic-tracing` | `ProsaicLayer` converts structured events into prose |

## Capabilities × feature matrix

| Capability | Required features | Notes |
|------------|-------------------|-------|
| Basic slot rendering | `default-features = false` + language crate | Minimal surface; no REG, no polish, no time |
| Compile-time template validation | `prosaic-derive::prosaic_template!` | No runtime features needed; pure proc macro |
| Compile-time monomorphized render | `prosaic-derive::prosaic_template_compiled!` | Bare slots only (no pipes) |
| Discourse-aware pronouns | `reg` | Full-form references use attributes when REG is on |
| Referring expressions with relations | `reg` + `Engine::reg_algorithm(RegAlgorithm::GraphBased)` | Krahmer 2003 graph-based REG |
| RST-labeled discourse markers | `default` | `DocumentPlan::from_events_with_relations` |
| Temporal anchoring | `time` | `{ts\|since_last}` pipe + `session.last_temporal_anchor` |
| Full Centering Theory transitions | `default` | `RenderExplanation.centering_transition` exposes Continue / Retain / SmoothShift / RoughShift |
| Forward conjunction reduction | `default` | Automatic via `render_batch`; no opt-in required |
| Gapping ellipsis | `default` | Automatic in `render_batch` for same-key + different-object runs |
| Faithfulness scoring (PARENT) | `default` | `score_faithfulness` + `assert_faithful!` macro |
| Parallel paragraph rendering | `parallel` | `DocumentPlan::render_parallel`; loses cross-paragraph temporal threading |
| Multi-lingual output | language crate (`prosaic-grammar-es`, `prosaic-grammar-de`) | Switch via `Engine::new(Spanish::new())` etc. |

## Example use cases

| Use case | Recommended targets/features | Pattern |
|----------|------------------------------|---------|
| CI changelog generator | Server, `default` + `serde`, `prosaic` | `prosaic --preset changelog` |
| Release-note bot | Server, `default` + `prosaic-vocab-release` | `Engine::new(English::new())` + `prosaic_vocab_release::register` |
| Incident narrative pages | Server, `default` + `prosaic-tracing` | Wire `ProsaicLayer` into `tracing_subscriber`; feed events through vocab templates |
| Browser live-preview | `prosaic-wasm`, no default features | `new ProsaicEngine()` + `engine.render(session, key, ctx)` from JS |
| Edge function (Cloudflare Workers) | `no_std + alloc` compile, `wasm32-wasi` or similar | Pass `engine.reference_time()` from request metadata |
| Spanish-language release notes | `prosaic-grammar-es` + `prosaic-vocab-release` `register_es` | `Engine::new(Spanish::new())` then `prosaic_vocab_release::register_es(&mut engine)?` |
| Large-batch report rendering | `default` + `parallel` | `DocumentPlan::render_parallel(&engine, &initial_session)` |
| Observability "narrative view" | `default` + `prosaic-tracing` | Tracing spans → vocab events → prose |

## Sample configurations

### Minimal server build

```toml
[dependencies]
prosaic-core = "0.6.1"
prosaic-grammar-en = "0.6.1"
```

### Size-optimized embedded build

```toml
[dependencies]
prosaic-core = { version = "0.6.1", default-features = false, features = ["reg"] }
prosaic-grammar-en = "0.6.1"
```

`std`, `time`, `polish` disabled. `reg` kept for `{name|refer}`. No
`SystemTime`; supply `engine.reference_time()` from your host.

### Browser WASM build

```toml
[dependencies]
prosaic-wasm = "0.6.1"
```

Build with `wasm-pack build` or `cargo build --target wasm32-unknown-unknown
-p prosaic-wasm`. The `cdylib` artefact is the `.wasm` binary; `rlib`
supports Rust-side unit tests.

### Multi-lingual web API

```toml
[dependencies]
prosaic-core = { version = "0.6.1", features = ["serde"] }
prosaic-grammar-en = "0.6.1"
prosaic-grammar-es = "0.6.1"
prosaic-grammar-de = "0.6.1"
prosaic-vocab-release = "0.6.1"
```

### Parallel batch pipeline

```toml
[dependencies]
prosaic-core = { version = "0.6.1", features = ["parallel"] }
prosaic-grammar-en = "0.6.1"
```

## Engine threading model

- `Engine: Send + Sync` — construct once, share via `Arc<Engine>`.
- `Session: Send + Sync` — one per logical narrative. Cheap to clone when
  paragraph-level parallelism is wanted (see `render_parallel`).
- `DiscourseState` lives inside `Session` — never shared across threads.

```rust
use std::sync::Arc;
use prosaic_core::{Engine, Session};

let engine: Arc<Engine> = Arc::new(build_engine());

// Per request/thread:
let engine = Arc::clone(&engine);
let mut session = Session::new();
let prose = engine.render(&mut session, "event.key", &ctx)?;
```

## Building the WASM artefact

```sh
# With prosaic-wasm (recommended)
wasm-pack build prosaic-wasm --target web

# Or directly
cargo build --target wasm32-unknown-unknown -p prosaic-wasm --release
```

## CLI binary size

Release build of the `prosaic` CLI is approximately 2 MB on Linux x86-64 with
`opt-level = "z"` and `lto = true`. Dominant contributors are the grammar
tables in `prosaic-grammar-en` and serde codegen. Stripping reduces further:

```sh
cargo build --release -p prosaic
strip target/release/prosaic
```
