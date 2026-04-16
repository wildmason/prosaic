# Plan: RST-labeled `DocumentPlan` extension

**Owner:** sonnet agent
**Scope:** `prosaic-core/src/document.rs` + new `prosaic-core/src/rst.rs` + `prosaic-core/src/language.rs` + `prosaic-core/src/engine.rs` + `prosaic-grammar-es/src/lib.rs` + `prosaic-grammar-de/src/lib.rs`
**Estimated size:** ~400–500 LOC including tests
**Test gate:** 880 baseline tests still pass; adds ~15–20 new
**Branch discipline:** local only, one commit

---

## Why

Rhetorical Structure Theory (RST — Mann & Thompson, 1988) classifies how discourse units relate. Current `DocumentPlan` only groups events by entity or action; it doesn't know that event B is a *consequence* of event A vs. a *contrast* with A vs. an *elaboration* on A. When a caller can supply this label, the renderer can insert appropriate discourse markers:

- Elaboration: "… Furthermore, …" / "… In particular, …"
- Contrast: "… However, …"
- Cause/Result: "… As a result, …" / "… Because of this, …"
- Concession: "… Nevertheless, …"
- Sequence: "… Then, …" / "… Next, …"
- Condition: "… If this happens, …"
- Background: "… Meanwhile, …"
- Summary: "… In summary, …"

This is purely additive. Callers that don't supply RST labels see zero behavior change.

## Design

### `RstRelation` enum (new file `prosaic-core/src/rst.rs`)

```rust
//! Rhetorical Structure Theory relations for discourse labeling.

/// A rhetorical relation between a discourse unit and its predecessor.
///
/// See Mann & Thompson (1988). The subset below covers the eight most-common
/// relations in transaction/change-report narratives. When an RST relation
/// is attached to an event in a [`crate::DocumentPlan`], the renderer uses
/// the relation to pick a discourse marker ("Furthermore", "However",
/// "As a result", etc.) instead of the default inter-sentence space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum RstRelation {
    /// This unit adds detail to the previous unit.
    Elaboration,
    /// This unit contrasts with the previous unit.
    Contrast,
    /// This unit is caused by the previous unit.
    Cause,
    /// This unit is a result of the previous unit.
    Result,
    /// This unit runs counter to an expectation set up by the previous unit.
    Concession,
    /// This unit follows the previous unit in time.
    Sequence,
    /// This unit is conditional on the previous unit.
    Condition,
    /// This unit provides context for the previous unit.
    Background,
    /// This unit summarizes the preceding units.
    Summary,
}
```

### `Language::discourse_marker` trait method

```rust
// In language.rs, alongside realize_reference:

/// Return a discourse marker for the given RST relation.
///
/// Emitted at the START of a sentence (with trailing space), e.g.
/// `"Furthermore, "`. Return `None` to suppress the marker (the renderer
/// will fall back to a plain inter-sentence space).
///
/// The default implementation encodes English markers. Non-English grammars
/// override with locale-appropriate markers.
fn discourse_marker(&self, relation: crate::rst::RstRelation) -> Option<&'static str> {
    use crate::rst::RstRelation::*;
    Some(match relation {
        Elaboration => "Furthermore, ",
        Contrast    => "However, ",
        Cause       => "Because of this, ",
        Result      => "As a result, ",
        Concession  => "Nevertheless, ",
        Sequence    => "Then, ",
        Condition   => "If this happens, ",
        Background  => "Meanwhile, ",
        Summary     => "In summary, ",
    })
}
```

### Spanish override (in `prosaic-grammar-es/src/lib.rs`)

```rust
fn discourse_marker(&self, relation: RstRelation) -> Option<&'static str> {
    use RstRelation::*;
    Some(match relation {
        Elaboration => "Además, ",
        Contrast    => "Sin embargo, ",
        Cause       => "Debido a esto, ",
        Result      => "Como resultado, ",
        Concession  => "No obstante, ",
        Sequence    => "Luego, ",
        Condition   => "Si esto ocurre, ",
        Background  => "Mientras tanto, ",
        Summary     => "En resumen, ",
    })
}
```

### German override (in `prosaic-grammar-de/src/lib.rs`)

