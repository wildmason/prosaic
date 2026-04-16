# Plan: PARENT Faithfulness Metric

**Owner:** sonnet agent
**Scope:** `nlg-core` only (new module + Engine/Template helpers)
**Estimated size:** ~500–700 LOC including tests
**Test gate:** all 394 tests pass after every sub-phase; zero warnings
**Branch discipline:** local only, commit at each sub-phase checkpoint

---

## Why

The linguistics agent named this the #1 highest-gain/cost finding. It is the library's first automated quality gate and the foundational primitive for the "hallucination-proof prose for regulated systems" positioning.

It does double duty:

1. **Test harness for vocab crate authors** — `assert_faithful!(output, context, template)` and `#[nlg_faithful]` make template faithfulness a crate-level conformance guarantee. Catches bugs like "template has a hardcoded word that should be slot-driven" automatically.
2. **Default Verifier for the future `nlg-polish-llm` crate** — LLM paraphrases are gated by the same scorer. Polarity-flip rejection is enforced at the trait-conformance level.
3. **Optional runtime gate** — `engine.with_faithfulness_gate(threshold)` rejects renders below threshold with a diagnostic error. Opt-in, off by default.

This plan ships all three. One implementation, three consumers.

## Design decisions (locked — do not deviate)

### Scoring model

Reference-free PARENT: **precision only**. For a hypothesis (rendered output) and a source (Context + Template literals), score = fraction of hypothesis content tokens that are entailed.

- **Content token** = token after lowercasing and edge-punctuation stripping, length ≥ 3, not in `POLARITY_TOKENS`, not in `STOPWORDS \ POLARITY_TOKENS`, not purely numeric.
- **Source set** = union of tokens from every `Context` value (String / List items; Number values contribute their digit string) + every template literal's tokens.
- **Entailment** = (a) exact match on the interned token set, OR (b) singular form (via `Language::singularize`) matches any source token, OR (c) any source token's singular form matches the hypothesis token. Bidirectional singularization handles "service" ↔ "services".
- **Precision** ∈ `[0.0, 1.0]`. If hypothesis has zero content tokens, precision = `1.0` (vacuously faithful).

### Polarity check (separate gate, hard rule)

- `POLARITY_TOKENS = ["not", "never", "no", "none", "cannot", "won't", "neither", "nor"]` (lowercased, after stripping apostrophes: "won't" → "wont" if needed — use exact-match including apostrophe for v1).
- Count occurrences in hypothesis. Count occurrences in source (context + template literals).
- `polarity_match = hypothesis_count == source_count` for **each token**.
- Polarity mismatch = automatic failure regardless of precision score.

### Tokenization

- Lowercase via `str::to_lowercase`.
- Split on ASCII whitespace.
- Trim edge punctuation: `,.:;!?"'()[]—–-`.
- Keep inner hyphens / apostrophes (e.g., `won't`, `user-facing` stay as single tokens).

### Out of scope

- **Bigram / trigram entailment.** Unigram only in v1.
- **Synonym expansion.** A token matches only its own form and its singular/plural pair. No WordNet.
- **Stemming beyond singularize.** No Porter stemmer.
- **Cross-linguistic polarity tables.** Hardcoded English list with extension point documented. Future languages extend via a trait method (out of scope here).
- **Semantic entailment** — we're doing token-set entailment, not meaning entailment. Document the limitation.

## API shape (locked)

```rust
// nlg-core/src/faithfulness.rs (new module)

pub struct FaithfulnessScore {
    /// Fraction of hypothesis content tokens entailed by source. [0.0, 1.0].
    pub precision: f32,
    /// True iff polarity-token multisets match exactly between source and hypothesis.
    pub polarity_match: bool,
    /// Hypothesis content tokens not entailed by source.
    pub unentailed: Vec<String>,
    /// Polarity tokens whose counts differ between hypothesis and source.
    /// Empty iff polarity_match is true.
    pub polarity_drift: Vec<PolarityDrift>,
}

pub struct PolarityDrift {
    pub token: String,
    pub in_source: usize,
    pub in_hypothesis: usize,
}

impl FaithfulnessScore {
    /// True iff precision == 1.0 AND polarity_match.
    pub fn is_faithful(&self) -> bool { ... }

    /// True iff precision >= threshold AND polarity_match.
    pub fn passes(&self, threshold: f32) -> bool { ... }
}

/// Score the faithfulness of a rendered output against the Context
/// that produced it, plus the template literals that anchored it.
pub fn score_faithfulness(
    output: &str,
    context: &Context,
    template_literals: &[&str],
    language: &dyn Language,
) -> FaithfulnessScore { ... }
```

