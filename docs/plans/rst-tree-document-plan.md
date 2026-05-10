# RST-tree DocumentPlan structure

**Date:** 2026-05-09
**Status:** Plan note (not yet a spec)
**Inspiration:** Liu & Demberg, *RST-LoRA: A Discourse-Aware Low-Rank Adaptation for Long Document Abstractive Summarization* (NAACL 2024, arXiv:2405.00657)

## What this is

A design note exploring whether `DocumentPlan` should grow a hierarchical RST-tree variant alongside the existing flat-grouping strategies (`ByEntity`, `ByAction`). Not yet a spec — the open questions below have to resolve before code lands, and the priority is below the StyleProfile + Self-Refine retro-pass specs already in flight.

## Context

Liu & Demberg's RST-LoRA (NAACL 2024) integrates Rhetorical Structure Theory (Mann & Thompson, 1988) into a parameter-efficient fine-tuning method for long-document abstractive summarization. The neural mechanism (LoRA adapter) is irrelevant to Prosaic. The conceptual contribution worth examining is that **RST relations work better when applied as hierarchical tree structure than as flat per-edge labels** — at least in the long-document regime that paper studies. The paper "incorporate[s] the type and uncertainty of rhetorical relations" to enhance summarization quality.

Prosaic's current `DocumentPlan` is a flat grouping (`prosaic-core/src/document.rs`):

- `GroupingStrategy::ByEntity` — groups consecutive events that share an entity name; sorts paragraphs by salience.
- `GroupingStrategy::ByAction` — groups events by `RhetoricalCategory` (Removal, Addition, Modification, Other).

RST relations are then applied at the *inter-sentence* level via `Engine::render_batch_with_relations` and `discourse.rs::select_connective`. The relations themselves — contrast, sequence, elaboration, cause, etc. — are present, but they are flat labels between adjacent renders. The document does not carry a hierarchical RST tree.

For short documents (changelogs, status updates, alert prose) the flat structure is adequate and possibly optimal — adding hierarchy would buy little and cost a lot in plan complexity. For longer documents (multi-section release notes, incident postmortems, agent-narration logs spanning many tool invocations) the flat structure starts to under-organize. The RST-LoRA result suggests that hierarchical structure is the right primitive for long-document discourse.

## What an RST-tree DocumentPlan might look like

Sketch only — this is the conversation, not the spec:

```rust
/// Hierarchical RST organization of events. Each node is either a leaf
/// (one event) or an internal node with an RST relation between its
/// children. The nucleus/satellite distinction maps onto ordering: nucleus
/// reads first, satellites elaborate.
pub enum RstNode {
    Leaf(Event),
    Internal {
        relation: RstRelation,
        nucleus: Box<RstNode>,
        satellites: Vec<RstNode>,
    },
}

/// New variant alongside `GroupingStrategy::ByEntity` and `::ByAction`.
pub enum GroupingStrategy {
    ByEntity,
    ByAction,
    /// Build an RST tree by classifying inter-event relations and grouping
    /// hierarchically. Each section is rooted at a high-salience nucleus
    /// with subordinate elaboration / contrast / cause satellites.
    RstTree,
}
```

The `DocumentPlan::from_events_*` constructors gain an `RstTree` variant that runs an RST classifier over the event list and emits a tree. Rendering walks the tree depth-first, emitting connectives at each subtree boundary keyed off the relation label.

## Why this isn't already a spec

Three things have to resolve before this becomes a real design.

### 1. Is the RST classifier deterministic?

The classifier is the load-bearing part. RST-LoRA uses a neural classifier (the LoRA adapter is the parameter-efficient training surface). For Prosaic to use this approach the classifier must be deterministic — otherwise we lose the design thesis. Two paths:

- **Heuristic classifier.** A rule-based classifier from event type pairs to RST relations: e.g., `(Removal, Addition)` → `Contrast`, `(Modification, Modification)` on same entity → `Elaboration`, sequential timestamps → `Sequence`. This is what Prosaic already does at the connective-selection layer; lifting it to the document-structure layer is mechanical.
- **Pre-classified input.** Callers provide RST relations as part of the event stream (the input gets richer; the engine stays stateless about classification). This dovetails with how `Engine::render_batch_with_relations` already accepts explicit RST labels.

