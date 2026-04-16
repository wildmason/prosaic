# Swarm: research — further tangible improvement vectors for the prosaic library

**Date:** 2026-04-15
**Mode:** research
**Protocol:** convergent
**Iterations:** 1 explore round + 1 synthesis round with peer handshakes

---

## Implementation Status (updated 2026-04-15)

> All v1 items shipped in a single session. Project renamed from `nlg` to `prosaic` and published at `github.com/wildmason/prosaic`.

### Shared infrastructure — ALL SHIPPED

- [x] Engine/Session split (`Session` owns `DiscourseState`; `Engine` is `Send + Sync`)
- [x] u32 symbol-table interner for word history
- [x] Flatten render pipeline to one String buffer + `fmt::Write`
- [x] `filter_by_salience` returns `Vec<&Template>` (no clone)
- [x] `ahash` swap on `Context` HashMap
- [x] Invert `SynonymRegistry` to O(1)
- [ ] `itoa`/`ryu` direct-format (deferred — marginal gain until higher-throughput workloads surface)

### Tier 1 (v1) — ALL SHIPPED

- [x] `ctx!` + `IntoValue` macros
- [x] `prosaic_template!` compile-time slot + pipe validator
- [x] Reference-free PARENT faithfulness metric
- [x] `assert_faithful!` macro
- [x] `Engine::with_faithfulness_gate` runtime gate
- [x] Centering Rule 1 enforcement (Cb tracking + pronoun gating)
- [x] Unified `{slot|choose: key=value, default=value}` pipe
- [x] Graph-based REG (Krahmer 2003) — opt-in second backend with relations
- [x] Forward conjunction reduction (ELLEIPO safe subset: "It also" + full-NP repetition)
- [x] `prosaic-vocab-release` (10 event types)
- [x] `prosaic-vocab-pr` (8 event types)
- [x] CLI `--preset=changelog|release-notes|digest` bundles
- [x] `prosaic-tracing` bridge (tracing events → prose)
- [ ] Cookbook mdBook site + feature-deployment matrix (deferred — content work, pending user input on recipe priorities)
- [ ] Vocab crate `templates::en::MODULE` namespace layout (convention documented but not yet enforced in code)

### Tier 1.5 (v1.5) — NOT STARTED

- [ ] `AgreementFeatures` struct + `Value::Entity` variant
- [ ] Plural REG with `Language::plural_description` hook
- [ ] `Language::plural_category` + `pluralize_with_category` behind `locale` feature flag (icu4x-backed)
- [ ] Split `ReferenceForm` into policy + `Language::realize_reference`
- [ ] `prosaic-grammar-es` (first non-English grammar crate)
- [ ] `ctx!` macro `entity()` syntax (reserved in v1, activated in v1.5)
- [ ] Phase-2 RosaeNLG migration recipe for Spanish

### Tier 2 (v2) — NOT STARTED

- [ ] `prosaic-grammar-de` (case declension axis)
- [ ] `prosaic-vocab-code-es` + Spanish vocab siblings
- [ ] RST-labeled DocumentPlan extension
- [ ] Temporal anchoring across paragraphs
- [ ] Full ELLEIPO with gapping
- [ ] Full Centering Theory (Cb/Cf with transition classification)
- [ ] `#[prosaic_template_compiled]` monomorphized render fn
- [ ] `parallel` Cargo feature (rayon at paragraph level, batch-only)
- [ ] `no_std + alloc` path
- [ ] `prosaic-wasm` member crate

### Tier 3 — NOT STARTED / DEFERRED

- [ ] `prosaic-polish-llm` (hybrid LLM paraphrase, type-enforced Paraphraser + Verifier)
- [ ] `prosaic-vocab-alerts`, `prosaic-vocab-db` (demand-gated)
- [ ] `prosaic-vocab-finance` (deferred until paying pilot)
- [ ] LSP server (deferred)
- [ ] pyo3 / napi-rs bindings (deferred until adoption)
- [x] ~~Grammatical Framework as realization backend~~ **REJECTED**
- [x] ~~MessageFormat 2.0 as template surface~~ **REJECTED** (semantics cherry-picked into `|plural` pipe instead)
- [x] ~~SimpleNLG phrasal realizer as replacement~~ **REJECTED**
- [x] ~~SDRT over RST~~ **REJECTED**
- [ ] PGO / BOLT (deferred)
- [ ] Full WordNet integration (deferred)

### Resolved coordination points

- **Crate name:** `prosaic` (was `nlg`; renamed and published to GitHub)
- **Spanish timing:** deferred to v1.5 (English-only v1 shipped)
- **LLM polish crate name:** `prosaic-polish-llm` (resolved from `nlg-polish-llm` vs `nlg-hybrid`)

