# Plan: Buffer Flatten — Single-Buffer Render Pipeline

**Owner:** sonnet agent
**Scope:** `nlg-core/src/` only — `engine.rs`, `length.rs`, `punctuation.rs`, any helper touched
**Estimated size:** ~400–600 LOC touched (mostly signatures + small refactors)
**Test gate:** all 376 tests pass after every sub-phase; zero new warnings
**Branch discipline:** local only, commit at each sub-phase checkpoint

---

## Why

Today `render_tx` produces `String`, then sequentially passes it through several post-processing helpers that each allocate a fresh `String`:

```
render_template → output: String          (1 alloc)
prepend_replacing_subject → new String    (1 alloc)
format!("{conn} …", lowercase_first(&output)) → new String (1+ allocs)
capitalize_first(&output) → new String    (1 alloc)
cleanup_artifacts(&output, strictness) → new String (1+ allocs internally)
terminate_sentence(&output) → new String  (1 alloc)
split_long(&output, max) → new String     (1+ allocs, recursive)
smart_quotes(&output) → new String        (1 alloc)
```

Plus `render_slot` inside `render_template` allocates per slot for pipe-applied values.

The total per render is 8–20 `String` allocations depending on post-processing configuration. Flattening to a single owned `String` buffer threaded by `&mut` cuts this to **one allocation per render** (plus however many the rendered content genuinely requires for pipe outputs).

Expected delta: **2–5× wall-clock on `render_single_rename_medium`**, larger on polish-heavy workloads.

This also unlocks:
- **`itoa`/`ryu` direct-format** on numeric slots — only cheap once you're writing into a buffer.
- **Streaming `render_into(w: impl fmt::Write)`** — trivial once the internal path is already `fmt::Write`-shaped.
- **PARENT faithfulness** — the interner lookups are `&str`-based; having the output in one contiguous buffer lets us tokenize over it without extra allocation.

## Non-goals (do not do these here)

- **Do not** add `itoa` / `ryu` dep. Mentioned as a follow-up; separate plan.
- **Do not** add a public `render_into` API. The internal pipeline becomes buffer-based, but the public surface still returns `String`. Streaming is a separate plan.
- **Do not** change `DiscourseState`, `Session`, or any discourse internals.
- **Do not** change the polish-pass algorithms — only their signatures and allocation behaviour.
- **Do not** change `Template` parsing or pipe application logic.
- **Do not** change any public API. External callers see no difference.
- **Do not** introduce `unsafe`.

## Success criteria

1. All 376 tests pass on `--all-features`, `--no-default-features`, `--features serde` after every sub-phase.
2. `cargo clippy --all-features -- -D warnings` clean after every sub-phase.
3. `cargo bench --bench engine -- --test` runs successfully at end.
4. Zero public API changes.
5. `Engine: Send + Sync` compile-time assert still passes.
6. Internal post-processing helpers operate in-place (take `&mut String`) or consume-and-return for recursive cases; no gratuitous intermediate `String` allocations.
7. 4 commits, one per sub-phase.

---

## Phase 0 — Baseline

```bash
cargo test --all-features
cargo clippy --all-features -- -D warnings
cargo bench --bench engine -- --test
```

Expected: **376 passing, 0 failing, 0 warnings**, benches compile and run.

Optional: `cargo bench --bench engine` for a real baseline; save stdout to `docs/plans/.buffer-flatten-baseline.txt` for before/after comparison. Skip if it takes >2 min.

**No commit.**

---

## Phase 1 — Convert `cleanup_artifacts` and `terminate_sentence` to in-place

**File:** `nlg-core/src/engine.rs`

**Current signatures (approximate):**

```rust
fn cleanup_artifacts(input: &str, strictness: Strictness) -> String { ... }
fn terminate_sentence(input: &str) -> String { ... }
```

**New signatures:**

```rust
fn cleanup_artifacts_in_place(output: &mut String, strictness: Strictness) { ... }
fn terminate_sentence_in_place(output: &mut String) { ... }
```

### 1.1 `cleanup_artifacts_in_place`

Today this function:
1. Collapses runs of whitespace to single spaces
2. Strips orphan conjunctions ("and", "but", "or") left by missing slots in `Silent` mode
3. Strips dangling trailing prepositions (e.g., "was modified by " → "was modified")

All three passes can mutate in place. Two approaches:

**Approach A (simpler):** do the whole thing with two `String` swaps — not a win. Skip.

**Approach B (preferred):** use `String::retain` and `String::truncate` primitives, plus a small byte-span stripper for the orphan-conjunction pattern.

- **Whitespace collapse:** use a single-pass retain with a "saw-space" flag:
  ```rust
  let mut last_was_space = false;
  output.retain(|c| {
      if c.is_ascii_whitespace() {
          if last_was_space { return false; }
          last_was_space = true;
          true
      } else {
          last_was_space = false;
          true
      }
  });
  ```
  (Note: for Unicode correctness, use `c.is_whitespace()` — keep whatever the current code uses.)

- **Trailing preposition strip:** check the end of the string against a known-prep list, trim with `output.truncate(new_len)`.

- **Orphan conjunction strip (Silent mode only):** needs a scan for patterns like ", and ," or " and " before EOS. This may require rebuilding into a buffer — if so, do it once:
  ```rust
  let mut tmp = String::with_capacity(output.len());
  // scan and copy into tmp, skipping orphans
  std::mem::swap(output, &mut tmp);
  ```
  The one swap is acceptable; the alternative (many in-place truncations) is harder to get right.

Preserve **all current behaviour**. These tests must still pass unchanged:
- `silent_strips_orphan_conjunction`
- `silent_strips_dangling_preposition_with_punct`
- `silent_strips_trailing_dangling_preposition`
- `silent_strips_chained_orphans`
- `silent_preserves_content_when_orphans_would_empty_output`
- `whitespace_collapsing_runs_regardless_of_strictness`

### 1.2 `terminate_sentence_in_place`

Current: trims trailing whitespace/punctuation, then pushes a `.` if not already terminated.

New:
```rust
fn terminate_sentence_in_place(output: &mut String) {
    // Trim trailing whitespace.
    let trimmed_len = output.trim_end().len();
    output.truncate(trimmed_len);
    // If already terminated, stop.
    if output.ends_with(|c: char| matches!(c, '.' | '!' | '?')) { return; }
    output.push('.');
}
```

### 1.3 Update call site in `render_tx`

```rust
let mut output = self.render_template(...)?;
// ... prepend connective handling (unchanged for now — Phase 3 tackles that)
cleanup_artifacts_in_place(&mut output, self.strictness);
terminate_sentence_in_place(&mut output);
```

The old `output = cleanup_artifacts(&output, ...)` and `output = terminate_sentence(&output)` lines go away.

### 1.4 Delete the old non-in-place versions

Once no call sites remain, delete `cleanup_artifacts` and `terminate_sentence`. No `#[deprecated]` shims — local-only project, full cutover.

### 1.5 Verify

```bash
cargo test --all-features
cargo clippy --all-features -- -D warnings
```

**Commit:** `Convert cleanup_artifacts and terminate_sentence to in-place`

---

## Phase 2 — Convert `split_long` and `smart_quotes` to in-place

**Files:** `nlg-core/src/length.rs`, `nlg-core/src/punctuation.rs`

### 2.1 `smart_quotes_in_place`

Current: builds a new `String` char-by-char via a state machine.

New: this is harder to do purely in-place because quote chars can swap from `'` (1 byte) to U+2019 (3 bytes in UTF-8). The string length changes per substitution.

**Approach:** keep an internal buffer for the transformed output, then swap. It still saves an allocation vs the current `smart_quotes(&str) -> String` because the outer `output` buffer is reused across renders (future-wise when Session pooling is added) — but more importantly the *call site* stops doing `output = smart_quotes(&output)` and switches to `smart_quotes_in_place(&mut output)` which drops one intermediate binding.

```rust
pub fn smart_quotes_in_place(output: &mut String) {
    // State-machine identical to current smart_quotes, but the result
    // goes into a scratch String, then is swapped into `output`.
    let mut scratch = String::with_capacity(output.len());
    // ... same state machine writing into scratch ...
    std::mem::swap(output, &mut scratch);
}
```

(Not a dramatic win on its own, but it's consistent with the flattened call chain and removes one allocation per render when smart_quotes is enabled — the `scratch` local is gone after the swap, `output`'s old allocation is dropped, net is one allocation instead of two.)

If time permits and the improvement is worth it: write truly in-place via `String::replace_range` + careful index tracking. Probably not worth the bug risk. Ship the swap version.

