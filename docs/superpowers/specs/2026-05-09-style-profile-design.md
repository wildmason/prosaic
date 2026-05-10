# StyleProfile Design Spec

**Date:** 2026-05-09
**Status:** Approved (open questions resolved 2026-05-09)

## Overview

Add a `StyleProfile` to Prosaic — a deterministic, declarative configuration object that biases the engine's existing rendering choices toward a target voice. Profiles persist across renders, are loaded from `prosaic.toml` via `prosaic-project`, and compose with the existing builder dials (`Variation`, `Strictness`, `sentence_rhythm`, salience thresholds) without breaking determinism or backwards compatibility. They are the deterministic, audit-friendly analog of the "long-term persona memory" idea explored in Xu et al., *Personalized Generation In Large Model Era: A Survey* (arXiv:2503.02614) — adapted away from neural-memory architectures and into explicit dials.

This is also the spec for the "style-guide enforcement layer" deferred from the Prosaic Studio design (`2026-04-17-prosaic-studio-design.md` §29, §517).

## Background

Prosaic currently models intra-session discourse state — entity focus, word frequency, list-style cycle, connective recency, sentence-rhythm window — but has no cross-session memory of preferences. Each `Engine` is constructed identically; each render makes the same choices given the same inputs. Two consequences follow:

1. **No durable voice.** A consumer that wants its prose to read terse-and-formal one place and verbose-and-conversational another has to re-build the engine with bespoke registrations each time, or branch templates, or post-process the output. None of these scale.

2. **No optimization target for retrospective refinement.** The companion Self-Refine retrospective-pass spec (forthcoming) needs a scoring function. Without a profile, "what is good prose?" is a hardcoded composite of repetition + cadence; with a profile, "good" becomes "matches the declared target distribution."

The Personalized Generation survey frames this as a transition from style transfer (one-shot) to long-term memory (persistent). For LLMs the implementation is Mem0-style neural memory; for Prosaic the implementation is a declarative configuration object the operator authors and version-controls. The *concept* (durable per-consumer voice) ports cleanly; the *mechanism* (learned weights) does not.

## Goals

1. A consumer of `prosaic-core` can construct an `Engine` with a named `StyleProfile` and get measurably different prose for the same inputs versus a neutral profile.
2. A `prosaic-project` author can declare a profile in `prosaic.toml` and have it loaded automatically when the project is materialized.
3. Determinism is preserved end-to-end. Same `(template, context, session, language, profile)` always produces the same output.
4. The neutral profile (`StyleProfile::neutral()`) preserves existing behavior byte-for-byte. No engine-level breaking change.
5. The Self-Refine retrospective pass (separate spec) can read profile parameters as a scoring target without re-deriving them from output statistics.

## Non-Goals

1. Learned profiles. Profiles are authored, not inferred from usage. An "auto-tune from corpus" feature is explicitly out of scope; if it ever ships it is a separate workstream that emits a profile, not a runtime that mutates one.
2. Per-render profile mutation. Profiles are immutable for the lifetime of an `Engine`. Switching profiles between renders requires constructing a new engine (or passing a per-render override on a future `render_with_profile()` path — also out of scope for v1).
3. Profile-conditional templates. A template's *content* must not branch on the profile (no `{?profile.formal}…{/?}`). Profiles affect *selection* among already-authored variants, not the variants themselves. This boundary protects auditability — the set of possible outputs of a template stays inspectable from the template source alone.
4. Multi-user profile management at the engine layer. The call site is responsible for selecting the right profile per request; the engine only consumes the one it was built with.
5. Style transfer between profiles. There is no "rewrite output A in profile B" path. Profile changes mean re-rendering from input.

## Design — the dials

A `StyleProfile` is a struct of small, orthogonal dials. Each dial maps onto a specific decision the engine already makes; the profile shifts the bias of that decision rather than replacing it. The full surface for v1:

### 1. Verbosity

```rust
pub enum Verbosity { Terse, Neutral, Verbose }
```

Biases salience-tier selection when multiple variants are registered for the same template key. `Terse` prefers the lowest-detail variant available; `Verbose` prefers the highest. `Neutral` (default) lets the existing salience-from-context logic decide.

### 2. Sentence-length distribution target

```rust
pub struct LengthDistribution {
    pub short: f32,   // proportion of sentences ≤ N words
    pub medium: f32,  // ≤ M words
    pub long: f32,    // > M words
}
```

A target distribution the rhythm scorer optimizes toward. The current rhythm scorer is symmetric — it penalizes monotony but has no preferred shape. A profile distribution turns it into an asymmetric optimizer: e.g., `{short: 0.5, medium: 0.3, long: 0.2}` biases toward short-leading prose. Thresholds for short/medium/long boundaries live in the profile as well, with sensible defaults.