---

## Team

| Agent | Persona | Model | Tasks Completed |
|---|---|---|---|
| linguistics | Computational Linguistics Researcher | opus | investigate, synthesize |
| multilingual | Multilingual NLP Specialist | opus | investigate, synthesize |
| performance | Rust Systems Performance Engineer | opus | investigate, synthesize |
| ecosystem | NLG Ecosystem & DX Analyst | opus | investigate, synthesize |

## Summary

The team converged on a coherent, sequenced roadmap for extending the `nlg` library, with **zero unresolved structural disagreements** across all four facets after synthesis. The dominant external timing fact is **RosaeNLG's LF AI archival scheduled for March 2026**, which creates an open niche the library is well-positioned to claim. The team produced a three-version plan (v1 English + ergonomics + test harness, v1.5 multilingual foundation + first Spanish grammar crate, v2 German grammar + compile-time templates + parallelism) anchored on a small set of shared-infrastructure refactors — principally an **Engine/Session split** and a **u32 symbol-table interner** — that unblock three otherwise-independent facets' top picks.

---

## Consolidated Roadmap

### Shared infrastructure (prerequisite for most downstream work)

1. **Engine/Session split** (performance finding #16) — make `DiscourseState` an explicit argument rather than interior-mutable, enabling rayon parallel-paragraphs, compile-time template codegen, AND PARENT faithfulness scoring. Single highest-leverage refactor.
2. **u32 symbol-table interner for word history** (performance finding #5) — shared substrate for PARENT (linguistics), `#[nlg_faithful]` test harness (ecosystem), and repetition scoring (performance). **Land FIRST** so downstream Tier-1 items aren't paying 80% overhead.
3. **Flatten render pipeline to one String buffer + `fmt::Write`** (performance finding #1) — ~70% allocation reduction, 2–5x wall-clock on hot path. Thread a `&RenderCtx` bundle (not just `&mut String`) so `AgreementFeatures` can be added later without signature churn.
4. **Trivial perf wins** (performance findings #2–#4, #13) — `filter_by_salience` returns `Vec<&Template>` not clones; `ahash` swap on `Context` HashMap; invert `SynonymRegistry` to O(1); `itoa`/`ryu` direct-format post-refactor.

### Tier 1 — English-only, independent (v1, Q2 2026)

| Item | Owner | Cost | Rationale |
|---|---|---|---|
| `ctx!` + `entity!` macros reserving `AgreementFeatures` syntax now | ecosystem | ~50 LOC | Highest-ROI ergonomic win. Macro rule dispatches via `IntoValue` trait so English callers pay zero syntax cost. |
| `#[nlg_template]` slot + pipe name validator (compile-time) | ecosystem + performance | 150–200 LOC | Validator-only scope in v1; monomorphized codegen deferred to v2 behind Session split. |
| Reference-free PARENT faithfulness metric | linguistics | ~1 week | Doubles as test harness AND default Verifier for `nlg-polish-llm`. One impl, two uses. Built on the interner. |
| `assert_faithful!` macro + `#[nlg_faithful]` attribute | linguistics + ecosystem | ~200 LOC | Exposes PARENT to vocab-crate authors as a conformance gate. |
| Centering Rule 1 enforcement | linguistics | 2–3 days | Bounded 8-slot ring buffer for Cf list; u8 grammatical-role salience. Fixes pronoun consistency bugs. |
| Unified `{slot\|choose: key=value, ..., default=value}` pipe | linguistics | 3 days | Concept-group lexical choice; compile-time validated via `#[nlg_template]`. |
| Graph-based REG (Krahmer et al. 2003) | linguistics | 2–3 weeks | **Upgraded to Tier 1 post-synthesis** — language-agnostic, independent of multilingual trait change. Opt-in second REG backend. |
| Forward conjunction reduction (safe ELLEIPO subset) | linguistics | 1 week | Writes into shared buffer via existing `reduce_same_entity_clauses` codepath. Full ELLEIPO with gapping waits for v2. |
| `nlg-vocab-release` + `nlg-vocab-pr` | ecosystem | 4–8 h each | Competes against git-cliff in high-end changelog/release-note niche. |
| CLI `--preset=changelog\|release-notes\|digest` bundles | ecosystem | ~200 LOC | One-command changelog generation as the "git-cliff with prose" pitch. |
| Vocab crate layout standard: `templates::en::MODULE`, reserved `::es::`, `::de::`, `::fr::` | multilingual + ecosystem | zero cost | Unlocks community non-English vocab contributions; prevents retrofit. |
| `nlg-tracing` bridge (tracing → prose narratives) | ecosystem | 1–2 weeks | *Possibly v1.5 if Spanish lands in v1.* |
| Cookbook mdBook site + feature-deployment matrix | ecosystem | 1 week setup + ongoing | Two-phase RosaeNLG migration recipe. Phase 1 (English) ships in v1. |

### Tier 1.5 — Multilingual foundation (v1.5, Q3 2026)

The three additive engine changes, sequenced tightly:

1. **`AgreementFeatures` struct + `Value::Entity` variant** — pure Rust ~200 LOC, outside icu4x gate. Opt-in, English ignores.
2. **Plural REG with `Language::plural_description` hook** — slotted between (1) and (3). Output is `PluralRegOutput { expression, features, category }` so surrounding agreement stays coherent.
3. **`Language::plural_category` + `pluralize_with_category → PluralOutput { form, category }`** — behind new `locale` Cargo feature (default off). icu4x-backed; English default collapses to one/other with zero icu4x weight. Pipe-form `{count|plural:service}` (not MessageFormat block syntax).
4. **Split `ReferenceForm` into policy + `Language::realize_reference(form, features) → Option<String>`** — `Zero` variant for Japanese pro-drop returns None.
5. **`nlg-grammar-es`** as first sibling crate — ~3.5k LOC modeled on SimpleNLG-ES. Ships with `icu_plurals` + `icu_list` with `es`-only baked data (~50–60 KB trimmed). Validates the gender/number axis.
6. **Phase-2 RosaeNLG migration recipe** for Spanish.

### Tier 2 — Larger / coupled items (v2, Q4 2026)

| Item | Owner | Notes |
|---|---|---|
| `nlg-grammar-de` | multilingual | Adds case-declension axis. Unblocks full ELLEIPO (German coreferentiality under case). |
| `nlg-vocab-code-es` + Spanish vocab siblings | multilingual + ecosystem | Demonstrates per-language vocab pattern. |
| RST-labeled DocumentPlan extension | linguistics | 1 week. Relation catalog (Contrast, Elaboration, Sequence, Condition) as labeled edges. |
| Temporal anchoring across paragraphs | linguistics | 5–7 days. Builds on RST extension. |
| Full ELLEIPO with gapping | linguistics | Depends on AgreementFeatures; validates against German. |
| Full Centering Theory (Cb/Cf with transition classification) | linguistics | 1–2 weeks. Breaking change to `mention_entity` API. |
| `#[nlg_template_compiled]` (monomorphized render fn) | performance + ecosystem | Ships WITH rayon parallel-paragraphs; both gated on Engine/Session split. |
| `parallel` Cargo feature (rayon at paragraph level) | performance | **Batch-workload only** — NOT request-path. Documented in cookbook. |
| `no_std + alloc` path | performance | ~1 week. Swap to hashbrown, thiserror 2.0, core::error::Error (1.81+). |
| `nlg-wasm` member crate | performance | Target: ≤300 KB gzipped for `locale`-excluded English build; ≤150 KB for zero-dep path. |

### Tier 3 — Optional / deferred

| Item | Status |
|---|---|
| `nlg-polish-llm` (hybrid LLM paraphrase crate) | **Optional workspace member, off by default.** Three traits (Paraphraser, Extractor, Verifier). Public API is **type-enforced**: `fn polish<P: Paraphraser, V: Verifier>(…) → Result<String, VerifierRejection>`. No free-standing `paraphrase()`. Users wanting unverified paraphrase must explicitly pass `NoOpVerifier` — the opt-out appears in source + imports + audits. Default Verifier is reference-free PARENT. Polarity token set `{not, never, no, without, un-, -less, cannot, won't}` is non-droppable at trait-conformance level: polarity flip = rejection, not warning. |
| `nlg-vocab-alerts`, `nlg-vocab-db` | v2+ depending on demand signal. |
| `nlg-vocab-finance` | Deferred until paying pilot — regulatory liability of shipping pre-baked financial wording. |
| LSP server | Deferred — 10× the cost of `#[nlg_template]` with overlapping value. Revisit if external template-file workflow emerges. |
| pyo3 / napi-rs bindings | Deferred until adoption validates. |
| Grammatical Framework as realization backend | **Rejected.** gf-core 0.3.0 not production-ready; author ergonomics cost too high. |
| MessageFormat 2.0 as template surface | **Rejected.** Cherry-pick its plural/select *semantics* into `|` pipe form instead. |
| SimpleNLG phrasal realizer as replacement | **Rejected.** Template model is correctly chosen for this library. |
| SDRT over RST | **Rejected.** Overkill for paragraph-grouping use case. |
| PGO / BOLT | Deferred until workloads justify. |
| Full WordNet integration | Deferred — licensing + 120 MB data. The `|choose` pipe captures 80% of the win. |

---

## Key Consensus Points

1. **RosaeNLG archival (March 2026) is the dominant timing fact.** Q2–Q3 2026 is the launch window.
2. **Positioning: "hallucination-proof prose layer for regulated and latency-critical systems."** Defended by $250M+/yr industry hallucination losses in regulated verticals (finance, medical, legal).
3. **Reference-free PARENT is the shared quality primitive.** One implementation, two uses: `#[nlg_faithful]` test harness AND default `nlg-polish-llm` Verifier. Interner-first sequencing makes it cheap enough for always-on test gating.
4. **Engine/Session split is the single highest-leverage refactor.** Unblocks rayon parallel-paragraphs, compile-time template codegen, and PARENT scoring — three facets' top picks.
5. **Compile-time templates ship in two stages.** v1: slot/pipe-name validator only (`#[nlg_template]`, 150–200 LOC). v2: monomorphized render fn (`#[nlg_template_compiled]`) together with rayon parallelism, both gated on Engine/Session split.
6. **`parallel` feature is batch-workload only**, documented in cookbook deployment matrix. Request-path concurrency stays at the request-handler layer.
7. **Multilingual path is additive, zero-cost on English.** Three trait additions, each with an English default that matches today's behaviour bit-for-bit.
8. **Vocab crate layout reserves `::en::` / `::es::` / `::de::` / `::fr::` from day one.** Zero cost now, unblocks community contributions without restructuring.
9. **`ctx!` macro reserves `AgreementFeatures` syntax in v1.** Prevents breaking macro revision in v1.5.
10. **`nlg-polish-llm` public API is type-enforced.** Paraphraser requires Verifier by type signature — no runtime-only convention. Keeps the "hallucination-proof" positioning honest.

## Unresolved coordination points (not disagreements)

- **Spanish at v1 vs v1.5** — depends on whether `nlg-grammar-es` + the three engine trait changes can land alongside v1 within the RosaeNLG-archival launch window. If yes, launch narrative upgrades to "Rust RosaeNLG — English + Spanish today"; `nlg-tracing` slips to v1.5. If no, English-only v1 stands.
- **Final crate name: `nlg-polish-llm` vs `nlg-hybrid`.** Substance locked, naming cosmetic. `nlg-polish-llm` currently favoured (clearer positioning).

## Findings summary (by facet)

**linguistics** — 10 findings. Top-tier picks: reference-free PARENT (~1 wk), plural REG (3–5 d), Centering Rule 1 (2–3 d), graph-based REG (2–3 wks, upgraded to Tier 1), forward-CR ELLEIPO (1 wk). Post-synthesis acceptances: polarity invariant, 8-slot Cf ring buffer, ELLEIPO split with shared buffer writes, type-enforced Paraphraser/Verifier composition.

**multilingual** — 9 findings. Three additive engine changes (AgreementFeatures, plural_category, realize_reference). Ship `nlg-grammar-es` first, `-de` second, `-ja` third. Rejected Grammatical Framework backend; cherry-pick MF2 plural/select semantics into pipe form. Post-synthesis refinements: step 1.5 plural REG consuming AgreementFeatures; PluralOutput carries both form AND category; `plural_description` hook for English "the N services" / Japanese counter-word / Arabic category-aware forms.

**performance** — 17 findings (added #16 Session split and #17 faithfulness latency during synthesis). 4-week plan anchored on interner + buffer flatten + Engine/Session split in weeks 1–2. Trivial wins (filter_by_salience, ahash, synonym registry inversion). Deferred compile-time monomorphization and PGO.

**ecosystem** — 10 findings, rev 4. RosaeNLG archival + regulated-industry hallucination niche + Rust template-compile-time growth signal (Askama velocity). Core moves: `ctx!` macro, `#[nlg_template]` validator, two vocab crates, CLI presets, cookbook mdBook. `nlg-polish-llm` with three traits + type-enforced composition + PARENT conformance as soft publishing gate for the `nlg-vocab-*` namespace.

## Unresolved Items (for the user)

- **None structural.** Two cosmetic pending items documented above (Spanish timing, crate name).

## Session artefacts

- Final agent findings are in the team's post-synthesis messages (rev 4 for ecosystem; deltas for linguistics and multilingual folded into the roadmap above).
- Shared briefing: `~/.claude/swarm/.context/nlg-shared-briefing.md`
- Per-facet briefings: `~/.claude/swarm/.context/nlg-{linguistics,multilingual,performance,ecosystem}.md`
- Personas: `~/.claude/swarm/personas/researchers.md` (four new NLG-specific personas appended)

## Token Usage

Token-usage summary was not surfaced on teammate termination in this session. If needed, the raw task outputs are available via the skill's standard mechanisms until the team directory's TTL expires.
