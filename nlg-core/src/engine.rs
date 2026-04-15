use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::session::Session;

use crate::context::{Context, IntoContext, Value};
use crate::discourse::{ListStyle, ReferenceForm};
use crate::error::NlgError;
use crate::language::{Conjunction, Language, Person, VerbForm};
use crate::antonyms::{insert_not, AntonymRegistry};
use crate::hedge::{hedge as hedge_fn, parse_mode as parse_hedge_mode, HedgeMode};
#[cfg(feature = "polish")]
use crate::length::split_long;
#[cfg(feature = "polish")]
use crate::punctuation::smart_quotes;
use crate::quantify::{parse_mode as parse_quantify_mode, quantify as quantify_fn, QuantifyMode};
#[cfg(feature = "reg")]
use crate::reg::{distinguishing_attributes, EntityDescriptor, EntityRegistry};
use crate::salience::{Salience, SalienceThresholds};
use crate::synonyms::SynonymRegistry;
use crate::template::{Pipe, PipeArg, Segment, Template};
#[cfg(feature = "time")]
use crate::time::format_relative;

/// Controls how missing slots are handled during rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Strictness {
    /// Missing slot produces an error.
    #[default]
    Strict,
    /// Missing slot renders as `[missing: slot_name]`.
    Lenient,
    /// Missing slot renders as an empty string.
    Silent,
}

/// Controls how template alternatives are selected.
///
/// `Fixed` and `RoundRobin` are literal: they honour the contract exactly
/// (first alternative every time / strict rotation in registration order).
/// `Seeded` and `Random` additionally layer discourse-aware choose-best
/// scoring on top, so candidates that repeat words from recent output are
/// penalised.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Variation {
    /// Always pick the first registered template, every render.
    #[default]
    Fixed,
    /// Deterministic hash-based selection. Same seed + key = same index.
    /// Further refined by discourse-aware choose-best scoring.
    Seeded(u64),
    /// Cycle through alternatives in registration order, strictly.
    RoundRobin,
    /// Select randomly (non-deterministic). Layered with choose-best scoring.
    Random,
}

/// A template registered under a key, with its salience level.
type SalientTemplate = (Salience, Template);

/// Per-render diagnostics — everything the engine decided along the
/// way to produce the final output. Returned by
/// [`Engine::render_explained`]. Useful for template-author debugging
/// ("why did variant B win?") and for vocab-module linting.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RenderExplanation {
    /// The final rendered string after all post-processing.
    pub output: String,
    /// Template key that was rendered.
    pub template_key: String,
    /// Index (within the salience-filtered alternative set) of the
    /// variant that was emitted.
    pub variant_index: usize,
    /// Source string of the selected variant.
    pub variant_source: String,
    /// Salience bucket used for filtering alternatives.
    pub salience: Salience,
    /// Choose-best scores for each alternative that was considered (in
    /// the same order as the filtered alternative set). `None` when
    /// choose-best wasn't applicable (first render, or Fixed /
    /// RoundRobin variation).
    pub candidate_scores: Option<Vec<f64>>,
    /// Reference form chosen by `{name|refer}` on the primary entity,
    /// if a refer pipe fired.
    pub reference_form: Option<ReferenceForm>,
    /// Discourse connective prepended to the output, if any.
    pub connective: Option<&'static str>,
    /// List style used, if a join pipe fired.
    pub list_style: Option<ListStyle>,
    /// Whether the focus subject was plural (compound) at render time.
    pub focus_is_plural: bool,
    /// Whether the sentence was split by the length-budgeting pass.
    pub length_split_applied: bool,
    /// Whether the silent-mode cleanup stripped any trailing orphan
    /// words from the output.
    pub cleanup_stripped_tail: bool,
}

/// Iterator returned by [`Engine::render_iter`]. Wraps the batch
/// rendering logic so callers can consume the output sentence-by-sentence
/// without waiting for the full batch to complete.
pub struct RenderIter<'a> {
    engine: &'a Engine,
    session: &'a mut Session,
    events: &'a [(&'a str, Context)],
    i: usize,
}

impl<'a> Iterator for RenderIter<'a> {
    type Item = Result<String, NlgError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.i >= self.events.len() {
            return None;
        }

        // Mirror the logic in `render_batch` but emit one sentence per
        // `.next()` call.
        let action_end = self.engine.find_same_action_run(self.events, self.i);
        if action_end > self.i + 1 {
            let key = self.events[self.i].0;
            let run = &self.events[self.i..action_end];
            let sentence = match self.engine.render_aggregated_subjects(self.session, key, run) {
                Ok(s) => s,
                Err(e) => return Some(Err(e)),
            };
            self.i = action_end;
            return Some(Ok(sentence));
        }

        let entity_end = self.engine.find_same_entity_run(self.events, self.i);
        if entity_end > self.i + 1 {
            let mut run_rendered: Vec<String> = Vec::with_capacity(entity_end - self.i);
            for (key, ctx) in &self.events[self.i..entity_end] {
                match self.engine.render(self.session, key, ctx) {
                    Ok(s) => run_rendered.push(s),
                    Err(e) => return Some(Err(e)),
                }
            }
            self.i = entity_end;
            if let Some(reduced) = reduce_same_entity_clauses(&run_rendered) {
                return Some(Ok(reduced));
            }
            // No reduction: emit the first, stash the rest back into the
            // queue by rewinding `i`. Simpler: join with spaces so this
            // single `.next()` still corresponds to the same logical run.
            return Some(Ok(run_rendered.join(" ")));
        }

        let (key, ctx) = &self.events[self.i];
        self.i += 1;
        Some(self.engine.render(self.session, key, ctx))
    }
}

/// Diagnostic output from [`Engine::score_variants`]: one entry per
/// variant that would be considered for the given key and context, with
/// the choose-best score the engine would assign and a flag marking the
/// variant that `render()` would currently emit.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VariantScore {
    /// Index within the salience-filtered alternative set.
    pub index: usize,
    /// Original template source string.
    pub source: String,
    /// What the variant renders to with the given context.
    pub rendered: String,
    /// Choose-best repetition score — lower is better.
    pub score: f64,
    /// Salience bucket this variant was registered at.
    pub salience: Salience,
    /// Whether this variant was the most recently selected for the same
    /// key (anti-repeat will try to avoid it on the next render).
    pub is_last_selected: bool,
    /// Whether `render()` would emit this variant right now.
    pub selected: bool,
}

/// The core NLG engine. Holds a language implementation, template registry,
/// and immutable configuration. All per-render mutable state lives in
/// [`Session`], which callers pass into render methods.
pub struct Engine {
    language: Box<dyn Language>,
    templates: HashMap<String, Vec<SalientTemplate>>,
    strictness: Strictness,
    variation: Variation,
    salience_thresholds: SalienceThresholds,
    /// Per-key initial counters for RoundRobin variation. These are
    /// initialized at `register_template` time and read-only thereafter;
    /// the live counter lives in `Session::round_robin_counters`.
    rr_initial: HashMap<String, usize>,
    #[cfg(feature = "reg")]
    entity_registry: EntityRegistry,
    #[cfg(feature = "reg")]
    reg_preference: Vec<String>,
    synonyms: SynonymRegistry,
    #[cfg(feature = "time")]
    reference_time: Option<i64>,
    antonyms: AntonymRegistry,
    #[cfg(feature = "polish")]
    max_sentence_length: Option<usize>,
    #[cfg(feature = "polish")]
    smart_quotes: bool,
    partials: HashMap<String, Template>,
}

/// Bundle of an immutable engine reference and mutable session state,
/// used internally to thread session through all render helpers without
/// duplicating parameters everywhere.
struct RenderCtx<'e, 's> {
    engine: &'e Engine,
    session: &'s mut Session,
}

impl<'e, 's> RenderCtx<'e, 's> {
    fn new(engine: &'e Engine, session: &'s mut Session) -> Self {
        Self { engine, session }
    }

    /// The body of a render call, performed against live session state.
    /// Callers snapshot state beforehand and restore on error.
    fn render_tx(
        &mut self,
        key: &str,
        all_alternatives: &[SalientTemplate],
        context: &Context,
    ) -> Result<String, NlgError> {
        // Advance discourse state
        self.session.discourse.begin_render();

        // Extract entity info from context for discourse tracking
        let entity_name = context
            .get("name")
            .or_else(|| context.get("old_name"))
            .map(|v| v.as_display());
        let entity_type = context.get("entity_type").map(|v| v.as_display());

        // Detect discourse connective
        let relation = self.session.discourse.detect_relation(key, entity_name.as_deref());
        let connective = self.session.discourse.select_connective(&relation);

        // Filter templates by salience level matching the context magnitude.
        let target_salience = self.engine.context_salience(context);
        let alternatives = filter_by_salience(all_alternatives, target_salience);

        // Select template with choosebest scoring and anti-repeat
        let (template, variant_index) =
            self.select_alternative_scored(key, &alternatives, context)?;

        // Record template choice
        self.session.discourse.record_template_choice(key, variant_index);

        // Render the selected template.
        let mut output = self.render_template(key, template, context)?;

        // Prepend discourse connective if applicable
        if let Some(conn) = connective {
            if conn.starts_with("It ") {
                output = prepend_replacing_subject(&output, conn);
            } else {
                output = format!("{conn} {}", lowercase_first(&output));
            }
        }

        // Capitalize if the template starts with a refer pipe
        if starts_with_refer_pipe(template) {
            output = capitalize_first(&output);
        }

        // Clean up whitespace and silent-mode gaps
        output = cleanup_artifacts(&output, self.engine.strictness);

        // Terminate the sentence
        output = terminate_sentence(&output);

        // Length budget
        #[cfg(feature = "polish")]
        if let Some(max_chars) = self.engine.max_sentence_length {
            output = split_long(&output, max_chars);
        }

        // Typographic polish
        #[cfg(feature = "polish")]
        if self.engine.smart_quotes {
            output = smart_quotes(&output);
        }

        // Record entity mention in discourse state
        if let (Some(name), Some(etype)) = (&entity_name, &entity_type) {
            self.session.discourse.mention_entity(name, etype);
        }

        // Record output words for future repetition scoring
        self.session.discourse.record_output_words(&output);

        Ok(output)
    }

