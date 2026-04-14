use crate::context::Context;
use crate::engine::Engine;
use crate::error::NlgError;
use crate::salience::Salience;

/// A paragraph in a document plan — a group of related events rendered together.
#[derive(Debug, Clone)]
pub struct Paragraph {
    /// The events in this paragraph, in render order.
    pub events: Vec<(String, Context)>,
    /// The highest salience in this paragraph, used for ordering.
    pub salience: Salience,
}

impl Paragraph {
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            salience: Salience::Low,
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

    /// Build a document plan from a flat set of events.
    ///
    /// Organization strategy:
    /// 1. Assign each event a salience (from context or explicit thresholds).
    /// 2. Group consecutive events that share an entity into the same paragraph.
    /// 3. Order paragraphs by highest-salience first.
    ///
    /// Within a paragraph, events keep their original order (which the engine's
    /// discourse state can then leverage for pronouns and connectives).
    pub fn from_events(
        events: &[(&str, Context)],
        engine: &Engine,
    ) -> Self {
        let mut plan = Self::new();
        if events.is_empty() {
            return plan;
        }

        // Group consecutive events that share an entity name into paragraphs
        let mut current = Paragraph::new();
        let mut current_entity: Option<String> = None;

        for (key, ctx) in events {
            let ctx = ctx.clone();
            let salience = engine.context_salience(&ctx);
            let entity_name = ctx
                .get("name")
                .or_else(|| ctx.get("old_name"))
                .map(|v| v.as_display());

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
    pub fn render(&self, engine: &Engine) -> Result<String, NlgError> {
        let mut paragraphs = Vec::new();

        for (idx, p) in self.paragraphs.iter().enumerate() {
            if idx > 0 {
                engine.reset();
            }

            let events: Vec<(&str, Context)> = p
                .events
                .iter()
                .map(|(k, c)| (k.as_str(), c.clone()))
                .collect();

            let rendered = engine.render_batch(&events)?;
            if !rendered.is_empty() {
                paragraphs.push(rendered);
            }
        }

        Ok(paragraphs.join("\n\n"))
    }
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
        assert_eq!(plan.render(&engine).unwrap(), "");
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
        let rendered = plan.render(&engine).unwrap();

        assert!(rendered.contains("\n\n"), "Expected paragraph break, got: {rendered}");
    }
}