### 3. Connective vocabulary

```rust
pub struct ConnectivePreferences {
    /// Per-RST-relation allowed-connective pools. Missing key for a relation
    /// = use the engine's default pool for that relation. Some(Vec) restricts
    /// to the listed connectives. Empty Vec is forbidden by validation
    /// (would empty the pool and force fallback).
    pub allowed: HashMap<RstRelation, Vec<String>>,
    /// Per-RST-relation tie-breaker weights. Missing key = uniform weights.
    pub preferred: HashMap<RstRelation, Vec<(String, f32)>>,
}
```

Constrains and weights the connective recency window's selection, **per RST relation**. The map is keyed by `RstRelation` (additive, contrast, sequential, causal, elaboration, etc.) so a profile can say "use *Additionally* and *Furthermore* for additive, but *However* and *Conversely* for contrast" without a single flat-pool list muddling the two. Missing keys fall through to the engine's default pool for that relation; explicit empty pools are rejected at validation to prevent footgun configurations. The recency window's family-budget enforcement runs unchanged — the profile narrows the candidate set; the budget governs rotation within whatever set survives filtering.

### 4. List-style bias

```rust
pub enum ListStyleBias { Auto, Including, SuchAs, Dash, Bracketed }
```

When a `{items|join}` pipe needs to pick a style and the cycle isn't already constrained by recency, this dial breaks ties. `Auto` (default) preserves current rotation. The other variants nudge toward a specific opener while still respecting the anti-repeat rule. (You cannot use a profile to *force* one style every time — that's what `{items|join:bracketed}` is for. The profile sets the prior, not the verdict.)

### 5. Pronoun density

```rust
pub enum PronounDensity { Low, Default, High }
```

Adjusts the threshold at which `{name|refer}` switches from full form to short form to pronoun. `Low` keeps full forms longer (formal register). `High` switches to pronouns earlier (conversational register). Implementation is a small offset on the existing centering-theory transition rules; the rules themselves don't change.

### 6. Hedging calibration

```rust
pub struct HedgingCalibration {
    pub offset: i8,        // -50..=+50 added to confidence before hedge mapping
    pub forbid: Vec<String>, // hedges to never emit, e.g., ["perhaps"]
}
```

Shifts the deterministic confidence-to-hedge mapping. `offset: -10` over a context value of `conf=60` produces the hedge for `conf=50`. `forbid` removes specific hedges from the output set, falling through to the next applicable level.

### 7. Salience bias

```rust
pub enum SalienceBias { Lower, Auto, Higher }
```

Composes with `Verbosity` but applies even when only one variant exists per tier. Shifts the engine's salience thresholds (the cutoffs that classify a context as Low/Medium/High) up or down by a fixed delta, so the same numeric inputs land in different tiers. `Auto` is the default and uses whatever thresholds the engine was constructed with.

### Composability

These seven dials are deliberately small, independent, and orthogonal. A profile composes by setting any subset; the rest take their neutral values. Two profiles cannot be merged in v1 (no inheritance, no overlay) — see Open Questions.

## Architecture

### Module placement

New file: `prosaic-core/src/style.rs`. Re-exported from `lib.rs`. No `no_std`/`alloc` issues — the struct is a plain configuration object.

### Type sketch

```rust
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StyleProfile {
    pub name: String,
    pub verbosity: Verbosity,
    pub sentence_length: LengthDistribution,
    pub connectives: ConnectivePreferences,
    pub list_style_bias: ListStyleBias,
    pub pronoun_density: PronounDensity,
    pub hedging: HedgingCalibration,
    pub salience: SalienceBias,
}

impl StyleProfile {
    /// Baseline. Behaves identically to no profile — every dial in its
    /// neutral position. Use as the starting point for builders.
    pub fn neutral() -> Self { ... }

    pub fn builder(name: impl Into<String>) -> StyleProfileBuilder { ... }

    #[cfg(feature = "serde")]
    pub fn from_toml(s: &str) -> Result<Self, StyleProfileError> { ... }
}
```

### Builder integration

```rust
impl Engine {
    pub fn style_profile(mut self, profile: StyleProfile) -> Self {
        self.style_profile = profile;
        // Recompute any derived state that depends on profile.
        // For v1 this is just salience thresholds; the rest are read
        // at decision time from self.style_profile.
        self
    }
}
```

The profile lives on the engine, not the session. Sessions may consult the profile at decision time but do not own it.

### Decision-site wiring

Each dial maps to one or two specific decision sites. The implementation plan is to pass the profile (or a reference to it) into those sites' existing functions, not to add a new abstraction layer:

