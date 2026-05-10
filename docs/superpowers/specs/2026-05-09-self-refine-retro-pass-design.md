# Self-Refine Retrospective Pass Design Spec

**Date:** 2026-05-09
**Status:** Approved (open questions resolved 2026-05-09)
**Companion spec:** [`2026-05-09-style-profile-design.md`](./2026-05-09-style-profile-design.md)

## Overview

Add a deterministic, document-scope refinement loop to Prosaic: render a `DocumentPlan` once, run pluggable diagnosers over the result to detect emergent failure modes the per-decision state machinery cannot catch, generate adversarial constraints from those diagnoses, re-render with the constraints applied, and iterate until the composite score converges or a bounded iteration ceiling is reached. The loop is the deterministic, audit-friendly Prosaic adaptation of the generate → critique → refine pattern from Madaan et al., *Self-Refine: Iterative Refinement with Self-Feedback* (arXiv:2303.17651).

The retro-pass is opt-in via a builder method, off by default, and preserves byte-for-byte determinism end-to-end. It composes with the `StyleProfile` from the companion spec — when a profile is set, the diagnoser pool gains a profile-distribution-divergence detector and the scorer optimizes against the profile's target dials.

## Background — what Self-Refine actually is

Madaan et al. demonstrate that LLMs improve on their own outputs by running a generate → self-critique → revise loop, with no fine-tuning. The mechanism in their setting is prompt-driven and stochastic; the principle is that some output failures are only visible *post-hoc*, after the whole artifact has been produced and can be inspected as a whole.

Prosaic's existing realism work — `connector-family budgets`, `sentence_rhythm` cadence penalty, `burst-pivot` amplitude scoring — is all **prospective single-pass**: the engine constrains decisions at the moment they're made, using sliding-window state. That mechanism cannot catch failures that only emerge across paragraph boundaries, across many sentences, or as a function of cumulative distribution. Examples already encountered during the realism pass:

- A document whose 4 paragraphs all open with `Additionally,` because each paragraph's local connective recency window is independent.
- A list-style cycle that gets reset at paragraph boundaries and then opens 3 consecutive paragraphs with `including X among others`.
- A 60-sentence document that lands on 78% medium-length sentences despite the per-decision rhythm scorer doing the right thing locally.
- A document whose connective frequencies satisfy every per-decision budget but, at the document level, drift far from the `StyleProfile`'s declared `preferred` weights.

Each of these failure modes is invisible to a sliding window but trivial to detect over the rendered document. The retro-pass is the architectural extension that lets Prosaic see and react to them.

## Goals

1. A consumer of `prosaic-core` can opt into retrospective refinement and observe measurably better document-scope statistics for the same input.
2. Determinism is preserved. Same `(plan, engine, session, profile, RefineConfig)` always produces the same output and the same iteration count.
3. Bounded cost. The loop terminates in `≤ max_iterations` (default 3), and each iteration is monotonic-or-halt.
4. Faithfulness is invariant. Every iteration's output satisfies `is_faithful() == true`. A re-render that breaks faithfulness is rejected and the loop falls back to the previous best.
5. The diagnoser set is open: callers can register their own `Diagnoser` implementations alongside the built-ins.
6. No effect on existing behavior when refinement is disabled. `RefineConfig::off()` (the default) is a no-op.

## Non-Goals

1. Single-render refinement. The retro-pass is document-scope only — it operates over `DocumentPlan` outputs, not individual `render()` calls. Per-render failures are already addressed by prospective scoring.
2. Stochastic perturbation. Iteration alternatives are enumerated deterministically from the diagnoses, not sampled. There is no "try a random variant and see if it scores better."
3. Mutation of the `Context`. Refinement adjusts engine state and candidate pools; it never edits the input.
4. Mutation of templates. The retro-pass cannot rewrite a template; it can only constrain selection among existing variants.
5. LLM critics, anywhere in the stack. Diagnosis is performed by deterministic detectors only. There is no LLM-judge dev-time tool, no gated/optional LLM dependency, no external-model eval companion crate scoped to this spec. Quality validation lives in the deterministic toolchain (PARENT precision gate, golden corpora, document-scope diagnosers below) — full stop.
6. Convergence to a global optimum. The loop is a bounded local-improvement procedure, not a search.

## Design — the loop

Pseudocode for the retro-pass:

```
best_output, best_score, best_diagnosis ← render_once(plan)
if not RefineConfig.enabled: return best_output

for iter in 0..config.max_iterations:
    constraints ← derive_constraints(best_diagnosis, profile)
    if constraints is empty: break  // nothing to refine
    candidate ← render_with_constraints(plan, constraints)
    if not candidate.is_faithful(): continue  // reject, retry next iter
    candidate_score, candidate_diagnosis ← score(candidate, profile)
    if candidate_score - best_score < config.min_improvement: break
    best_output, best_score, best_diagnosis ← candidate, candidate_score, candidate_diagnosis

return best_output
```

The four moving parts are diagnosers, constraint generators, scorer, and the iteration controller. Each is pluggable; the built-ins ship in `prosaic-core`.

### Diagnosers

A `Diagnoser` inspects a rendered document and emits zero or more `Diagnostic`s. The built-in set for v1:

| Name | What it detects |
|---|---|
| `ParagraphOpenerMonotony` | Same connective opens ≥ 3 of N paragraphs (configurable threshold) |
| `ListStyleFatigue` | Same `ListStyle` used in ≥ K of M most recent list positions across paragraphs |
| `RstRelationImbalance` | Any single RST relation accounts for > 60% of inter-sentence connectives |
| `DocumentScopeRhythm` | Sentence-length distribution stdev across the document drops below threshold (the per-decision rhythm scorer's local stdev was fine but the aggregate is flat) |
| `ConnectiveFamilySaturation` | Any connective family (additive, contrast, sequential, causal) emits more than its document-scope budget |
| `ProfileDistributionDrift` | (Active only when `StyleProfile` is set.) Any of the profile's target distributions diverges from observed by more than `delta`. Specifically targets `LengthDistribution`, `ConnectivePreferences.preferred` weights, and `ListStyleBias` frequency. |

Each `Diagnostic` carries a `severity: f32` (used by the scorer) and a hint about what constraint would address it.

### Constraint generators

A `Diagnostic` maps to one or more `RefineConstraint`s — additive instructions for the next render that narrow the engine's choices:

```rust
pub enum RefineConstraint {
    /// Forbid this connective for the next render.
    BlacklistConnective(String),
    /// Forbid this list style for the next render.
    BlacklistListStyle(ListStyle),
    /// Prepopulate discourse state with phantom history entries so the
    /// recency window starts already-saturated for these patterns.
    PrimeRecencyWindow { connectives: Vec<String>, list_styles: Vec<ListStyle> },
    /// Override the salience bias for this render only.
    OverrideSalienceBias(SalienceBias),
    /// Force a particular variant tier for a specific template key.
    ForceVariantTier { template_key: String, tier: Salience },
    /// Tighten the target sentence-length distribution for this render.
    TightenLengthDistribution(LengthDistribution),
}
```

Constraints are *additive within an iteration*: the diagnoser pool generates a set, the iteration applies all of them. Constraints are *not preserved across iterations* — each new iteration re-derives constraints from the latest diagnosis. (Otherwise constraint accumulation would shrink the candidate pool monotonically and could empty it.)

### Scorer

The composite score is a weighted sum:

```
score(document, profile) =
    w_repetition  * repetition_compliance(document)         // existing word-history scorer
  + w_rhythm     * rhythm_compliance(document, profile?)    // existing cadence + profile target
  + w_connective * connective_family_compliance(document)   // existing budget
  + w_paragraph  * paragraph_opener_diversity(document)     // NEW
  + w_list       * list_style_diversity(document)           // NEW
  + w_rst        * rst_relation_balance(document)           // NEW
  + w_profile    * profile_distribution_match(document, profile?)  // NEW, gated on profile presence
```

Weights default to a balanced set documented in `RefineConfig::default_weights()` and are overridable. Higher score = better.

The scorer is a pure function of the rendered document plus an optional profile. It does not mutate state.

### Iteration controller

Four termination conditions, in priority order:

1. `iter >= max_iterations` (hard ceiling, default 3)
2. `best_diagnosis` is empty (no detected failures, nothing to refine)
3. `current_diagnosis == previous_diagnosis` (loop is going in circles — same failure pattern reproducing under the constraints, no further progress possible)
4. `candidate_score - best_score < min_improvement` (default 0.01 — diminishing returns)

The diagnosis-equality halt at #3 catches pathological cycles where the constraint generator keeps producing constraints that re-elicit the same failure pattern. Detection is cheap (compare diagnosis sets by `(diagnoser_name, severity, constraint_kind)` tuples) and the halt is never wrong — if the diagnosis hasn't changed, the loop has no path to improvement no matter how many iterations remain.

The controller also rejects any candidate whose faithfulness check fails — the iteration counter still advances, but `best_*` is unchanged.

In the worst case the loop performs `max_iterations + 1` renders. In the typical case (one or two refinement opportunities) it performs 2–3, often terminating early via the diagnosis-equality halt.

## Architecture

### Module placement

New files:

- `prosaic-core/src/refine.rs` — `RefineConfig`, `Diagnoser` trait, `Diagnostic`, `RefineConstraint`, the iteration controller.
- `prosaic-core/src/refine_diagnosers.rs` — built-in diagnoser implementations.
- `prosaic-core/src/refine_score.rs` — composite scorer.

Re-exported from `lib.rs`. No new `no_std`/`alloc` issues.

### Type sketch

```rust
#[derive(Debug, Clone)]
pub struct RefineConfig {
    pub max_iterations: u8,
    pub min_improvement: f32,
    pub weights: RefineWeights,
    pub diagnosers: Vec<Arc<dyn Diagnoser>>,
}

impl RefineConfig {
    pub fn off() -> Self { /* enabled: false */ }
    pub fn balanced() -> Self { /* the default opt-in shape */ }
    pub fn with_diagnoser(mut self, d: Arc<dyn Diagnoser>) -> Self { ... }
}

pub trait Diagnoser: Send + Sync {
    fn name(&self) -> &'static str;
    fn diagnose(
        &self,
        document: &RenderedDocument,
        profile: Option<&StyleProfile>,
    ) -> Vec<Diagnostic>;
}

pub struct Diagnostic {
    pub diagnoser: &'static str,
    pub severity: f32,
    pub constraints: Vec<RefineConstraint>,
}

pub struct RenderedDocument {
    pub text: String,
    pub paragraphs: Vec<RenderedParagraph>,
    pub sentences: Vec<RenderedSentence>,  // flat view for scorers
    pub connectives_used: Vec<UsedConnective>,
    pub list_styles_used: Vec<UsedListStyle>,
}
```

`RenderedDocument` is a structured view of the rendered output that diagnosers and the scorer consume. It's built once per render by the engine and threaded through. (Today's `DocumentPlan::render` returns a flat `String`; this spec promotes the structured intermediate representation.)