```rust
fn discourse_marker(&self, relation: RstRelation) -> Option<&'static str> {
    use RstRelation::*;
    Some(match relation {
        Elaboration => "Außerdem ",
        Contrast    => "Allerdings ",
        Cause       => "Deshalb ",
        Result      => "Folglich ",
        Concession  => "Dennoch ",
        Sequence    => "Dann ",
        Condition   => "Wenn dies geschieht, ",
        Background  => "Inzwischen ",
        Summary     => "Zusammenfassend ",
    })
}
```

### `Paragraph` extension

Add a parallel vector of optional relations:

```rust
pub struct Paragraph {
    pub events: Vec<(String, Context)>,
    /// Optional rhetorical relation for each event. `relations[i]` describes
    /// the relation between `events[i]` and `events[i-1]`. `relations[0]`
    /// is conventionally `None` (no predecessor within the paragraph).
    /// Same length as `events`.
    pub relations: Vec<Option<RstRelation>>,
    pub salience: Salience,
    pub category: Option<RhetoricalCategory>,
}
```

Update `Paragraph::new`, `push`, `is_empty`, and `Default` accordingly. `push` keeps its existing signature and pushes `None` into `relations`. Add:

```rust
pub fn push_with_relation(
    &mut self,
    key: String,
    ctx: Context,
    salience: Salience,
    relation: Option<RstRelation>,
) {
    self.events.push((key, ctx));
    self.relations.push(relation);
    if salience > self.salience {
        self.salience = salience;
    }
}
```

### `DocumentPlan::from_events_with_relations`

```rust
/// Build a [`GroupingStrategy::ByEntity`] plan where each event carries
/// an optional RST relation describing its rhetorical link to the
/// preceding event *within the same paragraph*.
///
/// Events that start a new paragraph (different entity) have their
/// relation silently dropped — relations are meaningful only within
/// a paragraph, not across paragraph boundaries.
pub fn from_events_with_relations(
    events: &[(&str, Context, Option<RstRelation>)],
    engine: &Engine,
) -> Self {
    // ... mirror build_by_entity, but thread the relation through push_with_relation.
    // When starting a new paragraph, push with None (relations don't cross paragraphs).
}
```

### `Engine::render_batch_with_relations`

Analogous to `render_batch` but inserts discourse markers when a relation is present. Key design point: when a relation is present on event[i] with i >= 1, do NOT run the aggregation paths (same-action / same-entity runs) that combine adjacent events — aggregation would swallow the discourse marker. Instead render each event individually and prepend the marker.

```rust
pub fn render_batch_with_relations(
    &self,
    session: &mut Session,
    events: &[(&str, Context, Option<RstRelation>)],
) -> Result<String, ProsaicError> {
    if events.is_empty() {
        return Ok(String::new());
    }

    // If every relation is None, delegate to render_batch to preserve
    // aggregation benefits.
    if events.iter().all(|(_, _, r)| r.is_none()) {
        let pairs: Vec<(&str, Context)> =
            events.iter().map(|(k, c, _)| (*k, c.clone())).collect();
        return self.render_batch(session, &pairs);
    }

    let mut output = String::new();
    for (i, (key, ctx, relation)) in events.iter().enumerate() {
        if i > 0 {
            if let Some(rel) = relation {
                if let Some(marker) = self.language.discourse_marker(*rel) {
                    output.push(' ');
                    output.push_str(marker);
                } else {
                    output.push(' ');
                }
            } else {
                output.push(' ');
            }
        }
        let sentence = self.render(session, key, ctx)?;
        // If marker was prepended AND sentence starts with capitalized
        // article/determiner, lowercase the first letter so the marker's
        // capitalization leads. E.g., "Furthermore, the class Foo ..."
        if i > 0 && relation.is_some() {
            output.push_str(&lowercase_first_if_determiner(&sentence));
        } else {
            output.push_str(&sentence);
        }
    }

    Ok(output)
}

fn lowercase_first_if_determiner(s: &str) -> String {
    // Common English determiners/articles: "The ", "A ", "An ".
    // Spanish: "El ", "La ", "Los ", "Las ", "Un ", "Una ".
    // Safe heuristic: if the first token is in the known determiner set,
    // lowercase only the first char. Otherwise return as-is.
    let first_word_end = s.find(char::is_whitespace).unwrap_or(s.len());
    let (first, rest) = s.split_at(first_word_end);
    const DETERMINERS: &[&str] = &[
        "The", "A", "An", "El", "La", "Los", "Las", "Un", "Una", "Der", "Die", "Das",
    ];
    if DETERMINERS.contains(&first) {
        let mut result = String::with_capacity(s.len());
        let mut chars = first.chars();
        if let Some(c) = chars.next() {
            result.extend(c.to_lowercase());
        }
        result.push_str(chars.as_str());
        result.push_str(rest);
        result
    } else {
        s.to_string()
    }
}
```

