# Plan: Engine/Session Split Refactor

**Owner:** sonnet agent
**Scope:** `nlg-core` and all downstream callers in the workspace
**Estimated size:** ~600–900 LOC touched, mostly mechanical
**Test gate:** all 375 tests must pass after every phase; no warnings introduced
**Branch discipline:** local only, no PRs, commit at each phase checkpoint

---

## Why

Today `Engine` owns `discourse: RefCell<DiscourseState>` and `round_robin_counters: HashMap<String, AtomicUsize>` as interior-mutable state. Every public render method takes `&self` and mutates hidden state. That design blocks three later improvements:

1. **rayon parallel-paragraphs in `DocumentPlan::render`** — RefCell is `!Sync`, and even if we replaced it with `Mutex` the sequential-dependency between paragraphs forces cloning the engine per task.
2. **Compile-time template codegen (`#[nlg_template_compiled]`)** — a generated render fn can't borrow into engine-owned mutable state cleanly; the caller needs to pass session state explicitly.
3. **PARENT faithfulness scoring as an opt-in gate** — needs to run candidate renders without committing them, which is currently bolted on with explicit `snapshot → render → restore on failure` logic in `render()`, `render_tx`, `score_variants`. Explicit `Session` turns snapshot/restore into `session.clone()` / `session = snapshot` at call sites and eliminates the failure-path plumbing inside the engine.

The refactor itself is mechanical — move mutable fields out of `Engine` into a new `Session`, thread `&mut Session` through every render path, update all callers. No algorithmic changes.

## Success criteria

1. `Engine` contains no `RefCell`, no `AtomicUsize`, no runtime-mutable state. All methods on `Engine` are `&self` after construction. It is `Send + Sync`.
2. New `Session` struct owns all runtime state. Public render API takes `&mut Session` explicitly.
3. All 375 existing tests pass with zero changes to their assertions (only the call-site updates to thread a `Session`).
4. No warnings. No new `#[allow]`.
5. `cargo bench` still compiles and runs. The baseline numbers may shift slightly but should not regress beyond noise.
6. `cargo doc --all-features` produces no new warnings.

## Non-goals (explicit — do not do these in this refactor)

- **Do not** flatten the render pipeline into a single `String` buffer with `fmt::Write`. That's a separate follow-up.
- **Do not** add the u32 symbol-table interner. Separate follow-up.
- **Do not** add rayon or any `parallel` feature gate. This refactor *enables* it; it does not ship it.
- **Do not** change `DiscourseState` internals — move it wholesale into `Session`, don't redesign it.
- **Do not** introduce `RenderCtx` bundle yet — the swarm recommended bundling `AgreementFeatures` alongside, but that ships with the multilingual work. This refactor is just Engine/Session.
- **Do not** change `thiserror`, `ahash`, or any dep. Pure refactor.
- **Do not** change feature flags.

---

## Phase 0 — Baseline (before touching code)

Commands to run, in order. Stop and report to the user if any fails:

```bash
cargo check --all-features
cargo test --all-features
cargo test --no-default-features
cargo test --features serde
cargo clippy --all-features -- -D warnings
```

Record the exact test count from `cargo test --all-features`. Expected: **375 passing, 0 failing**, plus the two ignored doctests. If the baseline is not clean, stop and report — do not proceed.

**Commit nothing in this phase.** This is purely a gate.

---

## Phase 1 — Introduce `Session` alongside existing state

Goal: `Session` exists and compiles, but the engine still uses `RefCell<DiscourseState>`. No behavioural change.

### 1.1 Create `nlg-core/src/session.rs`

