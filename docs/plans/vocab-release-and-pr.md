# Plan: `nlg-vocab-release` + `nlg-vocab-pr` Vocabulary Crates

**Owner:** sonnet agent
**Scope:** two new member crates + workspace wiring
**Estimated size:** ~400–600 LOC total including tests (≈250 per crate)
**Test gate:** all 499 existing tests continue to pass; new tests add ~20–30; zero warnings
**Branch discipline:** local only, commit at each sub-phase

---

## Why

Vocabulary crates make the library immediately useful and validate that the framework handles real domains. Existing `nlg-vocab-code` (code-change events) and `nlg-vocab-git` (commits/PRs/issues/releases at the event level) prove the pattern; the next two crates extend into adjacent use cases with direct adoption hooks:

- **`nlg-vocab-release`** — release-note / changelog narrative. Event vocabulary for the things a release is made of: breaking changes, feature adds, bug fixes, security fixes, deprecations, dependency updates. Directly feeds the planned `nlg --preset=release-notes` CLI flow. Competes against git-cliff in the "readable release notes, not commit bullets" niche.
- **`nlg-vocab-pr`** — higher-level PR narrative vocabulary (summary, review state, scope, merge readiness) that complements `nlg-vocab-git`'s raw event layer. Feeds the `nlg --preset=digest` PR-summary flow.

Both crates mirror the shape of `nlg-vocab-code` / `nlg-vocab-git` — no new framework concepts.

## Design (locked)

### Both crates: `register(engine: &mut Engine) -> Result<(), NlgError>`

Standard pattern. Each event type gets 2–3 template variants registered across Low / Medium / High salience. Templates use English-only syntax, standard pipe set (`refer`, `pluralize`, `join`, `truncate`, `quantify`, `verb`, `hedge`, etc.), conditional sections for optional slots.

No new engine features are required. No graph-based REG usage (attribute-only domain). No `#[nlg_template]` compile-time validation in v1 (vocab templates are static strings validated via `register_template` at runtime) — a future follow-up can add `#[nlg_template]` checking as crate-level conformance when that infra grows.

### `nlg-vocab-release` — event catalogue

| Event key | Slots (required unless marked) | Salience spread |
|---|---|---|
| `release.tagged` | `version`, `title` (opt), `commit_sha` (opt), `date` (opt) | Low / Medium / High |
| `release.feature_added` | `name`, `description` (opt) | Low / Medium / High |
| `release.breaking_change` | `name`, `migration_path` (opt) | High only (all breaking changes are high-salience) |
| `release.bugfix` | `description`, `issue_number` (opt) | Low / Medium |
| `release.security_fix` | `description`, `cve` (opt), `severity` (opt) | High only |
| `release.deprecation` | `name`, `replacement` (opt), `removal_version` (opt) | Medium / High |
| `release.dependency_update` | `name`, `from_version`, `to_version`, `reason` (opt) | Low / Medium |
| `release.contributor_summary` | `count`, `top_contributors` (list, opt) | Low / Medium / High |
| `release.stats` | `commits`, `files_changed`, `insertions`, `deletions` | Low / Medium |
| `release.summary` | `version`, `headline` | Medium / High |

Example templates (one per salience tier for `release.breaking_change`):

```
// Only High registered — breaking changes are always high salience:
"{name} introduces a breaking change{?migration_path}; migration: {migration_path}{/?}"
```

For `release.feature_added`:

```
// Low:
"Added {name}"
// Medium:
"New feature: {name}{?description} — {description}{/?}"
// High:
"This release introduces {name}{?description}. {description}{/?} Existing users may want to try it."
```

Use existing grammar-driven pipes where they buy naturalness: `{count|pluralize:bugfix}`, `{top_contributors|truncate:3|join}`, `{commits|quantify}`.

### `nlg-vocab-pr` — event catalogue

