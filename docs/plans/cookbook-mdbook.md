# Plan: Cookbook mdBook Site

**Owner:** sonnet agent
**Scope:** new `docs/cookbook/` directory with mdBook scaffold + 9 recipe chapters
**Estimated size:** ~2,000–3,000 words of prose + code samples across 9 chapters
**Test gate:** all 534 existing tests pass (no code changes); `mdbook build` succeeds if mdbook is installed
**Branch discipline:** local only, 1 commit

---

## Why

The README is a feature reference wall. A cookbook site surfaces *recipes* first — "how do I generate a changelog?", "how do I bridge tracing events?", "how do I test my templates?" — which is what users searching for solutions actually want. Every major Rust infrastructure crate (serde, tracing, tokio) has one.

## Structure

```
docs/cookbook/
├── book.toml
└── src/
    ├── SUMMARY.md
    ├── getting-started.md
    ├── template-authoring.md
    ├── generating-changelogs.md
    ├── monitoring-alerts.md
    ├── incident-narratives.md
    ├── building-a-vocab-crate.md
    ├── faithfulness-testing.md
    ├── deployment-matrix.md
    └── migrating-from-rosaenlg.md
```

## Chapter outlines

### 1. Getting Started (`getting-started.md`)

- Add `prosaic-core` + `prosaic-grammar-en` to Cargo.toml
- Create an engine, register a template, render
- Show `ctx!` macro for ergonomic context building
- Show `Session` lifecycle (create, render, reset)
- Output: a rendered sentence from a code-change event
- ~200 words + 1 complete code sample

### 2. Template Authoring Guide (`template-authoring.md`)

- Slot syntax: `{name}`, `{name|pipe}`, `{name|pipe:arg}`, `{name|pipe1|pipe2}`
- Pipe reference table (all 15 pipes with one-line description + example)
- Conditional sections: `{?key}...{/?}` — truthy rules (non-empty, non-zero)
- Partials: `{>name}` — shared template fragments
- The `|choose` pipe for value-based dispatch
- **Join-style gotcha:** when your template text already has a framing word ("impacting", "affecting"), use `|join:bracketed` to avoid "impacting including..." collisions
- ~400 words + inline examples per section

### 3. Generating Changelogs (`generating-changelogs.md`)

- Install the CLI: `cargo install prosaic-cli`
- One-liner: `echo '...' | prosaic --preset=changelog`
- Show a realistic 5-event JSONL input (feature added, bugfix, breaking change, dependency update, release tagged)
- Show the prose output
- Explain how `--preset=changelog` maps to vocab=code,git,release + strategy=by-action + max-length=120
- Override examples: `--preset=changelog --strategy=sequential`
- ~300 words + CLI examples

### 4. Monitoring Alerts to Prose (`monitoring-alerts.md`)

- Add `prosaic-tracing` to your existing tracing-subscriber stack
- Register templates for your alert keys
- Show `ProsaicLayer::new(engine, writer).key_mapper(|meta| ...)`
- Full working example: a web server handler that emits `tracing::warn!` and produces prose in a log file
- When to use this vs. a plain log formatter
- ~300 words + 1 complete code sample

### 5. Incident Narratives (`incident-narratives.md`)

- Build a sequence of incident events (detected, impact, mitigation, resolved)
- Feed them into `DocumentPlan::from_events_grouped` with `GroupingStrategy::ByEntity`
- Show the multi-paragraph narrative output
- Compare `ByEntity` vs `ByAction` grouping on the same events
- When to use batch rendering vs DocumentPlan
- ~300 words + code sample + two output comparisons

### 6. Building a Vocabulary Crate (`building-a-vocab-crate.md`)

- Scaffold: `cargo new prosaic-vocab-myapp --lib`
- Depend on `prosaic-core`
- Implement `pub fn register(engine: &mut Engine) -> Result<(), ProsaicError>`
- Template design tips: salience tiers (Low/Medium/High), conditional sections for optional slots, pipe selection
- Testing: one test per event type, `assert!` on key content
- Namespace convention: `templates::en::MODULE` for future multilingual
- ~400 words + scaffold code + one complete event type example

### 7. Faithfulness Testing (`faithfulness-testing.md`)

