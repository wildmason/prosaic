# Plan: Perf Sweep Phase 1 — Allocation & Lookup Wins

**Owner:** sonnet agent
**Scope:** `nlg-core` only (plus Cargo.toml for ahash dep)
**Estimated size:** ~250–350 LOC touched
**Test gate:** all 376 tests pass after every sub-phase; zero new warnings
**Branch discipline:** local only, commit at each sub-phase checkpoint

---

## Why

The Engine/Session split just landed. Four small, independent perf wins that the swarm flagged are now clean to execute in sequence. All four share one property: each is a small mechanical change with a measurable allocation/lookup improvement, no algorithmic risk, and no touched public API.

Bundling them as one "perf sweep" so we capture one before/after bench delta for the whole batch instead of four noisy tiny deltas.

Order is important: the interner lands last because it's the largest and most likely to surface subtle issues. The three trivial wins come first to pay down the easy allocation debt, then the interner tackles the biggest remaining hot spot (discourse word-history `HashSet<String>` allocation per render).

## Success criteria

1. All 376 tests pass after every sub-phase.
2. `cargo clippy --all-features -- -D warnings` clean after every sub-phase.
3. `cargo bench --bench engine` runs successfully at end; no benchmark crashes.
4. Zero public API changes (no renamed methods, no new required arguments, no removed re-exports).
5. `Engine: Send + Sync` compile-time assert still passes.
6. No `RefCell`, no `Mutex`, no interior mutability reintroduced.
7. Four commits, one per sub-phase.

## Non-goals (do not do these here)

- Flatten render pipeline to a single `String` buffer + `fmt::Write`. That's the next plan.
- PARENT metric. Follows this and the buffer flatten.
- Rayon / parallel feature. Separate plan.
- `no_std` / WASM crate. Separate plan.
- Compile-time templates. Separate plan.
- Any new public APIs.
- `itoa` / `ryu` integration — only pays off after buffer flatten lands.

---

## Phase 0 — Baseline

Run from the project root:

```bash
cargo test --all-features
cargo clippy --all-features -- -D warnings
cargo bench --bench engine -- --test
```

Expected: **376 passing, 0 failing, 0 warnings**, benches compile and execute in `--test` mode. If not clean, stop and report.

Optional: run `cargo bench --bench engine` (no `--test`) once and save the stdout to `docs/plans/.perf-sweep-baseline.txt` for later before/after comparison. Not a gate; skip if it takes more than 2 minutes.

**No commit.**

---

## Phase 1 — `filter_by_salience` returns `Vec<&Template>` not clones

**File:** `nlg-core/src/engine.rs`

**Current state:** `filter_by_salience` around line 2371 does `.map(|(_, t)| t.clone()).collect::<Vec<Template>>()`. Three call sites at lines 582, 673, 946.

**Change:** make the function return `Vec<&'a Template>` borrowing from the input slice.

```rust
fn filter_by_salience<'a>(
    all: &'a [SalientTemplate],
    target: Salience,
) -> Vec<&'a Template> {
    // ... same logic, but collect &t instead of t.clone()
}
```

Update the three call sites — each one currently binds `let alternatives = filter_by_salience(...);` and passes `&alternatives` or iterates. With the new signature, `alternatives: Vec<&Template>` and call sites pass `&alternatives` (still works) and iterate `.iter().copied()` where they currently iterate `.iter()` — OR iterate `.iter()` and dereference once (whichever is cleaner locally).

If any downstream method signature takes `&[Template]` and now needs `&[&Template]`, propagate the change. Minimise the blast radius — ideally only the immediate call-site loops change.

**Verify:**

```bash
cargo test --all-features
cargo clippy --all-features -- -D warnings
```

**Commit:** `Avoid cloning templates in filter_by_salience`

---

## Phase 2 — Swap `HashMap` to `AHashMap` on `Context`

**Files:** `nlg-core/src/context.rs`, `nlg-core/Cargo.toml`

**Current state:** `Context` uses `std::collections::HashMap<String, Value>` with SipHash-1-3. Template slot lookup is a hot path.

### 2.1 Add ahash dep

Edit `nlg-core/Cargo.toml`. Add to `[dependencies]`:

```toml
ahash = { version = "0.8", default-features = false }
```

`default-features = false` drops the `runtime-rng` feature — we're keyed by template-author-supplied strings, not network input, so DoS resistance isn't needed and the extra dep is waste.

Note: ahash 0.8 is stable and `no_std`-compatible, consistent with the plan to eventually go `no_std + alloc`.

If `serde` feature needs ahash's serde integration for `Context`'s serde impl, add `features = ["serde"]` to the dep. Check the current serde round-trip test (`nlg-core/tests/serde.rs`) still passes after the swap.

### 2.2 Swap the type

In `context.rs`:

```rust
use ahash::AHashMap;

pub struct Context {
    slots: AHashMap<String, Value>,
}
```

Update `Context::new()` to use `AHashMap::new()`. `AHashMap` is a drop-in for `std::HashMap` with the same API — no method signature changes needed.

### 2.3 Check other HashMap uses in nlg-core