### 2.2 `split_long_in_place`

Current: recursive, builds a `format!("{head}. {tail_final}")` at each split.

New signature:
```rust
pub fn split_long_in_place(output: &mut String, max_chars: usize) {
    // Recursive logic same as before, but operates on the buffer in place.
    // Use String::split_at / truncate / push_str instead of format!.
}
```

Implementation sketch:
```rust
pub fn split_long_in_place(output: &mut String, max_chars: usize) {
    if output.chars().count() <= max_chars { return; }

    // Find split point — same logic as today.
    let (split_at, tail_start, kind) = match find_split(output, max_chars) {
        Some(x) => x,
        None => return,
    };

    // Split the tail off as an owned String.
    let tail = output.split_off(tail_start);
    // Trim the head in place.
    output.truncate(split_at);
    let head_trimmed_len = output.trim_end_matches([',', ' ']).len();
    output.truncate(head_trimmed_len);

    // Rewrite the tail (this allocates one fresh String for the tail text).
    let tail_rewritten = rewrite_tail(tail.trim_start(), kind);

    // Recurse on the tail to handle further splits.
    let mut tail_buffer = tail_rewritten;
    split_long_in_place(&mut tail_buffer, max_chars);

    // Append ". " + rewritten tail back onto output.
    output.push('.');
    output.push(' ');
    output.push_str(&tail_buffer);
}
```

The recursive case still allocates the rewritten-tail String — that's inherent to the algorithm, we need to rewrite the prefix. Net improvement: the outer `format!` is gone; overall split_long now does one allocation per split level instead of two.

### 2.3 Update call site in `render_tx`

```rust
#[cfg(feature = "polish")]
if let Some(max_chars) = self.max_sentence_length {
    split_long_in_place(&mut output, max_chars);
}

#[cfg(feature = "polish")]
if self.smart_quotes {
    smart_quotes_in_place(&mut output);
}
```

### 2.4 Delete old versions or leave as thin shims?

- **Public helpers in `length.rs` / `punctuation.rs`:** current `split_long(&str, usize) -> String` and `smart_quotes(&str) -> String` are `pub` because they're re-exported at the crate root. External callers may depend on them.

  **Check `nlg-core/src/lib.rs` re-exports** — if `split_long` or `smart_quotes` are re-exported, keep the `&str → String` forms as thin wrappers:
  ```rust
  pub fn split_long(input: &str, max_chars: usize) -> String {
      let mut s = input.to_string();
      split_long_in_place(&mut s, max_chars);
      s
  }
  ```
  If NOT re-exported, delete the old forms.

- **All in-place versions (`*_in_place`) are `pub(crate)`.** Don't expose them publicly; they're an implementation detail.

### 2.5 Tests

The `length::tests::*` and `punctuation::tests::*` tests in those modules exercise the old `&str → String` forms. Keep those tests passing — they validate the wrapper (or the in-place version directly, whichever you expose).

If you remove the public wrappers, update the tests to use the in-place forms.

### 2.6 Verify

```bash
cargo test --all-features
cargo clippy --all-features -- -D warnings
```

**Commit:** `Convert split_long and smart_quotes to in-place operation`

---

## Phase 3 — Collapse `prepend_replacing_subject` / `lowercase_first` / `capitalize_first` into in-place mutations

**File:** `nlg-core/src/engine.rs`

**Current state:** `render_tx` does

```rust
if let Some(conn) = connective {
    if conn.starts_with("It ") {
        output = prepend_replacing_subject(&output, conn);
    } else {
        output = format!("{conn} {}", lowercase_first(&output));
    }
}
if starts_with_refer_pipe(template) {
    output = capitalize_first(&output);
}
```

Each branch reassigns `output` to a fresh `String`.

**New:**

```rust
fn lowercase_first_in_place(output: &mut String) { ... }
fn capitalize_first_in_place(output: &mut String) { ... }
fn prepend_replacing_subject_in_place(output: &mut String, conn: &str) { ... }
```

### 3.1 `capitalize_first_in_place`

UTF-8 safe version:
```rust
fn capitalize_first_in_place(output: &mut String) {
    let first = match output.chars().next() {
        Some(c) if c.is_lowercase() => c,
        _ => return,
    };
    let first_len = first.len_utf8();
    let upper: String = first.to_uppercase().collect();
    // Replace the first codepoint with its uppercase form.
    output.replace_range(0..first_len, &upper);
}
```

