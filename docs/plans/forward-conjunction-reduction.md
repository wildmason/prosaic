# Plan: Forward Conjunction Reduction — ELLEIPO Safe Subset v1

**Owner:** sonnet agent
**Scope:** `nlg-core/src/engine.rs` — extend `reduce_same_entity_clauses` and helpers
**Estimated size:** ~150–200 LOC including tests
**Test gate:** all 474 tests pass after every sub-phase; zero warnings
**Branch discipline:** local only, commit at each sub-phase

---

## Why

The engine's existing `reduce_same_entity_clauses` already handles the most common forward-CR case:

- `"The class Foo was renamed. It was modified. It was moved." → "The class Foo was renamed, modified, and moved."`

Two known gaps that the swarm's linguistics agent identified as "safe subset" additions:

### Gap 1: "It also" connective bails out

When the discourse layer prepends the `"It also"` connective, the existing reducer's pronoun-prefix matcher can't strip it cleanly (see the comment in `strip_leading_connective`: *"'It also' forms [...] Reduction will decline for 'It also' forms rather than risk mis-parsing — acceptable as a v1 limitation"*). Result: a discourse-aware render produces `"Foo was renamed. It also was modified."` as two sentences instead of `"Foo was renamed and modified."` — the reducer correctly declines today but it's a legitimate fusion candidate.

### Gap 2: Full-NP repetition not accepted

When a non-pronoun reference was emitted for the second render — typically because Centering Rule 1 demoted the pronoun, or a session was reset, or entity distance exceeded threshold — the sentences still have the same subject. Today the reducer requires `"It <aux>"` on sentences 2+. If they instead repeat the full NP `"The class Foo <aux>"`, reduction declines. The resulting output is awkwardly repetitive.

## Design (locked)

### Scope

Two additions to `reduce_same_entity_clauses`:

1. Strip `"It also"` from sentence 2+ and treat the remainder as if it began with `"It"`.
2. If sentence 2+ doesn't start with `"It <aux>"` but DOES start with the head's `<subject_aux>` prefix (verbatim), accept and reduce.

### Out of scope for v1 (do NOT implement)

- **Active-voice FCR** (e.g., `"Alice committed the fix. Alice updated the docs." → "Alice committed the fix and updated the docs."`). Without grammatical-role info the subject/predicate boundary detection is fragile. Defer to v2 when multilingual's AgreementFeatures lands.
- **Gapping** (e.g., `"Foo was renamed to A. Foo was renamed to B." → "Foo was renamed to A and to B."`). Risky — changes meaning if object-coreference detection is wrong.
- **Backward conjunction reduction**.
- **Mixed-aux reduction** (head has "was", next has "has been"). Already correctly rejected today; keep rejecting.
- **Rewriting into a shared `&mut String` buffer.** The perf refactor that eliminates allocations in this path is a separate plan.

### Safety invariants

Every extension preserves these existing rejection rules:
- `predicate_has_embedded_clause` still rejects `", which ..."`, `", affecting ..."`, etc. on both head and every follower.
- `aux` must match the head's `aux`. No mixed-aux fusion.
- Empty predicate after stripping → reject.
- Any parse failure anywhere → return `None` and let the caller emit unreduced sentences.

---

## Phase 0 — Baseline

```bash
cargo test --all-features
cargo clippy --all-features -- -D warnings
```

Expected: **474 tests passing**, 0 warnings.

**No commit.**

---

## Phase 1 — Fix "It also" connective handling

**File:** `nlg-core/src/engine.rs`

### 1.1 Update `strip_leading_connective`

