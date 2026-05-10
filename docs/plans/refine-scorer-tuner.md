# Offline scorer-weight tuner for the Self-Refine retrospective pass

**Date:** 2026-05-09
**Status:** Future work (deferred from v1)

## What this is

A note tracking a future deferred companion to `docs/superpowers/specs/2026-05-09-self-refine-retro-pass-design.md`: an offline tool that takes a corpus of structured-event sequences plus the rendered prose those sequences produce under various `RefineWeights` configurations, runs a deterministic parameter sweep, and emits an optimized `RefineWeights` TOML that consumers can drop into their `prosaic.toml`.

This is **not in v1** of the retro-pass. The v1 spec ships static `RefineWeights::default()` weights with per-engine override via `RefineConfig::with_weights()`. This plan note exists so that a future implementation has a place to land without hijacking the retro-pass spec or being lost as a TODO comment.

## Why we deferred it

The retro-pass spec resolution on 2026-05-09 chose static weights for v1. Three reasons hold:

1. **No corpus exists yet.** Offline tuning needs a labeled corpus of "good" rendered prose to optimize against. Prosaic doesn't have one curated for this purpose, and building one is its own workstream.
2. **No consumer demand yet.** The static defaults will be tuned by hand based on the realism-pass evidence (`docs/prosaic-realism-before-after-samples.md`, `docs/sentence-rhythm-variance-evidence.md`) plus follow-on golden fixtures from the retro-pass landing. Real consumer feedback should drive whether automated tuning is worth the binary surface.
3. **Risk of premature optimization.** Building a tuner before the retro-pass has shipped means tuning against a moving target. Better to ship the retro-pass, accumulate corpus + complaints, and only then build the tuner against stable inputs.

## Shape of the eventual tool

When this lands, the rough shape is:

- New binary crate `prosaic-tune` (or feature-gated subcommand on `prosaic-cli`).
- Reads a corpus directory: each entry is `{events.json, expected.txt}` — a structured-event payload and a hand-curated reference rendering of what good prose for that input looks like.
- Loads a `prosaic.toml` project (templates, vocab, optional `StyleProfile`).
- Runs a parameter sweep over `RefineWeights` — grid search across a small bounded space of weight combinations, deterministic ordering, fixed seed.
- For each weight configuration, renders every corpus entry through the retro-pass and computes a score against the expected reference using a deterministic similarity metric (PARENT-style entailment, sentence-rhythm distance, list-style histogram, etc.) — not an LLM judge, per the no-LLM rule.
- Emits the top-K weight configurations as a `RefineWeights` TOML that operators can drop into their project.

The tool is offline, deterministic, runs on CPU, ships no LLM dependency, and produces an artifact that gets reviewed by a human before being committed.

## What this is *not*

- **Not online tuning.** The runtime never adjusts its own weights. Tuning happens offline; the result is a TOML the operator commits.
- **Not LLM-judge driven.** Scoring inside the tuner uses the same deterministic metrics the retro-pass uses. No LLM-as-judge, no external model dependency.
- **Not a replacement for hand-curated defaults.** `RefineWeights::default()` stays the source of truth for projects that don't tune. The tuner produces *alternative* weights for projects that have a reason to deviate.

## Non-blockers for landing

When demand surfaces and we're ready to build this, the prerequisites are:

1. The retro-pass itself has shipped and been in real use long enough that we know which weight axes actually matter.
2. A small reference corpus exists somewhere — either built in-tree under `prosaic-tune/corpus/` or referenced from a real downstream project.
3. The `RefineWeights` struct surface is stable (no field churn; fields are `#[non_exhaustive]` per the retro-pass spec, so additions don't break tuning configs).

None of these are present today. All three should fall out naturally once the retro-pass lands and gets exercised.

## References

- `docs/superpowers/specs/2026-05-09-self-refine-retro-pass-design.md` — the retro-pass spec this tool would extend.
- `docs/superpowers/specs/2026-05-09-style-profile-design.md` — `StyleProfile` defines the per-project voice the tuner is optimizing toward when a profile is in scope.
- `feedback_no_llm_in_prosaic.md` (memory) — the rule that constrains the tuner to deterministic scoring methods.