Grep for `HashMap` inside `nlg-core/src`. Likely candidates for swap:

- `engine.rs` — `templates: HashMap<String, Vec<SalientTemplate>>`, `partials: HashMap<String, Template>`
- `antonyms.rs` — `map: HashMap<String, String>`
- `reg.rs` — `entries: HashMap<(String, String), EntityDescriptor>`

**Scope decision:** swap ONLY `Context` in Phase 2. The others are cold paths (template lookup is keyed by template key, usually once per render; antonym/REG lookups are small). Don't scope-creep. If time permits and the swap is genuinely trivial, include them; if it adds more than 20 LOC, defer.

### 2.4 Verify

```bash
cargo test --all-features
cargo test --no-default-features
cargo test --features serde
cargo clippy --all-features -- -D warnings
```

All must pass. Serde round-trip tests especially — ahash's serde format must round-trip cleanly.

**Commit:** `Swap Context HashMap to ahash::AHashMap for faster slot lookups`

---

## Phase 3 — Invert `SynonymRegistry` to O(1) lookup

**File:** `nlg-core/src/synonyms.rs`

**Current state:** `synonyms_for(word)` is O(groups × group_size) and allocates a `String` per lookup via `to_lowercase()`. Called on every `{word|syn}` pipe invocation.

**Change:** build an inverted index at registration time.

```rust
use ahash::AHashMap;

pub struct SynonymRegistry {
    groups: Vec<Vec<String>>,
    /// Lookup index from normalised (lowercased) word → index into `groups`.
    /// Populated on every `register_group` call.
    index: AHashMap<String, usize>,
}

impl SynonymRegistry {
    pub fn register_group(&mut self, words: &[&str]) {
        if words.is_empty() {
            return;
        }
        let group_idx = self.groups.len();
        let group: Vec<String> = words.iter().map(|w| w.to_string()).collect();
        // Pre-lowercase into the index so lookup is allocation-free.
        for w in &group {
            self.index.insert(w.to_lowercase(), group_idx);
        }
        self.groups.push(group);
    }

    pub fn synonyms_for(&self, word: &str) -> Option<&[String]> {
        // Fast path: ASCII-lowercase the input into a stack buffer where possible.
        // For v1 just use to_lowercase() — still O(1) lookup.
        let key = word.to_lowercase();
        self.index.get(&key).map(|&i| self.groups[i].as_slice())
    }
}
```

- The inversion stays internally consistent: `register_group` is the only mutator.
- `Default::default()` must initialize both fields — derive still works.
- Existing tests (`lookup_finds_registered_word`, `lookup_is_case_insensitive`, `unregistered_word_has_no_group`, `empty_group_is_ignored`) must pass unchanged.
- **Note:** `synonyms_for` still allocates one `String` per call via `to_lowercase()` on the *input*. A follow-up can use a `SmallVec<[u8; 32]>` or the `icu_casemap` crate to avoid it — out of scope here. This change is only about collapsing the linear group scan to O(1).

**Verify:**

```bash
cargo test --all-features
cargo clippy --all-features -- -D warnings
```

**Commit:** `Invert SynonymRegistry to O(1) lookup via pre-lowercased index`

---

## Phase 4 — u32 symbol-table interner for `DiscourseState::word_history`

**File:** `nlg-core/src/discourse.rs`

**Current state:** `word_history: VecDeque<(usize, HashSet<String>)>` stores a `HashSet<String>` per render. `record_output_words` allocates one `String` per non-stopword. `repetition_score` rebuilds the candidate word set and intersects — 3× allocation hit on scored variants.

**Target:** intern all words to `u32` ids; store `HashSet<u32>` per render; stopword check is an O(1) set-membership on a precomputed `HashSet<u32>`.

### 4.1 Add a tiny interner

Inside `discourse.rs`, add a private struct:

```rust
use ahash::AHashMap;

#[derive(Debug, Clone, Default)]
struct WordInterner {
    /// Lowercased word → u32 id. Lowercasing happens once at intern time.
    by_word: AHashMap<String, u32>,
    /// Reverse map for debugging / intersection fallbacks. Index by id.
    by_id: Vec<String>,
}

impl WordInterner {
    fn intern(&mut self, word: &str) -> u32 {
        // Fast path: existing entry.
        if let Some(&id) = self.by_word.get(word) {
            return id;
        }
        // Slow path: allocate once, insert into both maps.
        let id = self.by_id.len() as u32;
        let owned = word.to_string();
        self.by_word.insert(owned.clone(), id);
        self.by_id.push(owned);
        id
    }

    fn get(&self, word: &str) -> Option<u32> {
        self.by_word.get(word).copied()
    }
}
```

Interner lives on `DiscourseState`. Caller normalises the word (lowercase, trim) *before* calling `intern`. `intern` does not do normalisation.

### 4.2 Rewire `record_output_words` and related

Change the per-render storage:

```rust
word_history: VecDeque<(usize, std::collections::HashSet<u32>)>,
interner: WordInterner,
stopword_ids: std::collections::HashSet<u32>,
```

