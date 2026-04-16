# Plan: Full ELLEIPO — gapping + backward coordination reduction

**Owner:** sonnet agent
**Scope:** `prosaic-core/src/engine.rs` (new reduce_gapping / reduce_right_node_raising helpers + render_batch wiring)
**Estimated size:** ~350–450 LOC including tests
**Test gate:** 955 baseline tests still pass; ~15–20 new
**Branch discipline:** local only, one commit

---

## Why

Today the engine does two clause reductions:

1. **Subject aggregation** (`render_aggregated_subjects`): same template key + *compatible* (identical non-subject) contexts → "Foo, Bar, and Baz were renamed".
2. **Same-subject predicate reduction** (`reduce_same_entity_clauses`): same entity, different predicates → "It was renamed, modified, and moved."

Two ellipsis operations remain unused:

- **Gapping** (Harbusch & Kempen ELLEIPO): same verb + aux, different subjects AND different objects. Source sentences share a verb; the verb is elided in all but the first.
  - "Foo was moved to core. Bar was moved to util. Baz was moved to api." →
    "Foo was moved to core, Bar to util, and Baz to api."

- **Right-node raising / backward coordination reduction**: different verbs, shared right-peripheral object.
  - "Alice refactored the payment flow. Bob simplified the payment flow." →
    "Alice refactored, and Bob simplified, the payment flow."

Right-node raising produces awkward output in our rendered prose (the bracketing commas break rhythm). We'll **implement gapping only** for v1; leave backward coordination as future work behind a feature flag or a separate plan. The payoff is lower and the surface-form parser would need to recognize trailing object constituents — much harder.

## Design

### Gapping detection

Gapping applies to a run of events where:
1. All events have the **same template key**.
2. All events have a **subject** (extractable entity name).
3. All events render with the **same auxiliary/verb anchor** (e.g. " was moved ", " was renamed to ").
4. The **portion after the shared anchor differs** across events — the "object" (adjuncts, complements).
5. Any leading discourse connective on follower sentences is stripped before comparison, the same way `reduce_same_entity_clauses` does.

If those conditions hold, splice together:
- First event: full sentence (subject + verb + object).
- Follower events: subject + (GAPPED verb) + object.

Example:
- `[ "Foo was moved to core", "Bar was moved to util", "Baz was moved to api" ]` →
- `Foo was moved to core, Bar to util, and Baz to api.`

### Where the anchor comes from

The "shared anchor" is the common substring between the rendered first and second sentences, starting after the subject and spanning up to the first point where they diverge. Practically:

```
s1 = "Foo was moved to core"
s2 = "Bar was moved to util"
```

Token-align from the start: "Foo"≠"Bar" (different subjects — expected). Then: " was moved to "≈" was moved to " (match). Then "core"≠"util" (different objects). The anchor is " was moved to ". The shared anchor length is determined by the longest common prefix *after* the first word of each sentence.

Actually, a cleaner definition: find the longest token prefix that's identical in all sentences AFTER the subject. For gapping we want:
- Drop the first word of each sentence (subject).
- Compare the remaining tokens.
- The common prefix = shared anchor (verb + aux + preposition + other shared adjuncts).
- The divergent suffix = per-event object.

### Constraints

- **Minimum run length: 2.** With just one event there's no ellipsis opportunity.
- **Anchor must be at least 2 tokens long.** A one-word shared anchor ("was") is too generic — don't gap there, it produces awkward output.
- **Divergent suffix must be non-empty** for every event (can't gap if there's nothing left after the anchor).
- **Subjects must be distinct.** If two subjects repeat, we're not seeing a gapping opportunity; that was handled elsewhere.
- **No embedded clauses** in any sentence (same guard as `reduce_same_entity_clauses` uses: no `", which"` / `", affecting"` / etc.) — gapping across subordinate clauses is grammatically unreliable.

### Integration point

In `render_batch`, currently:

```
while i < events.len() {
    let action_end = find_same_action_run(events, i);
    if action_end > i + 1 {
        // compatible contexts → render_aggregated_subjects
        ...
    }
    let entity_end = find_same_entity_run(events, i);
    if entity_end > i + 1 {
        // reduce_same_entity_clauses
        ...
    }
    // single event fallback
    ...
}
```

`find_same_action_run` returns at most the first *compatible* run. Events with incompatible contexts (different non-subject slots) fall through to `find_same_entity_run`, which also fails (different entities), and then each event renders individually.