    fn select_alternative_scored<'a>(
        &mut self,
        key: &str,
        alternatives: &'a [Template],
        context: &Context,
    ) -> Result<(&'a Template, usize), NlgError> {
        if alternatives.len() == 1 {
            return Ok((&alternatives[0], 0));
        }

        let allow_choose_best = matches!(
            self.engine.variation,
            Variation::Seeded(_) | Variation::Random
        );

        if !allow_choose_best {
            let index = self.select_variant_index(key, alternatives.len());
            return Ok((&alternatives[index], index));
        }

        let last_variant = self.session.discourse.last_template_variant(key);
        let is_first = self.session.discourse.is_first_render();

        if is_first {
            let index = self.select_variant_index(key, alternatives.len());
            return Ok((&alternatives[index], index));
        }

        // Snapshot-and-restore around candidate rendering so state
        // is untouched by alternatives that aren't emitted.
        let snapshot = self.session.clone();

        let mut candidates: Vec<(usize, String)> = Vec::new();
        for (i, template) in alternatives.iter().enumerate() {
            if Some(i) == last_variant {
                continue;
            }
            let candidate = match self.render_template(key, template, context) {
                Ok(s) => s,
                Err(e) => {
                    *self.session = snapshot;
                    return Err(e);
                }
            };
            candidates.push((i, candidate));
        }

        *self.session = snapshot;

        if candidates.is_empty() {
            let index = last_variant.unwrap_or(0).min(alternatives.len() - 1);
            return Ok((&alternatives[index], index));
        }

        // Score against discourse history (immutable access only)
        let mut best_index = candidates[0].0;
        let mut best_score = f64::MAX;

        for (i, candidate) in &candidates {
            let score = self.session.discourse.repetition_score(candidate);
            if score < best_score {
                best_score = score;
                best_index = *i;
            }
        }

        Ok((&alternatives[best_index], best_index))
    }

    fn select_variant_index(&mut self, key: &str, count: usize) -> usize {
        match self.engine.variation {
            Variation::Fixed => 0,
            Variation::Seeded(seed) => {
                let hash = simple_hash(key, seed);
                hash as usize % count
            }
            Variation::RoundRobin => {
                let counter = self
                    .session
                    .round_robin_counters
                    .entry(key.to_string())
                    .or_insert_with(|| AtomicUsize::new(0))
                    .fetch_add(1, Ordering::Relaxed);
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
        &mut self,
        key: &str,
        template: &Template,
        context: &Context,
    ) -> Result<String, NlgError> {
        self.render_segments(key, &template.segments, context)
    }

    fn render_segments(
        &mut self,
        key: &str,
        segments: &[Segment],
        context: &Context,
    ) -> Result<String, NlgError> {
        let mut output = String::new();

        for segment in segments {
            match segment {
                Segment::Literal(text) => output.push_str(text),
                Segment::Slot {
                    key: slot_key,
                    pipes,
                } => {
                    let rendered = self.render_slot(key, slot_key, pipes, context)?;
                    output.push_str(&rendered);
                }
                Segment::Conditional {
                    condition_key,
                    inner,
                } => {
                    if is_truthy(context.get(condition_key)) {
                        let rendered = self.render_segments(key, inner, context)?;
                        output.push_str(&rendered);
                    }
                }
                Segment::Partial { name } => {
                    // Clone the partial segments to avoid borrow conflicts
                    let partial_segments = self.engine.partials.get(name).ok_or_else(|| {
                        NlgError::TemplateParseError {
                            template: key.to_string(),
                            position: 0,
                            reason: format!(
                                "unknown partial `{name}` — register it with `engine.register_partial`"
                            ),
                        }
                    })?.segments.clone();
                    let rendered = self.render_segments(key, &partial_segments, context)?;
                    output.push_str(&rendered);
                }
            }
        }

        Ok(output)
    }

    fn render_slot(
        &mut self,
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
        match self.engine.strictness {
            Strictness::Strict => Err(NlgError::MissingSlot {
                template: template_key.to_string(),
                slot: slot_key.to_string(),
            }),
            Strictness::Lenient => Ok(format!("[missing: {slot_key}]")),
            Strictness::Silent => Ok(String::new()),
        }
    }

    fn apply_pipe(
        &mut self,
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
            "verb" => self.pipe_verb(pipe, value),
            "syn" => self.pipe_syn(value),
            #[cfg(feature = "time")]
            "relative" => self.pipe_relative(value),
            "quantify" => self.pipe_quantify(pipe, value),
            "demonstrative" => self.pipe_demonstrative(value),
            "hedge" => self.pipe_hedge(pipe, value),
            "negated" => self.pipe_negated(value),
            _ => Err(NlgError::InvalidPipe {
                pipe: pipe.name.clone(),
                reason: "unknown pipe".to_string(),
            }),
        }
    }

    fn pipe_refer(
        &self,
        pipe: &Pipe,
        value: &Value,
        context: &Context,
    ) -> Result<Value, NlgError> {
        let name = value.as_display();

        let entity_type = match &pipe.arg {
            Some(PipeArg::String(t)) => t.clone(),
            _ => context
                .get("entity_type")
                .map(|v| v.as_display())
                .unwrap_or_default(),
        };

        let form = self.session.discourse.reference_form(&name);

        let rendered = match form {
            ReferenceForm::Full => self.engine.render_full_reference(&name, &entity_type),
            ReferenceForm::ShortName => name,
            ReferenceForm::Pronoun => {
                if self.session.discourse.focus_is_plural() {
                    "they".to_string()
                } else {
                    "it".to_string()
                }
            }
        };

        Ok(Value::String(rendered))
    }

    fn pipe_demonstrative(&self, value: &Value) -> Result<Value, NlgError> {
        let noun = value.as_display();
        if noun.is_empty() {
            return Ok(Value::String(noun));
        }

        let determiner = if self.session.discourse.has_prior_render() {
            "this"
        } else {
            "the"
        };

        Ok(Value::String(format!("{determiner} {noun}")))
    }

    fn pipe_syn(&self, value: &Value) -> Result<Value, NlgError> {
        let word = value.as_display();
        let synonyms = match self.engine.synonyms.synonyms_for(&word) {
            Some(s) => s,
            None => return Ok(Value::String(word)),
        };

        if synonyms.is_empty() {
            return Ok(Value::String(word));
        }

        let mut best = &synonyms[0];
        let mut best_score = self.session.discourse.word_frequency(&synonyms[0]);
        for syn in &synonyms[1..] {
            let score = self.session.discourse.word_frequency(syn);
            if score < best_score {
                best_score = score;
                best = syn;
            }
        }

        let result = if word
            .chars()
            .next()
            .map(|c| c.is_uppercase())
            .unwrap_or(false)
        {
            capitalize_first(best)
        } else {
            best.clone()
        };

        Ok(Value::String(result))
    }

    fn pipe_join(&mut self, pipe: &Pipe, value: &Value) -> Result<Value, NlgError> {
        let items = value.as_list().ok_or_else(|| NlgError::InvalidPipe {
            pipe: "join".to_string(),
            reason: "value must be a list".to_string(),
        })?;

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

        let style = forced_style.unwrap_or_else(|| {
            self.session.discourse.next_list_style()
        });

        let refs: Vec<&str> = items.iter().map(|s| s.as_str()).collect();

        let has_truncation = items.last().is_some_and(|last| {
            last.ends_with(" more")
                && last.split_whitespace().next().is_some_and(|w| w.parse::<usize>().is_ok())
        });

        if has_truncation && items.len() >= 2 {
            let shown = &refs[..refs.len() - 1];
            let remainder = &items[items.len() - 1];
            Ok(Value::String(format_truncated_list(
                shown,
                remainder,
                style,
                conjunction,
                &*self.engine.language,
            )))
        } else {
            let joined = self.engine.language.join_list(&refs, conjunction);
            Ok(Value::String(joined))
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

        Ok(Value::String(self.engine.language.pluralize(word, count)))
    }

    fn pipe_article(&self, value: &Value) -> Result<Value, NlgError> {
        let word = value.as_display();
        let article = self.engine.language.article(&word);
        Ok(Value::String(format!("{article} {word}")))
    }

    fn pipe_ordinal(&self, value: &Value) -> Result<Value, NlgError> {
        let n = value.as_number().ok_or_else(|| NlgError::InvalidPipe {
            pipe: "ordinal".to_string(),
            reason: "value must be a number".to_string(),
        })? as usize;

        Ok(Value::String(self.engine.language.ordinal(n)))
    }

    fn pipe_words(&self, value: &Value) -> Result<Value, NlgError> {
        let n = value.as_number().ok_or_else(|| NlgError::InvalidPipe {
            pipe: "words".to_string(),
            reason: "value must be a number".to_string(),
        })? as usize;

        Ok(Value::String(self.engine.language.number_to_words(n)))
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

    fn pipe_negated(&self, value: &Value) -> Result<Value, NlgError> {
        let phrase = value.as_display();
        if let Some(positive) = self.engine.antonyms.lookup(&phrase) {
            return Ok(Value::String(positive.to_string()));
        }
        Ok(Value::String(insert_not(&phrase)))
    }

    fn pipe_hedge(&self, pipe: &Pipe, value: &Value) -> Result<Value, NlgError> {
        let score = value.as_number().ok_or_else(|| NlgError::InvalidPipe {
            pipe: "hedge".to_string(),
            reason: "value must be a 0..=100 integer confidence score".to_string(),
        })?;

        let mode = match &pipe.arg {
            None => HedgeMode::Adverb,
            Some(PipeArg::String(s)) => {
                parse_hedge_mode(s).ok_or_else(|| NlgError::InvalidPipe {
                    pipe: "hedge".to_string(),
                    reason: format!(
                        "unknown hedge mode `{s}` — expected one of adverb, modal, prefix"
                    ),
                })?
            }
            Some(PipeArg::Number(_)) => {
                return Err(NlgError::InvalidPipe {
                    pipe: "hedge".to_string(),
                    reason: "hedge argument must be a mode name, not a number".to_string(),
                });
            }
        };

        Ok(Value::String(hedge_fn(score, mode).to_string()))
    }

    fn pipe_quantify(&self, pipe: &Pipe, value: &Value) -> Result<Value, NlgError> {
        let count = value.as_number().ok_or_else(|| NlgError::InvalidPipe {
            pipe: "quantify".to_string(),
            reason: "value must be a number".to_string(),
        })?;

        let mode = match &pipe.arg {
            None => QuantifyMode::Natural,
            Some(PipeArg::String(s)) => {
                parse_quantify_mode(s).ok_or_else(|| NlgError::InvalidPipe {
                    pipe: "quantify".to_string(),
                    reason: format!(
                        "unknown quantify mode `{s}` — expected one of natural, exact, hedged"
                    ),
                })?
            }
            Some(PipeArg::Number(_)) => {
                return Err(NlgError::InvalidPipe {
                    pipe: "quantify".to_string(),
                    reason: "quantify argument must be a mode name, not a number".to_string(),
                });
            }
        };

        Ok(Value::String(quantify_fn(count, mode, &*self.engine.language)))
    }

    fn pipe_verb(&self, pipe: &Pipe, value: &Value) -> Result<Value, NlgError> {
        let spec = match &pipe.arg {
            Some(PipeArg::String(s)) => s.as_str(),
            _ => {
                return Err(NlgError::InvalidPipe {
                    pipe: "verb".to_string(),
                    reason: "requires a form spec argument, e.g., \
                             {rename|verb:present_perfect}"
                        .to_string(),
                });
            }
        };

        let (form, voice) = VerbForm::parse_spec(spec).ok_or_else(|| NlgError::InvalidPipe {
            pipe: "verb".to_string(),
            reason: format!(
                "unknown verb form spec `{spec}` — expected one of past, present, future, \
                 present_perfect, past_perfect, future_perfect, present_progressive, \
                 past_progressive, conditional, conditional_perfect \
                 (optionally prefixed with `active_` or `passive_`)"
            ),
        })?;

        let verb = value.as_display();
        let phrase = self.engine.language.verb_phrase(&verb, form, voice, Person::Third);
        Ok(Value::String(phrase))
    }

    #[cfg(feature = "time")]
    fn pipe_relative(&self, value: &Value) -> Result<Value, NlgError> {
        let ts = value.as_number().ok_or_else(|| NlgError::InvalidPipe {
            pipe: "relative".to_string(),
            reason: "value must be a Unix-epoch integer (seconds)".to_string(),
        })?;

        let now = match self.engine.reference_time {
            Some(n) => n,
            None => {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs() as i64)
                        .unwrap_or(0)
                }
                #[cfg(target_arch = "wasm32")]
                {
                    return Err(NlgError::InvalidPipe {
                        pipe: "relative".to_string(),
                        reason: "on wasm32 targets the engine needs an \
                                 explicit reference time — call \
                                 `engine.reference_time(unix_secs)` before \
                                 rendering"
                            .to_string(),
                    });
                }
            }
        };

        let diff = now - ts;
        Ok(Value::String(format_relative(diff)))
    }

    /// Score every variant for a key using this session state (for diagnostics).
    fn score_all_variants(
        &mut self,
        key: &str,
        all: &[SalientTemplate],
        ctx: &Context,
    ) -> Result<Vec<VariantScore>, NlgError> {
        let target_salience = self.engine.context_salience(ctx);
        let alternatives = filter_by_salience(all, target_salience);

        // Snapshot so candidate renders leave no residue.
        let snapshot = self.session.clone();

        let last_variant = self.session.discourse.last_template_variant(key);
        let mut scores: Vec<VariantScore> = Vec::with_capacity(alternatives.len());

        for (i, template) in alternatives.iter().enumerate() {
            let candidate = match self.render_template(key, template, ctx) {
                Ok(s) => s,
                Err(e) => {
                    *self.session = snapshot;
                    return Err(e);
                }
            };
            scores.push(VariantScore {
                index: i,
                source: template.source.clone(),
                rendered: candidate,
                score: 0.0,
                salience: target_salience,
                is_last_selected: Some(i) == last_variant,
                selected: false,
            });
        }

        for s in scores.iter_mut() {
            s.score = self.session.discourse.repetition_score(&s.rendered);
        }

        // Determine the selected variant
        let selected_idx = self.pick_variant_index(
            key,
            &alternatives,
            last_variant,
            &scores,
        );
        if let Some(idx) = selected_idx
            && let Some(s) = scores.get_mut(idx)
        {
            s.selected = true;
        }

        *self.session = snapshot;
        Ok(scores)
    }

    fn pick_variant_index(
        &self,
        key: &str,
        alternatives: &[Template],
        last_variant: Option<usize>,
        scores: &[VariantScore],
    ) -> Option<usize> {
        if alternatives.is_empty() {
            return None;
        }
        if alternatives.len() == 1 {
            return Some(0);
        }

        let allow_choose_best = matches!(
            self.engine.variation,
            Variation::Seeded(_) | Variation::Random
        );

        let is_first = self.session.discourse.is_first_render();
        if !allow_choose_best || is_first {
            return Some(self.engine.pick_variant_index_static(key, alternatives.len()));
        }

        let mut best_idx: Option<usize> = None;
        let mut best_score = f64::MAX;
        for (i, s) in scores.iter().enumerate() {
            if Some(i) == last_variant && scores.len() > 1 {
                continue;
            }
            if s.score < best_score {
                best_score = s.score;
                best_idx = Some(i);
            }
        }
        best_idx.or(Some(0))
    }
}

