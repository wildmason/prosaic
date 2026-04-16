# Plan: `no_std + alloc` path for `prosaic-core`

**Owner:** sonnet agent
**Scope:** `prosaic-core/{Cargo.toml, src/**/*.rs}` + minor knock-on in `prosaic-grammar-en` if any imports surface. No changes to `-es` / `-de` / vocab crates (they depend on `prosaic-core` with `default-features = false` already).
**Estimated size:** ~400–600 LOC of changes across ~12 files
**Test gate:** 965 baseline tests pass with all features; new `cargo check -p prosaic-core --no-default-features --features alloc` succeeds; ~3–5 new tests
**Branch discipline:** local only, one commit

---

## Why

`prosaic-core` today uses `std::collections::{HashMap, HashSet, VecDeque, BTreeMap}` and `std::sync::atomic` pervasively. That forces an implicit `std` dependency even when callers don't need time / threads / file I/O, blocking WASM-no-std and embedded use. The library's *logic* is entirely deterministic CPU work — pure `alloc` is sufficient. This plan adds a `no_std + alloc` compile path behind a feature flag without changing any surface API.

## Design

### Feature matrix

| Feature | Default | Gates |
|---------|---------|-------|
| `std` | yes | Enables `SystemTime::now()` fallback in `Variation::Random` and in `pipe_relative` when `engine.reference_time` is unset. Adds `impl std::error::Error for ProsaicError` via `thiserror/std`. |
| `time` | yes | Unchanged — already optional. Depends on `std`. |
| `polish` | yes | Unchanged. |
| `reg` | yes | Unchanged. |
| `serde` | no | Unchanged. |

`default = ["std", "time", "polish", "reg"]`.

Callers compiling with `default-features = false` lose the `SystemTime::now()` fallbacks:
- `Variation::Random` degrades to `Variation::Fixed` (variant index 0).
- `pipe_relative` errors if `engine.reference_time` is unset (existing error path — we already require explicit reference_time for deterministic output).

### Dependency changes

```toml
[dependencies]
ahash = { version = "0.8", default-features = false, features = ["compile-time-rng"] }
hashbrown = { version = "0.15", default-features = false, features = ["default-hasher", "inline-more"] }
thiserror = { version = "2", default-features = false }
serde = { version = "1", default-features = false, features = ["derive", "alloc"], optional = true }

[features]
default = ["std", "time", "polish", "reg"]
std = ["ahash/std", "thiserror/std", "serde?/std"]
serde = ["dep:serde", "ahash/serde"]
time = ["std"]
polish = []
reg = []
```

Rationale:
- `ahash/std` was default-on before; now gated. When `std` is off, ahash falls back to its `compile-time-rng` seed (deterministic at build time, fine for our use).
- `hashbrown` provides the `HashMap` / `HashSet` we use everywhere. With `default-hasher`, it ships with a `foldhash`/`ahash` default; we'll pin it to `ahash::RandomState` for stability.
- `thiserror` needs `std` for `std::error::Error`; in no_std mode, `ProsaicError` implements `core::fmt::Display` (automatic via Debug+format) but not `std::error::Error`. `thiserror` v2 handles this automatically via the `std` feature.
- `serde/alloc` is available in no_std; `serde/std` adds a few extras that we don't strictly need but should be on with `std`.

### Collection replacements

Replace every `use std::collections::*` with a local alias module. Create `prosaic-core/src/collections.rs`:

```rust
//! Collection-type aliases for `no_std`-compatible builds.

pub type HashMap<K, V> = hashbrown::HashMap<K, V, ahash::RandomState>;
pub type HashSet<T>    = hashbrown::HashSet<T, ahash::RandomState>;
pub use alloc::collections::{BTreeMap, BTreeSet, VecDeque};
```

In `lib.rs`, add:
```rust
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

mod collections;
```

Update every site that says `use std::collections::{HashMap, HashSet, VecDeque, BTreeMap}` to `use crate::collections::{HashMap, HashSet, VecDeque, BTreeMap}`.

