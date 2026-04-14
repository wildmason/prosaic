use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::context::{Context, IntoContext, Value};
use crate::error::NlgError;
use crate::language::{Conjunction, Language};
use crate::template::{Pipe, PipeArg, Segment, Template};

/// Controls how missing slots are handled during rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strictness {
    /// Missing slot produces an error.
    Strict,
    /// Missing slot renders as `[missing: slot_name]`.
    Lenient,
    /// Missing slot renders as an empty string.
    Silent,
}

impl Default for Strictness {
    fn default() -> Self {
        Self::Strict
    }
}

/// Controls how template alternatives are selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variation {
    /// Always pick the first registered template.
    Fixed,
    /// Deterministic selection based on a seed — varies across alternatives
    /// but produces the same result for the same seed.
    Seeded(u64),
    /// Cycle through alternatives in registration order.
    RoundRobin,
    /// Select randomly (non-deterministic).
    Random,
}

impl Default for Variation {
    fn default() -> Self {
        Self::Fixed
    }
}

/// The core NLG engine. Holds a language implementation, template registry,
/// and configuration for rendering.
pub struct Engine {
    language: Box<dyn Language>,
    templates: HashMap<String, Vec<Template>>,
    strictness: Strictness,
    variation: Variation,
    round_robin_counters: HashMap<String, AtomicUsize>,
}

impl Engine {
    /// Create a new engine with the given language implementation.
    pub fn new(language: impl Language + 'static) -> Self {
        Self {
            language: Box::new(language),
            templates: HashMap::new(),
            strictness: Strictness::default(),
            variation: Variation::default(),
            round_robin_counters: HashMap::new(),
        }
    }

    /// Set the strictness mode for missing slot handling.
    pub fn strictness(mut self, strictness: Strictness) -> Self {
        self.strictness = strictness;
        self
    }

    /// Set the variation strategy for template selection.
    pub fn variation(mut self, variation: Variation) -> Self {
        self.variation = variation;
        self
    }

    /// Get a reference to the language implementation.
    pub fn language(&self) -> &dyn Language {
        &*self.language
    }

    /// Register a template string under a key. Multiple templates registered
    /// under the same key become alternatives for variation.
    pub fn register_template(&mut self, key: &str, source: &str) -> Result<(), NlgError> {
        let template = Template::parse(source)?;
        self.templates
            .entry(key.to_string())
            .or_default()
            .push(template);
        Ok(())
    }

    /// Render a registered template with the given context.
    pub fn render(&self, key: &str, context: impl IntoContext) -> Result<String, NlgError> {
        let alternatives = self
            .templates
            .get(key)
            .ok_or_else(|| NlgError::UnknownTemplate(key.to_string()))?;

        let template = self.select_alternative(key, alternatives);
        let context = context.into_context();
        self.render_template(key, template, &context)
    }

    /// Render a one-off template string (not registered) with the given context.
    pub fn render_inline(&self, source: &str, context: impl IntoContext) -> Result<String, NlgError> {
        let template = Template::parse(source)?;
        let context = context.into_context();
        self.render_template("<inline>", &template, &context)
    }

