use crate::context::Context;
use crate::engine::Engine;
use crate::error::ProsaicError;
use crate::salience::Salience;
use crate::session::Session;

/// Rhetorical classification of an event based on its template key.
///
/// Used by [`DocumentPlan::from_events_grouped`] to organize a batch of
/// events into thematic sections — a breaking-changes paragraph, an
/// additions paragraph, etc. — instead of the default same-entity grouping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum RhetoricalCategory {
    /// Deletions, removals — typically breaking changes. Leads the narrative.
    Removal,
    /// New entities, features, introductions.
    Addition,
    /// Modifications, renames, moves, signature changes — existing code
    /// that was altered.
    Modification,
    /// Anything the default classifier doesn't recognize.
    Other,
}

/// How events should be grouped into paragraphs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum GroupingStrategy {
    /// Group consecutive events that share an entity name; sort paragraphs
    /// by highest salience first. This is the default and produces tight,
    /// entity-focused narratives.
    #[default]
    ByEntity,
    /// Group events by rhetorical category (removals, additions, each
    /// modification sub-type). Produces section-style narratives useful
    /// for release notes or high-level change summaries. Within a
    /// category, events are further grouped by entity so multiple changes
    /// to the same entity still flow together.
    ByAction,
}

/// Default template-key classifier used by [`GroupingStrategy::ByAction`].
///
/// Looks at the last dotted segment of the key and maps well-known action
/// names to [`RhetoricalCategory`]:
///
/// - `deleted`, `removed` → [`RhetoricalCategory::Removal`]
/// - `added`, `created`, `introduced` → [`RhetoricalCategory::Addition`]
/// - `modified`, `updated`, `renamed`, `moved`, `signature_changed` →
///   [`RhetoricalCategory::Modification`]
/// - anything else → [`RhetoricalCategory::Other`]
///
/// Use [`DocumentPlan::from_events_classified`] to supply a custom
/// classifier when the defaults don't fit the domain.
pub fn default_classifier(key: &str) -> RhetoricalCategory {
    let action = key.rsplit('.').next().unwrap_or("");
    match action {
        "deleted" | "removed" => RhetoricalCategory::Removal,
        "added" | "created" | "introduced" => RhetoricalCategory::Addition,
        "modified" | "updated" | "renamed" | "moved" | "signature_changed" => {
            RhetoricalCategory::Modification
        }
        _ => RhetoricalCategory::Other,
    }
}

/// Convention-ordered list of categories (Removal first — breaking changes
/// are usually the most important signal in a change report).
fn category_order() -> [RhetoricalCategory; 4] {
    [
        RhetoricalCategory::Removal,
        RhetoricalCategory::Addition,
        RhetoricalCategory::Modification,
        RhetoricalCategory::Other,
    ]
}

/// A paragraph in a document plan — a group of related events rendered together.
#[derive(Debug, Clone)]
pub struct Paragraph {
    /// The events in this paragraph, in render order.
    pub events: Vec<(String, Context)>,
    /// The highest salience in this paragraph, used for ordering.
    pub salience: Salience,
    /// Rhetorical category, when the plan was built with
    /// [`GroupingStrategy::ByAction`]. `None` for entity-grouped plans.
    pub category: Option<RhetoricalCategory>,
}