| Event key | Slots | Salience spread |
|---|---|---|
| `pr.summary` | `number`, `title`, `author`, `commit_count`, `files_changed` | Low / Medium / High |
| `pr.review_state` | `number`, `approvals`, `requested_changes`, `pending` | Low / Medium |
| `pr.scope` | `number`, `areas` (list) | Low / Medium |
| `pr.diff_stats` | `number`, `insertions`, `deletions` | Low |
| `pr.age` | `number`, `days_open`, `stale` (0/1) | Low / Medium |
| `pr.merge_readiness` | `number`, `ready` (0/1), `blockers` (list, opt) | Medium / High |
| `pr.ci_status` | `number`, `passing`, `failing` (opt) | Low / Medium |
| `pr.related_prs` | `number`, `depends_on` (list, opt), `blocks` (list, opt) | Medium |

Example: `pr.merge_readiness` — High variant:

```
"PR #{number} is {ready|choose: 1=ready to merge, default=not yet ready}{?blockers} \
— blocked by {blockers|join}{/?}"
```

Uses `|choose` (just landed), demonstrating the new pipe in practice.

### Workspace wiring

- Add `nlg-vocab-release` and `nlg-vocab-pr` to `Cargo.toml`'s `[workspace.members]`.
- Each crate's own `Cargo.toml` declares `nlg-core = { path = "../nlg-core" }` as a non-dev dep and `nlg-grammar-en = { path = "../nlg-grammar-en" }` as dev-dep for tests.

### Out of scope for v1

- **Do not** add a CLI integration here. The `--preset` flag is a separate plan.
- **Do not** add `nlg-tracing` bridge hooks. Separate plan.
- **Do not** use graph-based REG; these vocabs are attribute-only.
- **Do not** emit HTML, Markdown, or any specific output format. The crates produce plain prose; formatting is a downstream concern.
- **Do not** pre-register `EntityDescriptor`s in `register()`. The vocab crates only register templates; users register their own entities.
- **Do not** duplicate events that already live in `nlg-vocab-git` (e.g., `pr_opened`, `pr_merged`). `nlg-vocab-pr` is for higher-level narrative; raw event coverage is already in `-git`.

## Success criteria

1. Both crates compile and test-pass clean.
2. Each crate exports `pub fn register(&mut Engine) -> Result<(), NlgError>`.
3. Each crate has ≥1 test per event type confirming reasonable output (name appears, number appears, list renders, conditional section fires/skips correctly).
4. Workspace Cargo.toml includes both new members.
5. `cargo test --all-features`, `--no-default-features`, `--features serde` all clean.
6. `cargo clippy --all-features -- -D warnings` clean.
7. `cargo doc --all-features --no-deps` introduces no new warnings.
8. No regression in existing 499 tests.

---

## Phase 0 — Baseline

```bash
cargo test --all-features
cargo clippy --all-features -- -D warnings
```

Expected: **499 tests passing**, 0 warnings.

**No commit.**

---

## Phase 1 — Scaffold `nlg-vocab-release`

### 1.1 Create crate files

Directory structure:
```
nlg-vocab-release/
├── Cargo.toml
└── src/
    └── lib.rs
```

`nlg-vocab-release/Cargo.toml`:

```toml
[package]
name = "nlg-vocab-release"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "Release-note vocabulary templates for the nlg engine"

[dependencies]
nlg-core = { path = "../nlg-core", default-features = false }

[dev-dependencies]
nlg-grammar-en = { path = "../nlg-grammar-en" }
```

`default-features = false` on nlg-core to minimize the dep footprint. If specific templates need `reg`, `polish`, or `time` features, gate them conditionally later; v1 templates should be feature-independent.

### 1.2 Register function

`nlg-vocab-release/src/lib.rs` — mirror `nlg-vocab-git/src/lib.rs` structure:

