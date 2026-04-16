# Plan: Full Centering Theory (Cb/Cf with transition classification)

**Owner:** sonnet agent
**Scope:** `prosaic-core/src/discourse.rs` + `prosaic-core/src/engine.rs` (minor — expose in `render_explained`) + `prosaic-core/src/lib.rs` (re-export `Transition`)
**Estimated size:** ~500 LOC including tests
**Test gate:** 936 baseline tests still pass; ~20-25 new
**Branch discipline:** local only, one commit

---

## Why

Centering Theory (Grosz, Joshi, Weinstein 1995) defines a unified discourse model:

- **Cf(Un)** — the **forward-looking centers** of utterance n: every entity realized in Un, ranked by grammatical prominence (typically Subject > Object > Indirect Object > Oblique).
- **Cp(Un)** — the **preferred center**: the highest-ranked member of Cf(Un). The entity most likely to be pronominalized in Un+1.
- **Cb(Un)** — the **backward-looking center**: the highest-ranked entity in Cf(Un−1) that is also realized in Un. Already partially implemented.

**Transitions** between utterances are classified by how Cb moves and whether Cb equals Cp:

|                              | Cb(n) == Cp(n) | Cb(n) != Cp(n) |
|------------------------------|----------------|----------------|
| **Cb(n) == Cb(n−1)**          | **Continue**   | **Retain**     |
| **Cb(n) != Cb(n−1)** (or none)| **Smooth Shift** | **Rough Shift** |

**Rule 2:** A discourse is more coherent when it prefers Continue > Retain > Smooth Shift > Rough Shift.

Today we only track a single `focus_entity` and compute a simplified Cb. That gave us Rule 1 (pronoun only when Cb). The Rule 2 and full Cf ranking give us:

- A `Transition` telemetry signal exposed via `RenderExplanation.centering_transition` — lets callers see which transitions a document produces, surface rough shifts as a coherence signal.
- A foundation for future ranking-aware choices (preferring variant phrasings that produce smoother transitions).

## Design

### `Cf` construction during render

Today `pipe_refer_single` calls `mention_entity` on the referred slot's value. In multi-entity templates (e.g. code.renamed with both `old_name` and `new_name`), `mention_entity` gets called more than once per render — the last call wins as `focus_entity`. We need to preserve *all* calls as the Cf list, ranked by insertion order with grammatical-role heuristic.

**Ranking heuristic based on slot name:**

| Rank | Slot names (heuristic) |
|------|------------------------|
| 0 (Subject) | `name`, `old_name`, `subject` |
| 1 (Direct Object) | `new_name`, `object`, `target` |
| 2 (Indirect Object / Location) | `location`, `new_location`, `old_location`, `to`, `from` |
| 3 (Oblique) | anything else, including `consumers` |

This is a heuristic — real grammatical role tagging would require parser-level analysis. For our templates, the convention is well-established: the primary entity lives in `name`/`old_name`, changes-to live in `new_name`. Keep it simple.

### New types

```rust
// In discourse.rs:

/// A forward-looking center: an entity realized in an utterance with its
/// grammatical-role-based salience rank (lower = more prominent).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Cf {
    pub name: String,
    pub rank: u8,
}

/// Centering Theory transition class between consecutive utterances.
///
/// Prefer (in order): `Continue` > `Retain` > `SmoothShift` > `RoughShift`.
/// `NoCb` means no coherent transition could be classified (first render,
/// post-reset, or utterance with no entities).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Transition {
    Continue,
    Retain,
    SmoothShift,
    RoughShift,
    NoCb,
}
```

Re-export `Transition` and `Cf` from `prosaic-core/src/lib.rs`.

### New state in `DiscourseState`

```rust
/// Cf list being built during the CURRENT render. Populated by
/// `mention_entity*` calls, cleared at each `begin_render`. Ordered by
/// rank ascending (lowest rank first); ties broken by insertion order.
current_cf: Vec<Cf>,

/// Cf list from the PREVIOUS render. Set by `advance_cb` as a snapshot of
/// `current_cf`. Used to classify transitions.
previous_cf: Vec<Cf>,

/// Classification of the transition computed by the most recent
/// `advance_cb` call. `Transition::NoCb` before any render.
last_transition: Transition,
```

