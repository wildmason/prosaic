# Givenness Hierarchy for REG transitions

**Date:** 2026-05-09
**Status:** Plan note (not yet a spec)
**Inspiration:** Higger & Williams, *GAIA: A Givenness Hierarchy Theoretic Model of Situated Referring Expression Generation* (CogSci 2024)

## What this is

A small design note exploring whether to extend `prosaic-core/src/reg.rs` with Givenness-Hierarchy-based transition rules between `ReferenceForm` variants. Not yet a spec — the open question section below has to resolve before code lands.

## Context

The Givenness Hierarchy (Gundel, Hedberg, & Zacharski, 1993) ranks referent forms by *cognitive accessibility* — how active a referent is in the addressee's mental representation. The classical six-level ranking maps onto English referring expressions roughly as:

| Level | Form | Example |
|---|---|---|
| In focus | Pronoun | "it" |
| Activated | Demonstrative pronoun | "this", "that" |
| Familiar | Demonstrative + noun | "this change" |
| Uniquely identifiable | Definite NP | "the change" |
| Referential | Indefinite specific NP | "a particular change" |
| Type identifiable | Indefinite NP | "a change" |

Prosaic's `ReferenceForm` enum (`prosaic-core/src/discourse.rs`, realized in `prosaic-core/src/language.rs:393-413`) already has six variants — `Pronoun`, `Possessive`, `Demonstrative`, `Zero`, `Full`, `ShortName` — that span roughly the same range. What's missing is a principled set of *transition rules* that decide which form to emit on a given mention based on the addressee's likely cognitive state.

The current selection uses centering theory (Cb tracking, ambiguity gating) and a mention-count threshold. That works, but it doesn't model the asymmetry between *de-referencing* (full → pronoun once activation rises) and *re-referencing* (pronoun → full when activation drops or distractors enter). Givenness-Hierarchy theory makes that asymmetry explicit.

## What GAIA contributes — and the caveat

GAIA's title ends with **"Situated** Referring Expression Generation". Williams' MIRRORLab is an HRI lab. The closest reachable companion paper from the same author group is *Uncovering the Rules of Entity-Level Robotic Working Memory* — robot working-memory grounded, not text-generation grounded. The "situated" qualifier in GAIA is therefore not a hedge; it is the load-bearing premise. Two implications:

1. **The cognitive-state machinery probably ports.** Givenness Hierarchy is a theory about the addressee's mental state, not about physical environments. Transition rules between accessibility levels — what activates a referent, what de-activates it, what counts as a distractor — are the part of GAIA most likely to transfer.
2. **The situated/perceptual machinery almost certainly does not port.** GAIA's robot-working-memory framing means perceptual salience signals (visual prominence, spatial proximity, gaze, scene state) are likely woven into the activation rules rather than separable from them. The further we read into the Williams lab's broader output, the more we should expect the discourse-internal accessibility transitions to be entangled with situated machinery rather than cleanly factor-able.

This caveat is the reason this is a plan note and not a spec. Before any code commitment we need the full paper to confirm:

- Are the discourse-internal transition rules cleanly separable from the situated/perceptual rules? (Provisional expectation: probably not.)
- Does the paper define accessibility-level transitions in operational terms (when does a referent move up the hierarchy? when does it move down?), or only in terms of perceptual scene state?
- Are the transition rules stable across mention sequences, or do they require situational context the data-to-text use case doesn't provide?

If the discourse-internal subset is recoverable, this plan becomes a small spec. If it isn't — which is the more likely outcome given the lab's HRI-grounded research line — the routing pivots to: keep the existing centering-theory machinery, file a `docs/plans/centering-extensions.md` note that captures the parts of Givenness Hierarchy theory we *can* lift directly from Gundel/Hedberg/Zacharski (1993) without GAIA's HRI overlay (specifically, the activation/de-activation transitions for referents in pure linguistic context), and close this plan note as "GAIA-the-paper does not transfer; the underlying theory it builds on does."

The fallback path is concrete and known. The upside path is gated on the paper read. That asymmetry is why this stays a plan note rather than blocking on a paper fetch we may not be able to complete.

## Sketch of what would change

