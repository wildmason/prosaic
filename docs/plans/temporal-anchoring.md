# Plan: Temporal anchoring across paragraphs

**Owner:** sonnet agent
**Scope:** `prosaic-core/src/{time.rs, session.rs, engine.rs, language.rs, document.rs}` + `prosaic-derive/src/lib.rs` (pipe whitelist) + `prosaic-grammar-es/src/lib.rs` + `prosaic-grammar-de/src/lib.rs`
**Estimated size:** ~400 LOC including tests
**Test gate:** 895 baseline tests still pass; ~15-20 new
**Branch discipline:** local only, one commit

---

## Why

The existing `{timestamp|relative}` pipe renders absolute-relative phrases ("3 days ago"). That's great for standalone timestamps but terrible for narratives:

> The class Foo was modified 3 days ago. 3 days ago, Foo was moved. 3 days ago, its signature changed.

What narrative prose wants:

> The class Foo was modified 3 days ago. **Later that day**, Foo was moved. **The next morning**, its signature changed.

Each event's temporal framing is *relative to the previous event in the narrative*, not absolute. This is **temporal anchoring**: carry the previous event's timestamp forward and compute the delta.

The anchor must survive `session.reset()` between paragraphs, because narratives span paragraphs ("The initial deploy failed. *Two weeks later*, the team ...").

## Design

### New state: `session.last_temporal_anchor`

```rust
// In session.rs:
pub struct Session {
    pub(crate) discourse: DiscourseState,
    pub(crate) round_robin_counters: HashMap<String, AtomicUsize>,
    /// Unix-seconds timestamp of the most recently-rendered event. Used by
    /// the `{timestamp|since_last}` pipe to compute inter-event deltas
    /// ("the next day", "moments later"). Persists across
    /// [`Session::reset`] so narratives can span paragraphs. Starts as
    /// `None`; set automatically whenever an event's context contains a
    /// `timestamp` slot. Call [`Session::reset_temporal`] to clear it.
    pub(crate) last_temporal_anchor: Option<i64>,
}

impl Session {
    pub fn reset(&mut self) {
        self.discourse.reset();
        self.round_robin_counters.clear();
        // NOTE: last_temporal_anchor survives so narratives span paragraphs.
    }

    /// Clear the temporal anchor. Use when starting a temporally-disjoint
    /// narrative in the same session.
    pub fn reset_temporal(&mut self) {
        self.last_temporal_anchor = None;
    }
}
```

### Automatic anchor updates

At the top of `Engine::render`, after we confirm a template exists but before rendering, check whether the context contains a `timestamp` slot with a numeric value. Note its value. After a successful render, update `session.last_temporal_anchor` to that value.