### `DocumentPlan::render` integration

In `render`, when a paragraph has any relation != None, use `render_batch_with_relations`; otherwise use `render_batch` as today. Both paths produce the same output for relation-less paragraphs.

```rust
pub fn render(&self, engine: &Engine, session: &mut Session) -> Result<String, ProsaicError> {
    let mut paragraphs = Vec::new();

    for (idx, p) in self.paragraphs.iter().enumerate() {
        if idx > 0 {
            session.reset();
        }

        let rendered = if p.relations.iter().any(|r| r.is_some()) {
            let triples: Vec<(&str, Context, Option<RstRelation>)> = p
                .events
                .iter()
                .zip(p.relations.iter())
                .map(|((k, c), r)| (k.as_str(), c.clone(), *r))
                .collect();
            engine.render_batch_with_relations(session, &triples)?
        } else {
            let events: Vec<(&str, Context)> = p
                .events
                .iter()
                .map(|(k, c)| (k.as_str(), c.clone()))
                .collect();
            engine.render_batch(session, &events)?
        };

        if !rendered.is_empty() {
            paragraphs.push(rendered);
        }
    }

    Ok(paragraphs.join("\n\n"))
}
```

### `lib.rs` re-exports

```rust
pub use rst::RstRelation;
pub mod rst;
```

## Out of scope

- **Nucleus/satellite classification** — real RST has a nucleus and one or more satellites; we flatten to just "how does this relate to the previous unit".
- **Nested RST trees** — v2 might support it.
- **Automatic relation inference** — we never auto-assign relations. The caller labels them explicitly.
- **Markers within an aggregated run** — if a paragraph runs the aggregation path (no relations), we don't try to splice markers into conjunction-reduced output.

---

## Phase 0 — Baseline

```bash
cargo test --all-features
cargo clippy --all-features -- -D warnings
```

Expected: 880 tests passing.

---

## Phase 1 — Add `RstRelation` enum

1. Create `prosaic-core/src/rst.rs` with the enum per design.
2. In `prosaic-core/src/lib.rs`: add `pub mod rst;` and `pub use rst::RstRelation;`.

### Tests

In `rst.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variants_are_distinct() {
        assert_ne!(RstRelation::Elaboration, RstRelation::Contrast);
        assert_ne!(RstRelation::Cause, RstRelation::Result);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_round_trip() {
        let rel = RstRelation::Elaboration;
        let json = serde_json::to_string(&rel).unwrap();
        let back: RstRelation = serde_json::from_str(&json).unwrap();
        assert_eq!(rel, back);
    }
}
```

Verify: `cargo test -p prosaic-core --all-features`.

---

## Phase 2 — `Language::discourse_marker`

1. In `prosaic-core/src/language.rs`, add the trait method with English defaults.
2. In `prosaic-grammar-es/src/lib.rs`, add the Spanish override.
3. In `prosaic-grammar-de/src/lib.rs`, add the German override.

### Tests

In `language.rs` `#[cfg(test)]`:
```rust
#[test]
fn discourse_marker_english_defaults() {
    let lang = MiniLang; // existing helper
    use crate::rst::RstRelation::*;
    assert_eq!(lang.discourse_marker(Elaboration), Some("Furthermore, "));
    assert_eq!(lang.discourse_marker(Contrast), Some("However, "));
    assert_eq!(lang.discourse_marker(Result), Some("As a result, "));
}
```