Note: in some locales a single codepoint's uppercase is multiple codepoints (e.g. ß → SS). `replace_range` handles the length change correctly.

### 3.2 `lowercase_first_in_place`

Same pattern as `capitalize_first_in_place`, mirror operation.

### 3.3 `prepend_replacing_subject_in_place`

Current: strips "The {type} {name}" prefix, prepends connective. This is a subject-replacement, not purely a prepend.

Approach:
- Find the end of the subject NP (current code has a finder).
- Replace the subject with the connective via `output.replace_range(0..end, conn)`.
- Does one allocation if the connective is longer than the subject (the `String` may reallocate its buffer) — but only one, not a whole fresh `String`.

If the in-place version is awkward, keep the consume-return form as a private helper and swap:
```rust
let new = prepend_replacing_subject(&output, conn);
output = new;
```
— but that's no better than today. Prefer the in-place approach.

### 3.4 Update call site

```rust
if let Some(conn) = connective {
    if conn.starts_with("It ") {
        prepend_replacing_subject_in_place(&mut output, conn);
    } else {
        lowercase_first_in_place(&mut output);
        // Insert "{conn} " at the start.
        let mut buf = String::with_capacity(conn.len() + 1 + output.len());
        buf.push_str(conn);
        buf.push(' ');
        buf.push_str(&output);
        std::mem::swap(&mut output, &mut buf);
    }
}
if starts_with_refer_pipe(template) {
    capitalize_first_in_place(&mut output);
}
```

The "prepend {conn} " case still allocates once because prepending to a `String` is O(n) anyway — using `insert_str(0, ...)` would be the same cost. The swap-into-new-buf form is explicit and clear; leave it.

### 3.5 Verify

```bash
cargo test --all-features
cargo clippy --all-features -- -D warnings
```

**Commit:** `Convert subject-prepend, lowercase_first, capitalize_first to in-place`

---

## Phase 4 — Flatten `render_template` and `render_slot` to write into `&mut String`

**File:** `nlg-core/src/engine.rs`

**This is the biggest change of the plan.** `render_template` currently builds an owned `String`; `render_slot` returns `String` per slot.

### 4.1 Rewrite `render_template` to append into a caller-supplied buffer

New internal signature:
```rust
fn render_template_into(
    &self,
    session: &mut Session,
    out: &mut String,
    key: &str,
    template: &Template,
    context: &Context,
) -> Result<(), NlgError> { ... }
```

The public-facing `render_template` (currently private, returns `String`) is replaced. Internal callers:
- `render_tx` — build an empty String, call `render_template_into(&mut s, ...)`, then run the post-processing in-place.
- `score_variants` — same pattern. Each candidate render uses a fresh buffer (or a reused one via `buf.clear()` + reuse between candidates — nice-to-have; keep simple for v1).
- `render_inline` — same pattern.

### 4.2 Rewrite `render_slot` to append into the buffer

Current signature (approximate):
```rust
fn render_slot(&self, session: &mut Session, slot: &Slot, ctx: &Context) -> Result<String, NlgError> { ... }
```

New:
```rust
fn render_slot_into(
    &self,
    session: &mut Session,
    out: &mut String,
    slot: &Slot,
    ctx: &Context,
) -> Result<(), NlgError> { ... }
```

Pipe application gets interesting. A chained pipe expression `{list|truncate:3|join}` is today:
1. Resolve `list` to a `Value::List(Vec<String>)`.
2. Apply `truncate:3` → `Value::List(Vec<String>)` (shorter).
3. Apply `join` → `Value::String(String)`.
4. Return the final `Value::String`.

With the buffer approach, the final `Value::String` content is appended to `out`. Intermediate `Value`s still exist — pipes operate on `Value`, not on the buffer. **Do not** try to make every pipe buffer-aware; that's a rewrite of the pipe system. Keep the pipe chain running on `Value`, only the *final* step writes into `out`.

Sketch:
```rust
fn render_slot_into(...) -> Result<(), NlgError> {
    let value: Value = self.resolve_slot_value(session, slot, ctx)?;
    let rendered: Cow<str> = value.as_display();
    out.push_str(&rendered);
    Ok(())
}
```

`Value::as_display` already returns `Cow<str>` — `Value::Number` gives an owned `String`, `Value::String` gives a borrowed `&str`. For v1 this is fine; `itoa` integration (separate plan) converts `Value::Number` to a direct buffer write.