| Dial | Decision site |
|---|---|
| `Verbosity` | `Engine::select_variant` salience-tier preference |
| `LengthDistribution` | `Engine::sentence_rhythm_score` target shape |
| `ConnectivePreferences` | `DiscourseState::select_connective` candidate pool |
| `ListStyleBias` | List-style cycle tiebreaker in the join pipe |
| `PronounDensity` | REG transition thresholds in `reg.rs` |
| `HedgingCalibration` | `hedge` pipe's confidence → output mapping |
| `SalienceBias` | Salience threshold offsets in `salience.rs` |

No new state machines. No mutation. Each site reads the profile and adjusts a single existing computation.

## Persistence — `prosaic-project` integration

A `prosaic.toml` author declares the profile inline, or references a separate file:

```toml
# Inline
[style_profile]
name = "concise-professional"
verbosity = "terse"
list_style_bias = "bracketed"
pronoun_density = "high"

[style_profile.sentence_length]
short = 0.5
medium = 0.3
long = 0.2

[style_profile.connectives.allowed]
additive = ["Additionally", "Furthermore", "Also"]
contrast = ["However", "Conversely"]
sequential = ["Then", "Subsequently"]

[style_profile.connectives.preferred]
additive = [["Additionally", 1.0], ["Furthermore", 0.6]]

[style_profile.hedging]
offset = -10
forbid = ["perhaps"]
```

Or, by reference:

```toml
[style_profile]
extends = "profiles/concise-professional.toml"
```

`prosaic-project::Project::materialize()` reads the section, builds a `StyleProfile`, and applies it to the constructed `Engine` before returning. Projects with no `[style_profile]` section get `StyleProfile::neutral()` and are byte-for-byte identical to today's behavior.

A small **profile catalog** ships with `prosaic-project`: `neutral`, `concise-professional`, `verbose-narrative`, `regulatory-formal`. These are reference profiles, not defaults, and exist primarily to give new projects a starting point and to provide test fixtures with stable golden outputs.

## Composition with existing systems

### Variation

`Variation` controls *how* the engine picks among variants given the engine's seed/cycle state. `StyleProfile` controls *which subset of variants* the picker considers. Composition order is profile-filters-then-variation: the profile narrows the candidate pool, then `Variation::Fixed | Cyclic | Seeded` picks from that pool. If a profile's filter empties the pool, the engine falls back to the unfiltered pool — profile filtering is a preference, not a hard constraint, to avoid ProsaicError-by-misconfiguration.

### Salience

`Verbosity` and `SalienceBias` interact. `Verbosity` is an explicit preference at variant-selection time; `SalienceBias` shifts the input thresholds that decide which tier a context lands in. Both can be set; the rule is that `SalienceBias` runs first (changing tier classification), then `Verbosity` runs (preferring within-tier or cross-tier per its setting). This ordering is documented and tested.

### Self-Refine retrospective pass (forthcoming spec)

The retrospective-pass scorer needs a target. Without a profile its target is a hardcoded composite of repetition + cadence variance. With a profile, the scorer reads:

- `LengthDistribution` as the target sentence-length shape
- `ConnectivePreferences.preferred` as the target connective frequency
- `ListStyleBias` as the target list-style frequency
- `PronounDensity` as the target referring-expression density

and composes a single profile-aware score. The refine pass re-renders if the document-scope statistics drift too far from the target. This is the dial-eats-its-own-dogfood property: the profile both biases per-decision selection AND defines the target the post-hoc refinement optimizes toward.

## Testing strategy

### TDD layer (logic — pure functions)

1. `StyleProfile::neutral()` round-trips through every decision site without changing output. Add a property-test that renders a representative corpus with `None` profile vs `neutral()` profile and asserts byte equality.
2. Each dial in isolation: a focused test per dial that holds the other six neutral and asserts the targeted decision site changes its choice. E.g., `verbosity_terse_picks_lowest_salience_variant`, `list_style_bias_including_breaks_cycle_toward_including`.
3. Composition tests: pairs of dials that interact (`Verbosity` × `SalienceBias`, `ConnectivePreferences` × `Variation`) — assert the documented ordering.
4. TOML parsing: round-trip a representative profile through serde, assert struct equality.
5. Determinism: same `(profile, template, context, session)` input yields identical output across 1000 runs (existing pattern in the test suite).

### Test-after layer (golden corpus)

1. A new fixture under `prosaic-core/tests/style_profiles/` with the same input rendered under each catalog profile (`neutral`, `concise-professional`, `verbose-narrative`, `regulatory-formal`) plus a baseline. Outputs are golden files; CI fails on drift.
2. A "different profiles produce visibly different prose" assertion: render the same 20-event corpus under `concise-professional` and `verbose-narrative` and assert the diff is non-trivial (e.g., aggregate sentence-length stdev differs by ≥ 2 words, list-style histograms differ by ≥ 30 percentage points). This is the proof that profiles do something visible, not just internally.
3. Per-dial demo example under `prosaic-core/examples/style_profile_demo.rs` that renders a single corpus seven times, toggling each dial in turn, with explanatory annotations. Mirrors `sentence_rhythm_demo.rs` in shape and serves as the human-readable evidence artifact.

