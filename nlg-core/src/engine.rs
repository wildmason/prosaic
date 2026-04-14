use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::context::{Context, IntoContext, Value};
use crate::discourse::{DiscourseState, ListStyle};
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
/// configuration, and discourse state for natural cross-sentence rendering.
pub struct Engine {
    language: Box<dyn Language>,
    templates: HashMap<String, Vec<Template>>,
    strictness: Strictness,
    variation: Variation,
    round_robin_counters: HashMap<String, AtomicUsize>,
    discourse: RefCell<DiscourseState>,
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
            discourse: RefCell::new(DiscourseState::new()),
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

    /// Clear all discourse state. Call this between unrelated rendering contexts.
    pub fn reset(&self) {
        self.discourse.borrow_mut().reset();
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
    ///
    /// The engine tracks discourse state across calls: entity mentions,
    /// template history, word frequency. Each call benefits from context
    /// established by previous calls. Use `reset()` between unrelated sequences.
    pub fn render(&self, key: &str, context: impl IntoContext) -> Result<String, NlgError> {
        let alternatives = self
            .templates
            .get(key)
            .ok_or_else(|| NlgError::UnknownTemplate(key.to_string()))?;

        let context = context.into_context();

        // Advance discourse state
        self.discourse.borrow_mut().begin_render();

        // Extract entity info from context for discourse tracking
        let entity_name = context
            .get("name")
            .or_else(|| context.get("old_name"))
            .map(|v| v.as_display());
        let entity_type = context.get("entity_type").map(|v| v.as_display());

        // Detect discourse connective
        let connective = {
            let mut discourse = self.discourse.borrow_mut();
            let relation = discourse.detect_relation(key, entity_name.as_deref());
            discourse.select_connective(&relation)
        };

        // Select template with choosebest scoring and anti-repeat
        let (template, variant_index) =
            self.select_alternative_scored(key, alternatives, &context)?;

        // Record template choice
        self.discourse
            .borrow_mut()
            .record_template_choice(key, variant_index);

        // Render the template
        let mut output = self.render_template(key, template, &context)?;

        // Prepend discourse connective if applicable
        if let Some(conn) = connective {
            // "It also" needs to replace the subject, others prepend
            if conn.starts_with("It ") {
                // Replace "The {type} {name}" at the start with the connective
                output = prepend_replacing_subject(&output, conn);
            } else {
                output = format!("{conn} {}", lowercase_first(&output));
            }
        }

        // Record entity mention in discourse state
        if let (Some(name), Some(etype)) = (&entity_name, &entity_type) {
            self.discourse
                .borrow_mut()
                .mention_entity(name, etype);
        }

        // Record output words for future repetition scoring
        self.discourse
            .borrow_mut()
            .record_output_words(&output);

        Ok(output)
    }

    /// Render a one-off template string (not registered) with the given context.
    /// Inline templates do not participate in discourse tracking (no connectives,
    /// no entity tracking) but do record output words for repetition scoring.
    pub fn render_inline(
        &self,
        source: &str,
        context: impl IntoContext,
    ) -> Result<String, NlgError> {
        let template = Template::parse(source)?;
        let context = context.into_context();
        let output = self.render_template("<inline>", &template, &context)?;
        self.discourse
            .borrow_mut()
            .record_output_words(&output);
        Ok(output)
    }

    /// Render a batch of events as a cohesive paragraph.
    ///
    /// Compared to calling `render()` sequentially, batch rendering can:
    /// - Aggregate events with shared subjects ("was renamed and moved")
    /// - Order events for narrative flow
    /// - Insert discourse connectives between sentences
    pub fn render_batch(
        &self,
        events: &[(&str, Context)],
    ) -> Result<String, NlgError> {
        if events.is_empty() {
            return Ok(String::new());
        }

        // Group events by entity name for potential aggregation
        let mut sentences: Vec<String> = Vec::new();
        let mut i = 0;

        while i < events.len() {
            let (key, ref ctx) = events[i];
            let entity_name = ctx
                .get("name")
                .or_else(|| ctx.get("old_name"))
                .map(|v| v.as_display());

            // Look ahead for aggregation: same entity, different action
            let mut aggregated = vec![i];
            if let Some(ref name) = entity_name {
                let mut j = i + 1;
                while j < events.len() {
                    let (_, ref next_ctx) = events[j];
                    let next_name = next_ctx
                        .get("name")
                        .or_else(|| next_ctx.get("old_name"))
                        .map(|v| v.as_display());
                    if next_name.as_deref() == Some(name.as_str()) {
                        aggregated.push(j);
                        j += 1;
                    } else {
                        break;
                    }
                }
            }

            if aggregated.len() > 1 {
                // Render first event normally, then aggregate subsequent actions
                let sentence = self.render(key, &events[aggregated[0]].1)?;

                // Extract verb phrases from subsequent events and append with "and"
                let mut additional_actions: Vec<String> = Vec::new();
                for &idx in &aggregated[1..] {
                    let (agg_key, ref agg_ctx) = events[idx];
                    // Render the additional event and extract the action part
                    let full = self.render(agg_key, agg_ctx)?;
                    // Try to extract just the action (after "was " or similar)
                    if let Some(action) = extract_action_phrase(&full) {
                        additional_actions.push(action);
                    } else {
                        // Can't extract — render as separate sentence
                        sentences.push(full);
                    }
                }

                if additional_actions.is_empty() {
                    sentences.push(sentence);
                } else {
                    // Append aggregated actions with "and"
                    let combined = format!(
                        "{} and {}",
                        sentence.trim_end_matches('.'),
                        additional_actions.join(" and ")
                    );
                    sentences.push(combined);
                }

                i += aggregated.len();
            } else {
                sentences.push(self.render(key, &events[i].1)?);
                i += 1;
            }
        }

        Ok(sentences.join(" "))
    }

    fn select_alternative_scored<'a>(
        &self,
        key: &str,
        alternatives: &'a [Template],
        context: &Context,
    ) -> Result<(&'a Template, usize), NlgError> {
        if alternatives.len() == 1 {
            return Ok((&alternatives[0], 0));
        }

        // Extract what we need from discourse, then drop the borrow
        // so render_template can borrow_mut for list style selection.
        let (last_variant, is_first) = {
            let discourse = self.discourse.borrow();
            (
                discourse.last_template_variant(key),
                discourse.is_first_render(),
            )
        };

        // If we have discourse history and multiple alternatives, use choosebest
        if !is_first && alternatives.len() > 1 {
            // Render all candidates first (this may borrow_mut discourse for list styles)
            let mut candidates: Vec<(usize, String)> = Vec::new();
            for (i, template) in alternatives.iter().enumerate() {
                if Some(i) == last_variant {
                    continue;
                }
                let candidate = self.render_template(key, template, context)?;
                candidates.push((i, candidate));
            }

            // Now score against discourse history (immutable borrow only)
            let discourse = self.discourse.borrow();
            let mut best_index = candidates[0].0;
            let mut best_score = f64::MAX;

            for (i, candidate) in &candidates {
                let score = discourse.repetition_score(candidate);
                if score < best_score {
                    best_score = score;
                    best_index = *i;
                }
            }

            return Ok((&alternatives[best_index], best_index));
        }

        // Fall back to standard variation selection
        let index = self.select_variant_index(key, alternatives.len());
        Ok((&alternatives[index], index))
    }