In `prosaic-grammar-es/src/lib.rs` tests:
```rust
#[test]
fn discourse_marker_spanish() {
    let es = Spanish::new();
    use prosaic_core::RstRelation;
    assert_eq!(es.discourse_marker(RstRelation::Elaboration), Some("Además, "));
    assert_eq!(es.discourse_marker(RstRelation::Contrast), Some("Sin embargo, "));
}
```

Mirror for German.

---

## Phase 3 — Extend `Paragraph`

1. Add `relations: Vec<Option<RstRelation>>` field.
2. Update `Paragraph::new` / `Default` / `push` to keep `relations.len() == events.len()`.
3. Add `push_with_relation`.
4. Update internal callers in `build_by_entity` and `from_events_classified` — both use `push(...)` which now also pushes `None` into relations; no explicit changes needed if we keep `push` as the `None`-relation wrapper around `push_with_relation`.

### Tests

```rust
#[test]
fn paragraph_push_adds_none_relation() {
    let mut p = Paragraph::new();
    p.push("t".into(), Context::new(), Salience::Low);
    assert_eq!(p.relations.len(), 1);
    assert_eq!(p.relations[0], None);
}

#[test]
fn paragraph_push_with_relation_records_it() {
    let mut p = Paragraph::new();
    p.push_with_relation(
        "t".into(),
        Context::new(),
        Salience::Low,
        Some(RstRelation::Contrast),
    );
    assert_eq!(p.relations, vec![Some(RstRelation::Contrast)]);
}
```

---

## Phase 4 — `DocumentPlan::from_events_with_relations`

Follow the by-entity build pattern but thread relations. First-in-paragraph events get `None` (no predecessor inside the paragraph); subsequent same-entity events get the caller-supplied relation.

Implementation sketch:
```rust
pub fn from_events_with_relations(
    events: &[(&str, Context, Option<RstRelation>)],
    engine: &Engine,
) -> Self {
    let mut plan = Self::new();
    if events.is_empty() { return plan; }

    let mut current = Paragraph::new();
    let mut current_entity: Option<String> = None;

    for (key, ctx, relation) in events {
        let ctx = ctx.clone();
        let salience = engine.context_salience(&ctx);
        let entity_name = entity_key(&ctx);

        let same_entity = match (&current_entity, &entity_name) {
            (Some(a), Some(b)) => a == b,
            _ => false,
        };

        if !same_entity && !current.is_empty() {
            plan.paragraphs.push(std::mem::take(&mut current));
        }

        // When starting a new paragraph (current is empty) the relation
        // doesn't apply — cross-paragraph relations aren't supported in v1.
        let effective_relation = if current.is_empty() { None } else { *relation };

        current.push_with_relation(key.to_string(), ctx, salience, effective_relation);
        current_entity = entity_name;
    }

    if !current.is_empty() {
        plan.paragraphs.push(current);
    }

    plan.paragraphs.sort_by(|a, b| b.salience.cmp(&a.salience));
    plan
}
```

### Tests

```rust
#[test]
fn from_events_with_relations_threads_rel() {
    let engine = test_engine();
    let e1 = (("t", ctx_with_entity("Foo", 1), None));
    let e2 = (("t", ctx_with_entity("Foo", 1), Some(RstRelation::Elaboration)));
    let events = vec![e1, e2];
    let plan = DocumentPlan::from_events_with_relations(&events, &engine);
    assert_eq!(plan.paragraphs.len(), 1);
    assert_eq!(plan.paragraphs[0].relations[0], None);
    assert_eq!(plan.paragraphs[0].relations[1], Some(RstRelation::Elaboration));
}

#[test]
fn relations_are_dropped_at_paragraph_boundary() {
    let engine = test_engine();
    // Different entities → two paragraphs; the relation on e2 is dropped
    // because e2 starts a new paragraph.
    let events = vec![
        ("t", ctx_with_entity("Foo", 1), None),
        ("t", ctx_with_entity("Bar", 1), Some(RstRelation::Contrast)),
    ];
    let plan = DocumentPlan::from_events_with_relations(&events, &engine);
    assert_eq!(plan.paragraphs.len(), 2);
    // Both paragraphs have a single event with None relation.
    assert_eq!(plan.paragraphs[0].relations, vec![None]);
    assert_eq!(plan.paragraphs[1].relations, vec![None]);
}
```