The heuristic classifier is the right starting point. Pre-classified input is a follow-up if heuristic classification turns out to under-label.

### 2. Does the hierarchical structure actually improve output for Prosaic's use cases?

RST-LoRA's improvement over flat-RST is measured on long-document summarization datasets. Prosaic's typical render is much shorter — a changelog entry, an alert narration, a few-paragraph status update. The benefit of hierarchy may not show up at those lengths. Before any code, the right experiment is:

- Take an existing long-form fixture (release notes for a substantial version, an agent-narration log with 30+ events, a multi-section incident report).
- Render it under `ByEntity` / `ByAction` (current) vs. a hand-built RST tree.
- Score both with the existing realism metrics (sentence rhythm variance, connective family balance) and a profile-distribution score from the StyleProfile spec.
- If hierarchy doesn't move the metrics, the case for the spec is weak.

This experiment is cheap to run and probably worth doing even if it confirms the null. Negative results are publishable too.

### 3. How does this compose with the Self-Refine retrospective pass?

The retro-pass spec (`docs/superpowers/specs/2026-05-09-self-refine-retro-pass-design.md`) introduces a `RenderedDocument` structured intermediate over which document-scope diagnosers run. An RST-tree DocumentPlan would naturally produce a richer `RenderedDocument` — one where each sentence carries its tree position and relation label. The diagnosers and constraint generators could read that structure to make finer-grained decisions (e.g., a `RstRelationImbalance` diagnoser already in the retro-pass spec becomes much sharper when it can see *where in the tree* the imbalance occurs).

But: this is composition, not a hard dependency. Each spec stands alone. The question is whether to design the `RenderedDocument` with hierarchical structure in mind even if the RST-tree DocumentPlan never lands. Probably yes — keeping the door open is cheap; locking it shut is expensive to undo.

## Composition with existing systems

- **`GroupingStrategy::ByEntity` / `::ByAction`.** Both stay. `RstTree` is additive, not a replacement.
- **`render_batch_with_relations`.** The flat-relations path stays for short documents. For RST-tree-organized documents the engine reads relations from the tree structure instead of the explicit slice.
- **`StyleProfile`.** Profile dials read unchanged. The `RstRelationImbalance` diagnoser (per the retro-pass spec) gets sharper when it has tree position to report.
- **Faithfulness.** Unchanged. RST structure organizes the existing events; it does not synthesize new content.

## Open questions

1. **Heuristic RST classifier rule set.** What event-type pair patterns map to which RST relations? This is a small piece of corpus inspection across the existing vocab modules — not paper-driven.
2. **Tree depth limit.** Should there be a maximum tree depth? Long documents could in principle produce arbitrarily deep trees; in practice human authors rarely go past three or four levels of subordination per section.
3. **Tree visualization.** For the experiment in (2) above we'd want to render the tree itself for inspection — a `DocumentPlan::to_rst_string()` debug helper.
4. **Multi-language support.** RST relations are language-independent in theory; the *connective vocabulary* per relation is language-specific. The existing `Language::rst_markers` extension point already covers this for the flat-relations path. The tree path inherits unchanged.

## Tier and sequencing

Lower priority than `StyleProfile` and Self-Refine retro-pass. Those two specs ship first; the experiment in §2 can run anytime against either the old or new code; this plan turns into a spec only if the experiment shows hierarchy moves the metrics.

## References

- Liu, D., Demberg, V. *RST-LoRA: A Discourse-Aware Low-Rank Adaptation for Long Document Abstractive Summarization.* arXiv:2405.00657, NAACL 2024.
- Mann, W., Thompson, S. *Rhetorical Structure Theory: Toward a Functional Theory of Text Organization.* Text (1988).
- `prosaic-core/src/document.rs` — current `DocumentPlan`, `GroupingStrategy`, `RhetoricalCategory`.
- `prosaic-core/src/rst.rs` — current RST relation enum.
- `prosaic-core/src/discourse.rs::select_connective` — current flat per-edge connective selection from RST relations.
- `docs/superpowers/specs/2026-05-09-self-refine-retro-pass-design.md` — `RenderedDocument` and `RstRelationImbalance` diagnoser this work composes with.