impl Engine {
    /// Create a new engine with the given language implementation.
    pub fn new(language: impl Language + 'static) -> Self {
        Self {
            language: Box::new(language),
            templates: HashMap::new(),
            strictness: Strictness::default(),
            variation: Variation::default(),
            salience_thresholds: SalienceThresholds::default(),
            rr_initial: HashMap::new(),
            #[cfg(feature = "reg")]
            entity_registry: EntityRegistry::new(),
            #[cfg(feature = "reg")]
            reg_preference: Vec::new(),
            synonyms: SynonymRegistry::new(),
            #[cfg(feature = "time")]
            reference_time: None,
            antonyms: AntonymRegistry::new(),
            #[cfg(feature = "polish")]
            max_sentence_length: None,
            #[cfg(feature = "polish")]
            smart_quotes: false,
            partials: HashMap::new(),
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

    /// Set the thresholds for automatic salience derivation from context.
    pub fn salience_thresholds(mut self, thresholds: SalienceThresholds) -> Self {
        self.salience_thresholds = thresholds;
        self
    }

    /// Register an entity descriptor for referring-expression generation
    /// (REG). When the engine produces a *Full form* reference via the
    /// `{name|refer}` pipe, it consults registered entities — if the
    /// target shares its type with other registered entities, the Dale
    /// & Reiter incremental algorithm selects the shortest set of
    /// distinguishing attributes to include as premodifiers.
    ///
    /// Entities not registered here still render with just type + name.
    ///
    /// # Example
    ///
    /// ```
    /// use nlg_core::{Context, Engine, EntityDescriptor, Value};
    /// use nlg_grammar_en::English;
    ///
    /// let mut engine = Engine::new(English::new());
    /// engine.register_entity(
    ///     EntityDescriptor::new("UserService", "class")
    ///         .with_attribute("layer", "domain"),
    /// );
    /// engine.register_entity(
    ///     EntityDescriptor::new("AuthService", "class")
    ///         .with_attribute("layer", "infra"),
    /// );
    ///
    /// engine.register_template("t", "{name|refer} was modified").unwrap();
    /// let mut ctx = Context::new();
    /// ctx.insert("entity_type", Value::String("class".into()));
    /// ctx.insert("name", Value::String("UserService".into()));
    ///
    /// let mut session = nlg_core::Session::new();
    /// assert_eq!(
    ///     engine.render(&mut session, "t", &ctx).unwrap(),
    ///     "The domain class UserService was modified."
    /// );
    /// ```
    #[cfg(feature = "reg")]
    pub fn register_entity(&mut self, descriptor: EntityDescriptor) {
        self.entity_registry.insert(descriptor);
    }

    /// Set the preferred attribute walking order for REG. Attributes
    /// earlier in the list are tried first; unknown attributes are
    /// ignored. Attributes not mentioned here still participate but fall
    /// to the end, in the order they were registered on the target entity.
    ///
    /// Typical use: prefer semantic attributes ("layer", "scope") over
    /// physical ones ("color") when disambiguating code entities.
    #[cfg(feature = "reg")]
    pub fn attribute_preference(mut self, order: Vec<String>) -> Self {
        self.reg_preference = order;
        self
    }

    /// Override the engine's "now" reference for relative-time rendering.
    /// Useful for tests and for rendering a report "as of" a specific
    /// point in time. The value is seconds since Unix epoch.
    ///
    /// If not set, the engine reads `SystemTime::now()` on each call to
    /// the `{timestamp|relative}` pipe.
    ///
    /// # Example
    ///
    /// ```
    /// use nlg_core::{Context, Engine, Value};
    /// use nlg_grammar_en::English;
    ///
    /// let now = 1_700_000_000;
    /// let mut engine = Engine::new(English::new()).reference_time(now);
    /// engine.register_template("t", "The change landed {ts|relative}").unwrap();
    ///
    /// let mut ctx = Context::new();
    /// ctx.insert("ts", Value::Number(now - 86400 - 3600));
    /// let mut session = nlg_core::Session::new();
    /// assert_eq!(engine.render(&mut session, "t", &ctx).unwrap(), "The change landed yesterday.");
    /// ```
    #[cfg(feature = "time")]
    pub fn reference_time(mut self, unix_secs: i64) -> Self {
        self.reference_time = Some(unix_secs);
        self
    }

    /// Register a reusable template fragment under `name`. Inside any
    /// template, `{>name}` expands inline to the partial's content at
    /// render time. Partials share the same template syntax as the rest
    /// of the engine — slots, pipes, conditionals, and nested partials
    /// all work inside a partial.
    ///
    /// Example:
    ///
    /// ```ignore
    /// engine.register_partial(
    ///     "impact_tail",
    ///     "{?consumer_count}, affecting {consumer_count} \
    ///      {consumer_count|pluralize:consumer}{/?}",
    /// )?;
    /// engine.register_template("code.modified", "{name|refer} was modified{>impact_tail}")?;
    /// engine.register_template("code.renamed",  "{name|refer} was renamed to {new_name}{>impact_tail}")?;
    /// ```
    pub fn register_partial(&mut self, name: &str, source: &str) -> Result<(), NlgError> {
        let template = Template::parse(source)?;
        self.partials.insert(name.to_string(), template);
        Ok(())
    }

    /// Enable typographic ("smart") quote substitution on rendered output.
    /// Straight `"` becomes curly `\u{201C}`/`\u{201D}`; straight `'`
    /// becomes `\u{2018}`/`\u{2019}`; apostrophes inside words (Alice's,
    /// it's) become U+2019. Off by default — opt in for human-readable
    /// prose output, leave disabled for code-like outputs.
    ///
    /// # Example
    ///
    /// ```
    /// use nlg_core::{Context, Engine, Value};
    /// use nlg_grammar_en::English;
    ///
    /// let mut engine = Engine::new(English::new()).smart_quotes(true);
    /// engine.register_template("t", r#"Alice said "hello""#).unwrap();
    /// let mut session = nlg_core::Session::new();
    /// let out = engine.render(&mut session, "t", Context::new()).unwrap();
    /// assert!(out.contains('\u{201C}'));
    /// assert!(out.contains('\u{201D}'));
    /// ```
    #[cfg(feature = "polish")]
    pub fn smart_quotes(mut self, enabled: bool) -> Self {
        self.smart_quotes = enabled;
        self
    }

    /// Cap the character length of any single rendered sentence. When a
    /// sentence exceeds `max_chars`, the engine splits it at the latest
    /// natural boundary (subordinate clauses introduced by "which",
    /// "affecting", "impacting", "requiring"; list prefixes like
    /// "including"; em-dashes; explicit sentence breaks) and wraps the
    /// remainder as a follow-up sentence with a light grammatical fix-up.
    ///
    /// If no natural boundary exists inside the budget the sentence
    /// passes through unchanged — we never chop mid-word.
    ///
    /// # Example
    ///
    /// ```
    /// use nlg_core::{Context, Engine};
    /// use nlg_grammar_en::English;
    ///
    /// let mut engine = Engine::new(English::new()).max_sentence_length(60);
    /// engine.register_template(
    ///     "t",
    ///     "The class UserService was renamed to AccountService, \
    ///      which impacts 6 consumers",
    /// ).unwrap();
    ///
    /// let mut session = nlg_core::Session::new();
    /// let out = engine.render(&mut session, "t", Context::new()).unwrap();
    /// assert!(out.contains("This impacts 6 consumers"));
    /// ```
    #[cfg(feature = "polish")]
    pub fn max_sentence_length(mut self, max_chars: usize) -> Self {
        self.max_sentence_length = Some(max_chars);
        self
    }

    /// Register a positive-framing antonym for a negative verb phrase.
    /// The `{phrase|negated}` pipe will prefer the registered positive
    /// form (e.g. "remained unchanged") over the default "not <phrase>"
    /// fallback ("was not modified").
    ///
    /// Matching is case-insensitive.
    pub fn register_antonym(&mut self, negative: &str, positive: &str) {
        self.antonyms.register(negative, positive);
    }

    /// Register a group of synonym words for elegant variation. The
    /// `{word|syn}` pipe will look up the input word in the registered
    /// groups and pick whichever synonym from the group has appeared
    /// least recently in output. Ties break toward registration order.
    ///
    /// Example:
    ///
    /// ```ignore
    /// engine.register_synonyms(&["consumer", "dependent", "caller"]);
    /// // Template: "{count} {consumer|syn}{count|pluralize:}"
    /// // First render emits "consumer(s)"; next "dependent(s)"; next "caller(s)".
    /// ```
    pub fn register_synonyms(&mut self, group: &[&str]) {
        self.synonyms.register_group(group);
    }

    /// Get a reference to the language implementation.
    pub fn language(&self) -> &dyn Language {
        &*self.language
    }

    /// Register a template string under a key with Medium salience.
    /// Multiple templates registered under the same key become alternatives
    /// for variation at that salience level.
    ///
    /// # Example
    ///
    /// ```
    /// use nlg_core::{Context, Engine, Session, Value};
    /// use nlg_grammar_en::English;
    ///
    /// let mut engine = Engine::new(English::new());
    /// engine.register_template(
    ///     "count.items",
    ///     "You have {n} {n|pluralize:item}",
    /// ).unwrap();
    ///
    /// let mut ctx = Context::new();
    /// ctx.insert("n", Value::Number(3));
    /// let mut session = Session::new();
    /// assert_eq!(engine.render(&mut session, "count.items", &ctx).unwrap(), "You have 3 items.");
    /// ```
    pub fn register_template(&mut self, key: &str, source: &str) -> Result<(), NlgError> {
        self.register_template_at(key, source, Salience::Medium)
    }

    /// Register a template at a specific salience level. The engine selects
    /// templates at the salience matching the rendered event's magnitude.
    pub fn register_template_at(
        &mut self,
        key: &str,
        source: &str,
        salience: Salience,
    ) -> Result<(), NlgError> {
        let template = Template::parse(source)?;
        self.templates
            .entry(key.to_string())
            .or_default()
            .push((salience, template));
        // Track that this key exists so new Sessions can be pre-populated
        // with the correct initial counter value.
        self.rr_initial.entry(key.to_string()).or_insert(0);
        Ok(())
    }

    /// Compute the salience for a context using this engine's thresholds.
    pub fn context_salience(&self, ctx: &Context) -> Salience {
        Salience::from_context(ctx, self.salience_thresholds)
    }

    /// Render a registered template with the given context.
    ///
    /// The session tracks discourse state across calls: entity mentions,
    /// template history, word frequency. Each call benefits from context
    /// established by previous calls. Use `session.reset()` between unrelated
    /// sequences.
    ///
    /// Render is transactional: if any step fails (missing slot in Strict mode,
    /// unknown pipe, etc.), the discourse state is rolled back to what it was
    /// before the call. A caller that catches the error sees no residue from
    /// the failed render in subsequent output.
    ///
    /// # Example
    ///
    /// ```
    /// use nlg_core::{Context, Engine, Session, Value, Variation};
    /// use nlg_grammar_en::English;
    ///
    /// let mut engine = Engine::new(English::new()).variation(Variation::Fixed);
    /// engine.register_template("greet", "Hello {name}").unwrap();
    ///
    /// let mut session = Session::new();
    /// let mut ctx = Context::new();
    /// ctx.insert("name", Value::String("world".into()));
    /// assert_eq!(engine.render(&mut session, "greet", &ctx).unwrap(), "Hello world");
    /// ```
    pub fn render(
        &self,
        session: &mut Session,
        key: &str,
        context: impl IntoContext,
    ) -> Result<String, NlgError> {
        let all_alternatives = self
            .templates
            .get(key)
            .ok_or_else(|| NlgError::UnknownTemplate(key.to_string()))?;
        let context = context.into_context();

        let snapshot = session.clone();
        match RenderCtx::new(self, session).render_tx(key, all_alternatives, &context) {
            Ok(output) => Ok(output),
            Err(e) => {
                *session = snapshot;
                Err(e)
            }
        }
    }

    /// Score every registered variant for `key` against `context`, without
    /// committing any render. Returns one [`VariantScore`] per variant
    /// that matches the context's salience bucket, with the
    /// choose-best score the engine would compute and a `selected` flag
    /// marking which one `render()` would currently emit.
    ///
    /// Useful for template-author diagnostics ("why does variant B
    /// always win?") and vocab-module lints. Does not mutate discourse
    /// state — the function snapshots and restores state around candidate
    /// rendering.
    pub fn score_variants(
        &self,
        session: &mut Session,
        key: &str,
        context: impl IntoContext,
    ) -> Result<Vec<VariantScore>, NlgError> {
        let all = self
            .templates
            .get(key)
            .ok_or_else(|| NlgError::UnknownTemplate(key.to_string()))?;

        let ctx = context.into_context();
        // State is always restored by score_all_variants internally.
        RenderCtx::new(self, session).score_all_variants(key, all, &ctx)
    }

    /// Render a one-off template string (not registered) with the given context.
    /// Inline templates do not participate in discourse tracking (no connectives,
    /// no entity tracking) but do record output words for repetition scoring.
    pub fn render_inline(
        &self,
        session: &mut Session,
        source: &str,
        context: impl IntoContext,
    ) -> Result<String, NlgError> {
        let template = Template::parse(source)?;
        let context = context.into_context();
        let output = RenderCtx::new(self, session).render_template("<inline>", &template, &context)?;
        session.discourse.record_output_words(&output);
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
    ///
    /// # Example — clause reduction across same-entity events
    ///
    /// ```
    /// use nlg_core::{Context, Engine, Value};
    /// use nlg_grammar_en::English;
    ///
    /// let mut engine = Engine::new(English::new());
    /// engine.register_template("renamed", "{name|refer} was renamed").unwrap();
    /// engine.register_template("modified", "{name|refer} was modified").unwrap();
    /// engine.register_template("moved", "{name|refer} was moved").unwrap();
    ///
    /// let mut ctx = Context::new();
    /// ctx.insert("entity_type", Value::String("class".into()));
    /// ctx.insert("name", Value::String("UserService".into()));
    /// let events: Vec<(&str, Context)> = vec![
    ///     ("renamed", ctx.clone()),
    ///     ("modified", ctx.clone()),
    ///     ("moved", ctx.clone()),
    /// ];
    ///
    /// let mut session = nlg_core::Session::new();
    /// let out = engine.render_batch(&mut session, &events).unwrap();
    /// assert_eq!(
    ///     out,
    ///     "The class UserService was renamed, modified, and moved."
    /// );
    /// ```
    pub fn render_batch(
        &self,
        session: &mut Session,
        events: &[(&str, Context)],
    ) -> Result<String, NlgError> {
        if events.is_empty() {
            return Ok(String::new());
        }

        let mut sentences: Vec<String> = Vec::new();
        let mut i = 0;

        while i < events.len() {
            // Look for same-action-different-subject aggregation opportunity.
            let action_end = self.find_same_action_run(events, i);

            if action_end > i + 1 {
                // Multiple consecutive events with same template key but
                // different entities — aggregate their subjects.
                let sentence = self.render_aggregated_subjects(
                    session,
                    events[i].0,
                    &events[i..action_end],
                )?;
                sentences.push(sentence);
                i = action_end;
                continue;
            }

            // Look for same-entity-different-action aggregation opportunity.
            // "The class X was renamed. It was modified. It was moved." reduces
            // to "The class X was renamed, modified, and moved" when the voice
            // and tense line up and each predicate is simple.
            let entity_end = self.find_same_entity_run(events, i);
            if entity_end > i + 1 {
                let mut run_rendered: Vec<String> = Vec::with_capacity(entity_end - i);
                for (key, ctx) in &events[i..entity_end] {
                    run_rendered.push(self.render(session, key, ctx)?);
                }

                if let Some(reduced) = reduce_same_entity_clauses(&run_rendered) {
                    sentences.push(reduced);
                } else {
                    sentences.extend(run_rendered);
                }
                i = entity_end;
                continue;
            }

            // Single event — render normally with full discourse benefits.
            let (key, ref ctx) = events[i];
            sentences.push(self.render(session, key, ctx)?);
            i += 1;
        }

        Ok(sentences.join(" "))
    }

    /// Find the end index (exclusive) of a run of consecutive events that
    /// share the same entity (name + entity_type) but potentially differ in
    /// template key. Used by clause-reduction aggregation to turn a series
    /// of same-subject sentences into one conjunction-reduced sentence.
    ///
    /// Returns `start + 1` if no multi-event run exists.
    fn find_same_entity_run(
        &self,
        events: &[(&str, Context)],
        start: usize,
    ) -> usize {
        if start >= events.len() {
            return start;
        }

        let first_ctx = &events[start].1;
        let first_name = match entity_name_from_context(first_ctx) {
            Some(n) => n,
            None => return start + 1,
        };
        let first_type = first_ctx.get("entity_type").map(|v| v.as_display());

        let mut end = start + 1;
        while end < events.len() {
            let ctx = &events[end].1;
            let name = match entity_name_from_context(ctx) {
                Some(n) => n,
                None => break,
            };
            if name != first_name {
                break;
            }
            let ty = ctx.get("entity_type").map(|v| v.as_display());
            if ty != first_type {
                break;
            }
            end += 1;
        }

        end
    }

    /// Render a template and return both the output and a
    /// [`RenderExplanation`] describing the decisions the engine made.
    ///
    /// Functionally equivalent to `render()` — discourse state advances
    /// the same way; any error rolls back the same way — but each step's
    /// diagnostic is also captured. Use this for debugging template
    /// behavior: "why did variant B win?", "was this entity reference a
    /// pronoun or short-name?", "did the length budget split the
    /// output?".
    pub fn render_explained(
        &self,
        session: &mut Session,
        key: &str,
        context: impl IntoContext,
    ) -> Result<RenderExplanation, NlgError> {
        let all_alternatives = self
            .templates
            .get(key)
            .ok_or_else(|| NlgError::UnknownTemplate(key.to_string()))?;

        let context = context.into_context();
        let target_salience = self.context_salience(&context);
        let alternatives = filter_by_salience(all_alternatives, target_salience);

        // Pre-compute candidate scores for diagnostics when choose-best
        // would apply. Run in a snapshot/restore bubble so main session is
        // untouched until the real render below.
        let candidate_scores = {
            let allow_choose_best = matches!(
                self.variation,
                Variation::Seeded(_) | Variation::Random
            );
            let is_first = session.discourse.is_first_render();
            if !allow_choose_best || is_first || alternatives.len() < 2 {
                None
            } else {
                let mut scoring_session = session.clone();
                let snapshot = scoring_session.clone();
                let mut scored: Vec<f64> = Vec::with_capacity(alternatives.len());
                let mut scoring_failed = false;
                for template in &alternatives {
                    match RenderCtx::new(self, &mut scoring_session).render_template(key, template, &context) {
                        Ok(candidate) => {
                            let score = scoring_session.discourse.repetition_score(&candidate);
                            scored.push(score);
                        }
                        Err(_) => {
                            scoring_failed = true;
                            break;
                        }
                    }
                    scoring_session = snapshot.clone();
                }
                if scoring_failed { None } else { Some(scored) }
            }
        };

        // Capture the pre-render ref form for the primary entity (if any).
        let entity_name = context
            .get("name")
            .or_else(|| context.get("old_name"))
            .map(|v| v.as_display());
        let reference_form = entity_name
            .as_ref()
            .map(|n| session.discourse.reference_form(n));

        // Run the real render. Discourse state advances normally.
        let output = self.render(session, key, &context)?;

        // Recover diagnostic info from the (now-advanced) discourse state.
        let variant_index = session
            .discourse
            .last_template_variant(key)
            .unwrap_or(0)
            .min(alternatives.len().saturating_sub(1));
        let variant_source = alternatives
            .get(variant_index)
            .map(|t| t.source.clone())
            .unwrap_or_default();

        let focus_is_plural = session.discourse.focus_is_plural();

        #[cfg(feature = "polish")]
        let length_split_applied = self
            .max_sentence_length
            .is_some_and(|max| output.chars().count() > max && output.contains(". "));
        #[cfg(not(feature = "polish"))]
        let length_split_applied = false;

        let connective = detect_leading_connective(&output);

        Ok(RenderExplanation {
            output,
            template_key: key.to_string(),
            variant_index,
            variant_source,
            salience: target_salience,
            candidate_scores,
            reference_form,
            connective,
            list_style: None,
            focus_is_plural,
            length_split_applied,
            cleanup_stripped_tail: false,
        })
    }

    /// Iterator form of [`Engine::render_batch`]. Yields each sentence
    /// (or aggregated run) as it is produced, so callers concerned with
    /// time-to-first-sentence can stream output instead of waiting for
    /// the full batch. Each `.next()` call produces one sentence —
    /// which may correspond to multiple events (when aggregation or
    /// clause reduction fires) — and returns `None` once the events
    /// are exhausted.
    ///
    /// Errors propagate through the iterator: a failing render yields
    /// `Some(Err(_))` and the iterator remains usable for subsequent
    /// events (though callers should almost always abort on the first
    /// error).
    pub fn render_iter<'a>(
        &'a self,
        session: &'a mut Session,
        events: &'a [(&'a str, Context)],
    ) -> RenderIter<'a> {
        RenderIter {
            engine: self,
            session,
            events,
            i: 0,
        }
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
        session: &mut Session,
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
                sentences.push(self.render(session, k, ctx)?);
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
        let rendered = self.render(session, key, combined_ctx)?;

