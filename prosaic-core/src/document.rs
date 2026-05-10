#[cfg(not(feature = "std"))]
use alloc::string::{String, ToString};
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use crate::context::Context;
use crate::engine::Engine;
use crate::error::ProsaicError;
use crate::rst::RstRelation;
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
    /// Optional rhetorical relation for each event. `relations[i]` describes
    /// the relation between `events[i]` and `events[i-1]`. `relations[0]`
    /// is conventionally `None` (no predecessor within the paragraph).
    /// Same length as `events`.
    pub relations: Vec<Option<RstRelation>>,
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
            relations: Vec::new(),
            salience: Salience::Low,
            category: None,
        }
    }

    /// Push an event with no rhetorical relation (`None`).
    pub fn push(&mut self, key: String, ctx: Context, salience: Salience) {
        self.push_with_relation(key, ctx, salience, None);
    }

    /// Push an event with an optional RST relation to its predecessor.
    ///
    /// The relation describes how this event relates to the immediately
    /// preceding event in the same paragraph. Pass `None` when there is
    /// no predecessor or when the rhetorical link is unknown.
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
        use crate::collections::BTreeMap;
        let mut buckets: BTreeMap<RhetoricalCategory, Vec<(String, Context)>> = BTreeMap::new();

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
                    plan.paragraphs.push(core::mem::take(&mut para));
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
        let mut plan = Self::new();
        if events.is_empty() {
            return plan;
        }

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
                plan.paragraphs.push(core::mem::take(&mut current));
            }

            // When starting a new paragraph (current is empty) the relation
            // doesn't apply — cross-paragraph relations aren't supported.
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
                plan.paragraphs.push(core::mem::take(&mut current));
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

    /// Render paragraphs in parallel using rayon.
    ///
    /// Each paragraph gets its own freshly-reset clone of `initial_session`.
    /// Paragraphs render concurrently and the results are joined with `"\n\n"`
    /// in the original paragraph order.
    ///
    /// **Trade-off:** temporal-anchor threading and auto list-style rotation
    /// across paragraphs are lost; each paragraph renders from an independently
    /// cloned session. For coherent narratives (e.g. when you rely on
    /// `{ts|since_last}` or cross-paragraph `{items|join}` variety) use
    /// [`render`][DocumentPlan::render] instead.
    ///
    /// For paragraphs independent of temporal anchors and auto list-style
    /// cycling this produces byte-identical output to the sequential `render`.
    ///
    /// Requires the `parallel` feature.
    #[cfg(feature = "parallel")]
    pub fn render_parallel(
        &self,
        engine: &Engine,
        initial_session: &Session,
    ) -> Result<String, ProsaicError>
    where
        Engine: Sync,
        Session: Send,
    {
        use rayon::prelude::*;

        let rendered: Result<Vec<String>, ProsaicError> = self
            .paragraphs
            .par_iter()
            .map(|p| {
                let mut session = initial_session.clone();
                // Reset paragraph-local discourse while preserving the
                // initial session's narrative-level style seed.
                session.reset_for_paragraph();

                if p.relations.iter().any(|r| r.is_some()) {
                    let triples: Vec<(&str, Context, Option<RstRelation>)> = p
                        .events
                        .iter()
                        .zip(p.relations.iter())
                        .map(|((k, c), r)| (k.as_str(), c.clone(), *r))
                        .collect();
                    engine.render_batch_with_relations(&mut session, &triples)
                } else {
                    let events: Vec<(&str, Context)> = p
                        .events
                        .iter()
                        .map(|(k, c)| (k.as_str(), c.clone()))
                        .collect();
                    engine.render_batch(&mut session, &events)
                }
            })
            .filter(|r| !matches!(r, Ok(s) if s.is_empty()))
            .collect();

        Ok(rendered?.join("\n\n"))
    }

    /// Render the document plan into a narrative.
    ///
    /// Paragraphs are separated by a double newline. Between paragraphs the
    /// paragraph-local discourse state is reset so pronouns don't span paragraph
    /// boundaries, but narrative-level style rotation is preserved.
    ///
    /// When any event in a paragraph carries an RST relation, the paragraph
    /// is rendered via [`Engine::render_batch_with_relations`] which inserts
    /// discourse markers ("Furthermore, ", "However, ", etc.) between events.
    /// Paragraphs whose relations are all `None` fall back to the standard
    /// [`Engine::render_batch`] path so aggregation still applies.
    /// Render this plan into a [`RenderedDocument`] — the structured
    /// intermediate consumed by retrospective-pass diagnosers and the
    /// composite scorer.
    ///
    /// Behaviorally identical to [`Self::render`] for the **flat text**:
    /// `render_structured(engine, session)?.text == render(engine, session)?`
    /// holds when no inter-event gapping (forward conjunction reduction)
    /// applies inside any paragraph. When gapping does apply, this method
    /// produces sentence-by-sentence text without the gapped form, which
    /// is what diagnosers want — they reason at sentence granularity, not
    /// at the gapped-clause level. Callers that need the gapped flat
    /// string should keep using `render`.
    pub fn render_structured(
        &self,
        engine: &Engine,
        session: &mut Session,
    ) -> Result<crate::refine::RenderedDocument, ProsaicError> {
        use crate::refine::{EventMeta, ParagraphRender};
        let mut paragraphs = Vec::with_capacity(self.paragraphs.len());

        for (idx, p) in self.paragraphs.iter().enumerate() {
            if idx > 0 {
                session.reset_for_paragraph();
            }
            let mut paragraph_text = String::new();
            let mut events = Vec::with_capacity(p.events.len());
            for (event_idx, (key, ctx)) in p.events.iter().enumerate() {
                if event_idx > 0 {
                    paragraph_text.push(' ');
                }
                let exp = engine.render_explained(session, key, ctx)?;
                paragraph_text.push_str(&exp.output);
                events.push(EventMeta {
                    connective: exp.connective.map(|s| s.to_string()),
                    list_style: exp.list_style,
                });
            }
            paragraphs.push(ParagraphRender {
                text: paragraph_text,
                events,
            });
        }

        Ok(crate::refine::RenderedDocument::from_paragraphs(paragraphs))
    }

    /// Run the retrospective refine loop over this plan. Equivalent to
    /// [`Self::render`] when the engine's [`crate::RefineConfig`] is off,
    /// otherwise iterates with structural diagnosers per the loop spec.
    /// Always produces a complete output; faithfulness-failing iterations
    /// are silently rejected and the loop falls back to the previous best.
    pub fn render_refined(
        &self,
        engine: &Engine,
        session: &mut Session,
    ) -> Result<crate::refine::RefineOutcome, ProsaicError> {
        let config = engine.current_refine_config();
        let initial_session = session.clone();
        let initial = self.render_structured(engine, session)?;
        if config.is_off() {
            let final_score = crate::refine_score::score_document(
                &initial,
                &config.weights,
                Some(engine.current_style_profile()).filter(|p| !p.is_neutral()),
            );
            return Ok(crate::refine::RefineOutcome {
                text: initial.text,
                iterations_run: 0,
                final_score,
                converged_clean: true,
            });
        }
        let profile_ref = Some(engine.current_style_profile()).filter(|p| !p.is_neutral());
        crate::refine::run_refine_loop(
            config,
            profile_ref,
            initial,
            initial_session,
            session,
            |s| self.render_structured(engine, s),
        )
    }

    pub fn render(&self, engine: &Engine, session: &mut Session) -> Result<String, ProsaicError> {
        if !engine.current_refine_config().is_off() {
            return self
                .render_refined(engine, session)
                .map(|outcome| outcome.text);
        }
        let mut paragraphs = Vec::new();

        for (idx, p) in self.paragraphs.iter().enumerate() {
            if idx > 0 {
                session.reset_for_paragraph();
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
}

/// Extract the primary entity-name key from a render context.
fn entity_key(ctx: &Context) -> Option<String> {
    ctx.get("name")
        .or_else(|| ctx.get("old_name"))
        .map(|v| v.as_display())
}

// Verify Engine and Session are Send + Sync at compile time.
// This is a zero-cost assertion; the const fn is never called.
const fn _assert_engine_session_send_sync() {
    const fn check<T: Send + Sync>() {}
    check::<Engine>();
    check::<Session>();
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
    use crate::rst::RstRelation;
    use crate::session::Session;

    struct TestLang;

    impl Language for TestLang {
        fn pluralize(&self, word: &str, count: usize) -> String {
            if count == 1 {
                word.to_string()
            } else {
                format!("{word}s")
            }
        }
        fn singularize(&self, word: &str) -> String {
            word.strip_suffix('s').unwrap_or(word).to_string()
        }
        fn article(&self, _word: &str) -> &str {
            "a"
        }
        fn conjugate(&self, verb: &str, _t: Tense, _p: Person) -> String {
            verb.to_string()
        }
        fn past_participle(&self, verb: &str) -> String {
            format!("{verb}ed")
        }
        fn present_participle(&self, verb: &str) -> String {
            format!("{verb}ing")
        }
        fn join_list(&self, items: &[&str], _c: Conjunction) -> String {
            items.join(", ")
        }
        fn ordinal(&self, n: usize) -> String {
            format!("{n}th")
        }
        fn number_to_words(&self, n: usize) -> String {
            n.to_string()
        }
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

        let events: Vec<(&str, Context)> = vec![("t", c1), ("t", c2), ("t", c3)];

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

        let events: Vec<(&str, Context)> = vec![("t", low), ("t", high)];

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

        assert!(
            rendered.contains("\n\n"),
            "Expected paragraph break, got: {rendered}"
        );
    }

    // ── Rhetorical grouping ──────────────────────────────────────────────

    #[test]
    fn document_render_preserves_list_style_cycle_across_paragraphs() {
        let mut engine = Engine::new(TestLang)
            .strictness(Strictness::Strict)
            .variation(Variation::Fixed);
        engine
            .register_template(
                "t",
                "The {entity_type} {name} touched {items|truncate:1|join}",
            )
            .unwrap();

        fn list_ctx(name: &str, first_item: &str) -> Context {
            let mut ctx = Context::new();
            ctx.insert("entity_type", Value::String("class".into()));
            ctx.insert("name", Value::String(name.into()));
            ctx.insert(
                "items",
                Value::List(vec![first_item.into(), "cache".into(), "metrics".into()]),
            );
            ctx
        }

        let events: Vec<(&str, Context)> = vec![
            ("t", list_ctx("Alpha", "auth")),
            ("t", list_ctx("Beta", "billing")),
            ("t", list_ctx("Gamma", "search")),
            ("t", list_ctx("Delta", "alerts")),
        ];

        let plan = DocumentPlan::from_events(&events, &engine);
        let mut session = Session::new();
        let rendered = plan.render(&engine, &mut session).unwrap();

        assert_eq!(
            rendered,
            concat!(
                "The class Alpha touched including auth among others.\n\n",
                "The class Beta touched such as billing.\n\n",
                "The class Gamma touched \u{2014} notably search, plus 2 more.\n\n",
                "The class Delta touched [alerts, 2 more].",
            )
        );
        assert_eq!(
            rendered.matches("including ").count(),
            1,
            "paragraph resets must not restart every truncated list with the same style: {rendered}",
        );
    }

    #[test]
    fn document_render_does_not_replay_round_robin_variant_after_paragraph_break() {
        // RoundRobin variation cycles through registered variants by index.
        // Before the paragraph-reset fix, `Session::reset_for_paragraph` wiped
        // the per-key counter, so paragraph 2's first event always re-picked
        // index 0 — replaying paragraph 1's opener verbatim.
        let mut engine = Engine::new(TestLang)
            .strictness(Strictness::Strict)
            .variation(Variation::RoundRobin);
        engine.register_template("t", "First {name}").unwrap();
        engine.register_template("t", "Second {name}").unwrap();
        engine.register_template("t", "Third {name}").unwrap();

        let mut c1 = Context::new();
        c1.insert("entity_type", Value::String("class".into()));
        c1.insert("name", Value::String("Alpha".into()));
        let mut c2 = Context::new();
        c2.insert("entity_type", Value::String("class".into()));
        c2.insert("name", Value::String("Beta".into()));

        let events: Vec<(&str, Context)> = vec![("t", c1), ("t", c2)];
        let plan = DocumentPlan::from_events(&events, &engine);
        // Different entities → two paragraphs.
        assert_eq!(plan.paragraphs.len(), 2);

        let mut session = Session::new();
        let rendered = plan.render(&engine, &mut session).unwrap();

        // Paragraph 1 → "First Alpha"; paragraph 2 must rotate to the next
        // variant, not restart at "First Beta".
        assert!(
            rendered.starts_with("First Alpha"),
            "expected paragraph 1 to use variant 0: {rendered}"
        );
        assert!(
            rendered.contains("\n\nSecond Beta"),
            "expected paragraph 2 to advance to variant 1, not replay \"First\": {rendered}"
        );
        assert!(
            !rendered.contains("First Beta"),
            "round-robin counter must survive the paragraph reset: {rendered}"
        );
    }

    #[test]
    fn document_render_does_not_pronominalize_across_paragraph_break() {
        // Paragraph 1 establishes Foo as the focus entity. After the
        // paragraph break the entity table is cleared, so paragraph 2's
        // reference to Foo must reintroduce it in full form rather than
        // resolve as the lingering pronoun antecedent.
        let mut engine = Engine::new(TestLang)
            .strictness(Strictness::Strict)
            .variation(Variation::Fixed);
        engine
            .register_template("p1", "{name} was modified")
            .unwrap();
        engine
            .register_template("p2", "{other} also changed")
            .unwrap();

        let mut c1 = Context::new();
        c1.insert("entity_type", Value::String("class".into()));
        c1.insert("name", Value::String("Foo".into()));
        let mut c2 = Context::new();
        c2.insert("entity_type", Value::String("class".into()));
        c2.insert("other", Value::String("Bar".into()));

        // Two distinct entity contexts → two paragraphs.
        let events: Vec<(&str, Context)> = vec![("p1", c1), ("p2", c2)];
        let plan = DocumentPlan::from_events(&events, &engine);
        assert_eq!(plan.paragraphs.len(), 2);

        let mut session = Session::new();
        let _ = plan.render(&engine, &mut session).unwrap();

        // After the second paragraph renders, Foo's prior-paragraph mention
        // must NOT survive in the entity table — querying the discourse
        // state for Foo must return Full so any subsequent reference would
        // reintroduce the entity rather than emit a stranded pronoun.
        use crate::discourse::ReferenceForm;
        assert_eq!(
            session.discourse().reference_form("Foo"),
            ReferenceForm::Full,
            "paragraph reset must clear entity table to prevent anaphora leak",
        );
    }

    fn ctx_with_entity(name: &str, count: i64) -> Context {
        let mut c = Context::new();
        c.insert("entity_type", Value::String("class".into()));
        c.insert("name", Value::String(name.into()));
        c.insert("consumer_count", Value::Number(count));
        c
    }

    #[test]
    fn default_classifier_buckets_common_keys() {
        assert_eq!(
            default_classifier("code.deleted"),
            RhetoricalCategory::Removal
        );
        assert_eq!(
            default_classifier("code.removed"),
            RhetoricalCategory::Removal
        );
        assert_eq!(
            default_classifier("code.added"),
            RhetoricalCategory::Addition
        );
        assert_eq!(
            default_classifier("code.introduced"),
            RhetoricalCategory::Addition
        );
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

        let plan = DocumentPlan::from_events_grouped(&events, &engine, GroupingStrategy::ByAction);

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

        let plan = DocumentPlan::from_events_grouped(&events, &engine, GroupingStrategy::ByAction);

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

        let plan = DocumentPlan::from_events_grouped(&events, &engine, GroupingStrategy::ByAction);

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

    // ── Phase 3: Paragraph relations ────────────────────────────────────────

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

    #[test]
    fn paragraph_relations_len_matches_events_len() {
        let mut p = Paragraph::new();
        p.push("t".into(), Context::new(), Salience::Low);
        p.push_with_relation(
            "t".into(),
            Context::new(),
            Salience::Low,
            Some(RstRelation::Elaboration),
        );
        p.push("t".into(), Context::new(), Salience::Medium);
        assert_eq!(p.events.len(), p.relations.len());
        assert_eq!(p.relations.len(), 3);
    }

    // ── Phase 4: from_events_with_relations ─────────────────────────────────

    #[test]
    fn from_events_with_relations_threads_rel() {
        let engine = test_engine();
        let events = vec![
            ("t", ctx_with_entity("Foo", 1), None),
            (
                "t",
                ctx_with_entity("Foo", 1),
                Some(RstRelation::Elaboration),
            ),
        ];
        let plan = DocumentPlan::from_events_with_relations(&events, &engine);
        assert_eq!(plan.paragraphs.len(), 1);
        assert_eq!(plan.paragraphs[0].relations[0], None);
        assert_eq!(
            plan.paragraphs[0].relations[1],
            Some(RstRelation::Elaboration)
        );
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
        for p in &plan.paragraphs {
            assert_eq!(p.relations, vec![None]);
        }
    }

    // ── Phase 6: DocumentPlan::render with relations ─────────────────────────

    // ── Temporal anchor spans paragraphs ────────────────────────────────────

    #[cfg(feature = "time")]
    #[test]
    fn document_plan_temporal_anchor_spans_paragraphs() {
        let mut engine = Engine::new(TestLang)
            .strictness(Strictness::Strict)
            .variation(Variation::Fixed)
            .reference_time(1_700_000_000);
        engine
            .register_template("t", "{name} changed {ts|since_last}")
            .unwrap();

        let t1: i64 = 1_700_000_000;
        let t2: i64 = t1 + 86400;

        let mut c1 = ctx_with_entity("Foo", 1);
        c1.insert("ts", Value::Number(t1));
        c1.insert("timestamp", Value::Number(t1));

        let mut c2 = ctx_with_entity("Bar", 1);
        c2.insert("ts", Value::Number(t2));
        c2.insert("timestamp", Value::Number(t2));

        let events: Vec<(&str, Context)> = vec![("t", c1), ("t", c2)];
        let plan = DocumentPlan::from_events(&events, &engine);
        // Two different entities → two paragraphs.
        assert_eq!(plan.paragraphs.len(), 2);

        let mut s = Session::new();
        let out = plan.render(&engine, &mut s).unwrap();

        // The temporal anchor threads through session.reset() between paragraphs —
        // Bar's paragraph reads "the next day", not an absolute-now-based phrase.
        assert!(out.contains("the next day"), "got: {out}");
    }

    #[test]
    fn document_render_uses_marker_when_paragraph_has_relation() {
        let mut engine = test_engine();
        engine
            .register_template("t", "The class {name} was modified")
            .unwrap();

        let events = vec![
            ("t", ctx_with_entity("Foo", 1), None),
            ("t", ctx_with_entity("Foo", 1), Some(RstRelation::Contrast)),
        ];
        let plan = DocumentPlan::from_events_with_relations(&events, &engine);
        let mut s = Session::new();
        let rendered = plan.render(&engine, &mut s).unwrap();
        assert!(rendered.contains("However, "), "got: {rendered}");
    }

    // ── Phase 3: parallel rendering ─────────────────────────────────────────

    #[test]
    fn engine_and_session_are_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Engine>();
        assert_send_sync::<Session>();
    }

    #[cfg(feature = "parallel")]
    #[test]
    fn render_parallel_produces_same_output_for_independent_paragraphs() {
        // When paragraphs don't need temporal threading, parallel and sequential
        // must produce byte-identical output.
        let mut engine = Engine::new(TestLang)
            .strictness(Strictness::Strict)
            .variation(Variation::Fixed);
        engine.register_template("t", "{name} changed").unwrap();

        let events: Vec<(&str, Context)> = vec![
            ("t", ctx_with_entity("Alpha", 1)),
            ("t", ctx_with_entity("Beta", 1)),
        ];
        let plan = DocumentPlan::from_events(&events, &engine);

        let mut s1 = Session::new();
        let seq = plan.render(&engine, &mut s1).unwrap();

        let s2 = Session::new();
        let par = plan.render_parallel(&engine, &s2).unwrap();

        assert_eq!(seq, par);
    }

    #[cfg(feature = "parallel")]
    #[test]
    fn render_parallel_empty_plan_returns_empty_string() {
        let engine = test_engine();
        let plan = DocumentPlan::new();
        let s = Session::new();
        let out = plan.render_parallel(&engine, &s).unwrap();
        assert_eq!(out, "");
    }

    #[cfg(feature = "parallel")]
    #[test]
    fn render_parallel_single_paragraph_matches_sequential() {
        let engine = test_engine();
        let events: Vec<(&str, Context)> = vec![("t", ctx_with_entity("Foo", 5))];
        let plan = DocumentPlan::from_events(&events, &engine);

        let mut s1 = Session::new();
        let seq = plan.render(&engine, &mut s1).unwrap();

        let s2 = Session::new();
        let par = plan.render_parallel(&engine, &s2).unwrap();

        assert_eq!(seq, par);
    }
}
