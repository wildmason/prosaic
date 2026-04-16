# Plan: Centering Rule 1 Enforcement

**Owner:** sonnet agent
**Scope:** `nlg-core/src/discourse.rs` plus minor touches in `engine.rs` for tests
**Estimated size:** ~200–300 LOC including tests
**Test gate:** all 435 tests pass after every sub-phase; zero warnings
**Branch discipline:** local only, commit at each sub-phase

---

## Why

Classical Centering Theory (Grosz, Joshi & Weinstein 1995) defines Rule 1:

> If any element of Cf(Ui) is realized as a pronoun in Ui+1, then the Cb(Ui+1) must be realized as a pronoun also.

Cb (backward-looking center) is the most salient entity carried over from the previous utterance. In our engine, this translates to a practical rule:

**Pronominalize an entity only when it IS the Cb.** If the thing you're about to refer to as "it" isn't the current backward-center, a reader will have to work harder than they should to resolve the referent — that's the bug class. Use the short name instead.

Linguistics agent's example of the failure mode today:

- "Foo was renamed."
- "Bar was added."
- "It was modified." ← the current engine would render "it" confidently, but the referent is ambiguous: is "it" Foo or Bar? Rule 1 forces the engine to either produce a pronoun only when the referent is the tracked Cb, or otherwise demote to the short name.

Our existing `has_ambiguity` heuristic catches the naive two-entity case (refuses pronoun when any other recent entity is still tracked). Rule 1 is a stricter and more principled formulation that picks the *right* entity to pronominalize when one is available.

## Design (locked — do not deviate)

### Cb transition rule (v1)

After each completed render, compute and store Cb for the **next** utterance:

```
new_Cb =
  if current entity is same as prior focus:
    current entity                          (Continue transition — keep Cb)
  else if current entity appears in the last 3 renders AND was a prior Cb or focus:
    current entity                          (Retain transition — Cb shifts to newly-focused known entity)
  else if prior focus still exists in entity registry AND not too distant:
    prior focus                             (Smooth Shift — keep prior focus as Cb even though focus moved)
  else:
    None                                    (Rough Shift or first render — no Cb)
```

The simplification from full Centering: we don't track grammatical role rank (subject > object > obliques) because our single-slot-per-render templates rarely encode it. Role-aware Cb is a v2 concern.

### Rule 1 check at `reference_form`

When `reference_form(name)` would return `Pronoun` under the existing rules (distance == 1, focus match, no ambiguity), add one more gate:

```
if Cb is Some(cb_name) and cb_name != name:
    return ShortName      // demote — we'd be pronominalizing a non-Cb entity
if Cb is None and existing Pronoun rules hold:
    return Pronoun        // fresh discourse or legitimate reintroduction
if Cb is Some(name) and existing Pronoun rules hold:
    return Pronoun        // the canonical Rule 1 case — Cb is being pronominalized
```

For the null-Cb case (first render, or Rough Shift), fall through to the existing behaviour — don't introduce regressions in the "Foo was renamed. It was modified." simple case.

### API surface

**No public API changes.** Cb tracking is internal to `DiscourseState`. `reference_form` signature is unchanged. `mention_entity` signature is unchanged. All existing callers work.

Internal additions:
- `DiscourseState::cb: Option<String>` — new field
- `DiscourseState::cb_history: VecDeque<Option<String>>` — last N Cb values, bounded to 4 — used for Retain detection across near-past renders
- `DiscourseState::previous_focus: Option<String>` — focus of the render **before** the current one, for Cb transition computation at render start

### Out of scope for this plan

- Full Cf tracking (forward-looking centers) — v2
- Grammatical-role ranking — v2
- Transition classification (Continue/Retain/Smooth-Shift/Rough-Shift) — v2; we approximate via the transition rule above
- Multi-entity utterances — our templates are mostly single-entity; when a template has multiple `refer` slots, the second slot still goes through standard discourse logic. Rule 1 is enforced based on the primary (first-refer) entity only.
- Changing the engine's render pipeline beyond one additional `discourse.update_cb_for_next_render()` call

---

## Phase 0 — Baseline

```bash
cargo test --all-features
cargo clippy --all-features -- -D warnings
```

Expected: **435 tests passing**, 0 warnings.

**No commit.**

---

## Phase 1 — Add Cb tracking state to `DiscourseState`

**File:** `nlg-core/src/discourse.rs`

### 1.1 New fields

Add to `DiscourseState`:

```rust
pub struct DiscourseState {
    // ... existing fields ...

    /// Backward-looking center for the NEXT render. Updated at the end
    /// of each successful render. `None` before the first render, after
    /// a reset, or when no coherent transition is available.
    cb: Option<String>,

    /// Focus entity of the render immediately before the current one.
    /// Used to compute Cb transitions. Different from `focus_entity`:
    /// that tracks the current render's focus; this tracks the previous.
    previous_focus: Option<String>,
}
```