        // Mark the discourse focus as plural so any subsequent pronoun
        // reference uses "they" instead of "it".
        session.discourse.set_focus_plural(true);

        Ok(pluralize_agreement(&rendered, &*self.language))
    }

    /// Pure stateless variant index selection (no session needed).
    /// Used by RenderCtx::pick_variant_index for the non-scored path.
    fn pick_variant_index_static(&self, key: &str, count: usize) -> usize {
        match self.variation {
            Variation::Fixed => 0,
            Variation::Seeded(seed) => {
                let hash = simple_hash(key, seed);
                hash as usize % count
            }
            Variation::Random => {
                let nanos = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .subsec_nanos() as usize;
                nanos % count
            }
            // RoundRobin requires mutable state — callers that need RoundRobin
            // must go through RenderCtx::select_variant_index instead.
            Variation::RoundRobin => 0,
        }
    }

    /// Build a *Full form* reference. If the entity is in the registry,
    /// run Dale & Reiter REG against registered entities of the same type
    /// and include distinguishing attributes as premodifiers.
    ///
    /// Registry lookup uses `(entity_type, name)` so the same name can
    /// refer to distinct entities of different types. The context-supplied
    /// type is authoritative here — if the render says "type=class", we
    /// never substitute a registered trait of the same name.
    ///
    /// Fallbacks:
    /// - Unregistered entity with known type → "the <type> <name>".
    /// - Unregistered entity without a type  → just the name.
    fn render_full_reference(&self, name: &str, fallback_type: &str) -> String {
        // With the `reg` feature: look up by (type, name) and run
        // Dale & Reiter to pick distinguishing attributes. Without it:
        // degrade gracefully to "the <type> <name>" / just the name.
        #[cfg(feature = "reg")]
        let attrs: Vec<String> = {
            let registered = if fallback_type.is_empty() {
                None
            } else {
                self.entity_registry.get(fallback_type, name)
            };
            let target = match registered {
                Some(d) => d.clone(),
                None => {
                    if fallback_type.is_empty() {
                        return name.to_string();
                    }
                    EntityDescriptor::new(name, fallback_type)
                }
            };
            distinguishing_attributes(&target, &self.entity_registry, &self.reg_preference)
        };

        #[cfg(not(feature = "reg"))]
        let attrs: Vec<String> = {
            if fallback_type.is_empty() {
                return name.to_string();
            }
            Vec::new()
        };

        // The context-supplied type is authoritative. Only fall through to
        // a registered-descriptor type when context provides nothing.
        #[cfg(feature = "reg")]
        let entity_type = if fallback_type.is_empty() {
            self.entity_registry
                .get("", name)
                .map(|d| d.entity_type.clone())
                .unwrap_or_default()
        } else {
            fallback_type.to_string()
        };
        #[cfg(not(feature = "reg"))]
        let entity_type = fallback_type.to_string();

        if entity_type.is_empty() {
            if attrs.is_empty() {
                return name.to_string();
            }
            return format!("the {} {}", attrs.join(" "), name);
        }

        let lower_type = entity_type.to_lowercase();
        if attrs.is_empty() {
            format!("the {lower_type} {name}")
        } else {
            format!("the {} {lower_type} {name}", attrs.join(" "))
        }
    }

    /// Create a new [`Session`] compatible with this engine.
    ///
    /// Sugar for `Session::new()`. Equivalent, but clarifies intent at call
    /// sites where a reader might wonder which session type to construct.
    ///
    /// # Example
    ///
    /// ```
    /// use nlg_core::{Context, Engine, Value};
    /// use nlg_grammar_en::English;
    ///
    /// let mut engine = Engine::new(English::new());
    /// engine.register_template("hello", "Hello {name}!").unwrap();
    ///
    /// let mut session = engine.new_session();
    /// let mut ctx = Context::new();
    /// ctx.insert("name", Value::String("world".into()));
    /// assert_eq!(engine.render(&mut session, "hello", &ctx).unwrap(), "Hello world!");
    /// ```
    pub fn new_session(&self) -> Session {
        Session::new()
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

/// Auxiliary prefixes that signal a simple passive/perfect/progressive
/// verb phrase. Listed longest-first so prefix matching grabs "has been"
/// before "has" and "would have been" before "would have".
const AUX_PREFIXES: &[&str] = &[
    "would have been",
    "will have been",
    "would have",
    "will have",
    "has been",
    "had been",
    "have been",
    "is being",
    "was being",
    "are being",
    "were being",
    "will be",
    "would be",
    "is",
    "are",
    "was",
    "were",
    "has",
    "have",
    "had",
    "will",
];

/// Attempt conjunction reduction across a run of same-entity renders.
///
/// Given a list of rendered sentences all about the same subject, produce
/// a single sentence that shares the subject and auxiliary across all
/// clauses. Example:
///
/// ```text
/// [
///   "The class UserService was renamed to AccountService.",
///   "It was modified.",
///   "It was moved from src/ to lib/.",
/// ]
/// ```
///
/// reduces to:
///
/// ```text
/// "The class UserService was renamed to AccountService, modified, and moved from src/ to lib/."
/// ```
///
/// Returns `None` and leaves the caller to emit the sentences separately
/// when reduction would be lossy: mixed auxiliaries, embedded `which`
/// clauses, connectives that anchor to a previous sentence, or anything
/// the heuristic can't confidently parse.
fn reduce_same_entity_clauses(sentences: &[String]) -> Option<String> {
    if sentences.len() < 2 {
        return None;
    }

    // First sentence: find and keep the subject + aux + first predicate.
    let head = sentences[0].trim_end();
    let head_body = head.trim_end_matches(['.', '!', '?']);
    let (head_subject_aux, head_aux, head_predicate) = split_subject_aux(head_body)?;

    if predicate_has_embedded_clause(head_predicate) {
        return None;
    }

    // Each subsequent sentence must start with "It <aux> " where the aux
    // matches the first sentence, and the remaining predicate must be a
    // simple clause (no embedded "which", no connective prefix spillover).
    let mut predicates: Vec<String> = vec![head_predicate.to_string()];

    for s in &sentences[1..] {
        let trimmed = s.trim_end();
        // Connectives ("Additionally,", "Similarly,", …) get prepended by
        // the discourse system before we know a same-entity run is about
        // to be reduced. Strip them and then try to match the pronoun
        // pattern — the final conjunction ("and") subsumes the
        // connective's linking role.
        let without_conn = strip_leading_connective(trimmed);

        let body = without_conn.trim_end_matches(['.', '!', '?']);

        let (aux, predicate) = strip_it_aux_prefix(body)?;
        if aux != head_aux {
            return None;
        }
        if predicate_has_embedded_clause(predicate) {
            return None;
        }

        predicates.push(predicate.to_string());
    }

    // Join predicates with Oxford comma: "a, b, and c" / "a and b".
    let joined = match predicates.len() {
        0 => return None,
        1 => predicates.into_iter().next().unwrap(),
        2 => format!("{} and {}", predicates[0], predicates[1]),
        _ => {
            let last = predicates.pop().unwrap();
            let head = predicates.join(", ");
            format!("{head}, and {last}")
        }
    };

    Some(format!("{head_subject_aux} {joined}."))
}

/// Detect whether a predicate string would be clumsy to reduce because
/// it carries an embedded subordinate clause or a long list.
fn predicate_has_embedded_clause(predicate: &str) -> bool {
    // ", which ..." is the common case to avoid — reducing across a
    // sentence like "was renamed, which affects 6 consumers" would
    // produce "was renamed, which affects 6 consumers, modified, and moved"
    // which parses as the subordinate clause continuing into the next
    // verb. Same for a handful of other subordinators.
    let lower = predicate.to_lowercase();
    const MARKERS: &[&str] = &[
        ", which",
        ", affecting",
        ", impacting",
        ", requiring",
        ", including",
    ];
    MARKERS.iter().any(|m| lower.contains(m))
}

/// Detect a leading discourse connective, returning its canonical form
/// if present. Used by [`Engine::render_explained`] to report which
/// connective the discourse system prepended.
fn detect_leading_connective(s: &str) -> Option<&'static str> {
    const CONNECTIVES: &[&str] = &[
        "Additionally,",
        "Furthermore,",
        "Similarly,",
        "Likewise,",
        "Meanwhile,",
        "However,",
        "On the other hand,",
        "It also",
    ];
    CONNECTIVES.iter().copied().find(|c| s.starts_with(c))
}

/// Strip a leading discourse connective that the engine may have
/// prepended (e.g. "Additionally, …", "Similarly, …"). Returns the
/// original string when none of the known connectives match.
fn strip_leading_connective(s: &str) -> &str {
    const CONNECTIVES: &[&str] = &[
        "Additionally,",
        "Furthermore,",
        "Similarly,",
        "Likewise,",
        "Meanwhile,",
        "However,",
        "On the other hand,",
        // "It also" replaces the subject rather than prepending a comma —
        // handle it specially so the post-strip text still starts with
        // "it " / "It " for the pronoun match below.
    ];

    for conn in CONNECTIVES {
        if let Some(rest) = s.strip_prefix(conn) {
            return rest.trim_start();
        }
    }

    // "It also was modified" — replace "It also" with "It" so the
    // pronoun+aux matcher can still find its prefix.
    if let Some(rest) = s.strip_prefix("It also ") {
        // Leak a tiny static trick: borrow the tail starting from the
        // "It" position of the original — we build a synthetic view.
        // To keep lifetimes simple, fall through: callers accept that
        // "It also <aux>" is handled by treating the connective as
        // absent and relying on the aux matcher. Return the rest with
        // a synthetic "It " prefix is not possible without alloc, so
        // return the original and let the aux matcher fail gracefully.
        // (Reduction will decline for "It also" forms rather than risk
        // mis-parsing — acceptable as a v1 limitation.)
        let _ = rest;
    }

    s
}