### 4.3 Segment dispatch

`render_template_into` walks `Template`'s segments:

```rust
for seg in &template.segments {
    match seg {
        Segment::Literal(s) => out.push_str(s),
        Segment::Slot(slot) => self.render_slot_into(session, out, slot, ctx)?,
        Segment::Conditional { key, inner } => {
            if ctx_key_is_truthy(ctx, key) {
                // Recurse into `inner` — requires `inner` to be a Template
                // or slice of Segments.
                self.render_template_into(session, out, key_for_error, inner, ctx)?;
            }
        }
        Segment::Partial(name) => { /* look up + recurse */ }
    }
}
```

Check the existing segment variants — conditional section, partial reference, etc. — and mirror them all.

### 4.4 Error path — failed slot mid-render

If a slot errors out, the buffer may contain a partial sentence. That's fine — the caller in `render_tx` snapshots the `Session` BEFORE calling into `render_template_into`, and on error restores the session. The caller-level `out` buffer is discarded along with the returned error (the caller owns the buffer).

Confirm: does `render_tx` do this cleanly today with snapshot/restore on the `Session`? (Yes, per the Engine/Session split.) Same discipline here — failures don't leave session garbage; the error-path buffer is discarded.

### 4.5 `render_tx` becomes:

```rust
fn render_tx(
    &self,
    session: &mut Session,
    key: &str,
    all_alternatives: &[SalientTemplate],
    context: &Context,
) -> Result<String, NlgError> {
    session.discourse.begin_render();
    // ... connective detection, salience filter, variant select — unchanged
    session.discourse.record_template_choice(key, variant_index);

    // Preallocate a reasonable-size buffer. 128 bytes covers ~95% of renders.
    let mut out = String::with_capacity(128);
    self.render_template_into(session, &mut out, key, template, context)?;

    // Connective prepending, subject-replacement, capitalize-first,
    // cleanup_artifacts_in_place, terminate_sentence_in_place,
    // split_long_in_place, smart_quotes_in_place — all in-place on `out`.
    // ... (Phase 3 code for connective / capitalize block) ...
    cleanup_artifacts_in_place(&mut out, self.strictness);
    terminate_sentence_in_place(&mut out);
    #[cfg(feature = "polish")]
    if let Some(max) = self.max_sentence_length {
        split_long_in_place(&mut out, max);
    }
    #[cfg(feature = "polish")]
    if self.smart_quotes {
        smart_quotes_in_place(&mut out);
    }

    // Record entity mention / output words on discourse.
    if let (Some(name), Some(etype)) = (&entity_name, &entity_type) {
        session.discourse.mention_entity(name, etype);
    }
    session.discourse.record_output_words(&out);

    Ok(out)
}
```

### 4.6 Delete the old `render_template` / `render_slot` once all callers are converted

No shims. Full cutover.

### 4.7 Check `score_variants`

`score_variants` iterates candidate renders. Each candidate currently calls `self.render_template(key, template, &ctx)` and gets a `String`. With the new API, each candidate writes into a reusable scratch buffer that is cleared between candidates:

```rust
let mut scratch = String::with_capacity(128);
for (i, template) in alternatives.iter().enumerate() {
    scratch.clear();
    match self.render_template_into(session, &mut scratch, key, template, &ctx) {
        Ok(()) => { /* use scratch as the candidate */ }
        Err(e) => { /* restore session, return */ }
    }
    scores.push(VariantScore { rendered: scratch.clone(), ... });
}
```

That `scratch.clone()` per candidate is a regression from "one allocation per candidate" to "one clone per candidate" — but still less than the current "fresh String per slot × slots per candidate" path. Net improvement. A future optimisation: return a `Vec<String>` by reusing buffers via drain — leave it.

### 4.8 Verify

```bash
cargo test --all-features
cargo test --no-default-features
cargo test --features serde
cargo clippy --all-features -- -D warnings
cargo bench --bench engine -- --test
```

All pass.

**Commit:** `Flatten render pipeline to a single String buffer via render_template_into`

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

Optional: `cargo bench --bench engine` for real; save stdout to `docs/plans/.buffer-flatten-after.txt`. Compare against baseline — expect **2–5× throughput improvement** on `render_single_rename_medium`. Smaller improvements elsewhere are still wins. A regression is a signal to stop and investigate.

