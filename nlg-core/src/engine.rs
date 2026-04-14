use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::context::{Context, IntoContext, Value};
use crate::discourse::{DiscourseState, ListStyle, ReferenceForm};
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

        // If the output starts with a lowercase letter produced by the `refer` pipe,
        // capitalize it. We detect this by checking if the first non-whitespace
        // character is lowercase AND the template's first segment is a `refer` slot.
        if starts_with_refer_pipe(template) {
            output = capitalize_first(&output);
        }

        // Terminate the sentence with a period if it doesn't already end
        // with sentence-ending punctuation.
        output = terminate_sentence(&output);

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
    /// Each event is rendered sequentially through `render()`, which means
    /// the discourse system produces natural cross-sentence flow via
    /// referring expressions, connectives, template anti-repeat, and
    /// list style cycling.
    ///
    /// Additionally, consecutive events sharing a template key but with
    /// different entities are aggregated by combining subjects:
    /// "UserService and AuthService were renamed" instead of two sentences.
    pub fn render_batch(
        &self,
        events: &[(&str, Context)],
    ) -> Result<String, NlgError> {
        if events.is_empty() {
            return Ok(String::new());
        }

        let mut sentences: Vec<String> = Vec::new();
        let mut i = 0;

        while i < events.len() {
            // Look for same-action-different-subject aggregation opportunity
            let aggregation_end = self.find_same_action_run(events, i);

            if aggregation_end > i + 1 {
                // Multiple consecutive events with same template key but
                // different entities — aggregate their subjects.
                let sentence = self.render_aggregated_subjects(
                    events[i].0,
                    &events[i..aggregation_end],
                )?;
                sentences.push(sentence);
                i = aggregation_end;
            } else {
                // Single event — render normally with full discourse benefits
                let (key, ref ctx) = events[i];
                sentences.push(self.render(key, ctx)?);
                i += 1;
            }
        }

        Ok(sentences.join(" "))
    }

    /// Find the end index (exclusive) of a run of consecutive events that
    /// share the same template key AND matching non-subject context, but
    /// have different entity names.
    ///
    /// Only aggregates when the surrounding context (everything except the
    /// entity name) is identical — otherwise we'd lose information like
    /// different new_name targets or different consumer counts.
    ///
    /// Returns `start + 1` if no aggregation opportunity exists.
    fn find_same_action_run(
        &self,
        events: &[(&str, Context)],
        start: usize,
    ) -> usize {
        if start >= events.len() {
            return start;
        }

        let (first_key, ref first_ctx) = events[start];
        let first_name = entity_name_from_context(first_ctx);

        if first_name.is_none() {
            return start + 1;
        }

        let mut end = start + 1;
        let mut seen_names: std::collections::HashSet<String> = std::collections::HashSet::new();
        seen_names.insert(first_name.unwrap());

        while end < events.len() {
            let (key, ref ctx) = events[end];
            if key != first_key {
                break;
            }
            let name = match entity_name_from_context(ctx) {
                Some(n) => n,
                None => break,
            };
            if seen_names.contains(&name) {
                break;
            }
            // Only aggregate if the non-subject context matches.
            // If new_name, consumer_count, consumers, or location differ,
            // sequential rendering preserves more information.
            if !contexts_compatible_for_aggregation(first_ctx, ctx) {
                break;
            }
            seen_names.insert(name);
            end += 1;
        }

        end
    }

    /// Render an aggregated sentence combining multiple subjects for the
    /// same action: "UserService and AuthService were renamed."
    fn render_aggregated_subjects(
        &self,
        key: &str,
        events: &[(&str, Context)],
    ) -> Result<String, NlgError> {
        // Collect entity names
        let names: Vec<String> = events
            .iter()
            .filter_map(|(_, ctx)| entity_name_from_context(ctx))
            .collect();

        if names.is_empty() {
            // Fallback to sequential rendering
            let mut sentences = Vec::new();
            for (k, ctx) in events {
                sentences.push(self.render(k, ctx)?);
            }
            return Ok(sentences.join(" "));
        }

        // Build a synthetic context that uses the combined name
        // "UserService and AuthService" as the entity name
        let refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
        let combined_name = self
            .language
            .join_list(&refs, crate::language::Conjunction::And);

        // Use the first event's context as the base
        let mut combined_ctx = events[0].1.clone();

        // Override the name/old_name with the combined form
        if combined_ctx.get("old_name").is_some() {
            combined_ctx.insert("old_name", Value::String(combined_name.clone()));
        }
        if combined_ctx.get("name").is_some() {
            combined_ctx.insert("name", Value::String(combined_name.clone()));
        }

        // Render with combined subject, then apply plural agreement
        let rendered = self.render(key, combined_ctx)?;
        Ok(pluralize_agreement(&rendered, &*self.language))
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
            "refer" => self.pipe_refer(pipe, value, context),
            _ => Err(NlgError::InvalidPipe {
                pipe: pipe.name.clone(),
                reason: "unknown pipe".to_string(),
            }),
        }
    }

    /// Render a reference to a named entity based on discourse context.
    ///
    /// - First mention: "The {entity_type} {name}" (full form)
    /// - Recent mention as non-focus: "{name}" (short form)
    /// - Recent mention as focus with no ambiguity: "It" (pronoun)
    /// - Distant mention (3+ renders ago): re-introduce with full form
    ///
    /// Usage: `{name|refer}` uses `entity_type` from context.
    /// Usage: `{name|refer:class}` overrides the entity type explicitly.
    fn pipe_refer(
        &self,
        pipe: &Pipe,
        value: &Value,
        context: &Context,
    ) -> Result<Value, NlgError> {
        let name = value.as_display();

        // Determine entity type: explicit arg takes precedence, else context["entity_type"]
        let entity_type = match &pipe.arg {
            Some(PipeArg::String(t)) => t.clone(),
            _ => context
                .get("entity_type")
                .map(|v| v.as_display())
                .unwrap_or_default(),
        };

        let form = self.discourse.borrow().reference_form(&name);

        // Produce lowercase form — the engine will capitalize the first
        // character of the rendered output if needed. This handles both
        // sentence-start and mid-sentence positions correctly.
        let rendered = match form {
            ReferenceForm::Full => {
                if entity_type.is_empty() {
                    name
                } else {
                    format!("the {} {}", entity_type.to_lowercase(), name)
                }
            }
            ReferenceForm::ShortName => name,
            ReferenceForm::Pronoun => "it".to_string(),
        };

        Ok(Value::String(rendered))
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

/// Append a period to the output if it appears to be a sentence without
/// terminal punctuation. A sentence starts with a capital letter and has
/// multiple words. Fragments (single words, lists) are not terminated.
fn terminate_sentence(output: &str) -> String {
    let trimmed_end = output.trim_end();
    if trimmed_end.is_empty() {
        return output.to_string();
    }

    // Already ends with sentence-ending punctuation? Leave alone.
    let last = trimmed_end.chars().last().unwrap();
    if matches!(last, '.' | '!' | '?') {
        return output.to_string();
    }

    // Looks like a fragment (doesn't start with capital, or is short)?
    let first = trimmed_end.chars().next().unwrap();
    if !first.is_uppercase() {
        return output.to_string();
    }

    // Count words — single words or very short outputs are likely fragments
    let word_count = trimmed_end.split_whitespace().count();
    if word_count < 3 {
        return output.to_string();
    }

    // Add period (before trailing whitespace if any)
    let mut s = output.trim_end().to_string();
    s.push('.');
    s
}

/// Check if a template's first segment is a `refer` pipe, meaning the
/// rendered output may start with a lowercase word that needs capitalization.
fn starts_with_refer_pipe(template: &Template) -> bool {
    match template.segments.first() {
        Some(Segment::Slot { pipes, .. }) => {
            pipes.iter().any(|p| p.name == "refer")
        }
        _ => false,
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

/// Extract the primary entity name from a render context.
/// Checks "name" first, falls back to "old_name".
fn entity_name_from_context(context: &Context) -> Option<String> {
    context
        .get("name")
        .or_else(|| context.get("old_name"))
        .map(|v| v.as_display())
}

/// Check if two contexts match on all fields except the entity name.
/// Used to decide whether events can be safely aggregated without losing
/// information (different new_name targets, different consumer counts, etc.).
fn contexts_compatible_for_aggregation(a: &Context, b: &Context) -> bool {
    // Collect all keys from both contexts
    let entity_keys = ["name", "old_name"];

    // Get all keys that need to match
    let a_keys: Vec<&String> = a.keys().filter(|k| !entity_keys.contains(&k.as_str())).collect();
    let b_keys: Vec<&String> = b.keys().filter(|k| !entity_keys.contains(&k.as_str())).collect();

    // Same set of keys?
    if a_keys.len() != b_keys.len() {
        return false;
    }
    for key in &a_keys {
        if !b_keys.contains(key) {
            return false;
        }
        if a.get(key) != b.get(key) {
            return false;
        }
    }
    true
}

/// Adjust rendered output for plural subject agreement.
/// When aggregating multiple subjects, forms like "was" → "were" and
/// singular entity types like "class" → "classes" need to change.
fn pluralize_agreement(output: &str, lang: &dyn Language) -> String {
    let mut result = output.to_string();

    // "The class Foo, Bar, and Baz was" → "The classes Foo, Bar, and Baz were"
    // Common singular-to-plural verb patterns
    let verb_replacements = &[
        (" was ", " were "),
        (" has ", " have "),
        (" is ", " are "),
    ];
    for (singular, plural) in verb_replacements {
        result = result.replace(singular, plural);
    }

    // Pluralize entity type after "The": "The class UserService, Foo, and Bar"
    // This is fragile — only apply when pattern matches exactly.
    if let Some(rest) = result.strip_prefix("The ") {
        if let Some(space_idx) = rest.find(' ') {
            let type_word = &rest[..space_idx];
            // Only pluralize if it's a known simple noun (lowercase word)
            if type_word.chars().all(|c| c.is_lowercase()) && type_word.len() < 15 {
                let plural = lang.pluralize(type_word, 2);
                if plural != type_word {
                    result = format!("The {} {}", plural, &rest[space_idx + 1..]);
                }
            }
        }
    }

    result
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
            "The class Foo was renamed to Foobar which impacts 6 direct consumers."
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

    // ── Refer pipe tests ────────────────────────────────────────────────

    #[test]
    fn refer_first_mention_uses_full_form() {
        let mut engine = test_engine();
        engine
            .register_template("t", "{name|refer} was updated")
            .unwrap();

        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("class".into()));
        ctx.insert("name", Value::String("UserService".into()));

        let result = engine.render("t", &ctx).unwrap();
        assert_eq!(result, "The class UserService was updated.");
    }

    #[test]
    fn refer_second_mention_uses_pronoun() {
        let mut engine = test_engine();
        engine
            .register_template("first", "{name|refer} was modified")
            .unwrap();
        engine
            .register_template("second", "{name|refer} now has new behavior")
            .unwrap();

        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("class".into()));
        ctx.insert("name", Value::String("Foo".into()));

        let r1 = engine.render("first", &ctx).unwrap();
        let r2 = engine.render("second", &ctx).unwrap();

        assert_eq!(r1, "The class Foo was modified.");
        // Second render: pronoun + possibly a discourse connective prepended
        assert!(
            r2.contains("it now has new behavior") || r2.contains("It now has new behavior"),
            "Expected pronoun reference, got: {r2}"
        );
    }

    #[test]
    fn refer_ambiguity_prevents_pronoun() {
        let mut engine = test_engine();
        engine
            .register_template("t", "{name|refer} changed")
            .unwrap();

        // Render with entity A
        let mut ctx_a = Context::new();
        ctx_a.insert("entity_type", Value::String("class".into()));
        ctx_a.insert("name", Value::String("ServiceA".into()));
        engine.render("t", &ctx_a).unwrap();

        // Render with entity B (ambiguity introduced)
        let mut ctx_b = Context::new();
        ctx_b.insert("entity_type", Value::String("class".into()));
        ctx_b.insert("name", Value::String("ServiceB".into()));
        engine.render("t", &ctx_b).unwrap();

        // Back to entity A — ambiguous context, should not use "It"
        let result = engine.render("t", &ctx_a).unwrap();
        // May have a discourse connective prepended, but the key is NO pronoun
        assert!(
            result.contains("ServiceA changed") || result.contains("serviceA changed"),
            "Expected short name (not pronoun), got: {result}"
        );
        assert!(
            !result.contains("It changed") && !result.contains("it changed"),
            "Should not use pronoun with ambiguity, got: {result}"
        );
    }

    #[test]
    fn refer_explicit_entity_type() {
        let mut engine = test_engine();
        engine
            .register_template("t", "{name|refer:method} was called")
            .unwrap();

        let mut ctx = Context::new();
        ctx.insert("name", Value::String("processOrder".into()));

        let result = engine.render("t", &ctx).unwrap();
        assert_eq!(result, "The method processOrder was called.");
    }

    #[test]
    fn refer_reset_reintroduces_full_form() {
        let mut engine = test_engine();
        engine
            .register_template("t", "{name|refer} updated")
            .unwrap();

        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("class".into()));
        ctx.insert("name", Value::String("Foo".into()));

        engine.render("t", &ctx).unwrap();
        engine.reset();

        // After reset, should use full form again
        let result = engine.render("t", &ctx).unwrap();
        assert_eq!(result, "The class Foo updated.");
    }

    #[test]
    fn refer_distant_mention_reintroduces_full() {
        let mut engine = test_engine();
        engine
            .register_template("track", "{name|refer} was tracked")
            .unwrap();
        engine.register_template("other", "Something else happened").unwrap();

        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("class".into()));
        ctx.insert("name", Value::String("Foo".into()));

        let mut other_ctx = Context::new();
        other_ctx.insert("entity_type", Value::String("method".into()));
        other_ctx.insert("name", Value::String("bar".into()));

        // Mention Foo
        engine.render("track", &ctx).unwrap();

        // Three unrelated renders
        engine.render("other", &other_ctx).unwrap();
        engine.render("other", &other_ctx).unwrap();
        engine.render("other", &other_ctx).unwrap();

        // Foo should be re-introduced with full form
        let result = engine.render("track", &ctx).unwrap();
        assert_eq!(result, "The class Foo was tracked.");
    }

    #[test]
    fn refer_no_entity_type_falls_back_to_name() {
        let mut engine = test_engine();
        engine
            .register_template("t", "{name|refer} appeared")
            .unwrap();

        let mut ctx = Context::new();
        // No entity_type provided
        ctx.insert("name", Value::String("something".into()));

        let result = engine.render("t", &ctx).unwrap();
        // Falls back to just the name, with sentence-start capitalization
        // Note: "Something appeared" is 2 words so no period is added
        assert_eq!(result, "Something appeared");
    }
}