/// Split a head sentence into (full subject + aux prefix, aux word, rest).
/// Returns None when the sentence doesn't follow the "The X Y was …" or
/// "X was …" pattern that reduction expects.
fn split_subject_aux(body: &str) -> Option<(&str, &str, &str)> {
    for aux in AUX_PREFIXES {
        let marker = format!(" {aux} ");
        if let Some(pos) = body.find(&marker) {
            let subject_aux_end = pos + 1 + aux.len(); // include the aux word
            let subject_aux = &body[..subject_aux_end];
            let predicate = body[subject_aux_end..].trim_start();
            return Some((subject_aux, aux, predicate));
        }
    }
    None
}

/// Strip a leading "It <aux> " (or "it <aux> ") prefix and return the
/// aux word along with the remaining predicate. Returns None when the
/// sentence doesn't follow the pronoun-continuation pattern.
fn strip_it_aux_prefix(body: &str) -> Option<(&str, &str)> {
    let rest = body
        .strip_prefix("It ")
        .or_else(|| body.strip_prefix("it "))?;

    for aux in AUX_PREFIXES {
        let marker_with_space = format!("{aux} ");
        if let Some(tail) = rest.strip_prefix(&marker_with_space) {
            return Some((aux, tail.trim_start()));
        }
        // Aux at end of sentence (no tail content) — skip reduction.
        if rest == *aux {
            return None;
        }
    }
    None
}

/// Clean up rendering artifacts caused by omitted slots.
///
/// Two passes:
/// 1. **Always**: collapse runs of internal whitespace into a single space,
///    strip whitespace before common punctuation (`,`, `.`, `!`, `?`, `:`,
///    `;`, `)`, `]`), and trim leading/trailing whitespace. These are safe
///    transformations no matter which strictness mode is active.
/// 2. **Silent-mode only**: strip trailing orphan prepositions and
///    connectives left dangling by missing slots (e.g. `"was modified by "`
///    → `"was modified"`, `"renamed to "` → `"renamed"`). We only do this
///    under `Strictness::Silent` because those gaps are the user's
///    explicit choice to swallow missing slots — the dangling fragments
///    are artifacts of that choice, not of the template's intent.
fn cleanup_artifacts(output: &str, strictness: Strictness) -> String {
    let mut s = collapse_and_tidy(output);

    if strictness == Strictness::Silent {
        s = strip_dangling_tail_words(&s);
    }

    s
}

/// Collapse multi-space runs, strip whitespace before closing punctuation,
/// and trim outer whitespace.
fn collapse_and_tidy(s: &str) -> String {
    // Collapse interior whitespace to single spaces while preserving
    // meaningful content.
    let mut result = String::with_capacity(s.len());
    let mut last_was_space = false;
    let mut started = false;

    for c in s.chars() {
        if c.is_whitespace() {
            if started {
                last_was_space = true;
            }
        } else {
            if last_was_space {
                result.push(' ');
            }
            result.push(c);
            last_was_space = false;
            started = true;
        }
    }

    // Strip space before closing punctuation that might have been
    // introduced by the collapse above (unlikely, but cheap to handle).
    // Iterate through and skip " <punct>" → "<punct>". Implemented as a
    // second pass so the first stays simple.
    let mut tidied = String::with_capacity(result.len());
    let mut chars = result.chars().peekable();
    while let Some(c) = chars.next() {
        if c == ' '
            && let Some(&next) = chars.peek()
            && matches!(next, ',' | '.' | '!' | '?' | ':' | ';' | ')' | ']')
        {
            // drop the space
            continue;
        }
        tidied.push(c);
    }

    tidied
}

/// Words that are almost always followed by an argument — if they're
/// stranded at the very end of an output (optionally before terminal
/// punctuation), the argument must have been swallowed by Silent mode
/// and we strip the orphan.
const ORPHAN_TAIL_WORDS: &[&str] = &[
    // Prepositions taking an object
    "by", "to", "from", "in", "on", "at", "of", "with", "for", "into",
    "onto", "upon", "about", "between", "among", "through", "across",
    // Coordinating & correlative words that need another clause
    "and", "or", "but", "nor", "yet",
    // Subordinating words that need a clause
    "because", "since", "while", "when", "where", "whether", "unless",
    "until", "than",
];

/// Strip trailing words that were left orphaned by omitted slots. Repeats
/// until no more matching tails remain — handles chained gaps like
/// `"modified by in"`.
fn strip_dangling_tail_words(s: &str) -> String {
    let mut current = s.to_string();
    loop {
        // Consider any trailing punctuation separately — we'll preserve it.
        let (body, tail_punct) = split_trailing_punct(&current);
        let trimmed_body = body.trim_end();

        // Grab the last word
        let last_word_start = match trimmed_body.rfind(char::is_whitespace) {
            Some(idx) => idx + 1,
            None => {
                // Single word output — don't touch.
                return current;
            }
        };
        let last_word = &trimmed_body[last_word_start..];
        let last_word_lower = last_word.to_lowercase();

        if ORPHAN_TAIL_WORDS.contains(&last_word_lower.as_str()) {
            // Strip the orphan and any whitespace before it; retain trailing punctuation.
            let new_body = trimmed_body[..last_word_start].trim_end().to_string();
            if new_body.is_empty() {
                // The whole output was orphans — bail out to avoid erasing content.
                return current;
            }
            current = format!("{new_body}{tail_punct}");
            continue;
        }

        return current;
    }
}