### Engine integration

```rust
impl Engine {
    pub fn refine(mut self, config: RefineConfig) -> Self {
        self.refine_config = config;
        self
    }
}

impl DocumentPlan {
    /// Existing entry point — unchanged behavior when refinement disabled.
    pub fn render(&self, engine: &Engine, session: &mut Session)
        -> Result<String, ProsaicError>
    {
        if engine.refine_config.is_off() {
            return self.render_once(engine, session);  // existing path
        }
        let initial = self.render_once_structured(engine, session)?;
        let refined = engine.refine_loop(self, session, initial)?;
        Ok(refined.text)
    }

    /// New: returns the structured intermediate without flattening.
    pub fn render_structured(&self, engine: &Engine, session: &mut Session)
        -> Result<RenderedDocument, ProsaicError>
    {
        ...
    }
}
```

The structured render path becomes the canonical one internally; the legacy `String`-returning `render` remains the public entry point for backwards compatibility but flattens at the boundary.

### Constraint application

Constraints take effect by mutating the engine's per-render state *for the duration of one refinement iteration only*. Concretely:

- `BlacklistConnective` → temporarily added to `DiscourseState::forbidden_connectives` set.
- `BlacklistListStyle` → temporarily added to the list-style cycle's blacklist.
- `PrimeRecencyWindow` → recency windows are pre-loaded with phantom entries before the first paragraph renders.
- `OverrideSalienceBias` / `ForceVariantTier` → applied at variant selection.
- `TightenLengthDistribution` → overrides the profile's length target for this render only.

Each iteration starts from a fresh session reset (or a controlled clone of the session at retro-pass entry) so constraints from one iteration don't leak into the next.

## Composition

### With `StyleProfile`

When a profile is set:

- `ProfileDistributionDrift` diagnoser is included automatically.
- The scorer's `w_profile` term contributes proportionally to profile match quality.
- Constraint generation can synthesize `TightenLengthDistribution` and `BlacklistConnective` (for connectives the profile forbids that managed to slip in via the prospective fallback).

When no profile is set, the loop runs against the structural diagnosers only and the profile term contributes 0 to the score.

### With `DocumentPlan`

The retro-pass is gated on `DocumentPlan::render`. Single `Engine::render` calls do not invoke refinement (per the non-goal). `render_parallel` will skip refinement in v1 — the parallel path is for throughput-critical batch runs where the latency cost of multi-pass refinement is unacceptable; consumers who want both can run `render` (refined) and `render_parallel` (unrefined) for different code paths.

### With faithfulness gate

Every iteration's output is scored by `score_faithfulness`. If `is_faithful()` returns false the candidate is rejected and the iteration counter still advances. If `max_iterations` is reached without any faithful improvement, the loop returns `best_output` from the original render — never an unfaithful refinement.

This means the retro-pass is *strictly safe*: the worst case is identical to single-pass output. The gate makes refinement a Pareto improvement or no change.

### With `Variation`

`Variation` and `RefineConstraint` compose by intersection. The variation strategy picks among candidates *after* constraints have filtered the pool. If filtering empties the pool, the engine falls back to the unfiltered pool (same behavior as `StyleProfile` filter fallback).

### With `Strictness`

`Strictness::Strict` and the retro-pass are independent. Refinement does not introduce new ways for a render to fail; it just chooses among already-valid renders.

## Testing strategy

### TDD layer (logic — pure functions)

1. Each built-in diagnoser in isolation: a focused test feeds a hand-crafted `RenderedDocument` exhibiting the targeted pattern and asserts the diagnoser fires; a negative test feeds a clean document and asserts no diagnostic. Six diagnosers → twelve focused tests minimum.
2. Constraint generation determinism: same diagnostic set → same constraint set, every time. Property test over 1000 random diagnostic permutations.
3. Scorer monotonicity: for a synthetic pair of documents where one strictly improves a single dimension (e.g., adds paragraph-opener diversity without changing anything else), the score increases.
4. Iteration controller termination: synthetic engine that always produces the same output → loop terminates after 1 iteration. Synthetic engine that improves by exactly `min_improvement / 2` per iteration → loop terminates by `min_improvement` rule.
5. Faithfulness invariant: synthetic constraint that produces an unfaithful render → loop rejects and falls back to previous best.

### Test-after layer (golden corpus)

1. Six golden fixtures under `prosaic-core/tests/refine/`, one per built-in diagnoser, each containing:
   - An input plan that reliably triggers the diagnostic.
   - The pre-refine output (what the engine produces today).
   - The post-refine output (what one iteration of refinement produces).
   - The diagnostic delta.
2. A composite "kitchen sink" fixture exhibiting 3+ overlapping diagnostics, verifying the constraint generators don't collide.
3. A `StyleProfile` integration fixture: render under `concise-professional` with refinement on vs off, assert profile-match score improves by ≥ X.
4. Determinism: 1000-run repeatability across all fixtures.
5. Demo example `prosaic-core/examples/refine_pass_demo.rs` with annotated before/after pairs, mirroring `sentence_rhythm_demo.rs`.

### Performance regression gate

The retro-pass adds latency proportional to `max_iterations + 1` renders. CI tracks median latency for the kitchen-sink fixture under `refine: off` vs `refine: balanced` vs `refine: balanced(max_iterations=3)`; regressions beyond a documented budget fail the build.

## Resolved decisions

The five open questions on the original draft were resolved 2026-05-09. Decisions and rationale:

1. **Diagnoser execution — RESOLVED: sequential.** The default set of 6 diagnosers is small enough that sequential execution wins on simplicity; the `Diagnoser` trait is `Send + Sync` so future parallelization (rayon `par_iter` over the diagnoser slice) is non-breaking when custom diagnoser sets grow large enough to justify it.
2. **Scorer weights — RESOLVED: static for v1, offline tuner filed as future work.** `RefineWeights::default()` ships documented defaults; `RefineConfig::with_weights()` allows per-engine override. No tuning binary in v1. A future offline scorer-weight tuner that reads a corpus and emits an optimized `RefineWeights` TOML is tracked at `docs/plans/refine-scorer-tuner.md`. Constraint: the tuner stays deterministic and never introduces an LLM dependency, per the no-LLM rule.
3. **`RefineConstraint` Boost variants — RESOLVED: forbid-only.** Constraints in v1 are all forbid-or-prime (`BlacklistConnective`, `BlacklistListStyle`, `PrimeRecencyWindow`, `OverrideSalienceBias`, `ForceVariantTier`, `TightenLengthDistribution`). Symmetric `Boost*` variants are not included — they would require committing to a magnitude-vs-weight semantics that hasn't shown up as a real diagnoser need yet.
4. **Cross-iteration halt — RESOLVED: yes, add diagnosis-equality halt.** The iteration controller now has four termination conditions (see *Iteration controller* above), with diagnosis-equality landing at priority #3. Detection is cheap; halt is never wrong; pathological cycles terminate one iteration sooner.
5. **Language-pack hook — RESOLVED: add `Language::is_connective_opener()` now.** The `Language` trait grows the extension point in v1 alongside this spec's implementation, with an English default impl and explicit overrides for `prosaic-grammar-{es,de}`. Filing the hook now (rather than deferring) costs almost nothing and avoids a non-breaking-change gymnastic later when the first non-English diagnoser ships.

## Migration / backwards compatibility

- **No breaking changes.** `RefineConfig::off()` is the implicit default; engines without `.refine(...)` produce byte-identical output.
- **Public API additions only.** New types (`RefineConfig`, `Diagnoser`, `RenderedDocument`, etc.) and one new method (`Engine::refine`).
- **`DocumentPlan::render_structured` is additive.** Existing `DocumentPlan::render` keeps its `String` return type and flattens internally.
- **Cargo features.** Lives in default `prosaic-core`. No new feature flag (the loop must be a first-class capability, not an opt-in compilation surface).
- **Versioning.** Same v0.6.0 minor bump as `StyleProfile`. `RefineConfig` and `RefineConstraint` are `#[non_exhaustive]`.

## Out of scope (explicitly)

- Single-render refinement (`Engine::render` calls remain single-pass).
- LLM-driven diagnosis or LLM-judge tooling, runtime or dev-time. Prosaic does not depend on LLMs in any code path; this spec inherits that constraint without exception.
- Stochastic perturbation. The whole loop is deterministic.
- Refinement that mutates the input `Context`.
- Refinement of partials at the partial level (partials are inlined at render time; refinement operates over the resulting document).
- Adaptive `max_iterations` based on improvement velocity.
- Persistence of refinement traces. The pass produces an output; introspection of *why* a particular refinement was chosen is a Studio-side concern, not a `prosaic-core` runtime feature.

## Sequencing

This spec lands after `StyleProfile` because it consumes profile dials as scoring targets. Implementation order, once both are approved:

1. **StyleProfile** (per `2026-05-09-style-profile-design.md`).
2. **Self-Refine retro-pass** — `refine.rs`, the six built-in diagnosers, the scorer, the iteration controller, golden tests.
3. **Studio integration** — surface `RefineConfig` in `prosaic.toml`, show before/after diffs in Studio's preview pane. Separate spec; not required for the engine work to land.

The two engine specs are designed to be implementable in series with no rework: StyleProfile lands as a self-contained dial layer; the retro-pass lands on top, reading those dials as targets without modifying StyleProfile's surface.

## References

- Madaan, A., Tandon, N., Gupta, P., Hallinan, S., Gao, L., Wiegreffe, S., Alon, U., Dziri, N., Prabhumoye, S., Yang, Y., Gupta, S., Majumder, B. P., Hermann, K., Welleck, S., Yazdanbakhsh, A., Clark, P. *Self-Refine: Iterative Refinement with Self-Feedback.* arXiv:2303.17651 (Mar 2023, rev. May 2023). NeurIPS 2023.
- `docs/superpowers/specs/2026-05-09-style-profile-design.md` — the dial layer this spec consumes.
- `docs/superpowers/specs/2026-04-14-discourse-aware-rendering-design.md` — the discourse state the diagnosers inspect.
- `docs/hallucination-by-construction.md` — the faithfulness invariant every iteration must preserve.
- `docs/prosaic-realism-before-after-samples.md` — the prospective single-pass realism work this retro-pass complements.
- `prosaic-core/src/document.rs`, `discourse.rs`, `salience.rs`, `engine.rs` — the existing surfaces the loop wires into.