### Template literal extraction helper

```rust
// nlg-core/src/template.rs — added method
impl Template {
    /// Return a view of every literal segment's text content.
    /// Used by faithfulness scoring to include template boilerplate in
    /// the entailment source.
    pub fn literal_tokens(&self) -> Vec<&str> { ... }
}
```

### Engine builder option

```rust
// nlg-core/src/engine.rs
impl Engine {
    /// Enable a runtime faithfulness gate on every `render*` call.
    ///
    /// When set, each rendered output is scored against its input Context
    /// and the selected template's literal tokens. If the score's
    /// precision falls below `threshold` OR polarity tokens mismatch, the
    /// render returns `NlgError::FaithfulnessRejection` and the session
    /// state is restored as if the render had not occurred.
    ///
    /// Threshold is typically 1.0 (strict). Values below 1.0 tolerate a
    /// fraction of unentailed content tokens — useful when the engine
    /// emits rendered output that may legitimately include words not in
    /// the input context (e.g. hedged phrasings). Default: no gate.
    pub fn with_faithfulness_gate(mut self, threshold: f32) -> Self { ... }
}
```

### Test harness macro

```rust
// nlg-core/src/faithfulness.rs
#[macro_export]
macro_rules! assert_faithful {
    ($output:expr, $context:expr, $template_literals:expr, $language:expr $(,)?) => {{
        let score = $crate::score_faithfulness(
            &$output,
            &$context,
            &$template_literals,
            $language,
        );
        if !score.is_faithful() {
            panic!(
                "faithfulness violation:\n  precision: {:.3}\n  polarity_match: {}\n  unentailed: {:?}\n  polarity_drift: {:?}",
                score.precision, score.polarity_match, score.unentailed, score.polarity_drift
            );
        }
    }};
}
```

### New error variant

```rust
// nlg-core/src/error.rs
#[derive(Debug, thiserror::Error)]
pub enum NlgError {
    // ... existing variants
    #[error("faithfulness rejected: precision {precision:.3}, polarity_match {polarity_match}")]
    FaithfulnessRejection {
        precision: f32,
        polarity_match: bool,
    },
}
```

---

## Phase 0 — Baseline

```bash
cargo test --all-features
cargo clippy --all-features -- -D warnings
```

Expected: 394 tests passing, 0 warnings.

**No commit.**

---

## Phase 1 — Add `Template::literal_tokens()` helper

**File:** `nlg-core/src/template.rs`

Add an `impl Template` method that walks `self.segments` and returns a `Vec<&str>` of every `Segment::Literal` text plus every literal inside `Segment::Conditional` sections (recursively). This gives scoring access to the template boilerplate.

```rust
impl Template {
    pub fn literal_tokens(&self) -> Vec<&str> {
        let mut out = Vec::new();
        collect_literals(&self.segments, &mut out);
        out
    }
}

fn collect_literals<'a>(segments: &'a [Segment], out: &mut Vec<&'a str>) {
    for seg in segments {
        match seg {
            Segment::Literal(s) => out.push(s.as_str()),
            Segment::Slot { .. } => {}
            Segment::Conditional { inner, .. } => collect_literals(inner, out),
            Segment::Partial(_) => {} // literals inside partials reached via engine expansion
        }
    }
}
```

Note: double-check the exact `Segment` variants in `template.rs` before coding. The file shows `Literal`, `Slot`, `Conditional` with inner segments, and `Partial(name)`. `Partial` is expanded at render time via the engine's partials map — literals inside partials are only reachable if the engine resolves them first. For v1 scoring, assume the caller provides the fully-expanded template OR accept partials as opaque. Document.

### 1.1 Tests