    fn select_variant_index(&self, key: &str, count: usize) -> usize {
        match self.variation {
            Variation::Fixed => 0,
            Variation::Seeded(seed) => {
                let hash = simple_hash(key, seed);
                hash as usize % count
            }
            Variation::RoundRobin => {
                let counter = self
                    .round_robin_counters
                    .get(key)
                    .map(|c| c.fetch_add(1, Ordering::Relaxed))
                    .unwrap_or(0);
                counter % count
            }
            Variation::Random => {
                let nanos = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .subsec_nanos() as usize;
                nanos % count
            }
        }
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
                Segment::Slot {
                    key: slot_key,
                    pipes,
                } => {
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

        // Check for explicit style override
        let forced_style = match &pipe.arg {
            Some(PipeArg::String(s)) if s == "bracketed" => Some(ListStyle::Bracketed),
            Some(PipeArg::String(s)) if s == "including" => Some(ListStyle::Including),
            Some(PipeArg::String(s)) if s == "such_as" => Some(ListStyle::SuchAs),
            Some(PipeArg::String(s)) if s == "dash" => Some(ListStyle::Dash),
            _ => None,
        };

        let conjunction = match &pipe.arg {
            Some(PipeArg::String(s)) if s == "or" => Conjunction::Or,
            _ => Conjunction::And,
        };

        // Determine list style
        let style = forced_style.unwrap_or_else(|| {
            self.discourse.borrow_mut().next_list_style()
        });

        let refs: Vec<&str> = items.iter().map(|s| s.as_str()).collect();

        // Check if the list was truncated (last item matches "N more" pattern)
        let has_truncation = items.last().is_some_and(|last| {
            last.ends_with(" more")
                && last.split_whitespace().next().is_some_and(|w| w.parse::<usize>().is_ok())
        });

        if has_truncation && items.len() >= 2 {
            let shown = &refs[..refs.len() - 1];
            let remainder = &items[items.len() - 1]; // e.g., "3 more"
            Ok(Value::String(format_truncated_list(
                shown,
                remainder,
                style,
                conjunction,
                &*self.language,
            )))
        } else {
            // No truncation — use standard join, but apply list style wrapper
            let joined = self.language.join_list(&refs, conjunction);
            Ok(Value::String(joined))
        }
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

/// Format a truncated list with natural style.
fn format_truncated_list(
    shown: &[&str],
    remainder: &str,
    style: ListStyle,
    conjunction: Conjunction,
    language: &dyn Language,
) -> String {
    let joined = language.join_list(shown, conjunction);
    match style {
        ListStyle::Including => {
            format!("including {joined} among others")
        }
        ListStyle::SuchAs => {
            format!("such as {joined}")
        }
        ListStyle::Dash => {
            format!("\u{2014} notably {joined}, plus {remainder}")
        }
        ListStyle::Bracketed => {
            let refs: Vec<&str> = shown
                .iter()
                .copied()
                .chain(std::iter::once(remainder.trim()))
                .collect();
            let all_joined = language.join_list(&refs, conjunction);
            format!("[{all_joined}]")
        }
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

fn lowercase_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => {
            let mut result = String::with_capacity(s.len());
            for lower in c.to_lowercase() {
                result.push(lower);
            }
            result.extend(chars);
            result
        }
    }
}

/// Try to replace "The {type} {name} was ..." with a connective like "It also was ..."
fn prepend_replacing_subject(output: &str, connective: &str) -> String {
    // Look for pattern: "The <word> <word> was" or "The <word> <word> has"
    if let Some(rest) = output.strip_prefix("The ") {
        // Skip entity_type and name (two words)
        let words: Vec<&str> = rest.splitn(3, ' ').collect();
        if words.len() >= 3 {
            return format!("{connective} {}", words[2..].join(" "));
        }
    }
    // Fallback: just prepend
    format!("{connective} {}", lowercase_first(output))
}

/// Try to extract the action phrase from a rendered sentence.
/// e.g., from "The class Foo was renamed to Bar" → "renamed to Bar"
fn extract_action_phrase(sentence: &str) -> Option<String> {
    // Look for "was <past_participle> ..." pattern
    if let Some(idx) = sentence.find(" was ") {
        let after_was = &sentence[idx + 5..];
        if !after_was.is_empty() {
            return Some(after_was.to_string());
        }
    }
    None
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

    // ── Basic rendering (backward compatibility) ─────────────────────────

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

        engine.reset();
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

        engine.reset();
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
    fn render_truncate_then_join_bracketed() {
        let mut engine = test_engine();
        engine
            .register_template("t", "{items|truncate:2|join:bracketed}")
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
        assert_eq!(engine.render("t", &ctx).unwrap(), "[a, b, and 3 more]");
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
        // First render always picks first (no discourse history yet)
        let result = engine.render("t", &ctx).unwrap();
        assert_eq!(result, "first");
    }

    #[test]
    fn variation_seeded_is_deterministic() {
        let mut engine = test_engine().variation(Variation::Seeded(42));
        engine.register_template("t", "first").unwrap();
        engine.register_template("t", "second").unwrap();

        let ctx = Context::new();
        let result1 = engine.render("t", &ctx).unwrap();
        engine.reset();
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

    // ── Discourse-aware tests ────────────────────────────────────────────

    #[test]
    fn reset_clears_discourse_state() {
        let mut engine = test_engine();
        engine
            .register_template("t", "The {entity_type} {name} was modified")
            .unwrap();

        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("class".into()));
        ctx.insert("name", Value::String("Foo".into()));

        engine.render("t", &ctx).unwrap();
        engine.reset();

        // After reset, should behave as if first render
        let result = engine.render("t", &ctx).unwrap();
        assert!(result.starts_with("The class Foo"));
    }

    #[test]
    fn template_anti_repeat_with_multiple_variants() {
        let mut engine = test_engine();
        engine.register_template("t", "variant A").unwrap();
        engine.register_template("t", "variant B").unwrap();
        engine.register_template("t", "variant C").unwrap();

        let ctx = Context::new();
        let r1 = engine.render("t", &ctx).unwrap();
        let r2 = engine.render("t", &ctx).unwrap();

        // Second render should pick a different variant than the first
        assert_ne!(r1, r2);
    }

    #[test]
    fn list_style_cycles_across_renders() {
        let mut engine = test_engine();
        engine
            .register_template("t", "{items|truncate:1|join}")
            .unwrap();

        let mut ctx = Context::new();
        ctx.insert(
            "items",
            Value::List(vec!["alpha".into(), "beta".into(), "gamma".into()]),
        );

        let r1 = engine.render("t", &ctx).unwrap();
        let r2 = engine.render("t", &ctx).unwrap();
        let r3 = engine.render("t", &ctx).unwrap();
        let r4 = engine.render("t", &ctx).unwrap();

        // Each should use a different list style
        let results = vec![r1, r2, r3, r4];
        let unique: std::collections::HashSet<&String> = results.iter().collect();
        assert!(unique.len() >= 3, "Expected at least 3 unique list styles, got {}: {:?}", unique.len(), results);
    }

    #[test]
    fn bracketed_style_forced() {
        let mut engine = test_engine();
        engine
            .register_template("t", "{items|truncate:1|join:bracketed}")
            .unwrap();

        let mut ctx = Context::new();
        ctx.insert(
            "items",
            Value::List(vec!["alpha".into(), "beta".into(), "gamma".into()]),
        );

        let result = engine.render("t", &ctx).unwrap();
        assert!(result.starts_with('[') && result.ends_with(']'),
            "Expected bracketed format, got: {result}");
    }
}