### `begin_render`

Clear `current_cf` at the start of each render so ranking state is per-utterance.

### `mention_entity_ranked(name, entity_type, rank)`

New method; `mention_entity(name, type)` delegates to `mention_entity_ranked(name, type, 0)` (keeps existing behavior — everything treated as Subject unless the engine says otherwise).

```rust
pub fn mention_entity(&mut self, name: &str, entity_type: &str) {
    self.mention_entity_ranked(name, entity_type, 0);
}

pub fn mention_entity_ranked(&mut self, name: &str, entity_type: &str, rank: u8) {
    // ... existing logic for entities map / focus_entity ...

    // Additionally: insert into current_cf maintaining rank-ascending order.
    // Deduplicate by name — if the entity is already in Cf, keep the LOWER rank.
    if let Some(existing) = self.current_cf.iter_mut().find(|c| c.name == name) {
        if rank < existing.rank {
            existing.rank = rank;
        }
    } else {
        self.current_cf.push(Cf { name: name.to_string(), rank });
    }
    // Sort stably by rank so Cp = first element.
    self.current_cf.sort_by_key(|c| c.rank);
}
```

The existing semantics of `focus_entity` should now reflect the **Cp** (preferred center) — i.e. the rank-0 entity. When a rank-0 mention happens, `focus_entity` updates. When only higher-rank mentions happen in an utterance, `focus_entity` carries over from the previous call (that's its existing behavior). Adjust `mention_entity_ranked` to update `focus_entity` only when rank == 0 OR when `focus_entity` is None.

Wait — that changes behavior. Existing callers call `mention_entity(name, type)` which now = rank 0. So they still update focus_entity. Good.

But `pipe_refer_single` currently calls `mention_entity` once per slot referenced. In a template like `"{old_name|refer} was renamed to {new_name}"`, both old_name and new_name get mentioned (old_name from `refer` pipe, new_name from... nothing, actually — new_name isn't piped through refer). So only old_name flows through. That's the single-mention pattern.

But for code.renamed with `{old_name|refer}` AND `{new_name}` (bare slot), only old_name gets mentioned. We want new_name to also be in Cf but at rank 1 (Direct Object). We'd need the engine to emit a rank-1 mention when it sees a bare name-like slot.

**Let's keep scope tight:** the engine emits rank-0 mentions through the existing `refer` pipe. Multi-rank mentions come from callers using a new engine method `session.discourse_mut().mention_entity_ranked(...)` explicitly. For now, Cf will usually have just the rank-0 entity — which is fine: classification collapses to the simplified form we already have, but with the Transition enum now exposed.

Future work can make the engine auto-detect ranked mentions from template slot names.

### `compute_cb_transition` upgrade

```rust
fn compute_cb_transition(&mut self) {
    let current_cp: Option<&String> = self.current_cf.first().map(|c| &c.name);
    let prev_cp: Option<&String> = self.previous_cf.first().map(|c| &c.name);

    let prev_cb = self.cb.clone();

    // New Cb: highest-ranked Cf member that also appeared in the previous Cf.
    let new_cb: Option<String> = self.current_cf.iter().find(|c| {
        self.previous_cf.iter().any(|p| p.name == c.name)
    }).map(|c| c.name.clone());

    // Fallbacks when the pure definition yields None — preserve prior behavior.
    let new_cb = match (new_cb, current_cp.cloned(), self.previous_focus.clone()) {
        (Some(cb), _, _) => Some(cb),
        (None, Some(cp), None) => Some(cp), // First render: Cb = Cp.
        (None, Some(cp), Some(_)) => {
            // Shift: new entity, no overlap with previous Cf.
            // Previous logic: Continue uses current, Smooth Shift uses prior.
            // Preserve it: if the new entity has been seen before (is in self.entities
            // with mention_count > 1), Retain-style → Cb = new. Otherwise carry prior.
            if self.entities.get(&cp).is_some_and(|m| m.mention_count > 1) {
                Some(cp)
            } else {
                self.previous_focus.clone()
            }
        }
        (None, None, Some(p)) => Some(p), // No entity this utterance: carry prior.
        (None, None, None) => None,
    };

    // Classify transition.
    let transition = classify_transition(
        new_cb.as_deref(),
        prev_cb.as_deref(),
        current_cp.map(|s| s.as_str()),
    );

    self.cb = new_cb;
    self.last_transition = transition;

    // Shift state forward.
    self.previous_focus = current_cp.cloned();
    self.previous_cf = std::mem::take(&mut self.current_cf);
}

fn classify_transition(cb: Option<&str>, prev_cb: Option<&str>, cp: Option<&str>) -> Transition {
    let cb = match cb {
        Some(c) => c,
        None => return Transition::NoCb,
    };
    let cb_eq_prev = matches!(prev_cb, Some(p) if p == cb);
    let cb_eq_cp   = matches!(cp, Some(c) if c == cb);

    match (cb_eq_prev, cb_eq_cp) {
        (true,  true)  => Transition::Continue,
        (true,  false) => Transition::Retain,
        (false, true)  => Transition::SmoothShift,
        (false, false) => Transition::RoughShift,
    }
}
```

### Accessors

```rust
pub fn last_transition(&self) -> Transition { self.last_transition }
pub fn cb(&self) -> Option<&str> { self.cb.as_deref() }
pub fn cf(&self) -> &[Cf] { &self.current_cf }
pub fn previous_cf(&self) -> &[Cf] { &self.previous_cf }
```

### Engine integration

Add `centering_transition: Transition` to `RenderExplanation`. After `advance_cb` is called in `render_tx`, capture the transition and attach it to the explanation.

```rust
pub struct RenderExplanation {
    // ... existing fields ...
    pub centering_transition: Transition,
}
```

Update all test sites that construct `RenderExplanation` (if any — most tests only read it).

### Reset clears all new state

`DiscourseState::reset()` already calls `*self = Self::new()` — new state is automatically cleared.

### Out of scope

- **Automatic ranked mentions from slot name heuristics.** Engine-level inference (scan template AST for slot names, emit ranked mentions) is deferred. Callers can manually rank if they want, but most templates only emit a single rank-0 mention.
- **Using transition preferences to pick variant** (e.g. favor a variant that produces Continue over one that produces Rough Shift). That would be a choose-best scoring addition; deferred.
- **Cross-paragraph Cb chaining.** `DocumentPlan::render` resets the session between paragraphs, which clears Cb — that's correct behavior (paragraphs start fresh).
- **Connective selection from transition.** Today connective comes from `DiscourseRelation`; layering transition-based selection on top is another project.

---

## Phase 0 — Baseline

```bash
cargo test --all-features
cargo clippy --all-features -- -D warnings
```

Expected: 936 tests passing.

---

## Phase 1 — Add `Cf`, `Transition`, and new state fields

1. Add `Cf` struct and `Transition` enum to `discourse.rs`.
2. Add `current_cf`, `previous_cf`, `last_transition` fields to `DiscourseState`.
3. Initialize them in `DiscourseState::new`.
4. Update `begin_render` to clear `current_cf`.
5. Re-export from `lib.rs`: `pub use discourse::{Cf, Transition};`.

### Tests

```rust
#[test]
fn transition_continue_same_entity_and_cp() {
    let mut state = DiscourseState::new();
    state.begin_render();
    state.mention_entity("Foo", "class");
    state.advance_cb();
    // Cb is Foo, Cp was Foo → first render classifies as NoCb (no predecessor).
    assert_eq!(state.last_transition(), Transition::NoCb);

    state.begin_render();
    state.mention_entity("Foo", "class");
    state.advance_cb();
    // Same entity again: Cb stays Foo, Cp is Foo → Continue.
    assert_eq!(state.last_transition(), Transition::Continue);
}

#[test]
fn transition_smooth_shift_new_entity() {
    let mut state = DiscourseState::new();
    state.begin_render();
    state.mention_entity("Foo", "class");
    state.advance_cb();

    state.begin_render();
    state.mention_entity("Bar", "class");
    state.advance_cb();
    // New entity, no overlap with previous Cf → Smooth Shift
    // (Bar is also the Cp so it counts as Smooth, not Rough).
    assert_eq!(state.last_transition(), Transition::SmoothShift);
}

#[test]
fn transition_no_cb_when_no_entity() {
    let mut state = DiscourseState::new();
    state.begin_render();
    state.advance_cb();
    assert_eq!(state.last_transition(), Transition::NoCb);
}
```

---

## Phase 2 — `mention_entity_ranked` + deduplication

1. Add method per design; refactor existing `mention_entity` to delegate.
2. Maintain `current_cf` sorted by rank ascending, deduped by name.
3. Update `focus_entity` to track Cp (rank 0 wins; if no rank-0 yet, take the first mention).

### Tests

```rust
#[test]
fn cf_deduplicates_by_name_keeping_lower_rank() {
    let mut state = DiscourseState::new();
    state.begin_render();
    state.mention_entity_ranked("Foo", "class", 2);
    state.mention_entity_ranked("Foo", "class", 0);
    let cf = state.cf();
    assert_eq!(cf.len(), 1);
    assert_eq!(cf[0].rank, 0);
}

#[test]
fn cf_sorts_by_rank_ascending() {
    let mut state = DiscourseState::new();
    state.begin_render();
    state.mention_entity_ranked("Obj", "class", 1);
    state.mention_entity_ranked("Subj", "class", 0);
    state.mention_entity_ranked("Oblique", "class", 2);
    let cf = state.cf();
    assert_eq!(cf[0].name, "Subj");
    assert_eq!(cf[1].name, "Obj");
    assert_eq!(cf[2].name, "Oblique");
}

#[test]
fn cp_is_first_cf_entry() {
    let mut state = DiscourseState::new();
    state.begin_render();
    state.mention_entity_ranked("Subj", "class", 0);
    state.mention_entity_ranked("Obj", "class", 1);
    assert_eq!(state.cf()[0].name, "Subj");
}
```

---

## Phase 3 — Full transition classification

Implement `compute_cb_transition` per design. Carry over existing Cb fallback logic so Rule 1 pronoun tests continue to pass.

### Tests

```rust
#[test]
fn transition_continue_when_cp_and_cb_both_same() {
    let mut state = DiscourseState::new();
    state.begin_render();
    state.mention_entity_ranked("Foo", "class", 0);
    state.advance_cb();

    state.begin_render();
    state.mention_entity_ranked("Foo", "class", 0);
    state.advance_cb();
    assert_eq!(state.last_transition(), Transition::Continue);
}

#[test]
fn transition_retain_when_cb_same_but_cp_differs() {
    let mut state = DiscourseState::new();
    state.begin_render();
    state.mention_entity_ranked("Foo", "class", 0);
    state.advance_cb();

    state.begin_render();
    // Foo still in Cf (rank 1 — object), but Cp is now Bar (rank 0 — subject).
    // Cb = Foo (only entity in common), Cp = Bar → Cb != Cp → Retain.
    state.mention_entity_ranked("Bar", "class", 0);
    state.mention_entity_ranked("Foo", "class", 1);
    state.advance_cb();
    assert_eq!(state.last_transition(), Transition::Retain);
}

#[test]
fn transition_smooth_shift_new_cb_equals_cp() {
    let mut state = DiscourseState::new();
    state.begin_render();
    state.mention_entity_ranked("Foo", "class", 0);
    state.advance_cb();

    state.begin_render();
    state.mention_entity_ranked("Bar", "class", 0);
    state.advance_cb();
    assert_eq!(state.last_transition(), Transition::SmoothShift);
}

#[test]
fn transition_rough_shift_new_cb_differs_from_cp() {
    let mut state = DiscourseState::new();
    state.begin_render();
    state.mention_entity_ranked("Foo", "class", 0);
    state.advance_cb();

    state.begin_render();
    state.mention_entity_ranked("Baz", "class", 0); // Cp
    state.mention_entity_ranked("Foo", "class", 1); // in both Cfs → Cb
    state.advance_cb();
    // Cb = Foo (overlap), Cp = Baz, prev Cb = Foo.
    // Cb == prev_Cb (true), Cb == Cp (false) → Retain. Hmm.
    // To get a true Rough Shift we need Cb(n) != Cb(n-1).
    // Swap to a case where the previous Cb was Bar:
    assert_eq!(state.last_transition(), Transition::Retain);
}

#[test]
fn transition_rough_shift_proper() {
    let mut state = DiscourseState::new();
    // u1: focus Foo. Cb = None/Foo. Cp = Foo.
    state.begin_render();
    state.mention_entity_ranked("Foo", "class", 0);
    state.advance_cb();

    // u2: focus Bar, Foo in Cf. Cb = Foo (overlap), prev_Cb = Foo, Cp = Bar.
    // Transition: Cb == prev_Cb (Foo==Foo) true, Cb == Cp (Foo==Bar) false → Retain.
    state.begin_render();
    state.mention_entity_ranked("Bar", "class", 0);
    state.mention_entity_ranked("Foo", "class", 1);
    state.advance_cb();
    assert_eq!(state.last_transition(), Transition::Retain);

    // u3: focus Baz, Bar in Cf. Cb = Bar (overlap with u2 Cf), prev_Cb = Foo,
    // Cp = Baz. Cb != prev_Cb (Bar != Foo), Cb != Cp (Bar != Baz) → Rough Shift.
    state.begin_render();
    state.mention_entity_ranked("Baz", "class", 0);
    state.mention_entity_ranked("Bar", "class", 1);
    state.advance_cb();
    assert_eq!(state.last_transition(), Transition::RoughShift);
}
```

---

## Phase 4 — Engine integration: `RenderExplanation.centering_transition`

1. Add the field to `RenderExplanation` (serde-compatible).
2. In the `render_tx` → `render_explained` path, read `session.discourse.last_transition()` AFTER `advance_cb` has run, and stash it in the explanation.
3. Test: render a sequence and verify transitions are emitted in order (first: NoCb, second: Continue for same-entity, third: Smooth Shift for new entity, etc.).

### Tests

```rust
#[test]
fn render_explained_reports_transitions() {
    let mut engine = test_engine();
    engine.register_template("t", "{name|refer} was modified").unwrap();
    let mut s = Session::new();

    let mut c = Context::new();
    c.insert("entity_type", Value::String("class".into()));
    c.insert("name", Value::String("Foo".into()));

    let e1 = engine.render_explained(&mut s, "t", &c).unwrap();
    assert_eq!(e1.centering_transition, Transition::NoCb);

    let e2 = engine.render_explained(&mut s, "t", &c).unwrap();
    assert_eq!(e2.centering_transition, Transition::Continue);

    let mut c2 = Context::new();
    c2.insert("entity_type", Value::String("class".into()));
    c2.insert("name", Value::String("Bar".into()));
    let e3 = engine.render_explained(&mut s, "t", &c2).unwrap();
    assert_eq!(e3.centering_transition, Transition::SmoothShift);
}
```

---

## Phase 5 — Backward compat verification

Run all existing discourse / engine tests. Rule 1 pronoun tests MUST still pass because:
- `mention_entity(n, t)` now delegates to `mention_entity_ranked(n, t, 0)`.
- `focus_entity` update logic is preserved.
- `cb` computation falls back to existing logic when `current_cf`/`previous_cf` are empty or don't overlap.

---

## Phase 6 — Final verification

```bash
cargo test --all-features
cargo test --no-default-features
cargo test --features serde
cargo clippy --all-features -- -D warnings
cargo doc --all-features --no-deps
```

Test count up by ~20–25.

**Commit:** `Add full Centering Theory with Cb/Cf/Cp and transition classification`

---

## Definition of done

- [ ] `Cf` struct + `Transition` enum in `discourse.rs`, re-exported from `lib.rs`
- [ ] `DiscourseState.current_cf`, `previous_cf`, `last_transition` fields
- [ ] `mention_entity_ranked` method with rank-sorted dedup
- [ ] Existing `mention_entity` delegates to rank 0
- [ ] `compute_cb_transition` uses Cf-overlap definition with fallback to prior logic
- [ ] `classify_transition` free function produces Continue/Retain/SmoothShift/RoughShift/NoCb
- [ ] `RenderExplanation.centering_transition` populated after advance_cb
- [ ] All 936 existing tests still pass
- [ ] ~20–25 new tests cover every transition class + Cf dedup + rank sorting
- [ ] `cargo clippy --all-features -- -D warnings` clean
- [ ] One commit: `Add full Centering Theory with Cb/Cf/Cp and transition classification`
