# Discourse-Aware Rendering Design Spec

**Date:** 2026-04-14
**Status:** Approved

## Overview

Evolve the engine from stateless (each `render()` is independent) to discourse-aware (each `render()` is informed by what came before). The consumer experience doesn't change beyond one new method: `engine.reset()`. Naturalness is the default, not an opt-in mode.

## Discourse State

The engine internally tracks four things, cleared by `reset()`:

### 1. Entity Registry

Tracks recently mentioned entities so subsequent references use shorter forms or pronouns.

Rules for reference form selection:
- **First mention**: full form — "The class UserService"
- **Immediately subsequent, same entity is subject**: pronoun — "It"
- **Subsequent, same entity but not subject**: short name — "UserService"
- **After 3+ renders or after a `reset()`**: reintroduce with full form
- **Ambiguity** (two entities recently mentioned): always use name, never pronoun

### 2. Template History

Tracks which template variant was last selected per key. When variation is active, the engine avoids the most recently used variant. Guarantees no immediate repetition even with `Seeded` or `Random` strategies.

### 3. Discourse Connectives

When the engine detects a relationship between the current render and the previous one, it can prepend a connective.

| Signal | Connective pool |
|---|---|
| Same entity, different action | "It also", "Additionally", "Furthermore" |
| Different entity, same action type | "Similarly", "Likewise" |
| Sequential actions (temporal) | "Then", "Subsequently", "Next" |
| Contrasting actions (add vs delete) | "Meanwhile", "However", "On the other hand" |
| Causal (modify after rename) | "As a result", "Consequently", "This also means" |
| No detectable relationship | No connective inserted |

The connective budget ensures no single connective is repeated within a window of 3 uses.

### 4. Word Frequency Map

Tracks non-stopword tokens from recent renders. Used by a choosebest-style scorer: when multiple template variants are available, the engine generates all candidates and selects the one with the lowest repetition score against recent history.

## List Formatting

Replace bracket-style lists with natural prose alternatives. The engine auto-selects from:

| Style | Example |
|---|---|
| Including | "including A, B, and C among others" |
| Such as | "such as A, B, and C" |
| Dash | "— notably A, B, and C, plus N others" |
| Bracketed (current) | "[A, B, and C, and N more]" |

Auto-cycling avoids immediate repetition. Explicit `{items|join:bracketed}` forces the old style.

## Batch Rendering

```rust
let paragraph: String = engine.render_batch(&events)?;
```

Advantages over sequential `render()`:
- **Global ordering**: reorder events for narrative flow
- **Aggregation**: merge related events into single sentences

Aggregation rules:
- Same entity + different actions → "was renamed to X and moved to Y"
- Same action + different entities → "UserService and AuthService were renamed"
- Only aggregate within the same template key family

## API Changes

| Method | Description |
|---|---|
| `engine.render(key, ctx)` | Same signature, now discourse-aware |
| `engine.reset()` | Clears all discourse state |
| `engine.render_batch(events)` | Batch render with ordering and aggregation |

No breaking changes. Existing code gets naturally better output without modification.

## Testing Strategy

- Entity registry: first mention, second mention, ambiguity, reset
- Template history: no-immediate-repeat with 2+ variants
- Discourse connectives: relationship detection, non-repetition window
- Word frequency: choosebest selects least-repetitive variant
- List styles: each style renders correctly, auto-cycling
- Batch: ordering, aggregation, combined output
- Reset: all discourse state cleared
- Backward compatibility: all existing tests pass unchanged