(`HashSet<u32>` from std is fine — u32 keys don't need ahash.)

On `DiscourseState::new()`, populate `stopword_ids` by interning every string in the existing `STOPWORDS` list. This happens once.

On `record_output_words(output: &str)`:

- Tokenise the output as today (existing split logic).
- For each non-stopword token: lowercase it, intern to u32, insert into the current render's `HashSet<u32>`.
- Stopword check is now `if stopword_ids.contains(&id) { continue; }` after interning, OR (faster) check the lowered-case string against a `&AHashMap<&'static str, ()>` lookup before interning. Easier to reason about: intern first, then check. Profiler can say which is faster later.

On `repetition_score(candidate: &str)`:

- Extract candidate words as before.
- Lowercase + intern each. (This *will* allocate on first-seen words from the candidate — accept this cost; future candidates hit the fast path.)
- Compute intersection against recent `HashSet<u32>` entries using `u32` set ops.

### 4.3 Preserve the snapshot/restore contract

`DiscourseState: Clone` must still work. The interner clones deeply — if the `WordInterner` ever gets too big for cheap cloning, swap `by_id: Vec<String>` for an `Arc<Vec<String>>` and revisit. For v1 just derive Clone.

Snapshot/restore in `Engine::render` → `Session::clone()` is the caller; internally it clones `DiscourseState`, which clones the interner. Interners clone in ~O(n) on word count — fine; this happens only on fallible renders.

### 4.4 Preserve public API of `DiscourseState`

The type's methods (`reset`, `begin_render`, `mention_entity`, `record_output_words`, `repetition_score`, `focus_*`, `reference_form`, etc.) keep the same signatures. The interner is private implementation detail.

### 4.5 Verify

```bash
cargo test --all-features
cargo test --no-default-features
cargo test --features serde
cargo clippy --all-features -- -D warnings
cargo bench --bench engine -- --test
```

All pass. Serde round-trips: `DiscourseState` is NOT currently `Serialize` — check `serde.rs` to confirm. If adding a `Serialize` impl for it is required, the interner's internal representation needs a custom serde impl that emits words (not ids). For v1, assume no `DiscourseState` serialization needed — confirm before committing.

**Commit:** `Intern discourse word_history to u32 for faster repetition scoring`

---

## Phase 5 — Final verification

```bash
cargo test --all-features
cargo test --no-default-features
cargo test --features serde
cargo clippy --all-features -- -D warnings
cargo doc --all-features --no-deps
cargo bench --bench engine -- --test
```

Optional: run `cargo bench --bench engine` for real and save the stdout to `docs/plans/.perf-sweep-after.txt` for diffing against the Phase 0 baseline.

**Report to user:** commit hashes (4 expected), test count (376), any bench delta observed, any surprises.

---

## Risk register

| Risk | Mitigation |
|---|---|
| ahash's `AHashMap::new()` requires `RandomState` that's not `Default` in some configs | Use `AHashMap::default()` if `new()` is awkward. Both work in ahash 0.8 with default features disabled. |
| Serde round-trip breaks on AHashMap | ahash has a `serde` feature; add it to the dep if needed. If it breaks and can't be fixed in <30 min, revert Phase 2 and report. |
| Interner makes `DiscourseState::clone()` expensive enough to regress snapshot/restore path | Bench the repetition_score bench specifically — it hits `score_variants` which snapshots/restores. If regression is >5%, wrap interner in `Rc` or `Arc` (inside `DiscourseState`, interior shared) — document the added complexity. |
| `HashSet<u32>` for word history changes iteration order, affecting tests that rely on it | Tests should assert on output *strings*, not on discourse internals. If a test asserts on iteration order, that's a test bug — flag it but don't fix in scope. |
| filter_by_salience borrow lifetime chain gets ugly | If `Vec<&Template>` makes any signature awkward, consider returning `impl Iterator<Item = &Template>` instead. Use judgment. |
| Downstream clippy complaint about elided lifetimes in `filter_by_salience` | Fix by naming the lifetime explicitly. Not worth arguing with clippy. |

## What NOT to do

- **Don't** change public APIs.
- **Don't** add `impl From<u32> for Word` or similar elaborate types — the interner is internal.
- **Don't** add a `locale` feature flag for ahash. ahash is unconditional here.
- **Don't** run `cargo bench` as a gate — it's a measurement, not a pass/fail.
- **Don't** amend commits. Separate commit for any fix.
- **Don't** skip clippy warnings.
- **Don't** touch anything outside `nlg-core/src` except `nlg-core/Cargo.toml`.

## Definition of done

- [ ] Phase 0 baseline clean; Phase 5 final verification clean
- [ ] 4 commits, one per sub-phase, subject lines as specified
- [ ] `filter_by_salience` returns `Vec<&Template>`
- [ ] `Context` uses `AHashMap`
- [ ] `SynonymRegistry` has an inverted index; O(1) lookup
- [ ] `DiscourseState::word_history` stores `HashSet<u32>`; interner private
- [ ] All 376 tests pass on all feature variants
- [ ] No clippy warnings
- [ ] `Engine: Send + Sync` assert still compiles
- [ ] No new public API surface