```rust
#[test]
fn literal_tokens_simple() {
    let t = Template::parse("The {type} {name} was modified").unwrap();
    let lits = t.literal_tokens();
    assert_eq!(lits, vec!["The ", " ", " was modified"]);
}

#[test]
fn literal_tokens_from_conditional_sections() {
    let t = Template::parse("{name}{?count}, impacting {count} consumers{/?}").unwrap();
    let lits = t.literal_tokens();
    assert!(lits.iter().any(|l| l.contains("impacting")));
    assert!(lits.iter().any(|l| l.contains("consumers")));
}

#[test]
fn literal_tokens_empty_for_all_slots() {
    let t = Template::parse("{a}{b}{c}").unwrap();
    assert!(t.literal_tokens().is_empty());
}
```

### 1.2 Verify

```bash
cargo test --all-features
cargo clippy --all-features -- -D warnings
```

**Commit:** `Add Template::literal_tokens helper for faithfulness scoring`

---

## Phase 2 — Add `faithfulness` module with scorer

**File:** `nlg-core/src/faithfulness.rs` (new)

### 2.1 Module skeleton

```rust
//! PARENT-style reference-free faithfulness scoring.
//!
//! Given a rendered output, the Context that produced it, and the
//! template's literal tokens, score how faithful the output is to its
//! inputs. Catches two failure modes:
//!
//! 1. **Hallucinated content** — tokens in the output that have no
//!    source in the Context or template literals. Surfaced as a low
//!    `precision` score and a list of unentailed tokens.
//! 2. **Polarity drift** — the hypothesis has a different count of
//!    negation tokens than the source. Surfaced as `polarity_match =
//!    false` and a list of drifted tokens.
//!
//! Morphological tolerance: singular/plural forms match via the
//! Language trait's `singularize` method. Beyond that, matching is
//! exact (after lowercasing and edge-punctuation stripping).
//!
//! See `docs/plans/parent-faithfulness.md` for design rationale.

use crate::context::{Context, Value};
use crate::language::Language;

const POLARITY_TOKENS: &[&str] = &[
    "not", "never", "no", "none", "cannot", "won't", "neither", "nor",
];

// Stopwords to exclude from content scoring. Note: POLARITY_TOKENS are
// NOT included here — they're handled separately.
const STOPWORDS: &[&str] = &[
    "a", "an", "the", "and", "or", "but", "in", "on", "at", "to", "for",
    "of", "with", "by", "from", "is", "was", "are", "were", "be", "been",
    "being", "have", "has", "had", "do", "does", "did", "will", "would",
    "could", "should", "may", "might", "shall", "can",
    "it", "its", "this", "that", "these", "those", "which", "who",
    "what", "where", "when", "how", "if", "then", "than", "so",
    "as", "up", "out", "into", "also", "just", "more", "most",
];
```

Note: this is intentionally a separate STOPWORDS list from `discourse.rs`'s. Factor out into a shared const in a later refactor. For v1 duplicate.

### 2.2 FaithfulnessScore struct

```rust
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FaithfulnessScore {
    pub precision: f32,
    pub polarity_match: bool,
    pub unentailed: Vec<String>,
    pub polarity_drift: Vec<PolarityDrift>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PolarityDrift {
    pub token: String,
    pub in_source: usize,
    pub in_hypothesis: usize,
}

impl FaithfulnessScore {
    pub fn is_faithful(&self) -> bool {
        self.precision >= 1.0 && self.polarity_match
    }

    pub fn passes(&self, threshold: f32) -> bool {
        self.precision >= threshold && self.polarity_match
    }
}
```

### 2.3 Tokenization

```rust
fn tokenize(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|raw| {
            raw.trim_matches(|c: char| {
                matches!(c, ',' | '.' | ':' | ';' | '!' | '?' | '"' | '\''
                           | '(' | ')' | '[' | ']' | '—' | '–' | '-')
            })
            .to_lowercase()
        })
        .filter(|s| !s.is_empty())
        .collect()
}
```

### 2.4 Classification predicates

```rust
fn is_stopword(tok: &str) -> bool {
    STOPWORDS.iter().any(|&s| s == tok)
}