- What PARENT measures: precision (are all output tokens grounded in the context + template?)
- What polarity checking catches: negation flips ("was merged" vs "was not merged")
- Using `assert_faithful!` in vocab crate tests
- Using `Engine::with_faithfulness_gate(1.0)` for runtime rejection
- Using `score_faithfulness` directly for custom thresholds
- When faithfulness scoring gives false positives (morphology edge cases, template literals that look like content)
- ~300 words + test examples

### 8. Deployment Matrix (`deployment-matrix.md`)

A table + notes:

| Target | Features | Approx binary size | Notes |
|--------|----------|-------------------|-------|
| Server (Linux/Mac/Windows) | `default` (all) | N/A (library) | Full feature set |
| CLI tool | `default` + serde | ~2 MB release | `cargo install prosaic-cli` |
| WASM (browser) | `default` minus `time` | ~150–300 KB gzipped (est.) | `time` feature requires `SystemTime`; gate off for WASM |
| WASM with locale | `default` + `locale` minus `time` | ~200–400 KB gzipped (est.) | icu4x data adds ~50-100 KB per locale |
| Embedded / no_std | Not yet supported | — | Planned for v2 (`no_std + alloc` path) |

Notes on each target: what features to enable/disable, what to watch for.

- ~200 words + the table

### 9. Migrating from RosaeNLG (`migrating-from-rosaenlg.md`)

- RosaeNLG concept → Prosaic concept mapping table:
  - Pug mixin → `register_template` call
  - `#[+value]` → `{slot}` or `{slot|pipe}`
  - `synz` (synonym) → `{word|syn}` pipe + `SynonymRegistry`
  - `#[+verb]` → `{action|verb:past}` pipe
  - `#[+subjectVerb]` → Sentence builder API
  - `choosebest` → `Variation::Seeded` + discourse anti-repeat
  - `forEach` aggregation → `render_batch` + subject aggregation
  - Referring expression → `{name|refer}` pipe
  - Multilingual `.pug` files → `prosaic-grammar-es` (v1.5, coming soon)
- Key differences: Prosaic is Rust-native (no Node.js runtime), deterministic by default, has PARENT faithfulness built in
- What's not yet supported: RosaeNLG's 5 languages (Prosaic is English-only in v1; Spanish in v1.5)
- ~400 words + mapping table

## book.toml

```toml
[book]
title = "Prosaic Cookbook"
description = "Recipes for deterministic prose generation with the Prosaic engine"
authors = ["Matthew MacKinnon"]
language = "en"
src = "src"

[output.html]
default-theme = "rust"
git-repository-url = "https://github.com/wildmason/prosaic"
```

## SUMMARY.md

```markdown
# Summary

- [Getting Started](getting-started.md)
- [Template Authoring Guide](template-authoring.md)
- [Generating Changelogs](generating-changelogs.md)
- [Monitoring Alerts to Prose](monitoring-alerts.md)
- [Incident Narratives](incident-narratives.md)
- [Building a Vocabulary Crate](building-a-vocab-crate.md)
- [Faithfulness Testing](faithfulness-testing.md)
- [Deployment Matrix](deployment-matrix.md)
- [Migrating from RosaeNLG](migrating-from-rosaenlg.md)
```

## Content guidelines for the agent

- **Tone:** direct, practical, no fluff. Mirror the Rust-by-Example style: show the code, explain what it does, move on.
- **Code samples:** must be complete enough to copy-paste and run. Include `use` imports, `fn main()`, and expected output in comments where helpful.
- **No emojis.**
- **No "In this chapter we will..." preambles.** Start with the problem or the code.
- **Link to docs.rs for API details** rather than duplicating reference material.
- **Every code sample that uses `prosaic-core` must show the `Session` parameter** in render calls — that's the current API.
- **Use `ctx!` macro** in all examples (not the verbose `Context::new()` + `insert` pattern) — showcase the ergonomic win.

## Verification

```bash
# If mdbook is installed:
cd docs/cookbook && mdbook build

# Otherwise just verify the files exist and are valid markdown:
find docs/cookbook -name "*.md" -exec echo {} \;
```

No code changes to the engine — just docs. All 534 tests remain passing.

**Commit:** `Add cookbook mdBook site with 9 recipe chapters`