```rust
//! Release-note vocabulary for the `nlg` engine.
//!
//! Registers event templates covering the lifecycle of a software
//! release: version tagging, feature additions, breaking changes, bug
//! fixes, security fixes, deprecations, dependency updates, contributor
//! summaries, stats, and overall release summary.
//!
//! Every event type is registered at two or three salience levels so
//! the engine's importance-aware verbosity picks appropriate phrasing
//! automatically.
//!
//! # Context keys per event
//!
//! - `release.tagged`             — version, title (opt), commit_sha (opt), date (opt)
//! - `release.feature_added`      — name, description (opt)
//! - `release.breaking_change`    — name, migration_path (opt)
//! - `release.bugfix`             — description, issue_number (opt)
//! - `release.security_fix`       — description, cve (opt), severity (opt)
//! - `release.deprecation`        — name, replacement (opt), removal_version (opt)
//! - `release.dependency_update`  — name, from_version, to_version, reason (opt)
//! - `release.contributor_summary` — count, top_contributors (list, opt)
//! - `release.stats`              — commits, files_changed, insertions, deletions
//! - `release.summary`            — version, headline

use nlg_core::{Engine, NlgError, Salience};

pub fn register(engine: &mut Engine) -> Result<(), NlgError> {
    register_tagged(engine)?;
    register_feature_added(engine)?;
    register_breaking_change(engine)?;
    register_bugfix(engine)?;
    register_security_fix(engine)?;
    register_deprecation(engine)?;
    register_dependency_update(engine)?;
    register_contributor_summary(engine)?;
    register_stats(engine)?;
    register_summary(engine)?;
    Ok(())
}

// One `register_*` function per event — follow the nlg-vocab-git style.
```

Implement each `register_*` with reasonable English templates. Look at `nlg-vocab-git/src/lib.rs` for the exact pattern of `engine.register_template(...)` and `engine.register_template_at(..., Salience::High)`.

### 1.3 Workspace Cargo.toml

Add `"nlg-vocab-release"` to the `[workspace.members]` array in the root `Cargo.toml`.

### 1.4 Tests