---

## Phase 5 — `Engine::render_batch_with_relations`

Implement per design. Add `lowercase_first_if_determiner` helper as a private free function in `engine.rs`.

### Tests

Add to `engine.rs` `#[cfg(test)]`:
```rust
#[test]
fn render_batch_with_relations_inserts_marker() {
    let mut engine = test_engine();
    engine.register_template("t", "The class {name} was modified").unwrap();
    let mut s = Session::new();
    let mut ctx = Context::new();
    ctx.insert("name", Value::String("Foo".into()));

    let events = vec![
        ("t", ctx.clone(), None),
        ("t", ctx, Some(RstRelation::Elaboration)),
    ];
    let out = engine.render_batch_with_relations(&mut s, &events).unwrap();
    assert!(out.contains("Furthermore, "), "got: {out}");
}

#[test]
fn render_batch_with_relations_lowercases_determiner_after_marker() {
    let mut engine = test_engine();
    engine.register_template("t", "The class {name} was modified").unwrap();
    let mut s = Session::new();
    let mut ctx = Context::new();
    ctx.insert("name", Value::String("Foo".into()));

    let events = vec![
        ("t", ctx.clone(), None),
        ("t", ctx, Some(RstRelation::Contrast)),
    ];
    let out = engine.render_batch_with_relations(&mut s, &events).unwrap();
    // "However, the class Foo..." — note lowercase "the".
    assert!(out.contains("However, the class"), "got: {out}");
}

#[test]
fn render_batch_with_all_none_delegates_to_render_batch() {
    let mut engine = test_engine();
    engine.register_template("t", "{name} was modified").unwrap();
    let mut s = Session::new();
    let mut ctx = Context::new();
    ctx.insert("name", Value::String("Foo".into()));

    let triples = vec![("t", ctx.clone(), None), ("t", ctx.clone(), None)];
    let pairs: Vec<_> = triples.iter().map(|(k, c, _)| (*k, c.clone())).collect();

    let mut s2 = Session::new();
    let from_triples = engine.render_batch_with_relations(&mut s, &triples).unwrap();
    let from_pairs   = engine.render_batch(&mut s2, &pairs).unwrap();
    assert_eq!(from_triples, from_pairs);
}
```

---

## Phase 6 — Integrate into `DocumentPlan::render`

Update `render` per design. Add a test:

```rust
#[test]
fn document_render_uses_marker_when_paragraph_has_relation() {
    let mut engine = test_engine();
    engine.register_template("t", "The class {name} was modified").unwrap();

    let events = vec![
        ("t", ctx_with_entity("Foo", 1), None),
        ("t", ctx_with_entity("Foo", 1), Some(RstRelation::Contrast)),
    ];
    let plan = DocumentPlan::from_events_with_relations(&events, &engine);
    let mut s = Session::new();
    let rendered = plan.render(&engine, &mut s).unwrap();
    assert!(rendered.contains("However, "));
}
```

---

## Phase 7 — Final verification

```bash
cargo test --all-features
cargo test --no-default-features
cargo test --features serde
cargo clippy --all-features -- -D warnings
cargo doc --all-features --no-deps
```

Test count up by ~15–20.

**Commit:** `Add RST relation labels to DocumentPlan for discourse markers`

---

## Definition of done

- [ ] `RstRelation` enum in `prosaic-core/src/rst.rs`, re-exported from `lib.rs`
- [ ] `Language::discourse_marker` trait method with English defaults
- [ ] Spanish override in `prosaic-grammar-es`
- [ ] German override in `prosaic-grammar-de`
- [ ] `Paragraph.relations` parallel vector kept in sync with `events`
- [ ] `Paragraph::push_with_relation` helper
- [ ] `DocumentPlan::from_events_with_relations` builds a plan preserving same-entity paragraphing
- [ ] `Engine::render_batch_with_relations` inserts markers and lowercases determiners after them
- [ ] `DocumentPlan::render` uses the relation-aware path when any relation is set
- [ ] `cargo test --all-features` passes, count up by 15–20
- [ ] `cargo clippy --all-features -- -D warnings` clean
- [ ] One commit with subject: `Add RST relation labels to DocumentPlan for discourse markers`
