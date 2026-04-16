# Plan: `{slot|choose: …}` Unified Lexical-Choice Pipe

**Owner:** sonnet agent
**Scope:** `nlg-core/src/engine.rs` (new pipe) + `nlg-derive/src/lib.rs` (whitelist update)
**Estimated size:** ~150–200 LOC including tests
**Test gate:** all 463 tests pass after every sub-phase; zero warnings
**Branch discipline:** local only, commit at each sub-phase

---

## Why

Unified choice semantics across three previously-separate needs:

1. **Grammatical number / select:** `{count|choose: 1="is", default="are"}` → future-proof for full CLDR plural rules without MessageFormat 2.0 block syntax.
2. **Dispatching on enum-like slot values:** `{severity|choose: critical="CRITICAL", warn="WARN", default="INFO"}`.
3. **Concept-to-word mapping:** `{action|choose: rename="renamed", delete="removed", default="changed"}` — pragmatic lexical choice without a full SynonymRegistry concept layer.

Agreed by synthesis:
- Linguistics (finding #6 — concept-group lexical choice at 3 days)
- Multilingual (cherry-picked MessageFormat 2.0 select semantics into `|` pipes instead of adopting MF2 syntax)
- Ecosystem (`nlg_template!` validator already has `choose` rejection wired as a placeholder; this plan upgrades that to acceptance)

**Out of scope for v1:** LRU-rotation concept groups (that's a SynonymRegistry upgrade, separate plan). Numeric-range matching (`0..=10`). Regex matching. Nested `choose` pipes. CLDR plural categories as first-class keys.

## Design (locked)

### Syntax

```
{slot|choose: key1=value1, key2=value2, default=fallback}
```

The pipe argument is a single string — the existing template parser already grabs everything between `:` and `|`/`}` as one string. The `|choose` pipe function parses that string at render time.

**Arg grammar (v1):**
```
arg      ::= pair ("," pair)*
pair     ::= key "=" value
key      ::= [^=,]+ (trimmed of surrounding whitespace)
value    ::= [^,]*  (trimmed of surrounding whitespace)
```

Whitespace around `=` and `,` is tolerated. Keys and values cannot contain `,` or `=` (no escape mechanism in v1 — users needing richer values can use slots instead).

### Matching semantics

1. Extract the resolved slot value via `Value::as_display()` → `Cow<str>`.
2. Trim surrounding whitespace; lowercase for matching (case-insensitive).
3. Iterate parsed pairs in insertion order. First key whose lowercased form equals the slot value emits its `value`.
4. If no pair matched, look for a pair with key `"default"` — emit its value.
5. If no default present, dispatch on strictness:
   - **Strict** → `NlgError::InvalidPipe { pipe: "choose", reason: "no matching key for value '{x}' and no default" }`
   - **Lenient** → emit `[choose: no match for {x}]` placeholder
   - **Silent** → emit empty string

### Empty / missing slot

- If the slot is missing from the context, the slot pipeline already handles it before the pipe fires (existing strictness logic). `pipe_choose` receives a resolved value or never runs.

### Return type

String. Consistent with other value-producing pipes (`|refer`, `|verb`, `|quantify`).

### Parser changes

**None.** The existing template parser grabs `choose:KEY=VAL,KEY=VAL,default=X` as a single `PipeArg::String(...)`. Commas and `=` are pass-through in the argument portion.

### Valid-pipe whitelist updates

- `nlg-derive/src/lib.rs` — add `"choose"` to `VALID_PIPES`
- Any other hardcoded pipe list in the codebase — check with grep; update all sites.

---

## Phase 0 — Baseline

```bash
cargo test --all-features
cargo clippy --all-features -- -D warnings
```

Expected: **463 tests passing**, 0 warnings.

**No commit.**

---

## Phase 1 — Implement `pipe_choose` in `engine.rs`

**File:** `nlg-core/src/engine.rs`

### 1.1 Add the pipe handler

Find `apply_pipe` (around engine.rs line 528). Add a new arm:

```rust
"choose" => self.pipe_choose(pipe, value, context),
```

Add the handler method:

```rust
fn pipe_choose(
    &self,
    pipe: &Pipe,
    value: &Value,
    _context: &Context,
) -> Result<Value, NlgError> {
    let arg_str = match &pipe.arg {
        Some(PipeArg::String(s)) => s.as_str(),
        Some(PipeArg::Number(_)) | None => {
            return Err(NlgError::InvalidPipe {
                pipe: "choose".to_string(),
                reason: "choose requires an argument of the form 'key=value,key=value,default=value'".to_string(),
            });
        }
        // Match exhaustively — no wildcard. If PipeArg gains a variant later,
        // this forces us to decide what to do with it.
    };

    let pairs = parse_choose_pairs(arg_str)?;
    if pairs.is_empty() {
        return Err(NlgError::InvalidPipe {
            pipe: "choose".to_string(),
            reason: "choose argument is empty".to_string(),
        });
    }

    let display = value.as_display();
    let normalized = display.trim().to_lowercase();

    for (k, v) in &pairs {
        if k.to_lowercase() == normalized {
            return Ok(Value::String(v.clone()));
        }
    }

    // Fallback to default
    for (k, v) in &pairs {
        if k.eq_ignore_ascii_case("default") {
            return Ok(Value::String(v.clone()));
        }
    }

    // No match, no default — dispatch on strictness
    match self.strictness {
        Strictness::Strict => Err(NlgError::InvalidPipe {
            pipe: "choose".to_string(),
            reason: format!(
                "no matching key for value `{display}` and no default",
            ),
        }),
        Strictness::Lenient => Ok(Value::String(format!("[choose: no match for {display}]"))),
        Strictness::Silent => Ok(Value::String(String::new())),
    }
}

fn parse_choose_pairs(arg: &str) -> Result<Vec<(String, String)>, NlgError> {
    let mut out = Vec::new();
    for raw_pair in arg.split(',') {
        let pair = raw_pair.trim();
        if pair.is_empty() { continue; }
        let eq = pair.find('=').ok_or_else(|| NlgError::InvalidPipe {
            pipe: "choose".to_string(),
            reason: format!("pair `{pair}` is missing `=` separator"),
        })?;
        let key = pair[..eq].trim().to_string();
        let value = pair[eq + 1..].trim().to_string();
        if key.is_empty() {
            return Err(NlgError::InvalidPipe {
                pipe: "choose".to_string(),
                reason: format!("pair `{pair}` has empty key"),
            });
        }
        out.push((key, value));
    }
    Ok(out)
}
```

The `parse_choose_pairs` helper is a free function (module-private). Put it near the bottom of `engine.rs` next to the other pipe helpers. Do NOT memoise the parse — per-render parsing of a ~50-byte string is negligible, and memoisation would require storing parsed state on the `Pipe` struct which breaks encapsulation.

### 1.2 Verify

```bash
cargo test --all-features
cargo clippy --all-features -- -D warnings
```

All existing tests pass. No new functionality visible yet (no tests exercise the pipe), but infrastructure compiles.

**Commit:** `Implement {slot|choose} pipe for value-based lexical dispatch`

---

## Phase 2 — Tests and whitelist updates

### 2.1 Unit tests in `engine.rs`

Add to the `#[cfg(test)]` block:

```rust
#[test]
fn choose_pipe_exact_match() {
    let engine = basic_engine();
    let mut ctx = Context::new();
    ctx.insert("level", Value::String("critical".into()));
    let mut session = Session::new();
    let out = engine
        .render_inline(&mut session, "{level|choose: critical=URGENT, warn=WARN, default=INFO}", &ctx)
        .unwrap();
    assert_eq!(out.trim_end_matches('.').trim(), "URGENT");
}

#[test]
fn choose_pipe_case_insensitive_match() {
    let engine = basic_engine();
    let mut ctx = Context::new();
    ctx.insert("level", Value::String("CRITICAL".into()));
    let mut session = Session::new();
    let out = engine
        .render_inline(&mut session, "{level|choose: critical=URGENT, default=INFO}", &ctx)
        .unwrap();
    assert_eq!(out.trim_end_matches('.').trim(), "URGENT");
}

#[test]
fn choose_pipe_default_fallback() {
    let engine = basic_engine();
    let mut ctx = Context::new();
    ctx.insert("level", Value::String("info".into()));
    let mut session = Session::new();
    let out = engine
        .render_inline(&mut session, "{level|choose: critical=URGENT, default=INFO}", &ctx)
        .unwrap();
    assert_eq!(out.trim_end_matches('.').trim(), "INFO");
}

#[test]
fn choose_pipe_number_slot() {
    let engine = basic_engine();
    let mut ctx = Context::new();
    ctx.insert("count", Value::Number(1));
    let mut session = Session::new();
    let out = engine
        .render_inline(&mut session, "{count|choose: 1=is, default=are}", &ctx)
        .unwrap();
    assert_eq!(out.trim_end_matches('.').trim(), "is");
    let mut ctx = Context::new();
    ctx.insert("count", Value::Number(5));
    let mut session = Session::new();
    let out = engine
        .render_inline(&mut session, "{count|choose: 1=is, default=are}", &ctx)
        .unwrap();
    assert_eq!(out.trim_end_matches('.').trim(), "are");
}

#[test]
fn choose_pipe_chains_with_other_pipes() {
    let engine = basic_engine();
    let mut ctx = Context::new();
    ctx.insert("action", Value::String("modify".into()));
    let mut session = Session::new();
    let out = engine
        .render_inline(
            &mut session,
            "{action|choose: rename=renamed, modify=modified, default=changed|capitalize}",
            &ctx,
        )
        .unwrap();
    assert!(out.contains("Modified"), "got: {out}");
}

#[test]
fn choose_pipe_strict_no_match_no_default_errors() {
    let mut engine = Engine::new(English::new())
        .strictness(Strictness::Strict)
        .variation(Variation::Fixed);
    engine.register_template("t", "{level|choose: critical=URGENT}").unwrap();
    let mut ctx = Context::new();
    ctx.insert("level", Value::String("info".into()));
    let mut session = Session::new();
    let err = engine.render(&mut session, "t", &ctx).unwrap_err();
    assert!(matches!(err, NlgError::InvalidPipe { .. }));
}

#[test]
fn choose_pipe_silent_no_match_returns_empty() {
    let mut engine = Engine::new(English::new())
        .strictness(Strictness::Silent)
        .variation(Variation::Fixed);
    engine.register_template("t", "level is {level|choose: critical=URGENT}").unwrap();
    let mut ctx = Context::new();
    ctx.insert("level", Value::String("info".into()));
    let mut session = Session::new();
    let out = engine.render(&mut session, "t", &ctx).unwrap();
    // "level is" followed by an empty substitution, then terminated.
    assert!(out.trim_end_matches('.').trim_end().ends_with("is") || out.trim_end_matches('.').trim().eq("level is"));
}

#[test]
fn choose_pipe_missing_arg_errors() {
    let mut engine = Engine::new(English::new()).strictness(Strictness::Strict);
    engine.register_template("t", "{level|choose}").unwrap();
    let mut ctx = Context::new();
    ctx.insert("level", Value::String("info".into()));
    let mut session = Session::new();
    let err = engine.render(&mut session, "t", &ctx).unwrap_err();
    assert!(matches!(err, NlgError::InvalidPipe { .. }));
}

#[test]
fn choose_pipe_malformed_arg_errors() {
    let mut engine = Engine::new(English::new()).strictness(Strictness::Strict);
    engine.register_template("t", "{level|choose: no_equals_here}").unwrap();
    let mut ctx = Context::new();
    ctx.insert("level", Value::String("info".into()));
    let mut session = Session::new();
    let err = engine.render(&mut session, "t", &ctx).unwrap_err();
    assert!(matches!(err, NlgError::InvalidPipe { .. }));
}
```

Check the name of the existing test-setup helper (`basic_engine` or `engine()` — look at the existing test module). Use the same helper. Adapt method names (`render_inline` signature is `(session, template, context)` post-Session-split — verify).

### 2.2 Whitelist update in `nlg-derive/src/lib.rs`

```rust
const VALID_PIPES: &[&str] = &[
    "pluralize", "article", "join", "ordinal", "words", "truncate",
    "capitalize", "refer", "verb", "syn", "relative", "quantify",
    "hedge", "negated", "choose",
];
```

### 2.3 Update `nlg_template!` tests

The existing integration test for `nlg_template!` has a test that asserts `choose` is rejected. **Invert it** — now `choose` is valid, so the test should assert it's accepted. Example:

```rust
#[test]
fn choose_pipe_accepted() {
    let tpl = nlg_template! {
        template: "{level|choose: critical=URGENT, default=INFO}",
        slots: [level],
    };
    assert!(tpl.contains("|choose"));
}
```

Find the old `choose`-rejection test in `nlg-core/tests/nlg_template_macro.rs` (if it exists; `nlg_template!` was just shipped and may not have a specific rejection test for `choose`). If there's a generic "unknown pipe rejected" test using a different pipe name (e.g. `fakepipe`), it stays valid.

### 2.4 Update rustdoc examples

If any rustdoc in engine.rs lists the valid pipes, add `choose` to the list.

### 2.5 Verify

```bash
cargo test --all-features
cargo test --no-default-features
cargo test --features serde
cargo clippy --all-features -- -D warnings
cargo doc --all-features --no-deps
```

**Commit:** `Add choose pipe to valid-pipe whitelists and add tests`

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

Test count should increase by ~9–10 → **~473 total**.

**Report:** 2 commit hashes, test count delta, any surprises.

---

## Risk register

| Risk | Mitigation |
|---|---|
| `PipeArg` match is currently not exhaustive in other pipe handlers — adding a new pipe shouldn't introduce exhaustive-match regressions | The plan doesn't add a PipeArg variant. Other handlers stay unchanged. |
| `pipe_choose` called via `apply_pipe` which takes `&Context` — do we actually need context for choose? | No — the _context arg is unused. Prefix with underscore to silence clippy. |
| Trimming / normalization breaks when slot value has meaningful whitespace (e.g. "  critical  " from user input) | Intentional — we trim and lowercase for matching. Users wanting exact-string semantics should pre-normalize upstream. |
| `eq_ignore_ascii_case` vs `to_lowercase` mismatch for non-ASCII locales | English-only for v1. Document. Multilingual is v1.5 concern. |
| Strictness::Lenient placeholder format `[choose: no match for X]` is ad-hoc | Mirrors existing Lenient placeholder formats (e.g. `[missing: count]`). Consistent. |
| The `default` keyword collision — what if a user has a key literally named "default"? | They can't override; `default` is reserved. Document. Users with a value of "default" in their domain must map it differently (e.g. use "fallback" as their sentinel and handle upstream). |
| Numbers in keys/values — "1=is" keeps "1" as a string key; compared against `Value::Number(1).as_display() == "1"` | Works. Verified by the number_slot test case. |
| Commas and equals signs embedded in values | Not supported in v1; document. Users needing comma-containing values should compose via slots. |
| Existing `nlg_template!` macro's pipe rejection for `choose` | Update whitelist. Add an explicit "choose accepted" test. |
| Lazy parse per render is O(n) where n = arg length | Negligible for typical args. If ever a bottleneck, Phase 2 follow-up can pre-parse into `PipeArg::KvList`. Not worth the complexity now. |

## What NOT to do

- **Do not** extend `PipeArg` with a new variant. Parse lazily.
- **Do not** support comma or equals inside values.
- **Do not** add LRU rotation (that's the concept-group extension, separate plan).
- **Do not** add numeric-range key matching.
- **Do not** amend commits.
- **Do not** skip updating the `nlg_template!` whitelist — it breaks macro consumers.

## Definition of done

- [ ] Phase 0 baseline clean
- [ ] 2 commits with specified subject lines
- [ ] `apply_pipe` dispatches `"choose"` to `pipe_choose`
- [ ] `pipe_choose` implements the matching and fallback semantics
- [ ] `parse_choose_pairs` helper parses the arg string
- [ ] `nlg-derive` whitelist includes `"choose"`
- [ ] All ~473 tests pass across feature variants
- [ ] `cargo clippy --all-features -- -D warnings` clean
- [ ] `Engine: Send + Sync` assert still compiles
- [ ] No new public API beyond the pipe being recognised