Touched files (from earlier grep):
- `antonyms.rs`
- `discourse.rs`
- `document.rs`
- `engine.rs`
- `faithfulness.rs`
- `reg.rs`
- `session.rs`
- `ahash.rs` (already uses ahash::AHashMap — verify hashbrown works the same)

Anywhere that calls `.entry(K).or_insert(...)` or similar HashMap APIs: hashbrown matches std's API one-to-one. Shouldn't require code changes beyond the import.

### `std::sync::atomic` → `core::sync::atomic`

Affects `engine.rs` and `session.rs`. Direct replacement — `AtomicUsize` / `Ordering` live at the same paths in `core`.

### `SystemTime` gating

Wrap every `std::time::SystemTime::now()` call in `#[cfg(feature = "std")]` with an else-branch fallback:

```rust
// In Variation::Random picker:
Variation::Random => {
    #[cfg(feature = "std")]
    {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos() as usize;
        nanos % count
    }
    #[cfg(not(feature = "std"))]
    {
        // Random-without-clock degrades to deterministic first variant.
        0
    }
}
```

### Ownership nitpicks

Anywhere we use `Box<dyn Error>` or similar needs core-compatible alternatives. Check `ProsaicError` — it likely uses `Box<dyn std::error::Error + Send + Sync>` somewhere. Replace with `alloc::boxed::Box<dyn core::fmt::Debug + Send + Sync>` or eliminate. Verify carefully — we may need a `std`-gated field.

### `#[cfg(test)]` code

Tests always run with `std`. No changes needed — tests can freely use `std::` because they're built with the `test` profile and default features.

### Documentation

Add a top-of-`lib.rs` comment:

```rust
//! # no_std support
//!
//! Disable default features to compile under `no_std`:
//!
//! ```toml
//! prosaic-core = { version = "0.1", default-features = false }
//! ```
//!
//! Without the `std` feature:
//! - `Variation::Random` falls back to `Variation::Fixed` (variant 0).
//! - `{timestamp|relative}` and `{timestamp|since_last}` require
//!   `engine.reference_time()` to be set.
//! - `ProsaicError` does not implement `std::error::Error` (only `Debug + Display`).
```

---

## Phase 0 — Baseline

```bash
cargo test --all-features
cargo clippy --all-features -- -D warnings
```

Expected: 965 tests passing.

---

## Phase 1 — Add `hashbrown` dep + `collections` alias module

1. Update `prosaic-core/Cargo.toml` per design (new `std` feature, add `hashbrown`).
2. Add `prosaic-core/src/collections.rs`.
3. Add `#![cfg_attr(not(feature = "std"), no_std)]` and `extern crate alloc;` to `lib.rs`.
4. Declare `mod collections;`.

Verify: `cargo check --all-features` succeeds. Do NOT yet replace any usage sites.

---

## Phase 2 — Replace `std::collections` imports

Rewrite every `use std::collections::{HashMap, HashSet, VecDeque, BTreeMap}` to the crate's alias module. If any file uses them only inline (e.g. `std::collections::BTreeMap::new()` without `use`), change those too.

Verify: `cargo test --all-features` passes. Tests still use HashMap from std in their own modules — that's fine because tests are always-std.

---

## Phase 3 — `std::sync::atomic` → `core::sync::atomic`

Change both `engine.rs` and `session.rs`.

Verify: `cargo test --all-features` passes.

---

## Phase 4 — Gate `SystemTime` behind `std`

Wrap every `SystemTime::now()` call site with `#[cfg(feature = "std")]` + fallback. Sites to check:
- `Variation::Random` branches (multiple copies in `engine.rs`)
- `pipe_relative` fallback when `reference_time` is unset
- `pipe_since_last` fallback

For `pipe_relative` / `pipe_since_last`: when `reference_time` is None AND `std` is off, return a `ProsaicError::PipeError` with a message explaining `engine.reference_time()` must be set.

Verify: `cargo test --all-features` passes; `cargo check -p prosaic-core --no-default-features` succeeds.

---

## Phase 5 — `ProsaicError` and `thiserror/std` gating