At the bottom of `lib.rs`, a `#[cfg(test)]` module. One test per event type. Pattern (mirror `nlg-vocab-git`):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use nlg_core::{Context, Session, Strictness, Value, Variation};
    use nlg_grammar_en::English;

    fn engine() -> Engine {
        let mut e = Engine::new(English::new())
            .strictness(Strictness::Strict)
            .variation(Variation::Fixed);
        register(&mut e).unwrap();
        e
    }

    #[test]
    fn tagged_event() {
        let engine = engine();
        let mut ctx = Context::new();
        ctx.insert("version", Value::String("1.2.0".into()));
        ctx.insert("title", Value::String("Retry pipeline".into()));
        let mut session = Session::new();
        let out = engine.render(&mut session, "release.tagged", &ctx).unwrap();
        assert!(out.contains("1.2.0"), "got: {out}");
    }

    // ... one test per event type
}
```

Target: ~10 tests. Every template variant doesn't need a dedicated test — pick one salience level per event, verify the canonical case plus one with-optional-slot case where the event has optional slots.

### 1.5 Verify

```bash
cargo test -p nlg-vocab-release --all-features
cargo test --all-features
cargo clippy --all-features -- -D warnings
```

**Commit:** `Add nlg-vocab-release crate with release-note templates`

---

## Phase 2 — Scaffold `nlg-vocab-pr`

Mirror Phase 1, adjusted for PR narrative vocabulary.

### 2.1 Create crate files

`nlg-vocab-pr/Cargo.toml` (same shape as vocab-release).

`nlg-vocab-pr/src/lib.rs` — event catalogue:

- `pr.summary`
- `pr.review_state`
- `pr.scope`
- `pr.diff_stats`
- `pr.age`
- `pr.merge_readiness`
- `pr.ci_status`
- `pr.related_prs`

Use `{slot|choose: key=value,default=value}` where appropriate (e.g., `stale`, `ready` booleans) to demonstrate the new pipe.

Example `pr.age`:

```rust
engine.register_template(
    "pr.age",
    "PR #{number} has been open for {days_open} \
     {days_open|pluralize:day}{?stale}{stale|choose: 1= and is stale, default=}{/?}",
)?;
```

Example `pr.merge_readiness` (Medium):

```rust
engine.register_template(
    "pr.merge_readiness",
    "PR #{number} is {ready|choose: 1=ready to merge, default=not yet ready}{?blockers}, \
     blocked by {blockers|truncate:3|join}{/?}",
)?;
```

Example `pr.review_state` (Medium):

```rust
engine.register_template(
    "pr.review_state",
    "PR #{number} has {approvals} {approvals|pluralize:approval}{?requested_changes}, \
     {requested_changes} {requested_changes|pluralize:request} for changes{/?}{?pending}, \
     and {pending} pending {pending|pluralize:review}{/?}",
)?;
```

### 2.2 Workspace wiring

Add `"nlg-vocab-pr"` to `[workspace.members]`.

### 2.3 Tests

~10 tests, one per event type, same pattern as Phase 1.

### 2.4 Verify

```bash
cargo test -p nlg-vocab-pr --all-features
cargo test --all-features
cargo test --no-default-features
cargo test --features serde
cargo clippy --all-features -- -D warnings
```

**Commit:** `Add nlg-vocab-pr crate with PR-narrative templates`

---

## Phase 3 — Final verification

```bash
cargo test --all-features
cargo test --no-default-features
cargo test --features serde
cargo clippy --all-features -- -D warnings
cargo doc --all-features --no-deps
cargo bench --bench engine -- --test
```

Test count: **~519–529 total** (499 + ~10 per crate).

**Report:** 2 commit hashes, final test counts per feature variant, any template-authoring surprises (e.g., a `|choose` usage that didn't render as expected, pluralization oddity from nlg-grammar-en, salience tier that wasn't useful for a given event).

---

## Risk register

| Risk | Mitigation |
|---|---|
| Template source contains a subtle grammar issue that only surfaces at render time (bad pipe name, missing slot) | `register_template` validates at registration time and returns `Err`. Test suite must exercise every event. |
| `nlg-grammar-en` pluralization produces awkward forms for PR-specific vocabulary (e.g., "PRs") | Spot-check in tests. If "PRs" pluralizes badly, use a different slot structure. |
| `|choose` pipe with `default=` (empty value) renders oddly | Test specifically. The empty-default case is meant to emit nothing; confirm trailing whitespace isn't an issue. |
| Workspace Cargo.toml ordering — adding two new members shouldn't affect the existing build | Insert in alphabetical order where possible; the file isn't picky. |
| A vocab template uses a slot that the engine's `|refer` pipe would treat as an entity (if context has `entity_type` set), producing unwanted Full/Short/Pronoun variation | These vocabs don't use `|refer` in v1. Keep templates plain-slot based. |
| New crates inherit the pre-existing `cargo doc` warnings from `nlg-core` | The warnings are in `nlg-core` rustdoc; new crates don't reference them. They'll show in a full workspace doc build but are not introduced by this work. |
| The `pr.review_state` template has chained conditional sections — does the parser handle nested `{?...}{/?}` inside a single template? | Yes — `Template::parse` supports them. Confirm with an existing test pattern (integration tests already exercise nested conditionals). |
| `assert!` in tests with loose matching (e.g., `out.contains("...")`) may pass for bugs | Accept — vocabulary crates are prose-level, byte-exact matching is brittle. Confirm the *meaningful* content appears; rely on downstream PARENT for faithfulness enforcement in a later plan. |

## What NOT to do

- **Do not** integrate with CLI (`--preset`). Separate plan.
- **Do not** bridge `tracing`. Separate plan.
- **Do not** emit Markdown or any output format. Prose only.
- **Do not** use `EntityDescriptor` / REG in these crates.
- **Do not** pre-populate synonym registries from these crates.
- **Do not** skip the `--no-default-features` and `--features serde` test runs — the crates must be feature-agnostic.
- **Do not** amend commits.

## Definition of done

- [ ] Phase 0 baseline clean
- [ ] 2 commits with specified subject lines
- [ ] `nlg-vocab-release` crate exists with 10 event templates and ~10 tests
- [ ] `nlg-vocab-pr` crate exists with 8 event templates and ~10 tests
- [ ] Workspace `Cargo.toml` includes both new members
- [ ] `register(engine)` function exported from each crate
- [ ] All existing 499 tests still pass; total goes up by ~20
- [ ] `cargo clippy --all-features -- -D warnings` clean
- [ ] `Engine: Send + Sync` assert still compiles
- [ ] No public API changes to `nlg-core`