fn is_polarity(tok: &str) -> bool {
    POLARITY_TOKENS.iter().any(|&s| s == tok)
}

fn is_numeric(tok: &str) -> bool {
    !tok.is_empty() && tok.chars().all(|c| c.is_ascii_digit())
}

fn is_content_token(tok: &str) -> bool {
    tok.len() >= 3 && !is_stopword(tok) && !is_polarity(tok) && !is_numeric(tok)
}
```

### 2.5 Source-set extraction

```rust
fn tokens_from_context(ctx: &Context) -> Vec<String> {
    let mut out = Vec::new();
    for (_key, value) in ctx.iter() {
        match value {
            Value::String(s) => out.extend(tokenize(s)),
            Value::Number(n) => out.push(n.to_string()),
            Value::List(items) => {
                for item in items { out.extend(tokenize(item)); }
            }
        }
    }
    out
}

fn tokens_from_literals(literals: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    for lit in literals { out.extend(tokenize(lit)); }
    out
}
```

**Check:** `Context` must expose `iter()` over `(&str, &Value)`. If not, add one or use another public accessor. Confirm before coding.

### 2.6 The scorer

```rust
pub fn score_faithfulness(
    output: &str,
    context: &Context,
    template_literals: &[&str],
    language: &dyn Language,
) -> FaithfulnessScore {
    let hyp_tokens = tokenize(output);
    let src_tokens: Vec<String> = {
        let mut v = tokens_from_context(context);
        v.extend(tokens_from_literals(template_literals));
        v
    };

    // Polarity check: multiset comparison on POLARITY_TOKENS only.
    let mut polarity_drift = Vec::new();
    let mut polarity_match = true;
    for &tok in POLARITY_TOKENS {
        let s = src_tokens.iter().filter(|t| t.as_str() == tok).count();
        let h = hyp_tokens.iter().filter(|t| t.as_str() == tok).count();
        if s != h {
            polarity_match = false;
            polarity_drift.push(PolarityDrift {
                token: tok.to_string(),
                in_source: s,
                in_hypothesis: h,
            });
        }
    }

    // Precision: over content tokens only.
    let hyp_content: Vec<&String> = hyp_tokens.iter().filter(|t| is_content_token(t)).collect();
    if hyp_content.is_empty() {
        return FaithfulnessScore {
            precision: 1.0,
            polarity_match,
            unentailed: Vec::new(),
            polarity_drift,
        };
    }

    // Build a normalised source set: each source token contributes its
    // own form AND its singularized form.
    let mut src_set: std::collections::HashSet<String> = std::collections::HashSet::new();
    for t in &src_tokens {
        src_set.insert(t.clone());
        src_set.insert(language.singularize(t));
    }

    let mut entailed = 0usize;
    let mut unentailed: Vec<String> = Vec::new();

    for t in &hyp_content {
        let t_sing = language.singularize(t);
        if src_set.contains(t.as_str()) || src_set.contains(&t_sing) {
            entailed += 1;
        } else {
            unentailed.push(t.to_string());
        }
    }

    let precision = entailed as f32 / hyp_content.len() as f32;

    FaithfulnessScore { precision, polarity_match, unentailed, polarity_drift }
}
```

### 2.7 The test-harness macro

```rust
#[macro_export]
macro_rules! assert_faithful {
    ($output:expr, $context:expr, $template_literals:expr, $language:expr $(,)?) => {{
        let score = $crate::score_faithfulness(
            &$output,
            &$context,
            &$template_literals,
            $language,
        );
        if !score.is_faithful() {
            panic!(
                "faithfulness violation:\n  precision: {:.3}\n  polarity_match: {}\n  unentailed: {:?}\n  polarity_drift: {:?}\n  output: {}\n",
                score.precision,
                score.polarity_match,
                score.unentailed,
                score.polarity_drift,
                $output,
            );
        }
    }};
}
```

### 2.8 Wire into `lib.rs`

```rust
mod faithfulness;
pub use faithfulness::{score_faithfulness, FaithfulnessScore, PolarityDrift};
// assert_faithful! is exported via #[macro_export]
```

### 2.9 Tests

Create a thorough `#[cfg(test)]` module at the bottom of `faithfulness.rs`:

- `tokenize_strips_edge_punctuation`
- `tokenize_lowercases`
- `tokenize_preserves_inner_hyphens_and_apostrophes` (e.g. "user-facing" stays one token)
- `content_token_respects_length_threshold` (length 2 excluded)
- `content_token_excludes_stopwords`
- `content_token_excludes_polarity`
- `content_token_excludes_pure_digits`
- `faithful_simple_rename` — context has `UserService`/`AccountService`, output mentions both → precision 1.0, polarity_match true
- `unentailed_word_lowers_precision` — output includes "removed" but context only has "modified" → precision < 1.0, unentailed contains "removed"
- `polarity_drift_detected` — source has zero "not" tokens, hypothesis has one "not" → polarity_match false
- `polarity_preservation_detected` — source has one "not", hypothesis has one "not" → polarity_match true
- `singular_plural_tolerance` — source has "service", output has "services" → entailed (via singularize)
- `template_literals_contribute_to_source` — output word is in template literals (e.g. "renamed") but not in context → still entailed
- `empty_output_is_vacuously_faithful` — precision 1.0
- `numeric_tokens_excluded_from_content` — "42" in output doesn't require "42" in source
- `is_faithful_requires_both_precision_and_polarity` — exactly covered
- `passes_threshold_semantics`

Use `English` from `nlg-grammar-en` in tests as the Language impl. Add dev-dependency if needed (check `Cargo.toml` — `nlg-grammar-en` is already a dev-dep per the existing integration tests).

### 2.10 Verify

```bash
cargo test --all-features
cargo test --no-default-features
cargo test --features serde
cargo clippy --all-features -- -D warnings
```

**Commit:** `Add faithfulness scoring module with polarity and precision gates`

---

## Phase 3 — Engine runtime gate

**File:** `nlg-core/src/engine.rs`, `nlg-core/src/error.rs`

### 3.1 New error variant

```rust
// error.rs
#[error("faithfulness rejected: precision {precision:.3}, polarity_match {polarity_match}")]
FaithfulnessRejection { precision: f32, polarity_match: bool },
```

### 3.2 Engine field and builder

Add `faithfulness_threshold: Option<f32>` to `Engine`. Initialize to `None` in `new()`. Builder:

```rust
pub fn with_faithfulness_gate(mut self, threshold: f32) -> Self {
    self.faithfulness_threshold = Some(threshold);
    self
}
```

### 3.3 Integrate into `render_tx`

After the rendered output is finalized and before returning it, if `self.faithfulness_threshold.is_some()`:

1. Extract the selected `Template`'s `literal_tokens()`.
2. Call `score_faithfulness(&out, context, &literals, &*self.language)`.
3. If `!score.passes(threshold)`, return `Err(NlgError::FaithfulnessRejection { precision, polarity_match })`. The outer `render()` method already handles snapshot/restore on error — no additional plumbing.

Where in the pipeline: after `cleanup_artifacts_in_place` / `terminate_sentence_in_place` / any polish passes. We want to score the **final** output.