    fn select_alternative<'a>(&self, key: &str, alternatives: &'a [Template]) -> &'a Template {
        if alternatives.len() == 1 {
            return &alternatives[0];
        }

        let index = match self.variation {
            Variation::Fixed => 0,
            Variation::Seeded(seed) => {
                // Simple hash-based selection: mix seed with key
                let hash = simple_hash(key, seed);
                hash as usize % alternatives.len()
            }
            Variation::RoundRobin => {
                let counter = self
                    .round_robin_counters
                    .get(key)
                    .map(|c| c.fetch_add(1, Ordering::Relaxed))
                    .unwrap_or(0);
                counter % alternatives.len()
            }
            Variation::Random => {
                // Use a simple time-based approach for non-deterministic selection.
                // Not cryptographically random, but good enough for text variation.
                let nanos = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .subsec_nanos() as usize;
                nanos % alternatives.len()
            }
        };

        &alternatives[index]
    }

    fn render_template(
        &self,
        key: &str,
        template: &Template,
        context: &Context,
    ) -> Result<String, NlgError> {
        let mut output = String::new();

        for segment in &template.segments {
            match segment {
                Segment::Literal(text) => output.push_str(text),
                Segment::Slot { key: slot_key, pipes } => {
                    let rendered = self.render_slot(key, slot_key, pipes, context)?;
                    output.push_str(&rendered);
                }
            }
        }

        Ok(output)
    }

    fn render_slot(
        &self,
        template_key: &str,
        slot_key: &str,
        pipes: &[Pipe],
        context: &Context,
    ) -> Result<String, NlgError> {
        let value = match context.get(slot_key) {
            Some(v) => v.clone(),
            None => return self.handle_missing_slot(template_key, slot_key),
        };

        if pipes.is_empty() {
            return Ok(value.as_display());
        }

        let mut current = value;
        for pipe in pipes {
            current = self.apply_pipe(pipe, &current, context)?;
        }

        Ok(current.as_display())
    }

    fn handle_missing_slot(
        &self,
        template_key: &str,
        slot_key: &str,
    ) -> Result<String, NlgError> {
        match self.strictness {
            Strictness::Strict => Err(NlgError::MissingSlot {
                template: template_key.to_string(),
                slot: slot_key.to_string(),
            }),
            Strictness::Lenient => Ok(format!("[missing: {slot_key}]")),
            Strictness::Silent => Ok(String::new()),
        }
    }

    fn apply_pipe(
        &self,
        pipe: &Pipe,
        value: &Value,
        context: &Context,
    ) -> Result<Value, NlgError> {
        match pipe.name.as_str() {
            "pluralize" => self.pipe_pluralize(pipe, value, context),
            "article" => self.pipe_article(value),
            "join" => self.pipe_join(pipe, value),
            "ordinal" => self.pipe_ordinal(value),
            "words" => self.pipe_words(value),
            "truncate" => self.pipe_truncate(pipe, value),
            "capitalize" => self.pipe_capitalize(value),
            _ => Err(NlgError::InvalidPipe {
                pipe: pipe.name.clone(),
                reason: "unknown pipe".to_string(),
            }),
        }
    }

    fn pipe_pluralize(
        &self,
        pipe: &Pipe,
        value: &Value,
        _context: &Context,
    ) -> Result<Value, NlgError> {
        let word = match &pipe.arg {
            Some(PipeArg::String(w)) => w.as_str(),
            _ => {
                return Err(NlgError::InvalidPipe {
                    pipe: "pluralize".to_string(),
                    reason: "requires a word argument, e.g., {count|pluralize:item}".to_string(),
                });
            }
        };

        let count = value.as_number().ok_or_else(|| NlgError::InvalidPipe {
            pipe: "pluralize".to_string(),
            reason: "value must be a number".to_string(),
        })? as usize;

        Ok(Value::String(self.language.pluralize(word, count)))
    }

    fn pipe_article(&self, value: &Value) -> Result<Value, NlgError> {
        let word = value.as_display();
        let article = self.language.article(&word);
        Ok(Value::String(format!("{article} {word}")))
    }

    fn pipe_join(&self, pipe: &Pipe, value: &Value) -> Result<Value, NlgError> {
        let items = value.as_list().ok_or_else(|| NlgError::InvalidPipe {
            pipe: "join".to_string(),
            reason: "value must be a list".to_string(),
        })?;

        let conjunction = match &pipe.arg {
            Some(PipeArg::String(s)) if s == "or" => Conjunction::Or,
            _ => Conjunction::And,
        };

        let refs: Vec<&str> = items.iter().map(|s| s.as_str()).collect();
        Ok(Value::String(self.language.join_list(&refs, conjunction)))
    }

    fn pipe_ordinal(&self, value: &Value) -> Result<Value, NlgError> {
        let n = value.as_number().ok_or_else(|| NlgError::InvalidPipe {
            pipe: "ordinal".to_string(),
            reason: "value must be a number".to_string(),
        })? as usize;

        Ok(Value::String(self.language.ordinal(n)))
    }

    fn pipe_words(&self, value: &Value) -> Result<Value, NlgError> {
        let n = value.as_number().ok_or_else(|| NlgError::InvalidPipe {
            pipe: "words".to_string(),
            reason: "value must be a number".to_string(),
        })? as usize;

        Ok(Value::String(self.language.number_to_words(n)))
    }

    fn pipe_truncate(&self, pipe: &Pipe, value: &Value) -> Result<Value, NlgError> {
        let max = match &pipe.arg {
            Some(PipeArg::Number(n)) => *n,
            _ => {
                return Err(NlgError::InvalidPipe {
                    pipe: "truncate".to_string(),
                    reason: "requires a numeric argument, e.g., {items|truncate:3}".to_string(),
                });
            }
        };

        let items = value.as_list().ok_or_else(|| NlgError::InvalidPipe {
            pipe: "truncate".to_string(),
            reason: "value must be a list".to_string(),
        })?;

        if items.len() <= max {
            return Ok(value.clone());
        }

        let remaining = items.len() - max;
        let mut truncated: Vec<String> = items[..max].to_vec();
        // Use "{N} more" without "and" — the join pipe's conjunction
        // will supply "and" or "or" naturally.
        let suffix = format!("{remaining} more");
        truncated.push(suffix);

        Ok(Value::List(truncated))
    }

    fn pipe_capitalize(&self, value: &Value) -> Result<Value, NlgError> {
        let s = value.as_display();
        let capitalized = capitalize_first(&s);
        Ok(Value::String(capitalized))
    }
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => {
            let mut result = String::with_capacity(s.len());
            for upper in c.to_uppercase() {
                result.push(upper);
            }
            result.extend(chars);
            result
        }
    }
}