/// Split off a run of terminal sentence punctuation so we can preserve it
/// around tail-word stripping. Returns `(body, tail_punct)`.
fn split_trailing_punct(s: &str) -> (&str, &str) {
    let punct_start = s
        .char_indices()
        .rev()
        .take_while(|(_, c)| matches!(c, '.' | '!' | '?' | ','))
        .last()
        .map(|(i, _)| i)
        .unwrap_or(s.len());
    (&s[..punct_start], &s[punct_start..])
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

/// Filter templates to those matching the target salience level.
///
/// Fallback order:
/// 1. Templates registered at the exact target salience.
/// 2. Templates registered at Medium salience (the default).
/// 3. All registered templates (degrades gracefully).
fn filter_by_salience(
    alternatives: &[SalientTemplate],
    target: Salience,
) -> Vec<Template> {
    let exact: Vec<Template> = alternatives
        .iter()
        .filter(|(s, _)| *s == target)
        .map(|(_, t)| t.clone())
        .collect();
    if !exact.is_empty() {
        return exact;
    }

    let medium: Vec<Template> = alternatives
        .iter()
        .filter(|(s, _)| *s == Salience::Medium)
        .map(|(_, t)| t.clone())
        .collect();
    if !medium.is_empty() {
        return medium;
    }

    alternatives.iter().map(|(_, t)| t.clone()).collect()
}

/// Determine if a value is "truthy" for conditional rendering.
/// - `None` → false
/// - Number 0 → false, any other number → true
/// - Empty string → false, non-empty → true
/// - Empty list → false, non-empty → true
fn is_truthy(value: Option<&Value>) -> bool {
    match value {
        None => false,
        Some(Value::Number(n)) => *n != 0,
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::List(items)) => !items.is_empty(),
    }
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
    if let Some(rest) = result.strip_prefix("The ")
        && let Some(space_idx) = rest.find(' ')
    {
        let type_word = &rest[..space_idx];
        // Only pluralize if it's a known simple noun (lowercase word)
        if type_word.chars().all(|c| c.is_lowercase()) && type_word.len() < 15 {
            let plural = lang.pluralize(type_word, 2);
            if plural != type_word {
                result = format!("The {} {}", plural, &rest[space_idx + 1..]);
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
            match (verb, tense) {
                ("be", Tense::Past) => "was".to_string(),
                ("be", Tense::Present) => "is".to_string(),
                ("have", Tense::Present) => "has".to_string(),
                (_, Tense::Past) => format!("{verb}ed"),
                (_, Tense::Present) => verb.to_string(),
                (_, Tense::Future) => format!("will {verb}"),
            }
        }
        fn past_participle(&self, verb: &str) -> String {
            format!("{verb}ed")
        }
        fn present_participle(&self, verb: &str) -> String {
            format!("{verb}ing")
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

    fn test_session() -> Session {
        Session::new()
    }

    // ── Basic rendering (backward compatibility) ─────────────────────────

    #[test]
    fn render_simple_substitution() {
        let mut engine = test_engine();
        engine.register_template("greet", "Hello {name}!").unwrap();

        let mut ctx = Context::new();
        ctx.insert("name", Value::String("world".into()));

        let mut session = test_session();
        assert_eq!(engine.render(&mut session, "greet", &ctx).unwrap(), "Hello world!");
    }

    #[test]
    fn render_missing_slot_strict() {
        let mut engine = test_engine();
        engine.register_template("greet", "Hello {name}!").unwrap();
        let ctx = Context::new();

        let mut session = test_session();
        let result = engine.render(&mut session, "greet", &ctx);
        assert!(matches!(result, Err(NlgError::MissingSlot { .. })));
    }

    #[test]
    fn render_missing_slot_lenient() {
        let mut engine = test_engine().strictness(Strictness::Lenient);
        engine.register_template("greet", "Hello {name}!").unwrap();
        let ctx = Context::new();

        let mut session = test_session();
        assert_eq!(
            engine.render(&mut session, "greet", &ctx).unwrap(),
            "Hello [missing: name]!"
        );
    }

    #[test]
    fn render_missing_slot_silent() {
        let mut engine = test_engine().strictness(Strictness::Silent);
        engine.register_template("greet", "Hello {name}!").unwrap();
        let ctx = Context::new();

        let mut session = test_session();
        // Silent-mode cleanup collapses the " " before "!" produced by
        // the omitted slot.
        assert_eq!(engine.render(&mut session, "greet", &ctx).unwrap(), "Hello!");
    }

    #[test]
    fn render_unknown_template() {
        let engine = test_engine();
        let ctx = Context::new();

        let mut session = test_session();
        let result = engine.render(&mut session, "nonexistent", &ctx);
        assert!(matches!(result, Err(NlgError::UnknownTemplate(_))));
    }

    #[test]
    fn render_pluralize_pipe() {
        let mut engine = test_engine();
        engine
            .register_template("count", "{n} {n|pluralize:item}")
            .unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("n", Value::Number(1));
        assert_eq!(engine.render(&mut session, "count", &ctx).unwrap(), "1 item");

        session.reset();
        ctx.insert("n", Value::Number(5));
        assert_eq!(engine.render(&mut session, "count", &ctx).unwrap(), "5 items");
    }

    #[test]
    fn render_article_pipe() {
        let mut engine = test_engine();
        engine
            .register_template("a", "{thing|article}")
            .unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("thing", Value::String("apple".into()));
        assert_eq!(engine.render(&mut session, "a", &ctx).unwrap(), "an apple");

        session.reset();
        ctx.insert("thing", Value::String("banana".into()));
        assert_eq!(engine.render(&mut session, "a", &ctx).unwrap(), "a banana");
    }

    #[test]
    fn render_join_pipe() {
        let mut engine = test_engine();
        engine
            .register_template("list", "{items|join}")
            .unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert(
            "items",
            Value::List(vec!["a".into(), "b".into(), "c".into()]),
        );
        assert_eq!(engine.render(&mut session, "list", &ctx).unwrap(), "a, b, and c");
    }

    #[test]
    fn render_join_or_pipe() {
        let mut engine = test_engine();
        engine
            .register_template("list", "{items|join:or}")
            .unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert(
            "items",
            Value::List(vec!["a".into(), "b".into(), "c".into()]),
        );
        assert_eq!(engine.render(&mut session, "list", &ctx).unwrap(), "a, b, or c");
    }

    #[test]
    fn render_truncate_then_join_bracketed() {
        let mut engine = test_engine();
        engine
            .register_template("t", "{items|truncate:2|join:bracketed}")
            .unwrap();

        let mut session = test_session();
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
        assert_eq!(engine.render(&mut session, "t", &ctx).unwrap(), "[a, b, and 3 more]");
    }

    #[test]
    fn render_capitalize_pipe() {
        let mut engine = test_engine();
        engine
            .register_template("cap", "{word|capitalize}")
            .unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("word", Value::String("hello".into()));
        assert_eq!(engine.render(&mut session, "cap", &ctx).unwrap(), "Hello");
    }

    #[test]
    fn render_ordinal_pipe() {
        let mut engine = test_engine();
        engine.register_template("o", "{n|ordinal}").unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("n", Value::Number(3));
        assert_eq!(engine.render(&mut session, "o", &ctx).unwrap(), "3rd");
    }

    #[test]
    fn render_inline_template() {
        let engine = test_engine();
        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("name", Value::String("world".into()));

        assert_eq!(
            engine.render_inline(&mut session, "Hello {name}!", &ctx).unwrap(),
            "Hello world!"
        );
    }

    #[test]
    fn variation_fixed_always_picks_first() {
        let mut engine = test_engine().variation(Variation::Fixed);
        engine.register_template("t", "first").unwrap();
        engine.register_template("t", "second").unwrap();

        let mut session = test_session();
        let ctx = Context::new();
        // First render always picks first (no discourse history yet)
        let result = engine.render(&mut session, "t", &ctx).unwrap();
        assert_eq!(result, "first");
    }

    #[test]
    fn variation_seeded_is_deterministic() {
        let mut engine = test_engine().variation(Variation::Seeded(42));
        engine.register_template("t", "first").unwrap();
        engine.register_template("t", "second").unwrap();

        let ctx = Context::new();
        let mut session1 = test_session();
        let result1 = engine.render(&mut session1, "t", &ctx).unwrap();
        let mut session2 = test_session();
        let result2 = engine.render(&mut session2, "t", &ctx).unwrap();
        assert_eq!(result1, result2);
    }

    #[test]
    fn unknown_pipe_is_error() {
        let mut engine = test_engine();
        engine.register_template("t", "{name|nonexistent}").unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("name", Value::String("test".into()));

        let result = engine.render(&mut session, "t", &ctx);
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

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("class".into()));
        ctx.insert("old_name", Value::String("Foo".into()));
        ctx.insert("new_name", Value::String("Foobar".into()));
        ctx.insert("count", Value::Number(6));

        assert_eq!(
            engine.render(&mut session, "entity.renamed", &ctx).unwrap(),
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

        let mut session = test_session();
        engine.render(&mut session, "t", &ctx).unwrap();
        session.reset();

        // After reset, should behave as if first render
        let result = engine.render(&mut session, "t", &ctx).unwrap();
        assert!(result.starts_with("The class Foo"));
    }

    #[test]
    fn template_anti_repeat_with_multiple_variants() {
        // Choose-best scoring only runs under Seeded/Random variation
        // (Fixed/RoundRobin are literal by contract). Anti-repeat is
        // discourse-driven: candidate variants whose words overlap with
        // recent output are scored worse, so consecutive renders tend to
        // pick different variants.
        let mut engine = test_engine().variation(Variation::Seeded(1));
        engine.register_template("t", "alpha distinct tokens").unwrap();
        engine.register_template("t", "beta different tokens").unwrap();
        engine.register_template("t", "gamma unique tokens").unwrap();

        let mut session = test_session();
        let ctx = Context::new();
        let r1 = engine.render(&mut session, "t", &ctx).unwrap();
        let r2 = engine.render(&mut session, "t", &ctx).unwrap();

        // Second render must pick a different variant than the first —
        // choose-best plus explicit last-variant exclusion guarantees it.
        assert_ne!(r1, r2);
    }

    #[test]
    fn list_style_cycles_across_renders() {
        let mut engine = test_engine();
        engine
            .register_template("t", "{items|truncate:1|join}")
            .unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert(
            "items",
            Value::List(vec!["alpha".into(), "beta".into(), "gamma".into()]),
        );

        let r1 = engine.render(&mut session, "t", &ctx).unwrap();
        let r2 = engine.render(&mut session, "t", &ctx).unwrap();
        let r3 = engine.render(&mut session, "t", &ctx).unwrap();
        let r4 = engine.render(&mut session, "t", &ctx).unwrap();

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

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert(
            "items",
            Value::List(vec!["alpha".into(), "beta".into(), "gamma".into()]),
        );

        let result = engine.render(&mut session, "t", &ctx).unwrap();
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

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("class".into()));
        ctx.insert("name", Value::String("UserService".into()));

        let result = engine.render(&mut session, "t", &ctx).unwrap();
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

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("class".into()));
        ctx.insert("name", Value::String("Foo".into()));

        let r1 = engine.render(&mut session, "first", &ctx).unwrap();
        let r2 = engine.render(&mut session, "second", &ctx).unwrap();

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

        let mut session = test_session();

        // Render with entity A
        let mut ctx_a = Context::new();
        ctx_a.insert("entity_type", Value::String("class".into()));
        ctx_a.insert("name", Value::String("ServiceA".into()));
        engine.render(&mut session, "t", &ctx_a).unwrap();

        // Render with entity B (ambiguity introduced)
        let mut ctx_b = Context::new();
        ctx_b.insert("entity_type", Value::String("class".into()));
        ctx_b.insert("name", Value::String("ServiceB".into()));
        engine.render(&mut session, "t", &ctx_b).unwrap();

        // Back to entity A — ambiguous context, should not use "It"
        let result = engine.render(&mut session, "t", &ctx_a).unwrap();
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

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("name", Value::String("processOrder".into()));

        let result = engine.render(&mut session, "t", &ctx).unwrap();
        assert_eq!(result, "The method processOrder was called.");
    }

    #[test]
    fn refer_reset_reintroduces_full_form() {
        let mut engine = test_engine();
        engine
            .register_template("t", "{name|refer} updated")
            .unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("class".into()));
        ctx.insert("name", Value::String("Foo".into()));

        engine.render(&mut session, "t", &ctx).unwrap();
        session.reset();

        // After reset, should use full form again
        let result = engine.render(&mut session, "t", &ctx).unwrap();
        assert_eq!(result, "The class Foo updated.");
    }

    #[test]
    fn refer_distant_mention_reintroduces_full() {
        let mut engine = test_engine();
        engine
            .register_template("track", "{name|refer} was tracked")
            .unwrap();
        engine.register_template("other", "Something else happened").unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("class".into()));
        ctx.insert("name", Value::String("Foo".into()));

        let mut other_ctx = Context::new();
        other_ctx.insert("entity_type", Value::String("method".into()));
        other_ctx.insert("name", Value::String("bar".into()));

        // Mention Foo
        engine.render(&mut session, "track", &ctx).unwrap();

        // Three unrelated renders
        engine.render(&mut session, "other", &other_ctx).unwrap();
        engine.render(&mut session, "other", &other_ctx).unwrap();
        engine.render(&mut session, "other", &other_ctx).unwrap();

        // Foo should be re-introduced with full form
        let result = engine.render(&mut session, "track", &ctx).unwrap();
        assert_eq!(result, "The class Foo was tracked.");
    }

    // ── Explain output ───────────────────────────────────────────────────

    #[test]
    fn explain_reports_variant_index_and_source() {
        let mut engine = test_engine();
        engine.register_template("t", "alpha").unwrap();
        engine.register_template("t", "beta").unwrap();

        let mut session = test_session();
        let exp = engine.render_explained(&mut session, "t", Context::new()).unwrap();
        assert_eq!(exp.template_key, "t");
        assert_eq!(exp.variant_index, 0);
        assert_eq!(exp.variant_source, "alpha");
        assert_eq!(exp.salience, Salience::Medium);
    }

    #[test]
    fn explain_reports_reference_form_when_refer_pipe_fires() {
        let mut engine = test_engine();
        engine
            .register_template("t", "{name|refer} was modified")
            .unwrap();
        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("class".into()));
        ctx.insert("name", Value::String("Foo".into()));

        let mut session = test_session();
        let exp = engine.render_explained(&mut session, "t", &ctx).unwrap();
        // First mention → Full form.
        assert_eq!(exp.reference_form, Some(ReferenceForm::Full));
    }

    #[test]
    fn explain_captures_connective_on_continuation() {
        let mut engine = test_engine();
        engine
            .register_template("t", "The {entity_type} {name} was renamed")
            .unwrap();
        engine
            .register_template("u", "The {entity_type} {name} was modified")
            .unwrap();
        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("class".into()));
        ctx.insert("name", Value::String("Foo".into()));

        let mut session = test_session();
        // Prime.
        engine.render(&mut session, "t", &ctx).unwrap();
        // Same entity, different action → "Additionally," prepended.
        let exp = engine.render_explained(&mut session, "u", &ctx).unwrap();
        assert_eq!(exp.connective, Some("Additionally,"));
    }

    // ── Streaming render iterator ────────────────────────────────────────

    #[test]
    fn render_iter_yields_one_sentence_per_event_when_no_aggregation() {
        let mut engine = test_engine();
        engine
            .register_template("a", "Alpha was seen")
            .unwrap();
        engine
            .register_template("b", "Beta was found")
            .unwrap();

        let mut session = test_session();
        let events: Vec<(&str, Context)> = vec![("a", Context::new()), ("b", Context::new())];
        let results: Vec<_> = engine
            .render_iter(&mut session, &events)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        // Two different templates without shared entities — each yields
        // its own sentence, no aggregation.
        assert_eq!(results.len(), 2);
        assert!(results[0].contains("Alpha"));
        assert!(results[1].contains("Beta"));
    }

    #[test]
    fn render_iter_collapses_same_entity_run_into_one_sentence() {
        let mut engine = test_engine();
        engine
            .register_template("renamed", "{name|refer} was renamed")
            .unwrap();
        engine
            .register_template("modified", "{name|refer} was modified")
            .unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("class".into()));
        ctx.insert("name", Value::String("Foo".into()));
        let events: Vec<(&str, Context)> = vec![
            ("renamed", ctx.clone()),
            ("modified", ctx.clone()),
        ];
        let iter_results: Vec<_> = engine
            .render_iter(&mut session, &events)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        // Expected: exactly one reduced sentence for the same-entity run.
        assert_eq!(iter_results.len(), 1);
        assert!(
            iter_results[0].contains("renamed and modified"),
            "got: {}",
            iter_results[0]
        );
    }

    // ── Score variants harness ───────────────────────────────────────────

    #[test]
    fn score_variants_returns_one_entry_per_alternative() {
        let mut engine = test_engine();
        engine.register_template("t", "alpha").unwrap();
        engine.register_template("t", "beta").unwrap();
        engine.register_template("t", "gamma").unwrap();

        let mut session = test_session();
        let scores = engine.score_variants(&mut session, "t", Context::new()).unwrap();
        assert_eq!(scores.len(), 3);
        let sources: Vec<_> = scores.iter().map(|s| s.source.as_str()).collect();
        assert_eq!(sources, vec!["alpha", "beta", "gamma"]);
    }

    #[test]
    fn score_variants_marks_one_as_selected() {
        let mut engine = test_engine();
        engine.register_template("t", "alpha").unwrap();
        engine.register_template("t", "beta").unwrap();

        let mut session = test_session();
        let scores = engine.score_variants(&mut session, "t", Context::new()).unwrap();
        assert_eq!(scores.iter().filter(|s| s.selected).count(), 1);
    }

    #[test]
    fn score_variants_does_not_mutate_discourse() {
        let mut engine = test_engine();
        engine.register_template("t", "alpha").unwrap();
        engine.register_template("t", "beta").unwrap();

        // Confirm that scoring doesn't advance render_index or any other
        // discourse state — a follow-up render must behave as if the
        // score call never happened.
        let mut session = test_session();
        let _ = engine.score_variants(&mut session, "t", Context::new()).unwrap();
        let r1 = engine.render(&mut session, "t", Context::new()).unwrap();
        // Fresh discourse: expected Fixed variation returns the first
        // variant (index 0). Single-word output, so no sentence-end period.
        assert_eq!(r1, "alpha");
    }

    #[test]
    fn score_variants_unknown_key_errors() {
        let engine = test_engine();
        let mut session = test_session();
        let result = engine.score_variants(&mut session, "never_registered", Context::new());
        assert!(matches!(result, Err(NlgError::UnknownTemplate(_))));
    }

    // ── Template partials ────────────────────────────────────────────────

    #[test]
    fn partial_expands_inline() {
        let mut engine = test_engine();
        engine
            .register_partial(
                "tail",
                ", affecting {count} {count|pluralize:consumer}",
            )
            .unwrap();
        engine
            .register_template("t", "The class Foo was modified{>tail}")
            .unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("count", Value::Number(3));
        assert_eq!(
            engine.render(&mut session, "t", &ctx).unwrap(),
            "The class Foo was modified, affecting 3 consumers."
        );
    }

    #[test]
    fn partial_shared_across_templates() {
        let mut engine = test_engine();
        engine
            .register_partial(
                "tail",
                ", affecting {count} {count|pluralize:consumer}",
            )
            .unwrap();
        engine
            .register_template("modified", "The class {name} was modified{>tail}")
            .unwrap();
        engine
            .register_template("renamed", "The class {name} was renamed{>tail}")
            .unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("name", Value::String("Foo".into()));
        ctx.insert("count", Value::Number(2));

        assert_eq!(
            engine.render(&mut session, "modified", &ctx).unwrap(),
            "The class Foo was modified, affecting 2 consumers."
        );
        assert_eq!(
            engine.render(&mut session, "renamed", &ctx).unwrap(),
            "The class Foo was renamed, affecting 2 consumers."
        );
    }

    #[test]
    fn unknown_partial_errors() {
        let mut engine = test_engine();
        engine
            .register_template("t", "Hello{>missing_partial}")
            .unwrap();
        let mut session = test_session();
        let result = engine.render(&mut session, "t", Context::new());
        assert!(matches!(
            result,
            Err(NlgError::TemplateParseError { .. })
        ));
    }

    // ── Sentence-length budget ───────────────────────────────────────────

    #[test]
    fn length_budget_splits_long_sentence_at_which() {
        let mut engine = test_engine().max_sentence_length(50);
        engine
            .register_template(
                "t",
                "The class UserService was renamed to AccountService, \
                 which impacts 6 consumers",
            )
            .unwrap();

        let mut session = test_session();
        let out = engine.render(&mut session, "t", Context::new()).unwrap();
        assert!(
            out.contains("This impacts 6 consumers"),
            "got: {out}"
        );
        assert!(out.contains(". "), "expected a sentence break, got: {out}");
    }

    #[test]
    fn length_budget_does_nothing_when_sentence_fits() {
        let mut engine = test_engine().max_sentence_length(200);
        engine
            .register_template("t", "The class Foo was modified")
            .unwrap();

        let mut session = test_session();
        let out = engine.render(&mut session, "t", Context::new()).unwrap();
        assert_eq!(out, "The class Foo was modified.");
    }

    // ── Negation pipe ────────────────────────────────────────────────────

    #[test]
    fn negated_pipe_uses_registered_antonym() {
        let mut engine = test_engine();
        engine.register_antonym("was modified", "remained unchanged");
        engine.register_template("t", "The class Foo {p|negated}").unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("p", Value::String("was modified".into()));
        assert_eq!(
            engine.render(&mut session, "t", &ctx).unwrap(),
            "The class Foo remained unchanged."
        );
    }

    #[test]
    fn negated_pipe_inserts_not_when_no_antonym() {
        let mut engine = test_engine();
        engine.register_template("t", "The class Foo {p|negated}").unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("p", Value::String("was modified".into()));
        assert_eq!(
            engine.render(&mut session, "t", &ctx).unwrap(),
            "The class Foo was not modified."
        );
    }

    #[test]
    fn negated_pipe_handles_perfect_aux() {
        let mut engine = test_engine();
        engine.register_template("t", "The class Foo {p|negated}").unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("p", Value::String("has been renamed".into()));
        assert_eq!(
            engine.render(&mut session, "t", &ctx).unwrap(),
            "The class Foo has not been renamed."
        );
    }

    // ── Hedge pipe ───────────────────────────────────────────────────────

    #[test]
    fn hedge_pipe_default_adverb() {
        let mut engine = test_engine();
        engine
            .register_template("t", "The change {conf|hedge} broke the build")
            .unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("conf", Value::Number(60));
        assert_eq!(
            engine.render(&mut session, "t", &ctx).unwrap(),
            "The change probably broke the build."
        );
    }

    #[test]
    fn hedge_pipe_modal_mode() {
        let mut engine = test_engine();
        engine
            .register_template("t", "The change {conf|hedge:modal} break things")
            .unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("conf", Value::Number(40));
        assert_eq!(
            engine.render(&mut session, "t", &ctx).unwrap(),
            "The change might break things."
        );
    }

    #[test]
    fn hedge_pipe_rejects_unknown_mode() {
        let mut engine = test_engine();
        engine.register_template("t", "{c|hedge:bogus}").unwrap();
        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("c", Value::Number(60));
        assert!(matches!(
            engine.render(&mut session, "t", &ctx),
            Err(NlgError::InvalidPipe { .. })
        ));
    }

    // ── Anaphora: plural pronouns & demonstratives ───────────────────────

    #[test]
    fn demonstrative_uses_the_on_first_render() {
        let mut engine = test_engine();
        engine
            .register_template("t", "{noun|demonstrative}")
            .unwrap();
        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("noun", Value::String("change".into()));
        // Lowercase — demonstrative never capitalizes; callers that want
        // the demonstrative at a sentence start combine it with a
        // leading template word that already capitalizes, or with the
        // engine's refer-pipe capitalization path.
        assert_eq!(engine.render(&mut session, "t", &ctx).unwrap(), "the change");
    }

    #[test]
    fn demonstrative_uses_this_on_continuation() {
        let mut engine = test_engine();
        engine.register_template("prime", "setup").unwrap();
        engine
            .register_template("t", "{noun|demonstrative}")
            .unwrap();

        let mut session = test_session();
        // Prior render establishes discourse.
        engine.render(&mut session, "prime", Context::new()).unwrap();

        let mut ctx = Context::new();
        ctx.insert("noun", Value::String("change".into()));
        let result = engine.render(&mut session, "t", &ctx).unwrap();
        // Mid-sentence capitalization isn't applied (template doesn't
        // start with refer), so the value comes out lowercase.
        assert_eq!(result, "this change");
    }

    #[test]
    fn demonstrative_resets_to_the_after_reset() {
        let mut engine = test_engine();
        engine.register_template("prime", "setup").unwrap();
        engine
            .register_template("t", "{noun|demonstrative}")
            .unwrap();

        let mut session = test_session();
        engine.render(&mut session, "prime", Context::new()).unwrap();
        session.reset();

        let mut ctx = Context::new();
        ctx.insert("noun", Value::String("change".into()));
        assert_eq!(engine.render(&mut session, "t", &ctx).unwrap(), "the change");
    }

    // ── Quantify pipe ────────────────────────────────────────────────────

    #[test]
    fn quantify_pipe_natural_defaults() {
        // Uses the crate's own TestLang for number_to_words via
        // "<N>"-style stub. We still exercise the small-number spelling
        // path via QuantifyMode::Natural's language callback.
        let mut engine = test_engine();
        engine.register_template("t", "{n|quantify} consumer").unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("n", Value::Number(0));
        assert_eq!(engine.render(&mut session, "t", &ctx).unwrap(), "no consumer");
        session.reset();

        ctx.insert("n", Value::Number(1));
        assert_eq!(engine.render(&mut session, "t", &ctx).unwrap(), "a single consumer");
        session.reset();

        ctx.insert("n", Value::Number(300));
        assert_eq!(engine.render(&mut session, "t", &ctx).unwrap(), "hundreds of consumer");
    }

    #[test]
    fn quantify_pipe_exact_mode() {
        let mut engine = test_engine();
        engine
            .register_template("t", "{n|quantify:exact} callers")
            .unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("n", Value::Number(47));
        assert_eq!(engine.render(&mut session, "t", &ctx).unwrap(), "47 callers");
    }

    #[test]
    fn quantify_pipe_hedged_mode() {
        let mut engine = test_engine();
        engine
            .register_template("t", "{n|quantify:hedged} dependents")
            .unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("n", Value::Number(4));
        assert_eq!(engine.render(&mut session, "t", &ctx).unwrap(), "a few dependents");
    }

    #[test]
    fn quantify_pipe_rejects_unknown_mode() {
        let mut engine = test_engine();
        engine.register_template("t", "{n|quantify:bogus}").unwrap();
        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("n", Value::Number(5));
        assert!(matches!(
            engine.render(&mut session, "t", &ctx),
            Err(NlgError::InvalidPipe { .. })
        ));
    }

    // ── Relative time pipe ───────────────────────────────────────────────

    #[test]
    fn relative_pipe_renders_past_phrases() {
        // Fix the reference time so tests are deterministic.
        let now: i64 = 1_700_000_000;
        let mut engine = test_engine().reference_time(now);
        engine.register_template("t", "{ts|relative}").unwrap();

        let cases = [
            (now, "just now"),
            (now - 60, "1 minute ago"),
            (now - 3600, "an hour ago"),
            (now - 86400 - 3600, "yesterday"),
            (now - 3 * 86400, "3 days ago"),
            (now - 10 * 86400, "last week"),
            (now - 3 * 30 * 86400, "3 months ago"),
            (now - 2 * 365 * 86400, "2 years ago"),
        ];

        for (ts, expected) in cases {
            let mut session = test_session();
            let mut ctx = Context::new();
            ctx.insert("ts", Value::Number(ts));
            let rendered = engine.render(&mut session, "t", &ctx).unwrap();
            assert_eq!(rendered, expected, "for ts={ts}");
        }
    }

    #[test]
    fn relative_pipe_renders_future_phrases() {
        let now: i64 = 1_700_000_000;
        let mut engine = test_engine().reference_time(now);
        engine.register_template("t", "{ts|relative}").unwrap();

        let cases = [
            (now + 3600, "in an hour"),
            (now + 86400 + 3600, "tomorrow"),
            (now + 3 * 86400, "in 3 days"),
            (now + 10 * 86400, "next week"),
        ];

        for (ts, expected) in cases {
            let mut session = test_session();
            let mut ctx = Context::new();
            ctx.insert("ts", Value::Number(ts));
            let rendered = engine.render(&mut session, "t", &ctx).unwrap();
            assert_eq!(rendered, expected, "for ts={ts}");
        }
    }

    #[test]
    fn relative_pipe_rejects_non_numeric() {
        let mut engine = test_engine().reference_time(1_700_000_000);
        engine.register_template("t", "{x|relative}").unwrap();
        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("x", Value::String("not a number".into()));
        let result = engine.render(&mut session, "t", &ctx);
        assert!(matches!(result, Err(NlgError::InvalidPipe { .. })));
    }

    // ── Synonym pipe (elegant variation) ─────────────────────────────────

    #[test]
    fn syn_pipe_passes_through_unregistered_words() {
        let mut engine = test_engine();
        engine.register_template("t", "{word|syn}").unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("word", Value::String("unregistered".into()));
        assert_eq!(engine.render(&mut session, "t", &ctx).unwrap(), "unregistered");
    }

    #[test]
    fn syn_pipe_rotates_across_renders() {
        let mut engine = test_engine();
        engine.register_synonyms(&["class", "type", "kind"]);
        engine
            .register_template("t", "the {word|syn} was seen")
            .unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("word", Value::String("class".into()));

        let r1 = engine.render(&mut session, "t", &ctx).unwrap();
        let r2 = engine.render(&mut session, "t", &ctx).unwrap();
        let r3 = engine.render(&mut session, "t", &ctx).unwrap();

        // All three synonyms should appear across three renders —
        // least-recently-used scoring rotates them.
        let combined = format!("{r1} | {r2} | {r3}");
        assert!(combined.contains("class"), "got: {combined}");
        assert!(combined.contains("type"), "got: {combined}");
        assert!(combined.contains("kind"), "got: {combined}");
    }

    #[test]
    fn syn_pipe_preserves_capitalization() {
        let mut engine = test_engine();
        engine.register_synonyms(&["class", "type"]);
        engine.register_template("t", "{word|syn}").unwrap();

        let mut session = test_session();
        // Uppercase input → uppercase output synonym.
        let mut ctx = Context::new();
        ctx.insert("word", Value::String("Class".into()));
        let first = engine.render(&mut session, "t", &ctx).unwrap();
        assert!(
            first.chars().next().unwrap().is_uppercase(),
            "expected capitalized output, got: {first}"
        );
    }

    #[test]
    fn syn_pipe_deterministic_tie_break_first_registered_wins() {
        let mut engine = test_engine();
        engine.register_synonyms(&["alpha", "beta", "gamma"]);
        engine.register_template("t", "{word|syn}").unwrap();

        let mut session = test_session();
        // First render, no history → all tied at frequency 0. The
        // first-registered entry wins.
        let mut ctx = Context::new();
        ctx.insert("word", Value::String("alpha".into()));
        assert_eq!(engine.render(&mut session, "t", &ctx).unwrap(), "alpha");
    }

    // ── Clause aggregation / conjunction reduction ───────────────────────

    #[test]
    fn reduce_merges_three_simple_same_entity_passives() {
        let reduced = reduce_same_entity_clauses(&[
            "The class UserService was renamed to AccountService.".to_string(),
            "It was modified.".to_string(),
            "It was moved from src/ to lib/.".to_string(),
        ]);
        assert_eq!(
            reduced.as_deref(),
            Some(
                "The class UserService was renamed to AccountService, \
                 modified, and moved from src/ to lib/."
            )
        );
    }

    #[test]
    fn reduce_two_clauses_uses_and_without_oxford_comma() {
        let reduced = reduce_same_entity_clauses(&[
            "The class Foo was renamed.".to_string(),
            "It was modified.".to_string(),
        ]);
        assert_eq!(
            reduced.as_deref(),
            Some("The class Foo was renamed and modified.")
        );
    }

    #[test]
    fn reduce_rejects_mixed_auxiliaries() {
        // First sentence uses "was" (simple past passive), second uses
        // "has been" (present perfect passive). Merging would produce an
        // ungrammatical "was renamed and been modified".
        let reduced = reduce_same_entity_clauses(&[
            "The class Foo was renamed.".to_string(),
            "It has been modified.".to_string(),
        ]);
        assert!(reduced.is_none());
    }

    #[test]
    fn reduce_rejects_embedded_which_clauses() {
        // An embedded subordinate clause would absorb the following
        // predicate into its own scope if we fused.
        let reduced = reduce_same_entity_clauses(&[
            "The class Foo was renamed, which impacts 6 consumers.".to_string(),
            "It was modified.".to_string(),
        ]);
        assert!(reduced.is_none());
    }

    #[test]
    fn reduce_strips_connectives_and_merges() {
        // When the engine's discourse system has prepended "Additionally,"
        // / "Similarly," / etc. to a follow-up same-entity render, the
        // reducer strips those connectives — the final conjunction
        // ("and") linking the predicates subsumes their linking role.
        let reduced = reduce_same_entity_clauses(&[
            "The class Foo was renamed.".to_string(),
            "Additionally, it was modified.".to_string(),
            "Furthermore, it was moved.".to_string(),
        ]);
        assert_eq!(
            reduced.as_deref(),
            Some("The class Foo was renamed, modified, and moved.")
        );
    }

    #[test]
    fn reduce_rejects_single_sentence() {
        let reduced = reduce_same_entity_clauses(&[
            "The class Foo was renamed.".to_string(),
        ]);
        assert!(reduced.is_none());
    }

    #[test]
    fn reduce_rejects_when_continuation_has_no_pronoun() {
        // If a follow-up doesn't start with "It " the entity is being
        // re-introduced; keep them separate.
        let reduced = reduce_same_entity_clauses(&[
            "The class Foo was renamed.".to_string(),
            "The class Foo was modified.".to_string(),
        ]);
        assert!(reduced.is_none());
    }

    #[test]
    fn reduce_handles_has_been_perfect_passive() {
        let reduced = reduce_same_entity_clauses(&[
            "The class Foo has been renamed.".to_string(),
            "It has been modified.".to_string(),
            "It has been moved.".to_string(),
        ]);
        assert_eq!(
            reduced.as_deref(),
            Some("The class Foo has been renamed, modified, and moved.")
        );
    }

    // ── Silent-mode cleanup ─────────────────────────────────────────────

    #[test]
    fn silent_strips_trailing_dangling_preposition() {
        let mut engine = test_engine().strictness(Strictness::Silent);
        engine
            .register_template("t", "The file was modified by {author}")
            .unwrap();
        let mut session = test_session();
        let ctx = Context::new();
        assert_eq!(
            engine.render(&mut session, "t", &ctx).unwrap(),
            "The file was modified."
        );
    }

    #[test]
    fn silent_strips_dangling_preposition_with_punct() {
        let mut engine = test_engine().strictness(Strictness::Silent);
        engine
            .register_template("t", "The class was renamed to {new_name}.")
            .unwrap();
        let mut session = test_session();
        let ctx = Context::new();
        assert_eq!(
            engine.render(&mut session, "t", &ctx).unwrap(),
            "The class was renamed."
        );
    }

    #[test]
    fn silent_strips_orphan_conjunction() {
        let mut engine = test_engine().strictness(Strictness::Silent);
        engine
            .register_template("t", "The module exports {a} and {b}")
            .unwrap();
        let mut session = test_session();
        let ctx = Context::new();
        assert_eq!(
            engine.render(&mut session, "t", &ctx).unwrap(),
            "The module exports."
        );
    }

    #[test]
    fn silent_strips_chained_orphans() {
        let mut engine = test_engine().strictness(Strictness::Silent);
        engine
            .register_template("t", "The job was scheduled by {a} at {b}")
            .unwrap();
        let mut session = test_session();
        let ctx = Context::new();
        // Strips "at" then "by" in sequence.
        assert_eq!(
            engine.render(&mut session, "t", &ctx).unwrap(),
            "The job was scheduled."
        );
    }

    #[test]
    fn silent_preserves_content_when_orphans_would_empty_output() {
        let mut engine = test_engine().strictness(Strictness::Silent);
        // A template that's only a preposition + slot — nothing to keep.
        engine.register_template("t", "by {author}").unwrap();
        let mut session = test_session();
        let ctx = Context::new();
        // We refuse to empty the output; the orphan stays.
        assert_eq!(engine.render(&mut session, "t", &ctx).unwrap(), "by");
    }

    #[test]
    fn whitespace_collapsing_runs_regardless_of_strictness() {
        // Even in Strict mode, internal whitespace runs get normalized
        // (a safe transformation that doesn't depend on missing slots).
        let mut engine = test_engine();
        engine
            .register_template("t", "The  quick   brown fox")
            .unwrap();
        let mut session = test_session();
        let ctx = Context::new();
        assert_eq!(
            engine.render(&mut session, "t", &ctx).unwrap(),
            "The quick brown fox."
        );
    }

    #[test]
    fn strict_mode_unaffected_by_cleanup_tail_stripping() {
        // In Strict mode, a missing slot is an error, not an artifact,
        // so the dangling-preposition strip never runs.
        let mut engine = test_engine();
        engine
            .register_template("t", "modified by {author}")
            .unwrap();
        let mut session = test_session();
        let ctx = Context::new();
        assert!(engine.render(&mut session, "t", &ctx).is_err());
    }

    // ── Referring Expression Generation (Dale & Reiter) ─────────────────

    #[test]
    fn reg_with_no_registry_behaves_as_before() {
        let mut engine = test_engine();
        engine
            .register_template("t", "{name|refer} was modified")
            .unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("class".into()));
        ctx.insert("name", Value::String("UserService".into()));

        let result = engine.render(&mut session, "t", &ctx).unwrap();
        assert_eq!(result, "The class UserService was modified.");
    }

    #[test]
    fn reg_adds_distinguisher_when_same_type_registered() {
        let mut engine = test_engine();
        engine.register_entity(
            crate::EntityDescriptor::new("UserService", "class")
                .with_attribute("layer", "domain"),
        );
        engine.register_entity(
            crate::EntityDescriptor::new("AuthService", "class")
                .with_attribute("layer", "infra"),
        );
        engine
            .register_template("t", "{name|refer} was modified")
            .unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("class".into()));
        ctx.insert("name", Value::String("UserService".into()));

        let result = engine.render(&mut session, "t", &ctx).unwrap();
        assert_eq!(result, "The domain class UserService was modified.");
    }

    #[test]
    fn reg_no_distinguisher_needed_when_types_differ() {
        let mut engine = test_engine();
        engine.register_entity(
            crate::EntityDescriptor::new("UserService", "class")
                .with_attribute("layer", "domain"),
        );
        engine.register_entity(
            crate::EntityDescriptor::new("UserModule", "module")
                .with_attribute("layer", "infra"),
        );
        engine
            .register_template("t", "{name|refer} was modified")
            .unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("class".into()));
        ctx.insert("name", Value::String("UserService".into()));

        let result = engine.render(&mut session, "t", &ctx).unwrap();
        // Different types → head noun alone disambiguates, no attribute added.
        assert_eq!(result, "The class UserService was modified.");
    }

    #[test]
    fn reg_preference_order_steers_attribute_choice() {
        let mut engine = test_engine().attribute_preference(vec!["size".to_string()]);
        engine.register_entity(
            crate::EntityDescriptor::new("Foo", "widget")
                .with_attribute("color", "red")
                .with_attribute("size", "small"),
        );
        engine.register_entity(
            crate::EntityDescriptor::new("Bar", "widget")
                .with_attribute("color", "blue")
                .with_attribute("size", "large"),
        );
        engine
            .register_template("t", "{name|refer} appeared")
            .unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("widget".into()));
        ctx.insert("name", Value::String("Foo".into()));

        // Preference says size first; size alone disambiguates.
        let result = engine.render(&mut session, "t", &ctx).unwrap();
        assert_eq!(result, "The small widget Foo appeared.");
    }

    /// Regression: two entities sharing a name but not a type must stay
    /// independent in the registry. Rendering one must not substitute the
    /// other's type.
    #[test]
    fn reg_same_name_different_type_does_not_cross_contaminate() {
        let mut engine = test_engine();
        engine.register_entity(
            crate::EntityDescriptor::new("UserService", "class")
                .with_attribute("layer", "domain"),
        );
        engine.register_entity(
            crate::EntityDescriptor::new("UserService", "trait")
                .with_attribute("scope", "public"),
        );

        engine
            .register_template("t", "{name|refer} was modified")
            .unwrap();

        let mut session = test_session();
        let mut ctx_class = Context::new();
        ctx_class.insert("entity_type", Value::String("class".into()));
        ctx_class.insert("name", Value::String("UserService".into()));

        // Context says class → must render as class, never as trait.
        let r = engine.render(&mut session, "t", &ctx_class).unwrap();
        assert!(r.contains("class UserService"), "got: {r}");
        assert!(!r.contains("trait"), "got: {r}");
        // With only one class-typed UserService registered (the trait is
        // a different type), no distinguishing attribute is needed.
        assert_eq!(r, "The class UserService was modified.");

        let mut session2 = test_session();
        let mut ctx_trait = Context::new();
        ctx_trait.insert("entity_type", Value::String("trait".into()));
        ctx_trait.insert("name", Value::String("UserService".into()));
        let r2 = engine.render(&mut session2, "t", &ctx_trait).unwrap();
        assert_eq!(r2, "The trait UserService was modified.");
    }

    #[test]
    fn reg_multiple_attributes_needed() {
        let mut engine = test_engine();
        engine.register_entity(
            crate::EntityDescriptor::new("A", "widget")
                .with_attribute("color", "red")
                .with_attribute("size", "small"),
        );
        engine.register_entity(
            crate::EntityDescriptor::new("B", "widget")
                .with_attribute("color", "red")
                .with_attribute("size", "large"),
        );
        engine.register_entity(
            crate::EntityDescriptor::new("C", "widget")
                .with_attribute("color", "blue")
                .with_attribute("size", "small"),
        );
        engine = engine.attribute_preference(vec!["color".to_string(), "size".to_string()]);
        engine
            .register_template("t", "{name|refer} appeared")
            .unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("widget".into()));
        ctx.insert("name", Value::String("A".into()));

        // color rules out C; size then rules out B.
        let result = engine.render(&mut session, "t", &ctx).unwrap();
        assert_eq!(result, "The red small widget A appeared.");
    }

    #[test]
    fn refer_no_entity_type_falls_back_to_name() {
        let mut engine = test_engine();
        engine
            .register_template("t", "{name|refer} appeared")
            .unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        // No entity_type provided
        ctx.insert("name", Value::String("something".into()));

        let result = engine.render(&mut session, "t", &ctx).unwrap();
        // Falls back to just the name, with sentence-start capitalization
        // Note: "Something appeared" is 2 words so no period is added
        assert_eq!(result, "Something appeared");
    }

    // ── Regression tests for codex review findings ───────────────────────

    /// Failed renders must not leave traces in discourse state.
    #[test]
    fn failed_render_does_not_mutate_discourse() {
        let mut engine = test_engine();
        engine
            .register_template("ok", "{name|refer} was updated")
            .unwrap();
        engine
            .register_template("bad", "{missing_slot} fails here")
            .unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("class".into()));
        ctx.insert("name", Value::String("Foo".into()));

        // Successful render — Foo is now known, render index is 1.
        let r1 = engine.render(&mut session, "ok", &ctx).unwrap();
        assert!(r1.contains("class Foo"), "r1 = {r1}");

        // Attempt a failing render. The discourse state must NOT advance.
        let bad_ctx = Context::new();
        assert!(engine.render(&mut session, "bad", &bad_ctx).is_err());

        // Next successful render should behave as if the failure never
        // happened: Foo is still the focus entity at distance 1, so
        // the pronoun form fires.
        let r2 = engine.render(&mut session, "ok", &ctx).unwrap();
        assert!(
            r2.contains("it") || r2.contains("It"),
            "Expected pronoun reference after failed render was rolled back, got: {r2}"
        );
    }

    /// Regression: a failed render under RoundRobin must not advance the
    /// rotation counter. The next successful render must pick up exactly
    /// where the last successful one left off.
    #[test]
    fn round_robin_counter_is_transactional_on_failure() {
        let mut engine = test_engine().variation(Variation::RoundRobin);
        engine.register_template("ok", "alpha {name}").unwrap();
        engine.register_template("ok", "beta {name}").unwrap();
        engine.register_template("ok", "gamma {name}").unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("name", Value::String("x".into()));
        let empty = Context::new();

        // First successful render: alpha
        assert!(engine.render(&mut session, "ok", &ctx).unwrap().contains("alpha"));

        // A failed render between the two should NOT advance the counter
        // for "ok" — the missing slot aborts before commit.
        assert!(engine.render(&mut session, "ok", &empty).is_err());

        // Next successful render must be beta, not gamma.
        assert!(engine.render(&mut session, "ok", &ctx).unwrap().contains("beta"));
    }

    /// RoundRobin must rotate through every alternative in order.
    #[test]
    fn round_robin_actually_rotates() {
        let mut engine = test_engine().variation(Variation::RoundRobin);
        engine.register_template("t", "alpha").unwrap();
        engine.register_template("t", "beta").unwrap();
        engine.register_template("t", "gamma").unwrap();

        let mut session = test_session();
        let ctx = Context::new();
        let r1 = engine.render(&mut session, "t", &ctx).unwrap();
        let r2 = engine.render(&mut session, "t", &ctx).unwrap();
        let r3 = engine.render(&mut session, "t", &ctx).unwrap();
        let r4 = engine.render(&mut session, "t", &ctx).unwrap();

        // First three should be the three alternatives, in order.
        assert!(r1.starts_with("alpha"), "r1 = {r1}");
        // Second and third may pick up connectives; check the template body.
        assert!(r2.contains("beta"), "r2 = {r2}");
        assert!(r3.contains("gamma"), "r3 = {r3}");
        // Fourth wraps back to alpha.
        assert!(r4.contains("alpha"), "r4 = {r4}");
    }

    /// Variation::Fixed must always emit the first-registered template body,
    /// even after discourse history has accumulated.
    #[test]
    fn fixed_variation_stays_fixed_across_renders() {
        let mut engine = test_engine().variation(Variation::Fixed);
        engine.register_template("t", "alpha body here").unwrap();
        engine.register_template("t", "beta body here").unwrap();

        let mut session = test_session();
        let ctx = Context::new();
        for _ in 0..5 {
            let rendered = engine.render(&mut session, "t", &ctx).unwrap();
            assert!(
                rendered.contains("alpha body here"),
                "Fixed should always pick the first-registered template, got: {rendered}"
            );
            assert!(
                !rendered.contains("beta"),
                "Fixed must never emit a later-registered alternative, got: {rendered}"
            );
        }
    }

    // ── Verb pipe tests ──────────────────────────────────────────────────

    #[test]
    fn verb_pipe_simple_past_passive() {
        let mut engine = test_engine();
        engine
            .register_template("t", "{action|verb:past}")
            .unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("action", Value::String("rename".into()));
        assert_eq!(engine.render(&mut session, "t", &ctx).unwrap(), "was renameed");
    }

    #[test]
    fn verb_pipe_present_perfect_passive() {
        let mut engine = test_engine();
        engine
            .register_template("t", "{action|verb:present_perfect}")
            .unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("action", Value::String("rename".into()));
        assert_eq!(engine.render(&mut session, "t", &ctx).unwrap(), "has been renameed");
    }

    #[test]
    fn verb_pipe_present_progressive_passive() {
        let mut engine = test_engine();
        engine
            .register_template("t", "{action|verb:present_progressive}")
            .unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("action", Value::String("rename".into()));
        assert_eq!(engine.render(&mut session, "t", &ctx).unwrap(), "is being renameed");
    }

    #[test]
    fn verb_pipe_active_voice_prefix() {
        let mut engine = test_engine();
        engine
            .register_template("t", "{action|verb:active_present_perfect}")
            .unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("action", Value::String("rename".into()));
        assert_eq!(engine.render(&mut session, "t", &ctx).unwrap(), "has renameed");
    }

    #[test]
    fn verb_pipe_conditional() {
        let mut engine = test_engine();
        engine
            .register_template("t", "{action|verb:conditional}")
            .unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("action", Value::String("rename".into()));
        assert_eq!(engine.render(&mut session, "t", &ctx).unwrap(), "would be renameed");
    }

    #[test]
    fn verb_pipe_unknown_spec_is_error() {
        let mut engine = test_engine();
        engine
            .register_template("t", "{action|verb:bogus_form}")
            .unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("action", Value::String("rename".into()));
        let result = engine.render(&mut session, "t", &ctx);
        assert!(matches!(result, Err(NlgError::InvalidPipe { .. })));
    }

    #[test]
    fn verb_pipe_missing_spec_is_error() {
        let mut engine = test_engine();
        engine
            .register_template("t", "{action|verb}")
            .unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("action", Value::String("rename".into()));
        let result = engine.render(&mut session, "t", &ctx);
        assert!(matches!(result, Err(NlgError::InvalidPipe { .. })));
    }

    /// Choose-best scoring must not advance list-style state via candidate
    /// rendering — only the emitted render counts.
    #[test]
    fn candidate_scoring_does_not_advance_list_style() {
        // Seeded variation triggers choose-best on render 2.
        let mut engine = test_engine().variation(Variation::Seeded(1));
        // Two alternatives both consume a list style each; if candidate
        // rendering mutates state, the cycle is wrong.
        engine
            .register_template("t", "alpha uses {items|truncate:1|join}")
            .unwrap();
        engine
            .register_template("t", "beta uses {items|truncate:1|join}")
            .unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert(
            "items",
            Value::List(vec!["a".into(), "b".into(), "c".into()]),
        );

        let r1 = engine.render(&mut session, "t", &ctx).unwrap();
        let r2 = engine.render(&mut session, "t", &ctx).unwrap();
        let r3 = engine.render(&mut session, "t", &ctx).unwrap();

        // Three renders should show three consecutive list styles.
        // If candidate scoring leaked state, we'd see the cycle skip ahead
        // (e.g., render 2's candidate would consume a style, pushing render 3
        // onto the 4th style instead of the 3rd).
        let styles: std::collections::HashSet<&str> = [
            r1.as_str(),
            r2.as_str(),
            r3.as_str(),
        ]
        .into_iter()
        .collect();
        assert_eq!(
            styles.len(),
            3,
            "Expected three distinct list styles across three renders, got: {r1} / {r2} / {r3}"
        );
    }
}

#[cfg(test)]
mod engine_thread_safety {
    use super::Engine;

    // Compile-time assert: Engine is Send + Sync post-refactor.
    // If this ever breaks (e.g. RefCell re-introduced), compilation fails here.
    const _: fn() = || {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Engine>();
    };
}
