# Plan: Hierarchical Aggregation — Recursive Clause Fusion

**Owner:** Gemini CLI
**Scope:** `prosaic-core/src/engine.rs`, `prosaic-core/src/document.rs`, `prosaic-core/src/language.rs`
**Estimated size:** ~300–400 LOC
**Context:** Extends the existing flat "Forward Conjunction Reduction" (FCR) to handle nested relationships and contrastive actions.

---

## Why

Currently, Prosaic handles runs of same-entity events by flattening them into a single comma-separated list:
- *"The class Foo was renamed, modified, and moved."*

However, human-written prose often groups actions hierarchically, especially when contrasting actions or related entities are involved:
- *"The class Foo was renamed and modified, while its three dependents were updated to match."*
- *"A new module AuthService was introduced, whereas the legacy AuthGuard was removed."*

This plan introduces a recursive aggregation strategy that can nest these relationships, producing much more sophisticated narrative flow.

## Design

### 1. Hierarchical Document Plan
Currently, `DocumentPlan` is a flat list of `Paragraph`s, each containing a flat list of events. We will introduce an intermediate `DocumentNode` that can be either a single event or a "Cluster" of related events with an associated `AggregationType`.

```rust
pub enum DocumentNode {
    Leaf(TemplateKey, Context),
    Cluster {
        nodes: Vec<DocumentNode>,
        strategy: AggregationStrategy,
    },
}

pub enum AggregationStrategy {
    /// "A, B, and C" (Current FCR behavior)
    Conjunction,
    /// "A, whereas B" / "A, while B"
    Contrast,
    /// "A, which in turn B" (Related entity impact)
    Sequence,
}
```

### 2. Enhanced Grouping Algorithm
The `DocumentPlan::from_events` logic will be updated to detect "Contrastive Pairs" (add vs delete) and "Related Entity Cascades" (entity A modified, which impacts entity B).

- **Contrast Detection:** If two consecutive events have the same `entity_type` but contrasting actions (e.g., `code.added` and `code.deleted`), they are clustered with `AggregationStrategy::Contrast`.
- **Relationship Detection (REG-aware):** If event A refers to entity E1, and event B refers to entity E2 where E2 is a registered dependent of E1, they are clustered with `AggregationStrategy::Sequence`.

### 3. Recursive Realization
The `Engine::render_batch` logic will be updated to recursively render `DocumentNode` trees.

- **Leaf Nodes:** Render normally via `Engine::render`.
- **Conjunction Clusters:** Use the existing `reduce_same_entity_clauses` logic but apply it to the results of the child nodes.
- **Contrast Clusters:** Join two sub-renders with language-specific contrastive markers (e.g., English: `", whereas "`, `", while "` / Spanish: `", mientras que "` / German: `", wohingegen "`).

### 4. Language Trait Extensions
Add `Language::contrastive_marker()` to return markers like "whereas" or "while".

---

## Implementation Phases

### Phase 1: Data Model Refactor
- Define `DocumentNode` and `AggregationStrategy` in `document.rs`.
- Update `Paragraph` to hold `Vec<DocumentNode>`.
- **Validation:** Ensure existing flat-event rendering still works via `DocumentNode::Leaf`.

### Phase 2: Hierarchical Grouping Logic
- Implement `find_contrastive_pairs` in `engine.rs`.
- Implement `find_related_cascades` (utilizing the `EntityRegistry` if `reg` feature is enabled).
- Update `DocumentPlan::from_events` to build the tree.

### Phase 3: Recursive Rendering
- Implement `render_node` in `Engine`.
- Update `reduce_same_entity_clauses` to accept pre-rendered predicates rather than full sentences (to allow nesting).
- Integrate contrastive markers from the `Language` trait.

### Phase 4: Verification
- Add integration tests for nested aggregation:
    - Same-entity modified + related-entity updated.
    - Added X + Deleted Y (same type).
    - Multi-level: (A + B) whereas (C + D).

---

## Safety & Rejection Rules
- **Length Budget:** If a clustered sentence exceeds `max_sentence_length`, the recursive renderer must "break" the cluster and return to individual sentences.
- **Complexity Limit:** Clusters deeper than 2 levels should be rejected to avoid "garden path" sentences.
- **Voice/Tense Mismatch:** Contrastive aggregation only applies when voice/tense match (e.g., don't contrast a Past-Passive with a Future-Active).