```rust
//! Per-render-sequence mutable state. See module docs.
//!
//! A `Session` owns the `DiscourseState` (focus stack, template history,
//! word-frequency log, list-style cycle) and any other runtime-mutable
//! counters associated with a render sequence. Callers create one per
//! logical "document" — a batch, a DocumentPlan, a page of output — and
//! pass `&mut Session` into render calls.
//!
//! A fresh session = a fresh narrative. Calling `reset()` on an existing
//! session clears state without deallocating.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::discourse::DiscourseState;

/// Mutable state for a render sequence. See module docs.
#[derive(Debug)]
pub struct Session {
    pub(crate) discourse: DiscourseState,
    /// RoundRobin counters keyed by template key. Stored as AtomicUsize
    /// so a future `&Session`-only code path (e.g. read-only scoring)
    /// can still advance counters atomically without an outer borrow.
    pub(crate) round_robin_counters: HashMap<String, AtomicUsize>,
}

impl Session {
    pub fn new() -> Self {
        Self {
            discourse: DiscourseState::new(),
            round_robin_counters: HashMap::new(),
        }
    }

    /// Clear all session state. Equivalent to replacing with `Session::new()`
    /// but preserves allocations.
    pub fn reset(&mut self) {
        self.discourse.reset();
        self.round_robin_counters.clear();
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for Session {
    /// Deep clone. The RoundRobin counters are cloned by reading each
    /// atomic with `Ordering::Relaxed` — fine because clones are used
    /// as snapshot/restore checkpoints around fallible renders and there
    /// is no concurrent writer during a clone.
    fn clone(&self) -> Self {
        let mut counters = HashMap::with_capacity(self.round_robin_counters.len());
        for (k, v) in &self.round_robin_counters {
            counters.insert(k.clone(), AtomicUsize::new(v.load(Ordering::Relaxed)));
        }
        Self {
            discourse: self.discourse.clone(),
            round_robin_counters: counters,
        }
    }
}
```

### 1.2 Wire into `lib.rs`

Add `mod session;` and `pub use session::Session;` to `nlg-core/src/lib.rs`. Keep it next to the existing `mod discourse;` line.

### 1.3 Verify

```bash
cargo check --all-features
cargo clippy --all-features -- -D warnings
```

**Commit:** `Add Session type alongside existing engine state`

No tests change. No behaviour changes. Engine still has `RefCell<DiscourseState>` and `round_robin_counters` — Session just exists in parallel for now.

---

## Phase 2 — Thread `&mut Session` through the private render path

Goal: internal methods take `&mut Session`. The public API still looks unchanged — `render()` creates a throwaway `Session` internally and calls the new internal methods. One behavioural shift: after this phase, `engine.render()` called repeatedly produces *uncorrelated* output because each call gets a fresh Session. We fix that in Phase 3; for now, **this phase is allowed to break tests temporarily**. Phase 3 restores them.

Actually — don't break tests. Instead:

### 2.1 Dual-path strategy

Keep `Engine::discourse: RefCell<DiscourseState>` in place. Add a parallel set of internal methods that take `&mut Session`:

- `render_tx(&self, key, all_alternatives, context) -> Result<String, NlgError>` — **keep as-is**, used by current public `render`.
- Add `render_tx_with(&self, session: &mut Session, key, all_alternatives, context) -> Result<String, NlgError>` — same body, but reads/writes `session.discourse` and `session.round_robin_counters` instead of `self.discourse` and `self.round_robin_counters`.

Do this for every private method that touches discourse state. Helpful pattern: rename the current method body to `_impl` taking `&mut DiscourseState`, and have both wrapper methods delegate.

Methods to duplicate/adapt (full list — grep `self.discourse` in engine.rs to confirm):

- `render_tx`
- `score_variants` (already public — leave it, but internal mutating helpers it calls need adapting)
- `render_template` and its descendants: `render_slot`, `apply_pipe`, any helper that reads discourse for `refer`/`syn`/demonstrative pipes
- `select_alternative_scored`, `pick_variant_index`
- Connective-detection inlines at lines 572–576 and 637–640
- `cleanup_artifacts` does NOT touch discourse — leave it
- `mention_entity`, `record_output_words`, `next_list_style` calls inside helpers