Today the function returns the input unchanged for `"It also"` forms (see the existing code's `let _ = rest;` comment). Replace the dead branch with a proper rewrite.

Current code (around line 2159):

```rust
if let Some(rest) = s.strip_prefix("It also ") {
    // Returns s unchanged currently — see comment.
    let _ = rest;
}
s
```

New behaviour: return a string that starts with `"It "` instead of `"It also "`. Lifetime-wise, we can't synthesize a `&str` without allocation. Options:

**Option A (preferred):** change `strip_leading_connective` signature from `fn(&str) -> &str` to `fn(&str) -> Cow<'_, str>`. Most callers get the `Borrowed` case; the `"It also"` case returns `Owned(format!("It {rest}"))`.

**Option B:** return an enum or a tuple that encodes "strip happened, continue matching on rest". More invasive.

Go with Option A. Change the signature:

```rust
fn strip_leading_connective(s: &str) -> std::borrow::Cow<'_, str> {
    const CONNECTIVES: &[&str] = &[
        "Additionally,", "Furthermore,", "Similarly,", "Likewise,",
        "Meanwhile,", "However,", "On the other hand,",
    ];

    for conn in CONNECTIVES {
        if let Some(rest) = s.strip_prefix(conn) {
            return std::borrow::Cow::Borrowed(rest.trim_start());
        }
    }

    if let Some(rest) = s.strip_prefix("It also ") {
        return std::borrow::Cow::Owned(format!("It {}", rest.trim_start()));
    }

    std::borrow::Cow::Borrowed(s)
}
```

### 1.2 Update caller

In `reduce_same_entity_clauses`:

```rust
let without_conn = strip_leading_connective(trimmed);
let body = without_conn.trim_end_matches(['.', '!', '?']);
```

`without_conn` is now `Cow<'_, str>`. `body` is `&str` — derived via `without_conn.as_ref().trim_end_matches(...)`. Adjust as needed; `Cow::Deref` makes most things work, but the exact line might need `let body = without_conn.trim_end_matches(...);` written as `let body = AsRef::<str>::as_ref(&without_conn).trim_end_matches(...);` or a temporary `let s: &str = &without_conn;`. Pick the cleanest form.

### 1.3 Tests

Add to engine.rs `#[cfg(test)]`:

```rust
#[test]
fn reduce_accepts_it_also_connective() {
    let reduced = reduce_same_entity_clauses(&[
        "The class UserService was renamed.".to_string(),
        "It also was modified.".to_string(),
    ]);
    assert_eq!(
        reduced.as_deref(),
        Some("The class UserService was renamed and modified.")
    );
}

#[test]
fn reduce_accepts_mixed_discourse_connectives() {
    // Additionally on one sentence, It also on the next.
    let reduced = reduce_same_entity_clauses(&[
        "The class Foo was renamed.".to_string(),
        "Additionally, it was modified.".to_string(),
        "It also was moved.".to_string(),
    ]);
    assert_eq!(
        reduced.as_deref(),
        Some("The class Foo was renamed, modified, and moved.")
    );
}
```

Also add an integration-level test in `nlg-core/tests/integration.rs` or a new `nlg-core/tests/fcr.rs` that renders via `render_batch` and checks the connective-aware path end-to-end.

### 1.4 Verify

```bash
cargo test --all-features
cargo clippy --all-features -- -D warnings
```

**Commit:** `Support "It also" connective in forward conjunction reduction`

---

## Phase 2 — Accept full-NP repetition

**File:** `nlg-core/src/engine.rs`

### 2.1 Extend `reduce_same_entity_clauses`'s follower matcher

Today each follower must match `strip_it_aux_prefix`. Add a parallel branch that tries head-NP repetition when the pronoun form doesn't match.

The head's subject+aux prefix is already computed at the top: `head_subject_aux`. Each follower is first attempted via `strip_it_aux_prefix`. If that returns `None`, try `strip_head_subject_aux_prefix` — strip the head's subject+aux verbatim from the follower.

New helper:

```rust
/// If `body` begins with `subject_aux` followed by a space, return the
/// remaining predicate. Used as a fallback path when the pronoun matcher
/// declines — e.g. when Rule 1 demoted the follower to a full NP or a
/// session reset broke the pronoun chain.
fn strip_head_subject_prefix<'a>(body: &'a str, subject_aux: &str) -> Option<&'a str> {
    let with_space = format!("{subject_aux} ");
    body.strip_prefix(with_space.as_str()).map(str::trim_start)
}
```

In the follower loop of `reduce_same_entity_clauses`:

```rust
let without_conn = strip_leading_connective(trimmed);
let body_cow = without_conn;
let body = AsRef::<str>::as_ref(&body_cow).trim_end_matches(['.', '!', '?']);

// Try pronoun form first.
let (aux, predicate) = match strip_it_aux_prefix(body) {
    Some(parsed) => parsed,
    None => {
        // Fallback: full-NP repetition.
        let remainder = strip_head_subject_prefix(body, head_subject_aux)?;
        // `remainder` is what's after "<head_subject_aux> " — i.e., the
        // predicate. aux is head_aux by definition in this path.
        (head_aux, remainder)
    }
};

if aux != head_aux { return None; }
if predicate_has_embedded_clause(predicate) { return None; }
predicates.push(predicate.to_string());
```

Note: when the fallback fires, we KNOW aux == head_aux because we stripped `head_subject_aux` which already includes head_aux. The `if aux != head_aux` check is redundant in that branch but kept for uniformity. Clippy may complain; let it — uniform shape is easier to read.

### 2.2 Tests

```rust
#[test]
fn reduce_accepts_full_np_repetition() {
    let reduced = reduce_same_entity_clauses(&[
        "The class Foo was renamed.".to_string(),
        "The class Foo was modified.".to_string(),
    ]);
    assert_eq!(
        reduced.as_deref(),
        Some("The class Foo was renamed and modified.")
    );
}

#[test]
fn reduce_accepts_full_np_repetition_three_clauses() {
    let reduced = reduce_same_entity_clauses(&[
        "The class Foo was renamed.".to_string(),
        "The class Foo was modified.".to_string(),
        "The class Foo was moved.".to_string(),
    ]);
    assert_eq!(
        reduced.as_deref(),
        Some("The class Foo was renamed, modified, and moved.")
    );
}

#[test]
fn reduce_mixed_np_and_pronoun_accepted() {
    // Head full NP, follower 1 pronoun, follower 2 full NP — all reduce.
    let reduced = reduce_same_entity_clauses(&[
        "The class Foo was renamed.".to_string(),
        "It was modified.".to_string(),
        "The class Foo was moved.".to_string(),
    ]);
    assert_eq!(
        reduced.as_deref(),
        Some("The class Foo was renamed, modified, and moved.")
    );
}

#[test]
fn reduce_rejects_different_np_repetition() {
    // Head is "The class Foo was renamed", follower is "The class Bar was modified" —
    // different NPs, must not reduce.
    let reduced = reduce_same_entity_clauses(&[
        "The class Foo was renamed.".to_string(),
        "The class Bar was modified.".to_string(),
    ]);
    assert_eq!(reduced, None);
}

#[test]
fn reduce_rejects_full_np_with_embedded_clause() {
    let reduced = reduce_same_entity_clauses(&[
        "The class Foo was renamed.".to_string(),
        "The class Foo was modified, which affects 6 consumers.".to_string(),
    ]);
    assert_eq!(reduced, None);
}
```

Integration-level test: build an engine with `Session::reset()` between two renders of the same entity (pronoun won't be emitted because `cb` is reset), then `render_batch` over the two renders. Assert the output fuses. This proves the fallback path matters end-to-end.

### 2.3 Verify

```bash
cargo test --all-features
cargo test --no-default-features
cargo test --features serde
cargo clippy --all-features -- -D warnings
```

**Commit:** `Accept full-NP repetition in forward conjunction reduction`

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

Test count should increase by ~8–10 → **~483 total**.

**Report:** 2 commit hashes, test count delta, any existing tests that legitimately changed output (some that asserted two-sentence passive output may now produce fused output — update expectations with a code comment citing FCR).

---

## Risk register

| Risk | Mitigation |
|---|---|
| Changing `strip_leading_connective` to return `Cow` breaks other callers | Grep for callers. Only `reduce_same_entity_clauses` uses it today (check). If another caller exists, update it. |
| Existing integration tests assert two-sentence output that now fuses | Legitimate — update the test expectation and cite FCR in a comment. Do NOT roll back the fix. Report the list in the final summary. |
| The head's `subject_aux` might have trailing whitespace that breaks `strip_prefix` | `split_subject_aux` already strips trailing whitespace from the subject_aux span. Verify with a test case. |
| `AsRef<str>` on `Cow<str>` vs `&str` call sites get ugly | One local `let s: &str = without_conn.as_ref();` often simplifies. Use what reads cleanest. |
| "It also" rewrite path allocates per reduction attempt | Acceptable — reduction only runs on multi-sentence runs; the allocation is bounded and rare. Buffer-writing optimisation is a separate plan. |
| Full-NP repetition accepts too much, e.g., "The foo was X" + "The foo was bar, which ..." — but the `predicate_has_embedded_clause` check still fires on the latter's predicate | Verified by the `reduce_rejects_full_np_with_embedded_clause` test. |
| Subject NP differs only in trailing punctuation (shouldn't happen, but) | `trim_end_matches(['.', '!', '?'])` already strips terminators before matching. Fine. |
| Case-sensitivity: "the class Foo" vs "The class Foo" in sentence 2 | Rare in practice (we capitalize sentence starts). Case-sensitive match is correct for v1. |

## What NOT to do

- **Do not** implement active-voice FCR.
- **Do not** implement gapping.
- **Do not** implement backward CR.
- **Do not** allow mixed-aux fusion.
- **Do not** rewrite `reduce_same_entity_clauses` to write into a `&mut String` buffer. That's the perf follow-up.
- **Do not** amend commits.

## Definition of done

- [ ] Phase 0 baseline clean
- [ ] 2 commits with specified subject lines
- [ ] `strip_leading_connective` returns `Cow<str>` and handles `"It also"` correctly
- [ ] `reduce_same_entity_clauses` falls back to head-NP repetition when pronoun form doesn't match
- [ ] `predicate_has_embedded_clause` safety check still fires on both branches
- [ ] All ~483 tests pass across feature variants
- [ ] `cargo clippy --all-features -- -D warnings` clean
- [ ] `Engine: Send + Sync` assert still compiles
- [ ] No new public API