impl Paragraph {
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            salience: Salience::Low,
            category: None,
        }
    }

    pub fn push(&mut self, key: String, ctx: Context, salience: Salience) {
        self.events.push((key, ctx));
        if salience > self.salience {
            self.salience = salience;
        }
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

impl Default for Paragraph {
    fn default() -> Self {
        Self::new()
    }
}

/// A planned document — a structured narrative with paragraph breaks.
///
/// Use `DocumentPlan::from_events` to auto-organize events by salience and
/// entity groupings, then `.render(&engine)` to produce the final narrative.
#[derive(Debug, Clone)]
pub struct DocumentPlan {
    pub paragraphs: Vec<Paragraph>,
}

impl DocumentPlan {
    pub fn new() -> Self {
        Self {
            paragraphs: Vec::new(),
        }
    }

    /// Build a document plan from a flat set of events, using the default
    /// entity-grouping strategy.
    ///
    /// Organization:
    /// 1. Assign each event a salience (from context or explicit thresholds).
    /// 2. Group consecutive events that share an entity into the same paragraph.
    /// 3. Order paragraphs by highest-salience first.
    ///
    /// Within a paragraph, events keep their original order (which the engine's
    /// discourse state can then leverage for pronouns and connectives).
    ///
    /// To group by action category instead, use
    /// [`DocumentPlan::from_events_grouped`] or
    /// [`DocumentPlan::from_events_classified`].
    pub fn from_events(events: &[(&str, Context)], engine: &Engine) -> Self {
        Self::from_events_grouped(events, engine, GroupingStrategy::ByEntity)
    }

    /// Build a document plan with an explicit grouping strategy.
    pub fn from_events_grouped(
        events: &[(&str, Context)],
        engine: &Engine,
        strategy: GroupingStrategy,
    ) -> Self {
        match strategy {
            GroupingStrategy::ByEntity => Self::build_by_entity(events, engine),
            GroupingStrategy::ByAction => {
                Self::from_events_classified(events, engine, default_classifier)
            }
        }
    }

    /// Build a [`GroupingStrategy::ByAction`] plan with a custom classifier.
    /// Useful when template keys don't match the default classifier's
    /// vocabulary (e.g., domain-specific verbs like `"issue.closed"`).
    pub fn from_events_classified<F>(
        events: &[(&str, Context)],
        engine: &Engine,
        classifier: F,
    ) -> Self
    where
        F: Fn(&str) -> RhetoricalCategory,
    {
        let mut plan = Self::new();
        if events.is_empty() {
            return plan;
        }

        // Bucket events by category, preserving input order within each.
        use std::collections::BTreeMap;
        let mut buckets: BTreeMap<RhetoricalCategory, Vec<(String, Context)>> =
            BTreeMap::new();

        for (key, ctx) in events {
            let category = classifier(key);
            buckets
                .entry(category)
                .or_default()
                .push((key.to_string(), ctx.clone()));
        }

        // Walk categories in the canonical rhetorical order. For each
        // non-empty bucket, sub-group by entity (so multiple changes to
        // the same thing still cluster together) and emit a paragraph.
        for category in category_order() {
            let bucket = match buckets.remove(&category) {
                Some(b) if !b.is_empty() => b,
                _ => continue,
            };

            let mut para = Paragraph::new();
            para.category = Some(category);
            let mut current_entity: Option<String> = None;

            // Sort within the bucket so that events sharing an entity are
            // adjacent — stable to preserve user-provided ordering among
            // entity-free events.
            let mut sorted = bucket;
            sorted.sort_by(|a, b| entity_key(&a.1).cmp(&entity_key(&b.1)));

            for (key, ctx) in sorted {
                let salience = engine.context_salience(&ctx);
                let entity_name = entity_key(&ctx);

                // When the entity changes within a category, flush the
                // current paragraph and start a new one. This keeps
                // pronouns/connectives working within a run of same-entity
                // events and avoids awkward co-reference across unrelated
                // entities inside one paragraph.
                let same_entity = match (&current_entity, &entity_name) {
                    (Some(a), Some(b)) => a == b,
                    (None, None) => true,
                    _ => false,
                };

                if !same_entity && !para.is_empty() {
                    plan.paragraphs.push(std::mem::take(&mut para));
                    para.category = Some(category);
                }

                para.push(key, ctx, salience);
                current_entity = entity_name;
            }

            if !para.is_empty() {
                plan.paragraphs.push(para);
            }
        }

        // Any categories Left over (shouldn't happen since we iterate all
        // four, but defensive): append in arbitrary but stable order.
        for (category, bucket) in buckets {
            let mut para = Paragraph::new();
            para.category = Some(category);
            for (key, ctx) in bucket {
                let salience = engine.context_salience(&ctx);
                para.push(key, ctx, salience);
            }
            plan.paragraphs.push(para);
        }

        plan
    }

    fn build_by_entity(events: &[(&str, Context)], engine: &Engine) -> Self {
        let mut plan = Self::new();
        if events.is_empty() {
            return plan;
        }

        let mut current = Paragraph::new();
        let mut current_entity: Option<String> = None;

        for (key, ctx) in events {
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

            current.push(key.to_string(), ctx, salience);
            current_entity = entity_name;
        }

        if !current.is_empty() {
            plan.paragraphs.push(current);
        }

        // Sort paragraphs by highest salience first (stable to preserve tie order)
        plan.paragraphs.sort_by(|a, b| b.salience.cmp(&a.salience));

        plan
    }

    /// Render the document plan into a narrative.
    ///
    /// Paragraphs are separated by a double newline. Between paragraphs the
    /// discourse state is reset so pronouns don't span paragraph boundaries —
    /// each paragraph reintroduces its entity with the full form.
    pub fn render(&self, engine: &Engine, session: &mut Session) -> Result<String, ProsaicError> {
        let mut paragraphs = Vec::new();

        for (idx, p) in self.paragraphs.iter().enumerate() {
            if idx > 0 {
                session.reset();
            }

            let events: Vec<(&str, Context)> = p
                .events
                .iter()
                .map(|(k, c)| (k.as_str(), c.clone()))
                .collect();

            let rendered = engine.render_batch(session, &events)?;
            if !rendered.is_empty() {
                paragraphs.push(rendered);
            }
        }

        Ok(paragraphs.join("\n\n"))
    }
}

/// Extract the primary entity-name key from a render context.
fn entity_key(ctx: &Context) -> Option<String> {
    ctx.get("name")
        .or_else(|| ctx.get("old_name"))
        .map(|v| v.as_display())
}

impl Default for DocumentPlan {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::Value;
    use crate::engine::{Engine, Strictness, Variation};
    use crate::language::{Conjunction, Language, Person, Tense};
    use crate::session::Session;

    struct TestLang;

    impl Language for TestLang {
        fn pluralize(&self, word: &str, count: usize) -> String {
            if count == 1 { word.to_string() } else { format!("{word}s") }
        }
        fn singularize(&self, word: &str) -> String {
            word.strip_suffix('s').unwrap_or(word).to_string()
        }
        fn article(&self, _word: &str) -> &str { "a" }
        fn conjugate(&self, verb: &str, _t: Tense, _p: Person) -> String { verb.to_string() }
        fn past_participle(&self, verb: &str) -> String { format!("{verb}ed") }
        fn present_participle(&self, verb: &str) -> String { format!("{verb}ing") }
        fn join_list(&self, items: &[&str], _c: Conjunction) -> String {
            items.join(", ")
        }
        fn ordinal(&self, n: usize) -> String { format!("{n}th") }
        fn number_to_words(&self, n: usize) -> String { n.to_string() }
    }

    fn test_engine() -> Engine {
        let mut engine = Engine::new(TestLang)
            .strictness(Strictness::Strict)
            .variation(Variation::Fixed);
        engine.register_template("t", "{name} changed").unwrap();
        engine
    }

    #[test]
    fn empty_events_produces_empty_plan() {
        let engine = test_engine();
        let plan = DocumentPlan::from_events(&[], &engine);
        assert!(plan.paragraphs.is_empty());
        let mut session = Session::new();
        assert_eq!(plan.render(&engine, &mut session).unwrap(), "");
    }

    #[test]
    fn groups_consecutive_same_entity_events() {
        let engine = test_engine();
        let mut c1 = Context::new();
        c1.insert("entity_type", Value::String("class".into()));
        c1.insert("name", Value::String("Foo".into()));
        c1.insert("consumer_count", Value::Number(1));
        let mut c2 = Context::new();
        c2.insert("entity_type", Value::String("class".into()));
        c2.insert("name", Value::String("Foo".into()));
        c2.insert("consumer_count", Value::Number(1));
        let mut c3 = Context::new();
        c3.insert("entity_type", Value::String("class".into()));
        c3.insert("name", Value::String("Bar".into()));
        c3.insert("consumer_count", Value::Number(1));

        let events: Vec<(&str, Context)> = vec![
            ("t", c1),
            ("t", c2),
            ("t", c3),
        ];

        let plan = DocumentPlan::from_events(&events, &engine);
        assert_eq!(plan.paragraphs.len(), 2);
        assert_eq!(plan.paragraphs[0].events.len(), 2);
        assert_eq!(plan.paragraphs[1].events.len(), 1);
    }

    #[test]
    fn orders_paragraphs_by_highest_salience() {
        let engine = test_engine();
        let mut low = Context::new();
        low.insert("entity_type", Value::String("class".into()));
        low.insert("name", Value::String("Small".into()));
        low.insert("consumer_count", Value::Number(1));

        let mut high = Context::new();
        high.insert("entity_type", Value::String("class".into()));
        high.insert("name", Value::String("Big".into()));
        high.insert("consumer_count", Value::Number(50));

        let events: Vec<(&str, Context)> = vec![
            ("t", low),
            ("t", high),
        ];

        let plan = DocumentPlan::from_events(&events, &engine);
        assert_eq!(plan.paragraphs.len(), 2);
        // Highest salience paragraph comes first
        assert_eq!(plan.paragraphs[0].salience, Salience::High);
        assert_eq!(plan.paragraphs[1].salience, Salience::Low);
    }

    #[test]
    fn renders_paragraphs_separated_by_blank_line() {
        let engine = test_engine();
        let mut c1 = Context::new();
        c1.insert("entity_type", Value::String("class".into()));
        c1.insert("name", Value::String("Alpha".into()));
        c1.insert("consumer_count", Value::Number(5));

        let mut c2 = Context::new();
        c2.insert("entity_type", Value::String("class".into()));
        c2.insert("name", Value::String("Beta".into()));
        c2.insert("consumer_count", Value::Number(5));

        let events: Vec<(&str, Context)> = vec![("t", c1), ("t", c2)];

        let plan = DocumentPlan::from_events(&events, &engine);
        let mut session = Session::new();
        let rendered = plan.render(&engine, &mut session).unwrap();

        assert!(rendered.contains("\n\n"), "Expected paragraph break, got: {rendered}");
    }

    // ── Rhetorical grouping ──────────────────────────────────────────────

    fn ctx_with_entity(name: &str, count: i64) -> Context {
        let mut c = Context::new();
        c.insert("entity_type", Value::String("class".into()));
        c.insert("name", Value::String(name.into()));
        c.insert("consumer_count", Value::Number(count));
        c
    }

    #[test]
    fn default_classifier_buckets_common_keys() {
        assert_eq!(default_classifier("code.deleted"), RhetoricalCategory::Removal);
        assert_eq!(default_classifier("code.removed"), RhetoricalCategory::Removal);
        assert_eq!(default_classifier("code.added"), RhetoricalCategory::Addition);
        assert_eq!(default_classifier("code.introduced"), RhetoricalCategory::Addition);
        assert_eq!(
            default_classifier("code.modified"),
            RhetoricalCategory::Modification,
        );
        assert_eq!(
            default_classifier("code.renamed"),
            RhetoricalCategory::Modification,
        );
        assert_eq!(
            default_classifier("code.signature_changed"),
            RhetoricalCategory::Modification,
        );
        assert_eq!(default_classifier("random"), RhetoricalCategory::Other);
        assert_eq!(default_classifier(""), RhetoricalCategory::Other);
    }

    #[test]
    fn by_action_groups_removals_before_additions_before_modifications() {
        let engine = test_engine();

        let events: Vec<(&str, Context)> = vec![
            ("code.modified", ctx_with_entity("A", 1)),
            ("code.added", ctx_with_entity("B", 1)),
            ("code.deleted", ctx_with_entity("C", 1)),
        ];

        let plan = DocumentPlan::from_events_grouped(
            &events,
            &engine,
            GroupingStrategy::ByAction,
        );

        // Removal first, then Addition, then Modification.
        assert_eq!(plan.paragraphs.len(), 3);
        assert_eq!(
            plan.paragraphs[0].category,
            Some(RhetoricalCategory::Removal)
        );
        assert_eq!(
            plan.paragraphs[1].category,
            Some(RhetoricalCategory::Addition)
        );
        assert_eq!(
            plan.paragraphs[2].category,
            Some(RhetoricalCategory::Modification)
        );
    }

    #[test]
    fn by_action_splits_paragraphs_within_category_on_entity_change() {
        let engine = test_engine();

        // Two modifications, different entities → two paragraphs in the
        // Modification section so pronouns don't cross-link them.
        let events: Vec<(&str, Context)> = vec![
            ("code.modified", ctx_with_entity("Alpha", 1)),
            ("code.modified", ctx_with_entity("Beta", 1)),
        ];

        let plan = DocumentPlan::from_events_grouped(
            &events,
            &engine,
            GroupingStrategy::ByAction,
        );

        assert_eq!(plan.paragraphs.len(), 2);
        for p in &plan.paragraphs {
            assert_eq!(p.category, Some(RhetoricalCategory::Modification));
            assert_eq!(p.events.len(), 1);
        }
    }

    #[test]
    fn by_action_keeps_same_entity_events_together_within_category() {
        let engine = test_engine();

        let events: Vec<(&str, Context)> = vec![
            ("code.modified", ctx_with_entity("Alpha", 1)),
            ("code.renamed", ctx_with_entity("Alpha", 1)),
        ];

        let plan = DocumentPlan::from_events_grouped(
            &events,
            &engine,
            GroupingStrategy::ByAction,
        );

        // Both are Modification category, same entity → one paragraph, two events.
        assert_eq!(plan.paragraphs.len(), 1);
        assert_eq!(plan.paragraphs[0].events.len(), 2);
    }

    #[test]
    fn from_events_classified_accepts_custom_classifier() {
        let engine = test_engine();

        let events: Vec<(&str, Context)> = vec![
            ("issue.closed", ctx_with_entity("Bug1", 1)),
            ("issue.opened", ctx_with_entity("Bug2", 1)),
        ];

        let plan = DocumentPlan::from_events_classified(&events, &engine, |key| {
            match key.rsplit('.').next().unwrap_or("") {
                "closed" => RhetoricalCategory::Removal,
                "opened" => RhetoricalCategory::Addition,
                _ => RhetoricalCategory::Other,
            }
        });

        assert_eq!(plan.paragraphs.len(), 2);
        assert_eq!(
            plan.paragraphs[0].category,
            Some(RhetoricalCategory::Removal)
        );
        assert_eq!(
            plan.paragraphs[1].category,
            Some(RhetoricalCategory::Addition)
        );
    }

    #[test]
    fn by_entity_grouping_still_default() {
        let engine = test_engine();

        let events: Vec<(&str, Context)> = vec![
            ("code.modified", ctx_with_entity("Alpha", 1)),
            ("code.modified", ctx_with_entity("Alpha", 1)),
        ];

        let plan = DocumentPlan::from_events(&events, &engine);
        // Default remains the ByEntity strategy — same-entity consecutive
        // events end up in one paragraph.
        assert_eq!(plan.paragraphs.len(), 1);
        assert!(plan.paragraphs[0].category.is_none());
    }
}