Initialize both to `None` in `new()`. Reset both in `reset()`.

### 1.2 Cb update logic

Add a private helper:

```rust
/// Compute and store the Cb for the **next** render, based on the
/// entity just mentioned in the current render.
///
/// Called at the END of each successful render, after
/// `mention_entity` has updated current focus. Not part of
/// `mention_entity` itself — the caller (engine `render_tx`) triggers
/// it via a dedicated method so failed renders don't advance Cb.
fn compute_cb_transition(&mut self) {
    let current = self.focus_entity.as_deref();
    let prev = self.previous_focus.as_deref();

    self.cb = match (current, prev) {
        // First render ever, or reset just happened.
        (_, None) => current.map(str::to_string),

        // Continue: same entity as last time. Cb stays.
        (Some(c), Some(p)) if c == p => Some(c.to_string()),

        // Shift: different entity. Cb becomes the newly-focused
        // entity iff it's been mentioned before (i.e., re-focus after
        // a digression). Otherwise Cb becomes the prior focus,
        // preserving coherence across a single-utterance digression.
        (Some(c), Some(p)) => {
            if self.entities.contains_key(c) && self.entities[c].mention_count > 1 {
                // Re-focusing on a previously-seen entity: Retain.
                Some(c.to_string())
            } else {
                // New entity introduced; prior focus still the Cb
                // for one more utterance (Smooth Shift).
                Some(p.to_string())
            }
        }

        // Current render has no named entity; Cb carries forward.
        (None, Some(p)) => Some(p.to_string()),
    };

    // Shift previous_focus forward for the next render.
    self.previous_focus = current.map(str::to_string);
}
```

### 1.3 Expose an update entry point

Add a public method called by `engine.rs` at the end of each successful render:

```rust
/// Advance Cb tracking for the next render. Call this after all
/// mutations from the current render (mention_entity,
/// record_output_words) have completed and the render has committed.
/// On render failure, the Session snapshot restore will roll this back.
pub fn advance_cb(&mut self) {
    self.compute_cb_transition();
}
```

### 1.4 Unit tests

Add to `#[cfg(test)]` in `discourse.rs`:

```rust
#[test]
fn cb_none_before_first_render() {
    let state = DiscourseState::new();
    assert_eq!(state.cb, None);
}

#[test]
fn cb_becomes_focus_after_first_render() {
    let mut state = DiscourseState::new();
    state.begin_render();
    state.mention_entity("Foo", "class");
    state.advance_cb();
    assert_eq!(state.cb.as_deref(), Some("Foo"));
}

#[test]
fn cb_stays_on_continue_transition() {
    let mut state = DiscourseState::new();
    state.begin_render();
    state.mention_entity("Foo", "class");
    state.advance_cb();
    state.begin_render();
    state.mention_entity("Foo", "class");
    state.advance_cb();
    assert_eq!(state.cb.as_deref(), Some("Foo"));
}

#[test]
fn cb_shifts_to_prior_focus_on_new_entity_intro() {
    // Render 1: Foo → Cb becomes Foo
    // Render 2: Bar (new entity) → Cb stays Foo (Smooth Shift)
    let mut state = DiscourseState::new();
    state.begin_render();
    state.mention_entity("Foo", "class");
    state.advance_cb();
    state.begin_render();
    state.mention_entity("Bar", "class");
    state.advance_cb();
    assert_eq!(state.cb.as_deref(), Some("Foo"));
}

#[test]
fn cb_shifts_to_current_on_retain() {
    // Render 1: Foo
    // Render 2: Foo (continue)
    // Render 3: Foo (continue)
    // Render 4: Bar (new entity; Cb=Foo by Smooth Shift)
    // Render 5: Foo (re-focus on previously-seen entity; Cb=Foo by Retain)
    let mut state = DiscourseState::new();
    for name in ["Foo", "Foo", "Foo", "Bar", "Foo"] {
        state.begin_render();
        state.mention_entity(name, "class");
        state.advance_cb();
    }
    // Render 5: Foo has mention_count >= 2 → Retain → Cb=Foo
    assert_eq!(state.cb.as_deref(), Some("Foo"));
}

#[test]
fn cb_reset_clears_state() {
    let mut state = DiscourseState::new();
    state.begin_render();
    state.mention_entity("Foo", "class");
    state.advance_cb();
    state.reset();
    assert_eq!(state.cb, None);
    assert_eq!(state.previous_focus, None);
}
```

### 1.5 Verify

```bash
cargo test --all-features
cargo clippy --all-features -- -D warnings
```

**Commit:** `Track Cb (backward-looking center) on DiscourseState`

---