**Important:** we update *after* a successful render (so a render error doesn't leave a half-updated anchor).

The timestamp is recorded regardless of whether the template actually uses the `since_last` pipe — the anchor is a property of the rendered event, not of the template.

### `{timestamp|since_last}` pipe

Reads `session.last_temporal_anchor`, computes `current_timestamp - anchor`, formats via `Language::since_last_marker(diff_secs)`.

If `session.last_temporal_anchor` is `None` (first event in the narrative), fall back to the `relative` pipe behavior (anchored to `engine.reference_time` / `SystemTime::now()`).

### `Language::since_last_marker` trait method

```rust
// Default English:
fn since_last_marker(&self, diff_secs: i64) -> String {
    use crate::time::format_since_last;
    format_since_last(diff_secs)
}
```

### `format_since_last` free function (in `time.rs`)

```rust
/// Format a **positive-is-later** inter-event delta in seconds as a
/// narrative inter-event phrase.
///
/// Use when you want "the next day" / "moments later" style prose rather
/// than "3 days ago" style absolute relative phrases.
///
/// Zero or negative input (same moment or earlier) returns
/// `"at the same time"` — the caller is responsible for ordering.
pub fn format_since_last(diff_secs: i64) -> String {
    if diff_secs <= 0 {
        return "at the same time".to_string();
    }

    if diff_secs < 60 {
        return "moments later".to_string();
    }

    if diff_secs < HOUR {
        let n = (diff_secs + MINUTE / 2) / MINUTE;
        let n = n.max(1);
        return match n {
            1 => "a minute later".to_string(),
            _ => format!("{n} minutes later"),
        };
    }

    if diff_secs < DAY {
        let n = (diff_secs + HOUR / 2) / HOUR;
        let n = n.max(1);
        // Below 6h → "N hours later", 6h..DAY → "later that day".
        if n < 6 {
            return match n {
                1 => "an hour later".to_string(),
                _ => format!("{n} hours later"),
            };
        }
        return "later that day".to_string();
    }

    if diff_secs < 2 * DAY {
        return "the next day".to_string();
    }

    if diff_secs < WEEK {
        let n = diff_secs / DAY;
        return format!("{n} days later");
    }

    if diff_secs < 2 * WEEK {
        return "the following week".to_string();
    }

    if diff_secs < MONTH {
        let n = diff_secs / WEEK;
        return format!("{n} weeks later");
    }

    if diff_secs < 2 * MONTH {
        return "the following month".to_string();
    }

    if diff_secs < YEAR {
        let n = diff_secs / MONTH;
        return format!("{n} months later");
    }

    if diff_secs < 2 * YEAR {
        return "the following year".to_string();
    }

    let n = diff_secs / YEAR;
    format!("{n} years later")
}
```

### Spanish / German overrides

In `prosaic-grammar-es/src/lib.rs` add `Language::since_last_marker` returning Spanish:
- "momentos después" (moments later)
- "un minuto después" / "N minutos después"
- "una hora después" / "N horas después"
- "más tarde ese día" (later that day)
- "al día siguiente" (the next day)
- "N días después"
- "la semana siguiente"
- "N semanas después"
- "el mes siguiente"
- "N meses después"
- "el año siguiente"
- "al mismo tiempo" (at the same time)

In `prosaic-grammar-de/src/lib.rs`:
- "einen Augenblick später"
- "eine Minute später" / "N Minuten später"
- "eine Stunde später" / "N Stunden später"
- "später am Tag"
- "am nächsten Tag"
- "N Tage später"
- "in der folgenden Woche"
- "N Wochen später"
- "im folgenden Monat"
- "N Monate später"
- "im folgenden Jahr"
- "zur gleichen Zeit"

### Pipe dispatch in engine

In the existing `apply_pipe` match (which today has `"relative"`), add `"since_last"`:

```rust
"since_last" => self.pipe_since_last(value),
```

New method:
```rust
fn pipe_since_last(&mut self, value: &Value) -> Result<Value, ProsaicError> {
    let Some(ts) = value.as_i64() else {
        return Err(ProsaicError::PipeError {
            pipe: "since_last".to_string(),
            reason: "expected numeric Unix-seconds timestamp".to_string(),
        });
    };

    let marker = match self.session.last_temporal_anchor {
        Some(anchor) => self.engine.language.since_last_marker(ts - anchor),
        None => {
            // Fall back to absolute-relative behavior anchored at now/reference_time.
            let now = self.engine.reference_time.unwrap_or_else(system_now_secs);
            crate::time::format_relative(now - ts)
        }
    };

    Ok(Value::String(marker))
}
```

The fall-back behavior means the FIRST event in a narrative reads like `{timestamp|relative}` ("3 days ago"), and SUBSEQUENT events read like anchored deltas ("the next day"). That's exactly the narrative pattern.

### Engine::render — update anchor after rendering

```rust
pub fn render(&self, session: &mut Session, key: &str, ctx: &Context) -> Result<String, ProsaicError> {
    // ... existing render logic ...

    // After a successful render, update the temporal anchor if this event
    // has a timestamp.
    if let Some(Value::Number(ts)) = ctx.get("timestamp") {
        session.last_temporal_anchor = Some(*ts);
    }

    Ok(rendered)
}
```

### Derive crate pipe whitelist

Add `"since_last"` to `VALID_PIPES` in `prosaic-derive/src/lib.rs`.

### Out of scope

- **Pipe arguments for custom anchor threshold tuning.** Fixed thresholds are fine for v1.
- **Time-zone-aware phrasing** (e.g., "yesterday morning" vs "this morning"). Wall-clock-agnostic.
- **Pipe chaining into `capitalize`** — the caller can do `{ts|since_last|capitalize}` if they want "The next day, Foo was ..." at sentence start. This works automatically since `capitalize` exists.

---

## Phase 0 — Baseline

```bash
cargo test --all-features
cargo clippy --all-features -- -D warnings
```

Expected: 895 tests passing.

---

## Phase 1 — `format_since_last` + tests in `time.rs`

Implement the function. Tests cover every branch:
- 0 or negative → "at the same time"
- <60 → "moments later"
- 60, 120, 3599 → minutes ("a minute later", "2 minutes later")
- 3600, 5*HOUR → hours
- 6*HOUR, 12*HOUR, 23*HOUR → "later that day"
- DAY + 1 → "the next day"
- 3*DAY → "3 days later"
- WEEK + 1, 13*DAY → "the following week"
- 3*WEEK → "3 weeks later"
- MONTH + 1 → "the following month"
- 3*MONTH → "3 months later"
- YEAR + 1 → "the following year"
- 3*YEAR → "3 years later"

---

## Phase 2 — `Session.last_temporal_anchor` + `reset_temporal`

1. Add the field with default `None`.
2. Update `Session::new`, `Clone` impl.
3. Modify `Session::reset` to leave `last_temporal_anchor` untouched (add comment explaining why).
4. Add `Session::reset_temporal`.

### Tests

```rust
#[test]
fn session_reset_preserves_temporal_anchor() {
    let mut s = Session::new();
    s.last_temporal_anchor = Some(1_700_000_000);
    s.reset();
    assert_eq!(s.last_temporal_anchor, Some(1_700_000_000));
}

#[test]
fn session_reset_temporal_clears_anchor() {
    let mut s = Session::new();
    s.last_temporal_anchor = Some(1_700_000_000);
    s.reset_temporal();
    assert_eq!(s.last_temporal_anchor, None);
}
```

---

## Phase 3 — `Language::since_last_marker` trait method

Default impl delegates to `format_since_last`. Add Spanish + German overrides per design.

### Tests
- In `language.rs`: 3–4 representative deltas asserting English text.
- In `prosaic-grammar-es`: 2–3 asserting Spanish text.
- In `prosaic-grammar-de`: 2–3 asserting German text.

---

## Phase 4 — `{timestamp|since_last}` pipe + anchor auto-update

1. Register `"since_last"` in `apply_pipe` dispatch.
2. Implement `pipe_since_last` per design.
3. Update `Engine::render` to set `session.last_temporal_anchor` after a successful render when `ctx.get("timestamp")` is a `Value::Number`.
4. Add `"since_last"` to `prosaic-derive/src/lib.rs::VALID_PIPES`.

### Tests

```rust
#[test]
fn since_last_first_event_falls_back_to_relative() {
    let now = 1_700_000_000;
    let mut engine = test_engine().reference_time(now);
    engine.register_template("t", "{ts|since_last}").unwrap();
    let mut s = Session::new();
    let mut ctx = Context::new();
    ctx.insert("ts", Value::Number(now - 3 * 86400)); // 3 days ago
    ctx.insert("timestamp", Value::Number(now - 3 * 86400));
    let out = engine.render(&mut s, "t", &ctx).unwrap();
    assert!(out.contains("3 days ago"), "got: {out}");
}

#[test]
fn since_last_subsequent_event_uses_anchor() {
    let now = 1_700_000_000;
    let mut engine = test_engine().reference_time(now);
    engine.register_template("t", "{ts|since_last}").unwrap();
    let mut s = Session::new();

    // First event sets the anchor.
    let mut c1 = Context::new();
    let t1 = now - 3 * 86400;
    c1.insert("ts", Value::Number(t1));
    c1.insert("timestamp", Value::Number(t1));
    engine.render(&mut s, "t", &c1).unwrap();

    // Second event, one day later.
    let mut c2 = Context::new();
    let t2 = t1 + 86400;
    c2.insert("ts", Value::Number(t2));
    c2.insert("timestamp", Value::Number(t2));
    let out = engine.render(&mut s, "t", &c2).unwrap();
    assert!(out.contains("the next day"), "got: {out}");
}

#[test]
fn since_last_survives_session_reset() {
    let now = 1_700_000_000;
    let mut engine = test_engine().reference_time(now);
    engine.register_template("t", "{ts|since_last}").unwrap();
    let mut s = Session::new();

    let mut c1 = Context::new();
    let t1 = now - 3 * 86400;
    c1.insert("ts", Value::Number(t1));
    c1.insert("timestamp", Value::Number(t1));
    engine.render(&mut s, "t", &c1).unwrap();

    s.reset(); // Reset discourse, but NOT temporal.
    assert_eq!(s.last_temporal_anchor, Some(t1));

    let mut c2 = Context::new();
    let t2 = t1 + 86400;
    c2.insert("ts", Value::Number(t2));
    c2.insert("timestamp", Value::Number(t2));
    let out = engine.render(&mut s, "t", &c2).unwrap();
    assert!(out.contains("the next day"), "got: {out}");
}

#[test]
fn since_last_reset_temporal_restarts_narrative() {
    let now = 1_700_000_000;
    let mut engine = test_engine().reference_time(now);
    engine.register_template("t", "{ts|since_last}").unwrap();
    let mut s = Session::new();

    let mut c1 = Context::new();
    let t1 = now - 3 * 86400;
    c1.insert("ts", Value::Number(t1));
    c1.insert("timestamp", Value::Number(t1));
    engine.render(&mut s, "t", &c1).unwrap();
    s.reset_temporal();

    let mut c2 = Context::new();
    let t2 = t1 + 86400;
    c2.insert("ts", Value::Number(t2));
    c2.insert("timestamp", Value::Number(t2));
    let out = engine.render(&mut s, "t", &c2).unwrap();
    assert!(out.contains("2 days ago"), "got: {out}"); // now - t2 = 2 days
}
```

### Additional cross-paragraph test in `document.rs`

```rust
#[test]
fn document_plan_temporal_anchor_spans_paragraphs() {
    let mut engine = test_engine();
    engine.register_template("t", "{name} changed {ts|since_last}").unwrap();

    let t1 = 1_700_000_000;
    let t2 = t1 + 86400;

    let mut c1 = ctx_with_entity("Foo", 1);
    c1.insert("ts", Value::Number(t1));
    c1.insert("timestamp", Value::Number(t1));

    let mut c2 = ctx_with_entity("Bar", 1);
    c2.insert("ts", Value::Number(t2));
    c2.insert("timestamp", Value::Number(t2));

    let events = vec![("t", c1), ("t", c2)];
    let plan = DocumentPlan::from_events(&events, &engine);
    let mut s = Session::new();
    let out = plan.render(&engine, &mut s).unwrap();

    // Two paragraphs (different entities), but the temporal anchor
    // threads through the session.reset() between them — so Bar's
    // paragraph says "the next day", not "9 hours ago" (or whatever
    // the absolute-now-based phrase would be).
    assert!(out.contains("the next day"), "got: {out}");
}
```

---

## Phase 5 — Final verification

```bash
cargo test --all-features
cargo test --no-default-features
cargo test --features serde
cargo clippy --all-features -- -D warnings
cargo doc --all-features --no-deps
```

Test count up by ~15–20.

**Commit:** `Add temporal anchoring via since_last pipe, survives session reset`

---

## Definition of done

- [ ] `format_since_last` in `time.rs` with full range coverage
- [ ] `Session.last_temporal_anchor` field, survives `reset()`, cleared by `reset_temporal()`
- [ ] `Session::Clone` threads the anchor
- [ ] `Language::since_last_marker` trait method with English default + ES/DE overrides
- [ ] `{timestamp|since_last}` pipe: uses anchor when set, falls back to `format_relative` when unset
- [ ] Anchor auto-updates after every successful render that carries a `timestamp` slot
- [ ] `"since_last"` added to `prosaic-derive::VALID_PIPES`
- [ ] Narrative spans paragraphs — cross-paragraph temporal marker test passes
- [ ] `cargo test --all-features` passes, count up by 15–20
- [ ] `cargo clippy --all-features -- -D warnings` clean
- [ ] One commit: `Add temporal anchoring via since_last pipe, survives session reset`