We want to add a **gapping run** detector BETWEEN those two: after `find_same_action_run` returns a single-event run (no compatible aggregation), check for a same-key+different-context gapping run. If found, render each event separately then reduce via `reduce_gapping`.

```rust
let action_end = self.find_same_action_run(events, i);
if action_end > i + 1 {
    // compatible contexts → aggregated subjects
    ...
    continue;
}

let gap_end = self.find_gapping_run(events, i);
if gap_end > i + 1 {
    let mut rendered: Vec<String> = Vec::with_capacity(gap_end - i);
    for (k, ctx) in &events[i..gap_end] {
        rendered.push(self.render(session, k, ctx)?);
    }
    if let Some(gapped) = reduce_gapping(&rendered) {
        sentences.push(gapped);
    } else {
        sentences.extend(rendered);
    }
    i = gap_end;
    continue;
}

let entity_end = self.find_same_entity_run(events, i);
...
```

### `find_gapping_run`

Walks forward from `start`, collecting events with:
- Same template key as `events[start]`.
- Distinct, extractable entity name.
- Incompatible context with the first event (so they wouldn't have been grabbed by `find_same_action_run`).

Returns `end` such that `events[start..end]` is the candidate run. `end == start + 1` means no run.

```rust
fn find_gapping_run(&self, events: &[(&str, Context)], start: usize) -> usize {
    if start >= events.len() { return start; }

    let (first_key, first_ctx) = (events[start].0, &events[start].1);
    let Some(first_name) = entity_name_from_context(first_ctx) else {
        return start + 1;
    };

    let mut end = start + 1;
    let mut seen: std::collections::HashSet<String> =
        [first_name].into_iter().collect();

    while end < events.len() {
        let (k, ctx) = (events[end].0, &events[end].1);
        if k != first_key { break; }
        let Some(name) = entity_name_from_context(ctx) else { break; };
        if seen.contains(&name) { break; }
        // We WANT contexts to be incompatible — otherwise find_same_action_run
        // would have absorbed this event. If compatible, bail so the caller
        // falls back to the aggregated-subjects path.
        if contexts_compatible_for_aggregation(first_ctx, ctx) { break; }
        seen.insert(name);
        end += 1;
    }

    end
}
```

### `reduce_gapping`

```rust
fn reduce_gapping(sentences: &[String]) -> Option<String> {
    if sentences.len() < 2 { return None; }

    // Strip trailing punctuation and leading connectives for comparison.
    // Split each into (subject_word, rest_tokens).
    let parsed: Vec<(&str, Vec<&str>)> = sentences
        .iter()
        .map(|s| {
            let trimmed = strip_leading_connective(s.trim_end().trim_end_matches(['.', '!', '?']));
            // strip_leading_connective returns Cow; for comparison just need &str.
            // ... (use the same helper from reduce_same_entity_clauses) ...
            split_subject_and_rest(&trimmed)
        })
        .collect::<Option<Vec<_>>>()?;

    // No embedded clauses.
    if sentences.iter().any(|s| predicate_has_embedded_clause(s)) {
        return None;
    }

    // Subjects must be distinct.
    {
        let mut subjects: std::collections::HashSet<&str> = Default::default();
        for (subj, _) in &parsed {
            if !subjects.insert(*subj) { return None; }
        }
    }

    // Longest common prefix among the `rest_tokens` vectors.
    let anchor_len = longest_common_prefix_len(&parsed);
    if anchor_len < 2 { return None; }

    // Every divergent suffix must be non-empty.
    if parsed.iter().any(|(_, toks)| toks.len() <= anchor_len) {
        return None;
    }

    let anchor_tokens: &[&str] = &parsed[0].1[..anchor_len];
    let anchor = anchor_tokens.join(" ");

    // Per-event suffixes (divergent parts).
    let suffixes: Vec<String> = parsed
        .iter()
        .map(|(_, toks)| toks[anchor_len..].join(" "))
        .collect();

    // Assemble the output.
    // First sentence: "<subj1> <anchor> <suffix1>"
    // Followers     : "<subjN> <suffixN>" (no verb — gapped)
    let first = format!("{} {} {}", parsed[0].0, anchor, suffixes[0]);
    let tail: Vec<String> = parsed.iter().skip(1).zip(suffixes.iter().skip(1))
        .map(|((subj, _), suf)| format!("{subj} {suf}"))
        .collect();

    let joined = match tail.len() {
        1 => format!("{first}, and {}", tail[0]),
        _ => {
            let (last, rest) = tail.split_last().unwrap();
            format!("{first}, {}, and {last}", rest.join(", "))
        }
    };

    Some(format!("{joined}."))
}

fn longest_common_prefix_len(parsed: &[(&str, Vec<&str>)]) -> usize {
    if parsed.is_empty() { return 0; }
    let min_len = parsed.iter().map(|(_, t)| t.len()).min().unwrap_or(0);
    for i in 0..min_len {
        let candidate = parsed[0].1[i];
        if !parsed.iter().all(|(_, t)| t[i] == candidate) {
            return i;
        }
    }
    min_len
}

fn split_subject_and_rest(s: &str) -> Option<(&str, Vec<&str>)> {
    // The "subject" is everything up to and including the first aux verb,
    // per AUX_PREFIXES. Reuse split_subject_aux, which returns
    // (subject_aux_full, aux_word, rest). For gapping we need the raw
    // subject (without aux) and the rest WITH aux attached. Re-split:
    for aux in AUX_PREFIXES {
        let marker = format!(" {aux} ");
        if let Some(pos) = s.find(&marker) {
            let subject = &s[..pos];
            // rest_tokens = aux + everything after
            let rest_str = &s[pos + 1..]; // +1 to skip leading space
            let rest_tokens: Vec<&str> = rest_str.split_whitespace().collect();
            return Some((subject, rest_tokens));
        }
    }
    None
}
```

### Out of scope

- **Backward coordination / right-node raising** — deferred. Produces awkward comma-bracketed output in rendered prose.
- **Gapping across different aux forms** (mixing "was" and "were") — we gap only when the aux matches exactly.
- **Gapping across different template keys** — limited to same-key runs, because the shared-anchor detection via `split_subject_and_rest` is only reliable when the verb phrasing is identical.
- **Spanish / German gapping** — gapping is universal but the subject-aux-predicate split helpers (AUX_PREFIXES etc.) are English-specific. Language-aware gapping would require per-language anchor detection. Future work.

---

## Phase 0 — Baseline

```bash
cargo test --all-features
cargo clippy --all-features -- -D warnings
```

Expected: 955 tests passing.

---

## Phase 1 — `split_subject_and_rest` + `longest_common_prefix_len` + `reduce_gapping`

Add the three free functions in `engine.rs` near the existing `reduce_same_entity_clauses`.

### Tests (unit tests in `engine.rs`)

```rust
#[test]
fn reduce_gapping_two_events() {
    let ss = vec![
        "Foo was moved to core".to_string(),
        "Bar was moved to util".to_string(),
    ];
    let out = reduce_gapping(&ss).unwrap();
    assert_eq!(out, "Foo was moved to core, and Bar to util.");
}

#[test]
fn reduce_gapping_three_events() {
    let ss = vec![
        "Foo was moved to core".to_string(),
        "Bar was moved to util".to_string(),
        "Baz was moved to api".to_string(),
    ];
    let out = reduce_gapping(&ss).unwrap();
    assert_eq!(
        out,
        "Foo was moved to core, Bar to util, and Baz to api."
    );
}

#[test]
fn reduce_gapping_rejects_single() {
    let ss = vec!["Foo was moved to core".to_string()];
    assert!(reduce_gapping(&ss).is_none());
}

#[test]
fn reduce_gapping_rejects_short_anchor() {
    let ss = vec![
        "Foo was moved".to_string(),
        "Bar was modified".to_string(),
    ];
    // Anchor = ["was"] — length 1, below threshold.
    assert!(reduce_gapping(&ss).is_none());
}

#[test]
fn reduce_gapping_rejects_embedded_clause() {
    let ss = vec![
        "Foo was moved, affecting 3 consumers, to core".to_string(),
        "Bar was moved to util".to_string(),
    ];
    assert!(reduce_gapping(&ss).is_none());
}

#[test]
fn reduce_gapping_rejects_identical_sentences() {
    let ss = vec![
        "Foo was moved to core".to_string(),
        "Foo was moved to core".to_string(),
    ];
    // Identical subjects — no gapping.
    assert!(reduce_gapping(&ss).is_none());
}

#[test]
fn reduce_gapping_rejects_empty_suffix() {
    let ss = vec![
        "Foo was moved".to_string(),
        "Bar was moved".to_string(),
    ];
    // Anchor consumes everything; no divergent tail.
    assert!(reduce_gapping(&ss).is_none());
}
```

---

## Phase 2 — `find_gapping_run` + `render_batch` integration

1. Add method per design.
2. In `render_batch` (and `RenderIter::next`), after the `find_same_action_run` branch, check `find_gapping_run`. If it returns a multi-event run, render each sentence individually and call `reduce_gapping`. Fall back to concatenated sentences if reduction returns None.
3. Same integration in `RenderIter::next` so iterator-based callers get the same benefit.

### Tests (integration via Engine)

```rust
#[test]
fn render_batch_applies_gapping_when_objects_differ() {
    let mut engine = test_engine();
    engine.register_template(
        "code.moved",
        "{name} was moved to {new_location}",
    ).unwrap();

    let make = |name: &str, loc: &str| {
        let mut c = Context::new();
        c.insert("entity_type", Value::String("class".into()));
        c.insert("name", Value::String(name.into()));
        c.insert("new_location", Value::String(loc.into()));
        c
    };

    let events = vec![
        ("code.moved", make("Foo", "core")),
        ("code.moved", make("Bar", "util")),
        ("code.moved", make("Baz", "api")),
    ];

    let mut s = Session::new();
    let out = engine.render_batch(&mut s, &events).unwrap();
    assert_eq!(
        out,
        "Foo was moved to core, Bar to util, and Baz to api."
    );
}

#[test]
fn render_batch_gapping_does_not_apply_when_objects_match() {
    // Same template + same non-subject slots → subject aggregation wins,
    // not gapping. Verifies we don't regress the aggregated-subjects path.
    let mut engine = test_engine();
    engine.register_template(
        "code.moved",
        "{name} was moved to {new_location}",
    ).unwrap();

    let make = |name: &str| {
        let mut c = Context::new();
        c.insert("entity_type", Value::String("class".into()));
        c.insert("name", Value::String(name.into()));
        c.insert("new_location", Value::String("core".into()));
        c
    };

    let events = vec![
        ("code.moved", make("Foo")),
        ("code.moved", make("Bar")),
    ];

    let mut s = Session::new();
    let out = engine.render_batch(&mut s, &events).unwrap();
    // Subject aggregation path: "Foo and Bar were moved to core" (plural agreement).
    assert!(
        out.contains("Foo and Bar") && out.contains("core"),
        "got: {out}"
    );
    // NOT gapping-style output:
    assert!(!out.contains(", and Bar to "), "got: {out}");
}
```

---

## Phase 3 — `render_iter` parallel integration

Mirror the gapping branch in `RenderIter::next`. Use the same helpers. Add one iterator test.

```rust
#[test]
fn render_iter_applies_gapping() {
    // Identical setup to render_batch_applies_gapping_when_objects_differ,
    // but consumed via .render_iter().collect().
    let mut engine = test_engine();
    engine.register_template(
        "code.moved",
        "{name} was moved to {new_location}",
    ).unwrap();

    let events: Vec<_> = /* ... */;
    let mut s = Session::new();

    let collected: Result<Vec<_>, _> = engine.render_iter(&mut s, &events).collect();
    let collected = collected.unwrap();
    // One sentence emitted for the gapped run.
    assert_eq!(collected.len(), 1);
    assert_eq!(collected[0], "Foo was moved to core, Bar to util, and Baz to api.");
}
```

---

## Phase 4 — Final verification

```bash
cargo test --all-features
cargo test --no-default-features
cargo test --features serde
cargo clippy --all-features -- -D warnings
cargo doc --all-features --no-deps
```

Test count up by ~15–20.

**Commit:** `Add gapping ellipsis for same-action/different-object event runs`

---

## Definition of done

- [ ] `reduce_gapping` free function with all guards (anchor len, distinct subjects, non-empty suffixes, no embedded clauses)
- [ ] `find_gapping_run` method detects eligible runs not already captured by `find_same_action_run`
- [ ] `render_batch` and `RenderIter::next` both wire up the gapping path
- [ ] 7 unit tests of `reduce_gapping`, 2+ integration tests at `render_batch` level, 1 at `render_iter` level
- [ ] All 955 existing tests still pass
- [ ] `cargo clippy --all-features -- -D warnings` clean
- [ ] One commit: `Add gapping ellipsis for same-action/different-object event runs`