**Approach suggestion:** refactor by introducing a private helper type

```rust
struct RenderCtx<'a, 'b> {
    engine: &'a Engine,
    session: &'b mut Session,
}
```

and moving helpers as methods on `RenderCtx`. This avoids threading two parameters through every call.

### 2.2 Keep `Engine::render`, `render_inline`, `render_batch`, `render_explained`, `render_iter` signatures unchanged

Each of them internally does:

```rust
pub fn render(&self, key: &str, context: impl IntoContext) -> Result<String, NlgError> {
    // Temporary bridge: use the engine-owned RefCell as the session source.
    let mut session_borrowed = /* build a Session from self.discourse.borrow() clone + counters */;
    let result = self.render_with(&mut session_borrowed, key, context);
    // Write session state back into self.discourse and self.round_robin_counters.
    result
}
```

This is gnarly and temporary. The bridge logic lives for the duration of Phase 2 only. Phase 3 removes the RefCell entirely.

### 2.3 Verify

```bash
cargo test --all-features
```

Tests should still pass because behaviour is identical (the bridge writes state back in sync).

**Commit:** `Thread Session through private render path (dual-path bridge)`

---

## Phase 3 — Promote `Session` to the public API

Goal: remove the RefCell entirely. `Engine` becomes immutable-after-construction. Public methods take `&mut Session`.

### 3.1 Update `Engine`

- Remove `use std::cell::RefCell;`
- Remove `discourse: RefCell<DiscourseState>` field
- Remove `round_robin_counters: HashMap<String, AtomicUsize>` field
- Remove `Engine::reset(&self)` method (users call `session.reset()` instead)
- Everything else in `Engine` stays the same

### 3.2 Update public render API signatures

```rust
impl Engine {
    pub fn render(
        &self,
        session: &mut Session,
        key: &str,
        context: impl IntoContext,
    ) -> Result<String, NlgError> { ... }

    pub fn render_inline(
        &self,
        session: &mut Session,
        template: &str,
        context: impl IntoContext,
    ) -> Result<String, NlgError> { ... }

    pub fn render_batch(
        &self,
        session: &mut Session,
        events: &[(&str, Context)],
    ) -> Result<String, NlgError> { ... }

    pub fn render_explained(
        &self,
        session: &mut Session,
        key: &str,
        context: impl IntoContext,
    ) -> Result<RenderExplanation, NlgError> { ... }

    pub fn render_iter<'a>(
        &'a self,
        session: &'a mut Session,
        events: &'a [(&str, Context)],
    ) -> RenderIter<'a> { ... }

    pub fn score_variants(
        &self,
        session: &mut Session,
        key: &str,
        context: impl IntoContext,
    ) -> Result<Vec<VariantScore>, NlgError> { ... }
}
```

`session` is the **first parameter** after `&self`. Consistent placement across all methods.

### 3.3 Delete the Phase 2 bridge

Remove every `self.discourse.borrow()` / `borrow_mut()` call site — they should all be gone after Phase 2 anyway. Remove any dead helper that built the temporary Session from RefCell state.

### 3.4 Update `RenderIter`

`RenderIter` currently holds `&'a Engine`. It needs to hold `&'a Engine` + `&'a mut Session`. The iterator's `next()` implementation already mutates discourse via the engine; change it to mutate the session directly.

### 3.5 Verify

```bash
cargo check --all-features
```

This will produce many compile errors at call sites — those are fixed in Phase 4. The library crate itself must compile cleanly.

**Commit nothing yet.** Phase 3 and Phase 4 land together because the workspace won't compile between them.

---

## Phase 4 — Update all callers in the workspace

Goal: every `engine.render(...)` call site receives a `&mut Session` argument. Tests pass.

### 4.1 List of call sites to update

Grep inside the workspace for these patterns:

```
engine.render(
engine.render_inline(
engine.render_batch(
engine.render_explained(
engine.render_iter(
engine.score_variants(
engine.reset()
```

Expected files (approximate):

- `nlg-core/src/engine.rs` — internal doc examples
- `nlg-core/src/lib.rs` — crate-level doctest
- `nlg-core/src/document.rs` — `DocumentPlan::render` delegates to engine render
- `nlg-core/src/builder.rs` — `Sentence::render` delegates to engine
- `nlg-core/tests/integration.rs` — ~50 test cases
- `nlg-core/tests/properties.rs` — 8 proptest cases
- `nlg-core/tests/serde.rs` — may not call render; check
- `nlg-core/benches/engine.rs` — 5 benches
- `nlg-core/examples/demo.rs` — all demo functions
- `nlg-vocab-code/src/lib.rs` — `#[cfg(test)]` tests
- `nlg-vocab-git/src/lib.rs` — `#[cfg(test)]` tests
- `nlg-cli/src/main.rs` — CLI `run_sequential` and `run_document_plan`
- `README.md` — any inline code example (these are rustdoc-tested if marked `ignore` or `no_run`; check)

### 4.2 Mechanical translation pattern

Old:
```rust
let mut engine = Engine::new(English::new()).strictness(Strictness::Strict);
engine.register_template("t", "...").unwrap();
let result = engine.render("t", &ctx).unwrap();
```

New:
```rust
let mut engine = Engine::new(English::new()).strictness(Strictness::Strict);
engine.register_template("t", "...").unwrap();
let mut session = Session::new();
let result = engine.render(&mut session, "t", &ctx).unwrap();
```