1. Set `thiserror = { version = "2", default-features = false }` in `Cargo.toml`.
2. Enable `thiserror/std` through the `std` feature.
3. Check `error.rs` for any `Box<dyn std::error::Error>` fields. Replace with alloc-compatible types if present.

Verify: `cargo check -p prosaic-core --no-default-features` succeeds.

---

## Phase 6 — Smoke-test no_std build

Add a compile-time test in `prosaic-core/tests/no_std_compile.rs`:

```rust
// Smoke-test: the crate must compile cleanly under no_std + alloc.
// This test is compiled but never run; its existence as a passing
// `cargo test --no-default-features` target is the proof.
//
// Tests themselves need std for the harness, so we don't actually
// build a no_std binary here — we verify via `cargo check`. See
// the CI matrix.
```

Add this to the plan document: in Phase 7 we run a dedicated `cargo check --no-default-features` in addition to the usual matrix.

### Tests

Add ~3 sanity tests that run under all feature combinations:

```rust
#[test]
fn core_renders_without_default_features() {
    // Compiles only when default-features are off.
    #[cfg(not(any(feature = "time", feature = "polish", feature = "reg")))]
    {
        // ... minimal render verification with MiniLang ...
    }
}
```

Actually, this pattern is hard to wire. Simpler: assert that the `cargo check` command succeeds and let CI / manual verification enforce it.

---

## Phase 7 — Final verification

```bash
cargo test --all-features
cargo test --no-default-features
cargo test --features serde
cargo check -p prosaic-core --no-default-features          # no_std smoke
cargo check -p prosaic-core --no-default-features --features serde
cargo clippy --all-features -- -D warnings
cargo doc --all-features --no-deps
```

All seven must pass.

**Commit:** `Add no_std + alloc build path via std feature flag and hashbrown collections`

---

## What NOT to do

- **Do not** introduce `#![no_std]` at the workspace level or in `prosaic-grammar-en`/`-es`/`-de` or vocab crates. Only `prosaic-core`.
- **Do not** drop any existing default features. Everything that works today must still work out of the box.
- **Do not** change public API names or signatures — only internal imports and gated implementations.
- **Do not** force-cast or transmute. If a type change is needed (e.g. `hashbrown::HashMap` doesn't match `std::HashMap` in a public API), surface it as a type alias via `crate::collections`.

## Risk register

| Risk | Mitigation |
|------|------------|
| `hashbrown::HashMap` API differs subtly from std's | Hashbrown intentionally mirrors std's HashMap API one-to-one. If a call site fails, check the method's exact signature vs std docs. |
| `ahash::RandomState` needs `std` for seeding | With `compile-time-rng`, ahash provides a build-time seed. Works on no_std. |
| `thiserror` macros expand to std-only paths | `thiserror v2` with `default-features = false` is no_std-compatible; gate `std` feature for `std::error::Error`. |
| `Box<dyn Error>` in `ProsaicError` breaks no_std | Inspect; replace with concrete error or gate behind std. |
| `ProsaicError: Display` wording references std | Should be pure format_args!; verify. |
| Test breakage from changing HashMap type | Unit tests don't rely on HashMap's iteration order. Any flaky test is a pre-existing bug. |

## Definition of done

- [ ] `prosaic-core/Cargo.toml` has `std` feature in `default = [...]`, with `hashbrown` dep
- [ ] `prosaic-core/src/collections.rs` type-alias module
- [ ] `#![cfg_attr(not(feature = "std"), no_std)]` + `extern crate alloc;` in `lib.rs`
- [ ] No `use std::collections::*` in `src/*.rs` (test code is fine)
- [ ] No `use std::sync::*` in `src/*.rs`; use `core::sync::*`
- [ ] All `SystemTime::now()` calls gated behind `#[cfg(feature = "std")]` with fallback
- [ ] `cargo check -p prosaic-core --no-default-features` succeeds
- [ ] All existing tests pass under `--all-features`, `--no-default-features`, `--features serde`
- [ ] `cargo clippy --all-features -- -D warnings` clean
- [ ] One commit: `Add no_std + alloc build path via std feature flag and hashbrown collections`