## Phase 2 — Apply Rule 1 in `reference_form`

**File:** `nlg-core/src/discourse.rs`

### 2.1 Update `reference_form`

Modify the existing function. The current logic (roughly):

```rust
if distance >= ENTITY_REINTRODUCE_DISTANCE { return Full; }
if distance == 1 && focus_entity == Some(name) && !has_ambiguity(name) {
    return Pronoun;
}
if distance > 0 && distance < ENTITY_REINTRODUCE_DISTANCE { return ShortName; }
return Full;
```

New logic — insert a Rule 1 check before allowing Pronoun:

```rust
if distance >= ENTITY_REINTRODUCE_DISTANCE { return Full; }

// Candidate for pronoun under existing rules
let pronoun_candidate = distance == 1
    && self.focus_entity.as_deref() == Some(name)
    && !self.has_ambiguity(name);

if pronoun_candidate {
    // Rule 1: pronominalize only if the referent IS the Cb, or if
    // Cb is None (fresh discourse / post-reset / genuinely first
    // named entity). Otherwise demote to ShortName to avoid
    // ambiguous pronoun resolution.
    match self.cb.as_deref() {
        None => return ReferenceForm::Pronoun,           // no Cb to conflict with
        Some(cb_name) if cb_name == name => return ReferenceForm::Pronoun,
        Some(_) => return ReferenceForm::ShortName,       // Rule 1 demotion
    }
}

if distance > 0 && distance < ENTITY_REINTRODUCE_DISTANCE {
    return ReferenceForm::ShortName;
}

ReferenceForm::Full
```

### 2.2 Engine integration

**File:** `nlg-core/src/engine.rs`

In `render_tx`, after `record_output_words` (near the end of the successful-render path), add one line:

```rust
session.discourse.advance_cb();
```

This is AFTER all the other state mutations. If any earlier step errored, the session snapshot/restore path (already in place) rolls back `cb` along with everything else.

### 2.3 Existing test fallout

Existing tests that assert pronoun behaviour may now see ShortName instead. Specifically:
- `refer_second_mention_uses_pronoun` in `engine::tests`
- Any integration test asserting "it" appears in the rendered second-mention output

**Investigate each failing test:**
- If the test has only ONE entity in play across renders → Cb=Foo, second render asks for pronoun on Foo → Rule 1 allows it. Should still pass.
- If the test introduces a SECOND entity between mentions → Rule 1 demotes. Update the test's expected output from "it" to the short name, with a comment citing Rule 1.

Do NOT paper over legitimate demotions. If a test fails because Rule 1 correctly demoted a pronoun, update the test's expectation. Add a code comment in the test explaining why.

### 2.4 New Rule 1 tests

Add integration tests in `nlg-core/tests/integration.rs` (or a new `nlg-core/tests/centering.rs`):

```rust
#[test]
fn rule_1_single_entity_pronominalizes_normally() {
    // Cb=Foo, render about Foo → pronoun allowed.
    let mut engine = engine();
    engine.register_template("t", "{name|refer} was modified").unwrap();
    let mut session = Session::new();
    let mut ctx = Context::new();
    ctx.insert("entity_type", Value::String("class".into()));
    ctx.insert("name", Value::String("Foo".into()));

    let r1 = engine.render(&mut session, "t", &ctx).unwrap();
    let r2 = engine.render(&mut session, "t", &ctx).unwrap();

    assert!(r1.contains("The class Foo"));
    assert!(r2.starts_with("It ") || r2.contains("it "));
}

#[test]
fn rule_1_demotes_pronoun_when_referent_not_cb() {
    // Render 1: Foo. Cb becomes Foo.
    // Render 2: Bar. Cb is still Foo (Smooth Shift via Rule 1 design).
    // Render 3: Bar again. Referent Bar ≠ Cb Foo → demote to ShortName.
    let mut engine = engine();
    engine.register_template("t", "{name|refer} was modified").unwrap();
    let mut session = Session::new();

    let mut c1 = Context::new();
    c1.insert("entity_type", Value::String("class".into()));
    c1.insert("name", Value::String("Foo".into()));
    let r1 = engine.render(&mut session, "t", &c1).unwrap();

    let mut c2 = Context::new();
    c2.insert("entity_type", Value::String("class".into()));
    c2.insert("name", Value::String("Bar".into()));
    let r2 = engine.render(&mut session, "t", &c2).unwrap();
    let r3 = engine.render(&mut session, "t", &c2).unwrap();

    // r3 would naively pronominalize Bar ("it was modified") but Rule 1
    // demands the Cb (Foo) be pronominalized first. Since we're
    // pronominalizing a non-Cb entity, demote to ShortName.
    assert!(!r3.to_lowercase().starts_with("it"), "got: {r3}");
    assert!(r3.contains("Bar"), "got: {r3}");
}

#[test]
fn rule_1_resets_cb_on_engine_reset() {
    let mut engine = engine();
    engine.register_template("t", "{name|refer} was modified").unwrap();
    let mut session = Session::new();

    let mut c = Context::new();
    c.insert("entity_type", Value::String("class".into()));
    c.insert("name", Value::String("Foo".into()));
    engine.render(&mut session, "t", &c).unwrap();

    session.reset();

    // After reset, Cb is None; first-mention of Foo is Full again.
    let r = engine.render(&mut session, "t", &c).unwrap();
    assert!(r.contains("The class Foo"));
}
```