The `mut` on `engine` stays because `register_template` still mutates template registry (that's static setup, not per-render). After all registrations complete, `engine` could be rebound with `let engine = engine;` — but don't bother, not worth the churn.

### 4.3 `engine.reset()` becomes `session.reset()`

Every `engine.reset()` in tests becomes `session.reset()`. The `engine` reference no longer needs `&mut` purely for reset.

### 4.4 `DocumentPlan::render`

`DocumentPlan::render(&self, engine: &Engine) -> Result<String, NlgError>` must change to `DocumentPlan::render(&self, engine: &Engine, session: &mut Session) -> Result<String, NlgError>`. Every caller updates.

Similarly, `Sentence::render(&self, engine: &Engine) -> Result<String, NlgError>` becomes `Sentence::render(&self, engine: &Engine, session: &mut Session)`.

### 4.5 `nlg-cli`

In `nlg-cli/src/main.rs`:

- `run_sequential` creates one Session before the loop and passes `&mut session` on each event.
- `run_document_plan` creates one Session, builds the plan, then calls `plan.render(&engine, &mut session)`.
- Document between two runs: CLI state is per-invocation, so one Session per `main()` call is correct.

### 4.6 Rustdoc examples

Update `///` doctest examples in `engine.rs` and `lib.rs` to include `Session::new()`. Doctests run as part of `cargo test` so they catch regressions.

### 4.7 Verify

```bash
cargo check --all-features
cargo check --no-default-features
cargo test --all-features
cargo test --no-default-features
cargo test --features serde
cargo clippy --all-features -- -D warnings
cargo doc --all-features
```

All must pass. Test count must remain **375 passing, 0 failing** plus the 2 ignored doctests. Any new warning = fix before committing.

**Commit:** `Split Engine into Engine + Session; promote session to public API`

This is the big commit. It's fine that it's large — the diff is mostly mechanical `&mut session` additions at call sites.

---

## Phase 5 — Cleanup and API sugar

Goal: nice-to-have ergonomic touches now that the core refactor is done. Keep this phase small.

### 5.1 `Engine::new_session()` helper

```rust
impl Engine {
    /// Create a new Session compatible with this engine. Sugar for `Session::new()`;
    /// equivalent, but clarifies intent at call sites.
    pub fn new_session(&self) -> Session {
        Session::new()
    }
}
```

Use at call sites where clarity matters. Not required everywhere; `Session::new()` is fine.

### 5.2 `Engine: Send + Sync` static assert

Add to the bottom of `engine.rs`:

```rust
#[cfg(test)]
mod engine_thread_safety {
    use super::Engine;

    // Compile-time assert: Engine is Send + Sync post-refactor.
    // If this ever breaks, the refactor was undone.
    const _: fn() = || {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Engine>();
    };
}
```

This is a test-only compile-time check. Non-invasive.

### 5.3 Update `README.md` "Quick start"

One call-site update in the Quick start example: add `let mut session = Session::new();` and `&mut session` to the render call. Keep it minimal — don't rewrite the README.

### 5.4 Verify

Same battery as Phase 4.7.

**Commit:** `Add Engine::new_session sugar and Send+Sync invariant check`

---

## Phase 6 — Final verification

Run the full battery one last time:

```bash
cargo check --all-features
cargo check --no-default-features
cargo test --all-features
cargo test --no-default-features
cargo test --features serde
cargo clippy --all-features -- -D warnings
cargo doc --all-features --no-deps
cargo bench --bench engine -- --test
```

The last line runs the benches in test mode (no timing) to confirm they still compile and execute. If you have time/appetite, run `cargo bench --bench engine` for real and save the output to `docs/plans/engine-session-split-baseline.md` for later before/after comparison — but this is optional, not a gate.

**Report to user:** commit hashes for each phase (should be 4 commits: Phase 1, Phase 2, Phase 3+4 combined, Phase 5), test count, any behavioural surprises.

---

## Risk register

| Risk | Mitigation |
|---|---|
| Hidden call site uses `Engine` through `Arc` or across threads, which would have been illegal before and is now possible — might reveal a latent bug | Add the Send+Sync compile-time assert (5.2); run full test suite twice in Phase 6 |
| `RenderIter` lifetime churn — holding `&'a mut Session` alongside `&'a Engine` | If the natural lifetime annotation is awkward, split into two separate lifetime parameters `<'e, 's>`. Don't over-engineer; if tests pass, signature is fine. |
| Doctest examples that were silently passing because of shared engine state between examples | Each doctest is independent (Rust runs them as separate binaries), so no cross-example leakage is possible. Any failure here reveals a real bug in the example. |
| Benchmark numbers regress because of `Session::clone()` in `render` snapshot/restore | Expected: very mild regression (counter HashMap clone is not free) OR slight win (no RefCell borrow check overhead). Record actual numbers but do not treat small deltas as blocking. |
| User has uncommitted changes from another session | Verify `git status` is clean before Phase 1. If not, ask the user. |

## What NOT to do if something breaks

- **Don't** add `Mutex` or `RwLock` "just to get it compiling." The whole point is no interior mutability. If you're reaching for a lock, back up and think.
- **Don't** `#[allow(dead_code)]` stale helpers. Delete them.
- **Don't** amend a prior commit. Create a fresh commit for fixes.
- **Don't** skip `cargo clippy` — warnings become errors later and are much cheaper to fix at each phase than all at the end.
- **Don't** proceed past a failing `cargo test` — stop and report.

## Definition of done

- [ ] `Engine` has no `RefCell`, no `AtomicUsize`, no interior mutability
- [ ] `Engine: Send + Sync` compile-time assert passes
- [ ] `Session` exists as a public type, documented, with `new`/`reset`/`Clone`/`Default`
- [ ] All public render methods take `&mut Session` as their first post-`&self` argument
- [ ] All 375 tests pass with `--all-features`, `--no-default-features`, and `--features serde`
- [ ] `cargo clippy --all-features -- -D warnings` is clean
- [ ] `cargo doc --all-features` produces no new warnings
- [ ] Benchmarks compile and run
- [ ] 4 commits on `master`, one per major phase
- [ ] README Quick start updated to show Session usage
