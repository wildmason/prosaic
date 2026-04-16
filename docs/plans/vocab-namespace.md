# Plan: Vocab Crate `templates::en` Namespace Layout

**Owner:** sonnet agent
**Scope:** all 4 vocab crates (`prosaic-vocab-code`, `-git`, `-release`, `-pr`)
**Estimated size:** ~200 LOC moved (not new), ~50 LOC new wiring
**Test gate:** all 534 tests pass; zero warnings
**Branch discipline:** local only, 1 commit

---

## Why

The swarm roadmap requires all vocab crates to use a `pub mod en` subtree so future multilingual templates (`pub mod es`, `pub mod de`) drop in without restructuring. Today all template registration lives at the crate root. This plan moves it behind `en::` with a thin public `register()` that delegates.

## Pattern (same for all 4 crates)

### Before

```
prosaic-vocab-code/src/
└── lib.rs        (pub fn register + all register_* helpers + tests)
```

### After

```
prosaic-vocab-code/src/
├── lib.rs        (pub mod en; pub fn register → en::register; tests stay here)
└── en.rs         (pub fn register + all register_* helpers, moved from lib.rs)
```

### Changes per crate

1. Create `src/en.rs` — move all `fn register_*` helpers and the `pub fn register` body into it. Make `register` and its helpers `pub(crate)` or `pub` as needed.
2. Update `lib.rs`:
   - Add `pub mod en;`
   - Replace `register` body with delegation: `pub fn register(engine: &mut Engine) -> Result<(), ProsaicError> { en::register(engine) }`
   - Keep `#[cfg(test)] mod tests` in `lib.rs` — tests call the public `register()` which delegates to `en::register()`, so they exercise the full path without moving.
3. No external API change. Callers still write `prosaic_vocab_code::register(&mut engine)`.

### Future pattern (documented, not implemented)

```rust
// Future v1.5: callers can opt into a specific locale
pub fn register_locale(engine: &mut Engine, locale: &str) -> Result<(), ProsaicError> {
    match locale {
        "en" => en::register(engine),
        // "es" => es::register(engine),  // added by prosaic-grammar-es
        _ => Err(ProsaicError::UnknownTemplate(format!("unsupported locale: {locale}"))),
    }
}
```

Do NOT implement `register_locale` yet. Just add a doc comment on the `en` module noting the convention.

## Execution

Apply the same mechanical transformation to all 4 crates:
1. `prosaic-vocab-code`
2. `prosaic-vocab-git`
3. `prosaic-vocab-release`
4. `prosaic-vocab-pr`

## Verify

```bash
cargo test --all-features
cargo test --no-default-features
cargo clippy --all-features -- -D warnings
```

534 tests, 0 warnings.

**Commit:** `Move vocab template registration behind pub mod en namespace`

## What NOT to do

- Do not implement `register_locale`. Future work.
- Do not move tests into `en.rs`. Keep them in `lib.rs`.
- Do not change template keys (e.g., `"code.renamed"` stays `"code.renamed"`, not `"en.code.renamed"`).
- Do not change the public API signature of `register()`.
- Do not add new tests (the existing ones already exercise the full path).