### 2.5 Verify

```bash
cargo test --all-features
cargo test --no-default-features
cargo test --features serde
cargo clippy --all-features -- -D warnings
```

If any existing test fails due to a legitimate Rule 1 demotion, update it per 2.3 and include the update in the same commit.

**Commit:** `Enforce Centering Rule 1 in reference_form`

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

All clean. Test count should increase by ~10–12 (6 Cb tests + 3 Rule 1 integration tests + a few updates) — expect **~445 total**, up from 435.

**Report:** 2 commit hashes, test count delta per feature variant, any existing tests that needed updating for legitimate Rule 1 demotions (list them).

---

## Risk register

| Risk | Mitigation |
|---|---|
| An existing integration test asserts "it" appears in a two-entity-alternating scenario, and Rule 1 correctly demotes | Update the test; cite Rule 1 in the comment. Do NOT roll back the Rule 1 logic to "pass the test" — the test was encoding an actual bug. Report the list of updated tests. |
| `focus_entity` semantic subtly differs from what Cb tracking expects | Read `mention_entity`'s existing logic carefully. `focus_entity` is the most-recently-mentioned entity name — that's what we want. `previous_focus` is the value of `focus_entity` at the start of the current render, captured when `compute_cb_transition` runs. |
| `compute_cb_transition` fires before `mention_entity` updates focus — state ordering bug | `advance_cb` is explicitly called AFTER `mention_entity` in `render_tx`. The `previous_focus` update inside `compute_cb_transition` reads `focus_entity` (already current) and stores it for the NEXT call — i.e. `previous_focus` becomes the *previous* value of focus on the next advance. Read the helper carefully. If confusing, rename fields to make timing explicit. |
| Ambiguity check (`has_ambiguity`) now double-guards alongside Rule 1 | Fine. Both should pass before pronoun is allowed. Two belt-and-suspenders checks that encode different intuitions. |
| Rule 1 demotes too aggressively in legitimate single-entity runs after a brief aside | The Cb transition rule preserves prior focus across a single-utterance digression (Smooth Shift), so "Foo … Bar … Foo" keeps Cb=Foo through the Bar detour. Verify with test `cb_shifts_to_current_on_retain`. |
| `serde` serialization of `DiscourseState` needs to include new fields | `DiscourseState` is currently NOT `Serialize` (the `tests/serde.rs` suite doesn't cover it). Confirm before coding. If it becomes Serialize later, the new fields are String/Option<String> — serde-trivial. No action now. |
| Snapshot/restore path for failed renders — Cb must roll back | `Session::clone()` (via `DiscourseState::clone()`) already deep-clones all fields. Adding `cb` and `previous_focus` is automatic with the existing Clone derive. Verify the `failed_render_does_not_mutate_discourse` test still passes after Phase 2. |

## What NOT to do

- **Do not** introduce full Cf tracking, grammatical-role ranks, or transition-type classification. v1 is intentionally simpler.
- **Do not** change the public API of `DiscourseState` beyond adding `advance_cb`.
- **Do not** try to handle multi-`refer`-slot templates specially. Phase 2 only affects `reference_form` for the primary entity.
- **Do not** amend commits.
- **Do not** skip re-running the integration tests (`--all-features`) after Phase 2 — they reveal which existing expectations Rule 1 legitimately invalidates.
- **Do not** disable Rule 1 behind a feature flag. It's unconditional and correct.

## Definition of done

- [ ] Phase 0 baseline clean
- [ ] 2 commits with specified subject lines
- [ ] `DiscourseState` has `cb`, `previous_focus` fields and `advance_cb` method
- [ ] `DiscourseState::compute_cb_transition` implements the v1 transition rule
- [ ] `DiscourseState::reference_form` gates Pronoun on `Cb is None || Cb == name`
- [ ] `Engine::render_tx` calls `session.discourse.advance_cb()` at end of successful render
- [ ] All 435 → ~445 tests pass across feature variants
- [ ] `cargo clippy --all-features -- -D warnings` clean
- [ ] `Engine: Send + Sync` assert still compiles
- [ ] No new public API beyond `advance_cb` (which is used only internally by engine)
- [ ] No `unsafe`