### Faithfulness regression gate

Every catalog profile must preserve PARENT precision = 1.0 and `polarity_match = true` over the test corpus. Style biases must never break entailment.

## Resolved decisions

The five open questions on the original draft were resolved 2026-05-09. Decisions and rationale:

1. **Per-RST-relation connective pools — RESOLVED: per-relation map.** `ConnectivePreferences.allowed` and `.preferred` are both `HashMap<RstRelation, ...>` (see §3 above). Rationale: a flat pool conflates connective vocabulary across rhetorically distinct moves, which loses signal exactly where a real voice cares most ("this profile is concise on contrast but verbose on elaboration" is a distinction operators want and shouldn't have to give up to a flat-pool simplification). Authoring overhead is real but contained — most profiles will set 1–2 relations and fall through to defaults for the rest.
2. **`LengthDistribution` semantics — RESOLVED: soft prior.** The rhythm scorer treats the profile's distribution as a nudge alongside its existing repetition + cadence terms; the Self-Refine retro-pass uses it as one signal among several rather than a hard constraint. Rationale: hard targets risk infinite loops when the input doesn't admit the requested shape; soft priors degrade gracefully.
3. **Profile inheritance / overlay — RESOLVED: file-level only.** `extends = "path"` in `prosaic.toml` is the only composition mechanism in v1. No struct-level `base.with_overlay(overlay)` API. Rationale: keeps the authoring story clean (one profile per file, one file per project section); revisit only if multi-tenant or per-request-overlay scenarios surface.
4. **Profile-template compatibility check — RESOLVED: runtime warning, not hard error.** When a profile demands behavior the registered templates can't satisfy (e.g., `Verbosity::Verbose` with no high-tier variants), the engine logs a warning and proceeds. Rationale: profile-template compatibility is a soft signal; treating it as a hard error couples profile authoring to vocab completeness in a way that breaks load-time of incomplete-but-correct vocab modules.
5. **Hedging `forbid` fallthrough — RESOLVED: toward more confident.** When a profile forbids a specific hedge that would otherwise render for the given confidence, fall through to the next-strongest available hedge (toward higher confidence), not the next-weakest. Rationale: forbidding a hedge usually expresses "I don't want wishy-washy framing here" — falling through to a firmer hedge matches the operator's voice intent.

## Migration / backwards compatibility

- **No breaking changes.** Engines with no profile use `StyleProfile::neutral()` implicitly and produce byte-identical output.
- **Cargo features.** `StyleProfile` lives in default `prosaic-core` (no feature flag). TOML serde behind the existing `serde` feature.
- **Project format.** `[style_profile]` is optional. Existing `prosaic.toml` files keep working unchanged.
- **Versioning.** This is a v0.6.0 minor bump. The struct layout is `#[non_exhaustive]` so future dials can be added without breaking downstream pattern matches.

## Out of scope (explicitly)

- Auto-tuning profiles from observed output corpora.
- Profile-conditional template *content*.
- LLM-derived profile suggestions.
- Cross-language profile sharing (a profile is bound to its `Language` since some dials, e.g., pronoun density, are language-specific in their thresholds).
- Profile diffing / migration tooling.
- Profile UI inside Prosaic Studio. (Studio surfaces the catalog and lets the user pick one; the editor for authoring custom profiles is a follow-up Studio milestone.)

## Sequencing

This spec ships before the Self-Refine retrospective-pass spec, because the latter consumes the former's profile as its scoring target. The implementation order, once both specs are approved:

1. Land `StyleProfile` per this spec (engine + serde + project integration + catalog + golden tests).
2. Land Self-Refine retrospective pass with profile-aware scoring.
3. Studio profile picker (separate milestone, separate spec).

## References

- Xu, Y., Zhang, J., Salemi, A., Hu, X., Wang, W., Feng, F., Zamani, H., He, X., Chua, T-S. *Personalized Generation In Large Model Era: A Survey.* arXiv:2503.02614 (Mar 2025, rev. May 2025).
- `docs/superpowers/specs/2026-04-14-discourse-aware-rendering-design.md` — the discourse state this spec hooks into.
- `docs/superpowers/specs/2026-04-17-prosaic-studio-design.md` — the deferred "style guide enforcement layer" this spec fulfills (§29, §517).
- `docs/hallucination-by-construction.md` — the bounded-input/source-set guarantees the profile must not break.
- `prosaic-core/src/discourse.rs`, `salience.rs`, `reg.rs`, `hedge.rs` — the existing decision sites this spec wires into.