/// Simple non-cryptographic hash for seeded variation.
fn simple_hash(key: &str, seed: u64) -> u64 {
    let mut hash = seed;
    for byte in key.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(byte as u64);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::language::{Conjunction, Language, Person, Tense};

    /// Minimal language implementation for testing the engine in isolation.
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
        fn article(&self, word: &str) -> &str {
            if word.starts_with(|c: char| "aeiou".contains(c.to_ascii_lowercase())) {
                "an"
            } else {
                "a"
            }
        }
        fn conjugate(&self, verb: &str, tense: Tense, _person: Person) -> String {
            match tense {
                Tense::Past => format!("{verb}ed"),
                Tense::Present => verb.to_string(),
                Tense::Future => format!("will {verb}"),
            }
        }
        fn past_participle(&self, verb: &str) -> String {
            format!("{verb}ed")
        }
        fn join_list(&self, items: &[&str], conjunction: Conjunction) -> String {
            let conj = match conjunction {
                Conjunction::And => "and",
                Conjunction::Or => "or",
            };
            match items.len() {
                0 => String::new(),
                1 => items[0].to_string(),
                2 => format!("{} {conj} {}", items[0], items[1]),
                _ => {
                    let head = items[..items.len() - 1].join(", ");
                    format!("{head}, {conj} {}", items[items.len() - 1])
                }
            }
        }
        fn ordinal(&self, n: usize) -> String {
            let suffix = match n % 10 {
                1 if n % 100 != 11 => "st",
                2 if n % 100 != 12 => "nd",
                3 if n % 100 != 13 => "rd",
                _ => "th",
            };
            format!("{n}{suffix}")
        }
        fn number_to_words(&self, n: usize) -> String {
            format!("<{n}>") // stub
        }
    }

    fn test_engine() -> Engine {
        Engine::new(TestLang)
    }

    #[test]
    fn render_simple_substitution() {
        let mut engine = test_engine();
        engine.register_template("greet", "Hello {name}!").unwrap();

        let mut ctx = Context::new();
        ctx.insert("name", Value::String("world".into()));

        assert_eq!(engine.render("greet", &ctx).unwrap(), "Hello world!");
    }

    #[test]
    fn render_missing_slot_strict() {
        let mut engine = test_engine();
        engine.register_template("greet", "Hello {name}!").unwrap();
        let ctx = Context::new();

        let result = engine.render("greet", &ctx);
        assert!(matches!(result, Err(NlgError::MissingSlot { .. })));
    }

    #[test]
    fn render_missing_slot_lenient() {
        let mut engine = test_engine().strictness(Strictness::Lenient);
        engine.register_template("greet", "Hello {name}!").unwrap();
        let ctx = Context::new();

        assert_eq!(
            engine.render("greet", &ctx).unwrap(),
            "Hello [missing: name]!"
        );
    }

    #[test]
    fn render_missing_slot_silent() {
        let mut engine = test_engine().strictness(Strictness::Silent);
        engine.register_template("greet", "Hello {name}!").unwrap();
        let ctx = Context::new();

        assert_eq!(engine.render("greet", &ctx).unwrap(), "Hello !");
    }

    #[test]
    fn render_unknown_template() {
        let engine = test_engine();
        let ctx = Context::new();

        let result = engine.render("nonexistent", &ctx);
        assert!(matches!(result, Err(NlgError::UnknownTemplate(_))));
    }

    #[test]
    fn render_pluralize_pipe() {
        let mut engine = test_engine();
        engine
            .register_template("count", "{n} {n|pluralize:item}")
            .unwrap();

        let mut ctx = Context::new();
        ctx.insert("n", Value::Number(1));
        assert_eq!(engine.render("count", &ctx).unwrap(), "1 item");

        ctx.insert("n", Value::Number(5));
        assert_eq!(engine.render("count", &ctx).unwrap(), "5 items");
    }

    #[test]
    fn render_article_pipe() {
        let mut engine = test_engine();
        engine
            .register_template("a", "{thing|article}")
            .unwrap();

        let mut ctx = Context::new();
        ctx.insert("thing", Value::String("apple".into()));
        assert_eq!(engine.render("a", &ctx).unwrap(), "an apple");

        ctx.insert("thing", Value::String("banana".into()));
        assert_eq!(engine.render("a", &ctx).unwrap(), "a banana");
    }

    #[test]
    fn render_join_pipe() {
        let mut engine = test_engine();
        engine
            .register_template("list", "{items|join}")
            .unwrap();

        let mut ctx = Context::new();
        ctx.insert(
            "items",
            Value::List(vec!["a".into(), "b".into(), "c".into()]),
        );
        assert_eq!(engine.render("list", &ctx).unwrap(), "a, b, and c");
    }

    #[test]
    fn render_join_or_pipe() {
        let mut engine = test_engine();
        engine
            .register_template("list", "{items|join:or}")
            .unwrap();

        let mut ctx = Context::new();
        ctx.insert(
            "items",
            Value::List(vec!["a".into(), "b".into(), "c".into()]),
        );
        assert_eq!(engine.render("list", &ctx).unwrap(), "a, b, or c");
    }

    #[test]
    fn render_truncate_then_join() {
        let mut engine = test_engine();
        engine
            .register_template("t", "{items|truncate:2|join}")
            .unwrap();

        let mut ctx = Context::new();
        ctx.insert(
            "items",
            Value::List(vec![
                "a".into(),
                "b".into(),
                "c".into(),
                "d".into(),
                "e".into(),
            ]),
        );
        assert_eq!(engine.render("t", &ctx).unwrap(), "a, b, and 3 more");
    }

    #[test]
    fn render_capitalize_pipe() {
        let mut engine = test_engine();
        engine
            .register_template("cap", "{word|capitalize}")
            .unwrap();

        let mut ctx = Context::new();
        ctx.insert("word", Value::String("hello".into()));
        assert_eq!(engine.render("cap", &ctx).unwrap(), "Hello");
    }

    #[test]
    fn render_ordinal_pipe() {
        let mut engine = test_engine();
        engine.register_template("o", "{n|ordinal}").unwrap();

        let mut ctx = Context::new();
        ctx.insert("n", Value::Number(3));
        assert_eq!(engine.render("o", &ctx).unwrap(), "3rd");
    }

    #[test]
    fn render_inline_template() {
        let engine = test_engine();
        let mut ctx = Context::new();
        ctx.insert("name", Value::String("world".into()));

        assert_eq!(
            engine.render_inline("Hello {name}!", &ctx).unwrap(),
            "Hello world!"
        );
    }

    #[test]
    fn variation_fixed_always_picks_first() {
        let mut engine = test_engine().variation(Variation::Fixed);
        engine.register_template("t", "first").unwrap();
        engine.register_template("t", "second").unwrap();

        let ctx = Context::new();
        for _ in 0..10 {
            assert_eq!(engine.render("t", &ctx).unwrap(), "first");
        }
    }

    #[test]
    fn variation_seeded_is_deterministic() {
        let mut engine = test_engine().variation(Variation::Seeded(42));
        engine.register_template("t", "first").unwrap();
        engine.register_template("t", "second").unwrap();

        let ctx = Context::new();
        let result1 = engine.render("t", &ctx).unwrap();
        let result2 = engine.render("t", &ctx).unwrap();
        assert_eq!(result1, result2);
    }

    #[test]
    fn unknown_pipe_is_error() {
        let mut engine = test_engine();
        engine.register_template("t", "{name|nonexistent}").unwrap();

        let mut ctx = Context::new();
        ctx.insert("name", Value::String("test".into()));

        let result = engine.render("t", &ctx);
        assert!(matches!(result, Err(NlgError::InvalidPipe { .. })));
    }

    #[test]
    fn complex_template_end_to_end() {
        let mut engine = test_engine();
        engine
            .register_template(
                "entity.renamed",
                "The {entity_type} {old_name} was renamed to {new_name} \
                 which impacts {count} direct {count|pluralize:consumer}",
            )
            .unwrap();

        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("class".into()));
        ctx.insert("old_name", Value::String("Foo".into()));
        ctx.insert("new_name", Value::String("Foobar".into()));
        ctx.insert("count", Value::Number(6));

        assert_eq!(
            engine.render("entity.renamed", &ctx).unwrap(),
            "The class Foo was renamed to Foobar which impacts 6 direct consumers"
        );
    }
}