The Template reference is in scope inside `render_tx` (it's passed to `render_template_into`). Confirm and reuse.

### 3.4 Tests

Add integration tests in a new file `nlg-core/tests/faithfulness.rs`:

- `gate_off_by_default_allows_unfaithful_render` — build an engine without the gate, register a template that includes a word outside the context, render succeeds.
- `gate_on_rejects_below_threshold` — same template, threshold 1.0, render returns `FaithfulnessRejection`.
- `gate_on_permits_above_threshold` — threshold 0.5 with a 75% faithful output → render succeeds.
- `gate_rejects_on_polarity_drift` — template somehow introduces a stray "not" not in context → rejection.
- `session_state_restored_on_faithfulness_rejection` — state after rejection is identical to state before the rejected render (snapshot/restore works).

### 3.5 Verify

```bash
cargo test --all-features
cargo test --no-default-features
cargo test --features serde
cargo clippy --all-features -- -D warnings
cargo bench --bench engine -- --test
```

**Commit:** `Add optional faithfulness gate to Engine renders`

---

## Phase 4 — `#[nlg_faithful]` test attribute (deferred)

**SCOPE NOTE:** The attribute macro (`#[nlg_faithful]`) is a *proc-macro* and lives in `nlg-derive`. It requires parsing the test function body to find engine render calls, extract the template and context arguments, and inject `assert_faithful!` after. That's non-trivial and not a good fit for this plan.

**For this plan:** ship only `assert_faithful!` macro as the test harness. The proc-macro attribute is a follow-up. Note in rustdoc.

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

All clean. Test count should increase by ~20 (Phase 1) + ~16 (Phase 2) + ~5 (Phase 3) = **~435 total**, up from 394.

**Report:** 3 commit hashes, test count delta per feature variant, any surprises.

---

## Risk register

| Risk | Mitigation |
|---|---|
| `Context::iter()` doesn't exist or has a different signature | Check `context.rs` first. If needed, add `pub fn iter(&self) -> impl Iterator<Item = (&str, &Value)>` as part of Phase 2. |
| Tokenization differs from discourse.rs's existing tokenizer → scoring mismatch with how discourse tracks repetition | Accept the minor drift for v1; the two purposes differ (discourse tracks whole-word repetition, PARENT tracks entailment). Document. A future refactor can unify. |
| `Language::singularize` returns the same word unchanged for irregulars not in its table | That's fine — the exact-match path still works for irregulars present in both sides. If singular/plural mismatch slips through, the test `singular_plural_tolerance` catches it; add irregulars to `nlg-grammar-en` only if the test surfaces them. |
| `Segment::Partial` branch in `literal_tokens` leaves partial content unscored | Document. Partial-heavy templates may see artificially low precision; callers can disable the gate or pre-expand partials. |
| Polarity token list English-only | Explicit. Document the extension point (future `Language::polarity_tokens()` method). For v1 the gate is English-only. |
| `won't` tokenization — apostrophe is kept inside the token, so it matches `won't` in source, but a source containing "wont" (no apostrophe) wouldn't match | Accepted edge. Document. |
| `cargo bench` regression from faithfulness gate | Gate is off by default. When on, cost is O(n) where n = output tokens. For typical renders (~50 tokens) this is <10 μs. If a bench somehow enables it, flag. |
| Clippy may complain about `collect::<Vec<_>>` patterns or float-comparison in `is_faithful` | `precision >= 1.0` is intentional strict equality via >= — if clippy pushes `>= 1.0 - f32::EPSILON` or similar, accept. |

## What NOT to do

- **Do not** implement bigram/trigram PARENT. Unigram only.
- **Do not** add synonym expansion.
- **Do not** implement the `#[nlg_faithful]` proc-macro attribute in this plan.
- **Do not** remove `STOPWORDS` from `discourse.rs`. Leave the duplication for a later refactor.
- **Do not** change the tokenization in `discourse.rs` to match `faithfulness.rs`. Different purposes.
- **Do not** add new `Language` trait methods. `singularize` exists.
- **Do not** change public Context / Value APIs.
- **Do not** introduce `unsafe`.
- **Do not** skip the `--features serde` test — the `FaithfulnessScore` and `PolarityDrift` structs have `#[cfg_attr(feature = "serde", derive(...))]`, and the serde round-trip path must pass.
- **Do not** amend commits.

## Definition of done

- [ ] Phase 0 baseline clean; Phase 5 final verification clean
- [ ] 3 commits with specified subject lines
- [ ] `Template::literal_tokens()` present with tests
- [ ] `faithfulness` module with `FaithfulnessScore`, `PolarityDrift`, `score_faithfulness`
- [ ] `assert_faithful!` macro exported via `#[macro_export]`
- [ ] `NlgError::FaithfulnessRejection` variant present
- [ ] `Engine::with_faithfulness_gate(f32)` builder option present
- [ ] `render_tx` integrates the gate with proper snapshot/restore on rejection
- [ ] All ~435 tests pass across feature variants
- [ ] `cargo clippy --all-features -- -D warnings` clean
- [ ] `Engine: Send + Sync` assert still compiles
- [ ] No new public API surface beyond what's specified
- [ ] No `unsafe`