**Report to user:** 4 commit hashes, test counts per feature variant, clippy + doc + bench status, any surprises, bench delta if measured.

---

## Risk register

| Risk | Mitigation |
|---|---|
| `render_template_into` recursion for conditional sections / partials breaks on a subtle error path | Tests will catch — the conditional section tests (`conditional_section_skipped_when_zero`, `conditional_section_rendered_when_nonzero`, `conditional_section_skipped_for_empty_list`, `conditional_section_rendered_for_nonempty_list`, `conditional_section_skipped_when_key_missing`) exercise this path thoroughly. If they fail, fix immediately. |
| `render_slot_into` needs to handle pipe application — chained pipes operate on `Value`, final value writes to buffer | Keep pipe logic operating on `Value`; only the final `as_display` writes to buffer. Do NOT rewrite pipes. |
| Error-partial buffer: slot errors leave half a sentence in `out` | Caller (`render_tx`) owns the buffer and discards it on error — nothing leaks. Session snapshot/restore is on `Session`, buffer is a local. |
| `cleanup_artifacts_in_place` regresses behaviour on orphan-conjunction edge cases (Silent mode) | Those tests are specific; run them individually: `cargo test --test integration silent_` or `cargo test silent_` to isolate. If one fails, bisect by temporarily restoring the old `cleanup_artifacts` and calling it + `swap`. |
| `split_long_in_place` recursion can stack-overflow on pathological inputs | Current recursive version has the same risk. Don't change depth behaviour. If a test input reveals stack exhaustion, convert to a loop — but that's a separate concern. |
| `smart_quotes` in-place swap drops one allocation but creates temporary scratch — net zero on first read | True, but the swap removes the `output = smart_quotes(&output)` pattern and keeps `output`'s identity. Future buffer pooling can eliminate even the scratch. For this phase, consistency of the call chain is the win. |
| Public re-exports of `split_long` / `smart_quotes` break downstream usage | Check `lib.rs` re-exports. If re-exported, keep thin wrappers. If not, delete old forms. |
| `String::with_capacity(128)` preallocation is too small — causes reallocation mid-render | 128 bytes covers the p50 render. The `String` grows automatically. Not a correctness issue. A future optimization can sniff template complexity for a better default. |
| Scratch buffer reuse in `score_variants` breaks if a candidate errors mid-render | Caller catches the error, restores the Session snapshot, and returns. Scratch state doesn't matter. |
| `replace_range` with multi-byte characters panics if indices aren't on char boundaries | `len_utf8()` on the boundary char gives the correct byte length. Test: verify with a `ß → SS` case — add a unit test if one doesn't exist. |

## What NOT to do

- **Don't** add `unsafe`. This refactor is entirely safe Rust.
- **Don't** change pipe semantics. Pipes keep running on `Value`.
- **Don't** introduce `&mut String` through *every* helper — only the ones in the post-processing chain and the template-rendering core. `render_slot_into` is the boundary.
- **Don't** amend commits.
- **Don't** skip a sub-phase's verify step.
- **Don't** pool buffers across renders in this phase. Future Session-pooling is a separate plan.
- **Don't** add `itoa` / `ryu`. Separate plan.
- **Don't** add `render_into(w: impl fmt::Write)` public API. Separate plan.

## Definition of done

- [ ] Phase 0 baseline clean; Phase 5 final verification clean
- [ ] 4 commits with specified subject lines
- [ ] `cleanup_artifacts_in_place` operates on `&mut String`
- [ ] `terminate_sentence_in_place` operates on `&mut String`
- [ ] `split_long_in_place` operates on `&mut String` (recursive, allocates one tail per split level)
- [ ] `smart_quotes_in_place` operates on `&mut String` (swap with scratch buffer)
- [ ] `prepend_replacing_subject_in_place`, `lowercase_first_in_place`, `capitalize_first_in_place` operate on `&mut String`
- [ ] `render_template_into` writes into a caller-supplied `&mut String`
- [ ] `render_slot_into` writes into the same buffer; pipe chain still runs on `Value`
- [ ] `render_tx` allocates ONE outer `String` and threads it through the entire pipeline
- [ ] All 376 tests pass on all feature variants
- [ ] `Engine: Send + Sync` assert still compiles
- [ ] No new public API surface
- [ ] No new `unsafe`