Assuming the import is feasible, the change is small in surface area but conceptually meaningful:

1. **Add a `GivennessLevel` to `Session`** — enum tracking the highest accessibility level each entity has reached in this discourse. Resets per paragraph the way other discourse state already does.
2. **Replace mention-count thresholds in `discourse.rs::select_reference_form` with hierarchy-transition rules** — promote on consecutive subject mention, demote on long gap or distractor entry, etc. The exact rules are what the paper review is for.
3. **Add transition golden tests** — a fixture corpus that exhibits each transition (activation, de-activation, ambiguity-induced demotion) and asserts the right form is chosen.

No new public API. No breaking change. The `{name|refer}` and `{name|possessive}` pipes consume the new transition logic transparently.

## Composition with existing systems

- **Centering theory.** Givenness Hierarchy and centering aren't competing models — they describe different things. Centering tracks *who the discourse is about* (Cb, Cf); Givenness Hierarchy tracks *how active each referent is*. Both should run; the form selector reads from both.
- **REG (Dale & Reiter / Krahmer).** Unchanged. REG decides what *content* to put in the full form when one is needed (which attributes disambiguate); GAIA-style transitions decide *whether* a full form is needed at all.
- **`StyleProfile::PronounDensity`** (per `2026-05-09-style-profile-design.md`). The profile dial sits *on top of* the transition rules — `Low` makes the rules conservative (stay in full form longer), `High` makes them aggressive (promote to pronoun earlier). The dial is a multiplier on the threshold; the transition mechanism is unchanged.
- **`docs/hallucination-by-construction.md`.** Givenness-Hierarchy transitions don't change the entailment story — every emitted form is still a function of names already in the `Context`. The faithfulness gate runs unchanged.

## Open questions before this becomes a spec

1. **Confirm the discourse-internal rules separate from the situated rules in GAIA.** Read the paper.
2. **Decide on the `GivennessLevel` enum's exact level set.** The classical six-level hierarchy may not all map to forms Prosaic emits — `Referential` (indefinite specific) and `Type identifiable` (bare indefinite) may not be useful for data-to-text where every entity is, by construction, uniquely identifiable. A Prosaic-internal three- or four-level hierarchy may be cleaner.
3. **Decide on per-paragraph reset semantics.** Does activation persist across paragraph boundaries, or reset like the rest of the discourse state? Probably reset, but the paper may have data on this.
4. **What counts as a distractor for de-activation?** Centering already tracks ambiguity. The Givenness-Hierarchy transition rules may need a richer distractor signal (e.g., "another entity of the same type was mentioned in the last two sentences").
5. **Does the change affect the `Demonstrative` form's surface realization?** `language.rs:409` currently always emits `"this"` for English `Demonstrative`. The hierarchy might motivate differentiating between *pronominal* demonstrative (`"this"` alone, when referent is *Activated*) and *adnominal* demonstrative (`"this change"`, when referent is *Familiar*). That's a `Language::realize_reference` extension, not a `ReferenceForm` change.

## Tier and sequencing

Lower priority than `StyleProfile` and Self-Refine retro-pass — those have user signoff pending and a clearer path. This plan note is the open question; if the paper review answers cleanly, it becomes a Tier-2 enhancement to `reg.rs` / `discourse.rs`. If the paper review is messy, it stays at plan-note status indefinitely with a note explaining why.

## References

- Gundel, J. K., Hedberg, N., Zacharski, R. *Cognitive Status and the Form of Referring Expressions in Discourse.* Language 69 (1993).
- Higger, M., Williams, T. *GAIA: A Givenness Hierarchy Theoretic Model of Situated Referring Expression Generation.* Proceedings of the Annual Meeting of the Cognitive Science Society (CogSci 2024).
- `prosaic-core/src/reg.rs` — current Dale & Reiter / Krahmer REG implementation.
- `prosaic-core/src/discourse.rs` — current centering-theory state and `ReferenceForm` selection logic.
- `prosaic-core/src/language.rs:393-413` — current per-`ReferenceForm` surface realization for English.
- `docs/superpowers/specs/2026-05-09-style-profile-design.md` — `PronounDensity` dial that composes with this work.
