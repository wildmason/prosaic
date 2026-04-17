use core::sync::atomic::{AtomicUsize, Ordering};

#[cfg(not(feature = "std"))]
use alloc::boxed::Box;
#[cfg(not(feature = "std"))]
use alloc::format;
#[cfg(not(feature = "std"))]
use alloc::string::{String, ToString};
#[cfg(not(feature = "std"))]
use alloc::vec;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use crate::collections::{HashMap, HashSet, new_map, new_set};

use crate::faithfulness::score_faithfulness;
use crate::session::Session;

use crate::antonyms::{AntonymRegistry, insert_not};
use crate::context::{Context, IntoContext, Value};
use crate::discourse::{ListStyle, ReferenceForm, Transition};
use crate::error::ProsaicError;
use crate::hedge::{HedgeMode, hedge as hedge_fn, parse_mode as parse_hedge_mode};
use crate::language::{Conjunction, Language, Person, PluralCategory, VerbForm};
#[cfg(feature = "polish")]
use crate::length::split_long_in_place;
#[cfg(feature = "polish")]
use crate::punctuation::smart_quotes_in_place;
use crate::agreement::AgreementFeatures;
use crate::quantify::{QuantifyMode, parse_mode as parse_quantify_mode, quantify as quantify_fn};
#[cfg(feature = "reg")]
use crate::reg::{
    EntityDescriptor, EntityRegistry, distinguishing_attributes, distinguishing_subgraph,
};
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

/// Selects the REG (Referring Expression Generation) algorithm used by
/// the `{name|refer}` pipe when rendering the Full form of a reference.
///
/// The default is [`DaleReiter`][RegAlgorithm::DaleReiter], which matches
/// the historical behaviour of the engine.
#[cfg(feature = "reg")]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RegAlgorithm {
    /// Dale & Reiter 1995 Incremental Algorithm.
    ///
    /// Disambiguates same-type entities using unary attributes only —
    /// e.g. "the domain class UserService" vs "the infra class AuthService".
    /// Fast, well-understood, and the default.
    #[default]
    DaleReiter,
    /// Krahmer et al. 2003 graph-based greedy algorithm.
    ///
    /// Handles both unary attributes AND binary relations between entities.
    /// When attributes alone do not disambiguate, the algorithm appends one
    /// relation clause — e.g. "the api function LoginHandler that calls
    /// AuthService". Falls back silently to D&R behaviour when no relations
    /// are registered.
    GraphBased,
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
    /// Centering Theory transition class for this render. Reflects how the
    /// discourse center moved relative to the previous render. `NoCb` on the
    /// first render or after a session reset.
    pub centering_transition: Transition,
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
    type Item = Result<String, ProsaicError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.i >= self.events.len() {
            return None;
        }

        // Terminal-error helper: any render failure forces the iterator
        // to report `None` on subsequent calls. Continuing past an error
        // would replay the same failing run and — inside aggregated or
        // gapping runs — compound session state from earlier successful
        // renders. See `render_iter`'s doc comment.
        let fail = |this: &mut RenderIter<'_>, e: ProsaicError| -> Option<Self::Item> {
            this.i = this.events.len();
            Some(Err(e))
        };

        // Mirror the logic in `render_batch` but emit one sentence per
        // `.next()` call.
        let action_end = self.engine.find_same_action_run(self.events, self.i);
        if action_end > self.i + 1 {
            let key = self.events[self.i].0;
            let run = &self.events[self.i..action_end];
            let sentence = match self
                .engine
                .render_aggregated_subjects(self.session, key, run)
            {
                Ok(s) => s,
                Err(e) => return fail(self, e),
            };
            self.i = action_end;
            return Some(Ok(sentence));
        }

        // Gapping: same template key, different subjects, incompatible
        // non-subject context (different objects/complements).
        let gap_end = self.engine.find_gapping_run(self.events, self.i);
        if gap_end > self.i + 1 {
            let mut rendered: Vec<String> = Vec::with_capacity(gap_end - self.i);
            for (key, ctx) in &self.events[self.i..gap_end] {
                match self.engine.render(self.session, key, ctx) {
                    Ok(s) => rendered.push(s),
                    Err(e) => return fail(self, e),
                }
            }
            self.i = gap_end;
            if let Some(gapped) = reduce_gapping(&rendered) {
                return Some(Ok(gapped));
            }
            return Some(Ok(rendered.join(" ")));
        }

        let entity_end = self.engine.find_same_entity_run(self.events, self.i);
        if entity_end > self.i + 1 {
            let mut run_rendered: Vec<String> = Vec::with_capacity(entity_end - self.i);
            for (key, ctx) in &self.events[self.i..entity_end] {
                match self.engine.render(self.session, key, ctx) {
                    Ok(s) => run_rendered.push(s),
                    Err(e) => return fail(self, e),
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
        match self.engine.render(self.session, key, ctx) {
            Ok(s) => Some(Ok(s)),
            Err(e) => {
                self.i = self.events.len();
                Some(Err(e))
            }
        }
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
    #[cfg(feature = "reg")]
    reg_algorithm: RegAlgorithm,
    synonyms: SynonymRegistry,
    #[cfg(feature = "time")]
    reference_time: Option<i64>,
    antonyms: AntonymRegistry,
    #[cfg(feature = "polish")]
    max_sentence_length: Option<usize>,
    #[cfg(feature = "polish")]
    smart_quotes: bool,
    partials: HashMap<String, Template>,
    /// Optional faithfulness gate. When `Some(threshold)`, each rendered output
    /// is scored via PARENT precision + polarity check. If the score does not
    /// pass the threshold or polarity mismatches, the render returns
    /// `ProsaicError::FaithfulnessRejection` and session state is restored.
    faithfulness_threshold: Option<f32>,
}

/// Per-call render options used by internal paths that need to suppress
/// specific engine behaviours. Not part of the public API — exposed only
/// through wrapping methods like `render_batch_with_relations`.
#[derive(Debug, Clone, Copy, Default)]
struct RenderOptions {
    /// Skip automatic discourse connective selection and prepending.
    /// The session's `connective_history` ring buffer is **not** advanced.
    /// Used when an explicit RST marker will replace the auto-connective
    /// so session state stays consistent with the emitted prose.
    suppress_auto_connective: bool,
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
    ///
    /// Per-call options (currently only whether to suppress the engine's
    /// automatic discourse connective) are used by
    /// `render_batch_with_relations` when an explicit RST marker will be
    /// applied, so the session never records a connective that isn't
    /// actually emitted.
    fn render_tx_with_options(
        &mut self,
        key: &str,
        all_alternatives: &[SalientTemplate],
        context: &Context,
        options: RenderOptions,
    ) -> Result<String, ProsaicError> {
        // Advance discourse state
        self.session.discourse.begin_render();

        // Extract entity info from context for discourse tracking
        let entity_name = context
            .get("name")
            .or_else(|| context.get("old_name"))
            .map(|v| v.as_display());
        let entity_type = context.get("entity_type").map(|v| v.as_display());

        // Detect discourse connective — but only select (and advance the
        // no-repeat ring buffer) when the caller hasn't asked us to
        // suppress it. Skipping both the detect and select avoids
        // polluting `connective_history` with a connective that will
        // never reach the output.
        let connective = if options.suppress_auto_connective {
            None
        } else {
            let relation = self
                .session
                .discourse
                .detect_relation(key, entity_name.as_deref());
            self.session.discourse.select_connective(&relation)
        };

        // Filter templates by salience level matching the context magnitude.
        let target_salience = self.engine.context_salience(context);
        let alternatives = filter_by_salience(all_alternatives, target_salience);

        // Select template with choosebest scoring and anti-repeat
        let (template, variant_index) =
            self.select_alternative_scored(key, &alternatives, context)?;

        // Record template choice
        self.session
            .discourse
            .record_template_choice(key, variant_index);

        // Render the selected template into a preallocated buffer.
        let mut output = String::with_capacity(128);
        self.render_template_into(&mut output, key, template, context)?;

        // Prepend discourse connective if applicable
        if let Some(conn) = connective {
            if conn.starts_with("It ") {
                prepend_replacing_subject_in_place(&mut output, conn, entity_name.as_deref());
            } else {
                lowercase_first_in_place(&mut output);
                let mut buf = String::with_capacity(conn.len() + 1 + output.len());
                buf.push_str(conn);
                buf.push(' ');
                buf.push_str(&output);
                core::mem::swap(&mut output, &mut buf);
            }
        }

        // Capitalize if the template starts with a refer pipe
        if starts_with_refer_pipe(template) {
            capitalize_first_in_place(&mut output);
        }

        // Clean up whitespace and silent-mode gaps. Record whether the
        // orphan-tail pass removed anything so RenderExplanation can
        // surface it.
        let cleanup_stripped = cleanup_artifacts_in_place(&mut output, self.engine.strictness);
        self.session
            .discourse
            .set_cleanup_stripped_tail(cleanup_stripped);

        // Terminate the sentence
        terminate_sentence_in_place(&mut output);

        // Length budget
        #[cfg(feature = "polish")]
        if let Some(max_chars) = self.engine.max_sentence_length {
            split_long_in_place(&mut output, max_chars);
        }

        // Typographic polish
        #[cfg(feature = "polish")]
        if self.engine.smart_quotes {
            smart_quotes_in_place(&mut output);
        }

        // Faithfulness gate — checked against the fully-polished output.
        // If the gate is active and the output fails, propagate the error;
        // the caller's snapshot/restore in `render()` will undo session state.
        if let Some(threshold) = self.engine.faithfulness_threshold {
            let literals = template.literal_tokens();
            let score = score_faithfulness(&output, context, &literals, &*self.engine.language);
            if !score.passes(threshold) {
                return Err(ProsaicError::FaithfulnessRejection {
                    precision: score.precision,
                    polarity_match: score.polarity_match,
                });
            }
        }

        // Record entity mention in discourse state
        if let (Some(name), Some(etype)) = (&entity_name, &entity_type) {
            self.session.discourse.mention_entity(name, etype);
        }

        // Record output words for future repetition scoring
        self.session.discourse.record_output_words(&output);

        // Advance Cb (backward-looking center) for the next render.
        // Must be the last mutation so failed renders don't advance Cb
        // — the snapshot/restore path in render() rolls back via Clone.
        self.session.discourse.advance_cb();

        Ok(output)
    }

    fn select_alternative_scored<'a>(
        &mut self,
        key: &str,
        alternatives: &[&'a Template],
        context: &Context,
    ) -> Result<(&'a Template, usize), ProsaicError> {
        if alternatives.len() == 1 {
            return Ok((alternatives[0], 0));
        }

        let allow_choose_best = matches!(
            self.engine.variation,
            Variation::Seeded(_) | Variation::Random
        );

        if !allow_choose_best {
            let index = self.select_variant_index(key, alternatives.len());
            return Ok((alternatives[index], index));
        }

        let last_variant = self.session.discourse.last_template_variant(key);
        let is_first = self.session.discourse.is_first_render();

        if is_first {
            let index = self.select_variant_index(key, alternatives.len());
            return Ok((alternatives[index], index));
        }

        // Snapshot-and-restore around candidate rendering so state
        // is untouched by alternatives that aren't emitted.
        let snapshot = self.session.clone();

        let mut candidates: Vec<(usize, String)> = Vec::new();
        let mut scratch = String::with_capacity(128);
        for (i, template) in alternatives.iter().enumerate() {
            if Some(i) == last_variant {
                continue;
            }
            scratch.clear();
            match self.render_template_into(&mut scratch, key, template, context) {
                Ok(()) => {}
                Err(e) => {
                    *self.session = snapshot;
                    return Err(e);
                }
            }
            candidates.push((i, scratch.clone()));
        }

        *self.session = snapshot;

        if candidates.is_empty() {
            let index = last_variant.unwrap_or(0).min(alternatives.len() - 1);
            return Ok((alternatives[index], index));
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

        Ok((alternatives[best_index], best_index))
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
                #[cfg(feature = "std")]
                {
                    let nanos = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .subsec_nanos() as usize;
                    nanos % count
                }
                #[cfg(not(feature = "std"))]
                {
                    // Without std, fall back to variant 0 (deterministic).
                    let _ = count;
                    0
                }
            }
        }
    }

    fn render_template_into(
        &mut self,
        out: &mut String,
        key: &str,
        template: &Template,
        context: &Context,
    ) -> Result<(), ProsaicError> {
        self.render_segments_into(out, key, &template.segments, context)
    }

    fn render_segments_into(
        &mut self,
        out: &mut String,
        key: &str,
        segments: &[Segment],
        context: &Context,
    ) -> Result<(), ProsaicError> {
        for segment in segments {
            match segment {
                Segment::Literal(text) => out.push_str(text),
                Segment::Slot {
                    key: slot_key,
                    pipes,
                } => {
                    self.render_slot_into(out, key, slot_key, pipes, context)?;
                }
                Segment::Conditional {
                    condition_key,
                    inner,
                } => {
                    if is_truthy(context.get(condition_key)) {
                        self.render_segments_into(out, key, inner, context)?;
                    }
                }
                Segment::Partial { name } => {
                    // Clone the partial segments to avoid borrow conflicts
                    let partial_segments = self.engine.partials.get(name).ok_or_else(|| {
                        ProsaicError::TemplateParseError {
                            template: key.to_string(),
                            position: 0,
                            reason: format!(
                                "unknown partial `{name}` — register it with `engine.register_partial`"
                            ),
                        }
                    })?.segments.clone();
                    self.render_segments_into(out, key, &partial_segments, context)?;
                }
            }
        }

        Ok(())
    }

    fn render_slot_into(
        &mut self,
        out: &mut String,
        template_key: &str,
        slot_key: &str,
        pipes: &[Pipe],
        context: &Context,
    ) -> Result<(), ProsaicError> {
        let value = match context.get(slot_key) {
            Some(v) => v.clone(),
            None => {
                let s = self.handle_missing_slot(template_key, slot_key)?;
                out.push_str(&s);
                return Ok(());
            }
        };

        if pipes.is_empty() {
            out.push_str(&value.as_display());
            return Ok(());
        }

        let mut current = value;
        for pipe in pipes {
            current = self.apply_pipe(pipe, &current, context)?;
        }

        out.push_str(&current.as_display());
        Ok(())
    }

    fn handle_missing_slot(
        &self,
        template_key: &str,
        slot_key: &str,
    ) -> Result<String, ProsaicError> {
        match self.engine.strictness {
            Strictness::Strict => Err(ProsaicError::MissingSlot {
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
    ) -> Result<Value, ProsaicError> {
        match pipe.name.as_str() {
            "plural" => self.pipe_plural(pipe, value),
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
            #[cfg(feature = "time")]
            "since_last" => self.pipe_since_last(value),
            "quantify" => self.pipe_quantify(pipe, value),
            "proportion" => self.pipe_proportion(pipe, value, context),
            "demonstrative" => self.pipe_demonstrative(value),
            "hedge" => self.pipe_hedge(pipe, value),
            "negated" => self.pipe_negated(value),
            "choose" => self.pipe_choose(pipe, value),
            _ => Err(ProsaicError::InvalidPipe {
                pipe: pipe.name.clone(),
                reason: "unknown pipe".to_string(),
            }),
        }
    }

    fn pipe_choose(&self, pipe: &Pipe, value: &Value) -> Result<Value, ProsaicError> {
        let arg_str = match &pipe.arg {
            Some(PipeArg::String(s)) => s.as_str(),
            Some(PipeArg::Number(_)) | None => {
                return Err(ProsaicError::InvalidPipe {
                    pipe: "choose".to_string(),
                    reason: "choose requires an argument of the form \
                             'key=value,key=value,default=value'"
                        .to_string(),
                });
            }
        };

        let pairs = parse_choose_pairs(arg_str)?;
        if pairs.is_empty() {
            return Err(ProsaicError::InvalidPipe {
                pipe: "choose".to_string(),
                reason: "choose argument is empty".to_string(),
            });
        }

        let display = value.as_display();
        let normalized = display.trim().to_lowercase();

        for (k, v) in &pairs {
            if k.to_lowercase() == normalized {
                return Ok(Value::String(v.clone()));
            }
        }

        // Fallback to default key
        for (k, v) in &pairs {
            if k.eq_ignore_ascii_case("default") {
                return Ok(Value::String(v.clone()));
            }
        }

        // No match, no default — dispatch on strictness
        match self.engine.strictness {
            Strictness::Strict => Err(ProsaicError::InvalidPipe {
                pipe: "choose".to_string(),
                reason: format!("no matching key for value `{display}` and no default"),
            }),
            Strictness::Lenient => Ok(Value::String(format!("[choose: no match for {display}]"))),
            Strictness::Silent => Ok(Value::String(String::new())),
        }
    }

    fn pipe_refer(
        &mut self,
        pipe: &Pipe,
        value: &Value,
        context: &Context,
    ) -> Result<Value, ProsaicError> {
        // Plural REG: list of same-type entities — dispatch before the
        // single-entity path so Value::List never falls through to as_display().
        if let Value::List(names) = value {
            return self.pipe_refer_plural(pipe, names, context);
        }
        self.pipe_refer_single(pipe, value, context)
    }

    /// Single-entity refer path (existing logic, extracted for reuse by the
    /// plural dispatch).
    fn pipe_refer_single(
        &self,
        pipe: &Pipe,
        value: &Value,
        context: &Context,
    ) -> Result<Value, ProsaicError> {
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
            ReferenceForm::Pronoun | ReferenceForm::Demonstrative | ReferenceForm::Zero => {
                // Synthesize features from discourse state. Today this is just
                // the plural flag; v1.5+ multilingual grammars can thread richer
                // features through via Value::Entity at the call site.
                let features = crate::agreement::AgreementFeatures {
                    number: if self.session.discourse.focus_is_plural() {
                        crate::agreement::Number::Plural
                    } else {
                        crate::agreement::Number::Singular
                    },
                    ..crate::agreement::AgreementFeatures::default()
                };
                self.engine
                    .language
                    .realize_reference(form, &features)
                    .unwrap_or_default()
            }
        };

        Ok(Value::String(rendered))
    }

    /// Plural REG path: collapses a list of same-type entities to a plural
    /// description using [`crate::language::Language::plural_description`].
    ///
    /// - Empty list  → empty string (silent-mode convention).
    /// - Single item → delegates to single-entity path (`pipe_refer_single`).
    /// - Multi-item  → calls `plural_description` and updates discourse state.
    fn pipe_refer_plural(
        &mut self,
        pipe: &Pipe,
        names: &[String],
        context: &Context,
    ) -> Result<Value, ProsaicError> {
        match names.len() {
            0 => Ok(Value::String(String::new())),
            1 => {
                let v = Value::String(names[0].clone());
                self.pipe_refer_single(pipe, &v, context)
            }
            n => {
                let entity_type = match &pipe.arg {
                    Some(PipeArg::String(t)) => t.clone(),
                    _ => context
                        .get("entity_type")
                        .map(|v| v.as_display())
                        .unwrap_or_default(),
                };

                // Register each entity in discourse for future singular tracking.
                // mention_entity also resets focus_is_plural to false for each
                // individual mention, so we call set_focus_plural after the loop.
                if !entity_type.is_empty() {
                    for name in names {
                        self.session.discourse.mention_entity(name, &entity_type);
                    }
                }

                // Mark the discourse focus as plural so subsequent pronoun
                // references emit "they" rather than "it".
                // Note: this sets focus on the set as a whole; future work can
                // distinguish subject-vs-object position if needed.
                self.session.discourse.set_focus_plural(true);

                // Use default AgreementFeatures for v1; a future Value::EntityList
                // variant can carry per-entity features for richer agreement.
                let features = crate::agreement::AgreementFeatures::default();

                let output = self
                    .engine
                    .language
                    .plural_description(&entity_type, n, &features);
                Ok(Value::String(output))
            }
        }
    }

    fn pipe_demonstrative(&self, value: &Value) -> Result<Value, ProsaicError> {
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

    fn pipe_syn(&self, value: &Value) -> Result<Value, ProsaicError> {
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
            let mut s = best.clone();
            capitalize_first_in_place(&mut s);
            s
        } else {
            best.clone()
        };

        Ok(Value::String(result))
    }

    /// Join a list value into a prose-formatted string.
    ///
    /// # Styles
    ///
    /// | Syntax | Style | Example output |
    /// |--------|-------|----------------|
    /// | `{items\|join}` | Cycling (auto) | Rotates through Including → SuchAs → Dash → Bracketed across renders |
    /// | `{items\|join:including}` | Including | "including A, B, and C among others" |
    /// | `{items\|join:such_as}` | SuchAs | "such as A, B, and C" |
    /// | `{items\|join:dash}` | Dash | "— A, B, and C" |
    /// | `{items\|join:bracketed}` | Bracketed | "[A, B, and C]" or "[A, B, and 2 more]" |
    /// | `{items\|join:or}` | Or-conjunction | "A, B, or C" |
    ///
    /// # Choosing a style
    ///
    /// When your surrounding template text already contains a framing
    /// word like "impacting", "affecting", "across", or "including",
    /// use `join:bracketed` to avoid doubling up with the cycling
    /// list prefix. For example:
    ///
    /// - **Bad:** `"impacting {endpoints|join}"` → *"impacting including A, B, and C"*
    /// - **Good:** `"impacting {endpoints|join:bracketed}"` → *"impacting [A, B, and C]"*
    ///
    /// The cycling auto-style is best when the slot appears without
    /// a framing word, so the list prefix provides its own context:
    /// `"{consumers|join}"` → *"including A, B, and C"* on one render,
    /// *"such as A, B, and C"* on the next.
    fn pipe_join(&mut self, pipe: &Pipe, value: &Value) -> Result<Value, ProsaicError> {
        let items = value.as_list().ok_or_else(|| ProsaicError::InvalidPipe {
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

        let style = match forced_style {
            Some(s) => {
                // Still record the chosen style so render_explained can
                // report it even when the template forced one.
                self.session.discourse.record_list_style_used(s);
                s
            }
            None => self.session.discourse.next_list_style(),
        };

        let refs: Vec<&str> = items.iter().map(|s| s.as_str()).collect();

        let has_truncation = items.last().is_some_and(|last| {
            last.ends_with(" more")
                && last
                    .split_whitespace()
                    .next()
                    .is_some_and(|w| w.parse::<usize>().is_ok())
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
    ) -> Result<Value, ProsaicError> {
        let word = match &pipe.arg {
            Some(PipeArg::String(w)) => w.as_str(),
            _ => {
                return Err(ProsaicError::InvalidPipe {
                    pipe: "pluralize".to_string(),
                    reason: "requires a word argument, e.g., {count|pluralize:item}".to_string(),
                });
            }
        };

        let count = value.as_number().ok_or_else(|| ProsaicError::InvalidPipe {
            pipe: "pluralize".to_string(),
            reason: "value must be a number".to_string(),
        })? as usize;

        Ok(Value::String(self.engine.language.pluralize(word, count)))
    }

    /// CLDR-aware plural pipe: `{count|plural:noun}`.
    ///
    /// Reads the slot as an integer, classifies it with
    /// [`Language::plural_category`], then returns the correct word form
    /// via [`Language::pluralize_with_category`]. Unlike `|pluralize`, this
    /// pipe is category-aware and ready for non-English grammars that
    /// distinguish more than two number categories.
    fn pipe_plural(&self, pipe: &Pipe, value: &Value) -> Result<Value, ProsaicError> {
        let noun = match &pipe.arg {
            Some(PipeArg::String(s)) => s.as_str(),
            _ => {
                return Err(ProsaicError::InvalidPipe {
                    pipe: "plural".to_string(),
                    reason: "requires a singular noun argument, e.g., {count|plural:service}"
                        .to_string(),
                });
            }
        };

        let count = value.as_number().ok_or_else(|| ProsaicError::InvalidPipe {
            pipe: "plural".to_string(),
            reason: "requires a numeric slot value".to_string(),
        })?;

        let category: PluralCategory = self.engine.language.plural_category(count);
        Ok(Value::String(
            self.engine.language.pluralize_with_category(noun, category),
        ))
    }

    fn pipe_article(&self, value: &Value) -> Result<Value, ProsaicError> {
        let word = value.as_display();
        let article = self.engine.language.article(&word);
        Ok(Value::String(format!("{article} {word}")))
    }

    fn pipe_ordinal(&self, value: &Value) -> Result<Value, ProsaicError> {
        let n = value.as_number().ok_or_else(|| ProsaicError::InvalidPipe {
            pipe: "ordinal".to_string(),
            reason: "value must be a number".to_string(),
        })? as usize;

        Ok(Value::String(self.engine.language.ordinal(n)))
    }

    fn pipe_words(&self, value: &Value) -> Result<Value, ProsaicError> {
        let n = value.as_number().ok_or_else(|| ProsaicError::InvalidPipe {
            pipe: "words".to_string(),
            reason: "value must be a number".to_string(),
        })? as usize;

        Ok(Value::String(self.engine.language.number_to_words(n)))
    }

    fn pipe_truncate(&self, pipe: &Pipe, value: &Value) -> Result<Value, ProsaicError> {
        let max = match &pipe.arg {
            Some(PipeArg::Number(n)) => *n,
            _ => {
                return Err(ProsaicError::InvalidPipe {
                    pipe: "truncate".to_string(),
                    reason: "requires a numeric argument, e.g., {items|truncate:3}".to_string(),
                });
            }
        };

        let items = value.as_list().ok_or_else(|| ProsaicError::InvalidPipe {
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

    fn pipe_capitalize(&self, value: &Value) -> Result<Value, ProsaicError> {
        let mut s = value.as_display();
        capitalize_first_in_place(&mut s);
        Ok(Value::String(s))
    }

    fn pipe_negated(&self, value: &Value) -> Result<Value, ProsaicError> {
        let phrase = value.as_display();
        if let Some(positive) = self.engine.antonyms.lookup(&phrase) {
            return Ok(Value::String(positive.to_string()));
        }
        Ok(Value::String(insert_not(&phrase)))
    }

    fn pipe_hedge(&self, pipe: &Pipe, value: &Value) -> Result<Value, ProsaicError> {
        let score = value.as_number().ok_or_else(|| ProsaicError::InvalidPipe {
            pipe: "hedge".to_string(),
            reason: "value must be a 0..=100 integer confidence score".to_string(),
        })?;

        let mode = match &pipe.arg {
            None => HedgeMode::Adverb,
            Some(PipeArg::String(s)) => {
                parse_hedge_mode(s).ok_or_else(|| ProsaicError::InvalidPipe {
                    pipe: "hedge".to_string(),
                    reason: format!(
                        "unknown hedge mode `{s}` — expected one of adverb, modal, prefix"
                    ),
                })?
            }
            Some(PipeArg::Number(_)) => {
                return Err(ProsaicError::InvalidPipe {
                    pipe: "hedge".to_string(),
                    reason: "hedge argument must be a mode name, not a number".to_string(),
                });
            }
        };

        Ok(Value::String(hedge_fn(score, mode).to_string()))
    }

    fn pipe_proportion(
        &self,
        pipe: &Pipe,
        value: &Value,
        context: &Context,
    ) -> Result<Value, ProsaicError> {
        let arg_str = match &pipe.arg {
            Some(PipeArg::String(s)) => s.as_str(),
            Some(PipeArg::Number(_)) | None => {
                return Err(ProsaicError::InvalidPipe {
                    pipe: "proportion".to_string(),
                    reason: "requires an argument of the form \
                             `proportion:total_key[:singular_noun]`"
                        .to_string(),
                });
            }
        };

        // Split into total_key and optional noun. The noun may itself contain
        // spaces (e.g. "modified file"), so we only split on the first colon.
        let (total_key, noun) = match arg_str.split_once(':') {
            Some((k, n)) => {
                let n = n.trim();
                (k.trim(), if n.is_empty() { None } else { Some(n) })
            }
            None => (arg_str.trim(), None),
        };

        if total_key.is_empty() {
            return Err(ProsaicError::InvalidPipe {
                pipe: "proportion".to_string(),
                reason: "missing total context key — use `proportion:total_key[:noun]`"
                    .to_string(),
            });
        }

        let matching = value.as_number().ok_or_else(|| ProsaicError::InvalidPipe {
            pipe: "proportion".to_string(),
            reason: "value must be a number".to_string(),
        })?;

        let total_value = context
            .get(total_key)
            .ok_or_else(|| ProsaicError::InvalidPipe {
                pipe: "proportion".to_string(),
                reason: format!("total context key `{total_key}` not found"),
            })?;

        let total = total_value
            .as_number()
            .ok_or_else(|| ProsaicError::InvalidPipe {
                pipe: "proportion".to_string(),
                reason: format!("total context key `{total_key}` is not a number"),
            })?;

        let features = AgreementFeatures::default();
        let phrase =
            self.engine
                .language
                .proportion_phrase(matching, total, noun, &features);
        Ok(Value::String(phrase))
    }

    fn pipe_quantify(&self, pipe: &Pipe, value: &Value) -> Result<Value, ProsaicError> {
        let count = value.as_number().ok_or_else(|| ProsaicError::InvalidPipe {
            pipe: "quantify".to_string(),
            reason: "value must be a number".to_string(),
        })?;

        let mode = match &pipe.arg {
            None => QuantifyMode::Natural,
            Some(PipeArg::String(s)) => {
                parse_quantify_mode(s).ok_or_else(|| ProsaicError::InvalidPipe {
                    pipe: "quantify".to_string(),
                    reason: format!(
                        "unknown quantify mode `{s}` — expected one of natural, exact, hedged"
                    ),
                })?
            }
            Some(PipeArg::Number(_)) => {
                return Err(ProsaicError::InvalidPipe {
                    pipe: "quantify".to_string(),
                    reason: "quantify argument must be a mode name, not a number".to_string(),
                });
            }
        };

        Ok(Value::String(quantify_fn(
            count,
            mode,
            &*self.engine.language,
        )))
    }

    fn pipe_verb(&self, pipe: &Pipe, value: &Value) -> Result<Value, ProsaicError> {
        let spec = match &pipe.arg {
            Some(PipeArg::String(s)) => s.as_str(),
            _ => {
                return Err(ProsaicError::InvalidPipe {
                    pipe: "verb".to_string(),
                    reason: "requires a form spec argument, e.g., \
                             {rename|verb:present_perfect}"
                        .to_string(),
                });
            }
        };

        let (form, voice) =
            VerbForm::parse_spec(spec).ok_or_else(|| ProsaicError::InvalidPipe {
                pipe: "verb".to_string(),
                reason: format!(
                    "unknown verb form spec `{spec}` — expected one of past, present, future, \
                 present_perfect, past_perfect, future_perfect, present_progressive, \
                 past_progressive, conditional, conditional_perfect \
                 (optionally prefixed with `active_` or `passive_`)"
                ),
            })?;

        let verb = value.as_display();
        let phrase = self
            .engine
            .language
            .verb_phrase(&verb, form, voice, Person::Third);
        Ok(Value::String(phrase))
    }

    #[cfg(feature = "time")]
    fn pipe_relative(&self, value: &Value) -> Result<Value, ProsaicError> {
        let ts = value.as_number().ok_or_else(|| ProsaicError::InvalidPipe {
            pipe: "relative".to_string(),
            reason: "value must be a Unix-epoch integer (seconds)".to_string(),
        })?;

        let now = match self.engine.reference_time {
            Some(n) => n,
            None => {
                // `time` feature implies `std` (see Cargo.toml), so
                // `SystemTime::now()` is always available here.
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0)
            }
        };

        let diff = now - ts;
        Ok(Value::String(format_relative(diff)))
    }

    #[cfg(feature = "time")]
    fn pipe_since_last(&mut self, value: &Value) -> Result<Value, ProsaicError> {
        let Some(ts) = value.as_number() else {
            return Err(ProsaicError::InvalidPipe {
                pipe: "since_last".to_string(),
                reason: "expected numeric Unix-seconds timestamp".to_string(),
            });
        };

        let marker = match self.session.last_temporal_anchor {
            Some(anchor) => self.engine.language.since_last_marker(ts - anchor),
            None => {
                // Fall back to absolute-relative behavior anchored at now/reference_time.
                // This makes the first event in a narrative read like "3 days ago",
                // and subsequent events read like anchored deltas ("the next day").
                let now = match self.engine.reference_time {
                    Some(n) => n,
                    None => {
                        // `time` feature implies `std` (see Cargo.toml), so
                        // `SystemTime::now()` is always available here.
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs() as i64)
                            .unwrap_or(0)
                    }
                };
                format_relative(now - ts)
            }
        };

        Ok(Value::String(marker))
    }

    /// Score every variant for a key using this session state (for diagnostics).
    fn score_all_variants(
        &mut self,
        key: &str,
        all: &[SalientTemplate],
        ctx: &Context,
    ) -> Result<Vec<VariantScore>, ProsaicError> {
        let target_salience = self.engine.context_salience(ctx);
        let alternatives = filter_by_salience(all, target_salience);

        // Snapshot so candidate renders leave no residue.
        let snapshot = self.session.clone();

        let last_variant = self.session.discourse.last_template_variant(key);
        let mut scores: Vec<VariantScore> = Vec::with_capacity(alternatives.len());
        let mut scratch = String::with_capacity(128);

        for (i, template) in alternatives.iter().enumerate() {
            scratch.clear();
            match self.render_template_into(&mut scratch, key, template, ctx) {
                Ok(()) => {}
                Err(e) => {
                    *self.session = snapshot;
                    return Err(e);
                }
            }
            scores.push(VariantScore {
                index: i,
                source: template.source.clone(),
                rendered: scratch.clone(),
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
        let selected_idx = self.pick_variant_index(key, &alternatives, last_variant, &scores);
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
        alternatives: &[&Template],
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
            return Some(
                self.engine
                    .pick_variant_index_static(key, alternatives.len()),
            );
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
            templates: new_map(),
            strictness: Strictness::default(),
            variation: Variation::default(),
            salience_thresholds: SalienceThresholds::default(),
            rr_initial: new_map(),
            #[cfg(feature = "reg")]
            entity_registry: EntityRegistry::new(),
            #[cfg(feature = "reg")]
            reg_preference: Vec::new(),
            #[cfg(feature = "reg")]
            reg_algorithm: RegAlgorithm::default(),
            synonyms: SynonymRegistry::new(),
            #[cfg(feature = "time")]
            reference_time: None,
            antonyms: AntonymRegistry::new(),
            #[cfg(feature = "polish")]
            max_sentence_length: None,
            #[cfg(feature = "polish")]
            smart_quotes: false,
            partials: new_map(),
            faithfulness_threshold: None,
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
    /// use prosaic_core::{Context, Engine, EntityDescriptor, Value};
    /// use prosaic_grammar_en::English;
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
    /// let mut session = prosaic_core::Session::new();
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

    /// Select the REG algorithm used when rendering Full-form references via
    /// `{name|refer}`.
    ///
    /// The default is [`RegAlgorithm::DaleReiter`], which selects
    /// distinguishing unary attributes only. Use [`RegAlgorithm::GraphBased`]
    /// to enable the Krahmer 2003 greedy algorithm, which also considers
    /// labeled relations registered via
    /// [`EntityDescriptor::with_relation`](crate::EntityDescriptor::with_relation).
    #[cfg(feature = "reg")]
    pub fn reg_algorithm(mut self, algo: RegAlgorithm) -> Self {
        self.reg_algorithm = algo;
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
    /// use prosaic_core::{Context, Engine, Value};
    /// use prosaic_grammar_en::English;
    ///
    /// let now = 1_700_000_000;
    /// let mut engine = Engine::new(English::new()).reference_time(now);
    /// engine.register_template("t", "The change landed {ts|relative}").unwrap();
    ///
    /// let mut ctx = Context::new();
    /// ctx.insert("ts", Value::Number(now - 86400 - 3600));
    /// let mut session = prosaic_core::Session::new();
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
    pub fn register_partial(&mut self, name: &str, source: &str) -> Result<(), ProsaicError> {
        let template = Template::parse(source)?;

        // Insert, then verify the resulting partial graph is acyclic starting
        // from the new entry point. On any cycle, restore the prior entry (or
        // remove the new one) and return a descriptive error.
        let previous = self.partials.insert(name.to_string(), template);
        if let Err(cycle) = detect_partial_cycle(&self.partials, name) {
            match previous {
                Some(prior) => {
                    self.partials.insert(name.to_string(), prior);
                }
                None => {
                    self.partials.remove(name);
                }
            }
            return Err(ProsaicError::RecursivePartial { cycle });
        }
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
    /// use prosaic_core::{Context, Engine, Value};
    /// use prosaic_grammar_en::English;
    ///
    /// let mut engine = Engine::new(English::new()).smart_quotes(true);
    /// engine.register_template("t", r#"Alice said "hello""#).unwrap();
    /// let mut session = prosaic_core::Session::new();
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
    /// use prosaic_core::{Context, Engine};
    /// use prosaic_grammar_en::English;
    ///
    /// let mut engine = Engine::new(English::new()).max_sentence_length(60);
    /// engine.register_template(
    ///     "t",
    ///     "The class UserService was renamed to AccountService, \
    ///      which impacts 6 consumers",
    /// ).unwrap();
    ///
    /// let mut session = prosaic_core::Session::new();
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
    /// form (e.g. "remained unchanged") over the default "not {phrase}"
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

    /// Enable a runtime faithfulness gate on every `render*` call.
    ///
    /// When set, each rendered output is scored against its input Context
    /// and the selected template's literal tokens using PARENT-style
    /// precision + polarity checking. If the score's precision falls below
    /// `threshold` OR polarity tokens mismatch between source and output,
    /// the render returns [`ProsaicError::FaithfulnessRejection`] and the
    /// session state is restored as if the render had not occurred.
    ///
    /// `threshold` is typically `1.0` (strict: every content token in the
    /// output must be sourced from the context or template literals).
    /// Values below `1.0` tolerate a fraction of unentailed tokens — useful
    /// when the engine legitimately emits hedged phrasings that introduce
    /// words not present in the input (e.g. "approximately", "likely").
    ///
    /// Default: no gate (all renders pass).
    ///
    /// # Example
    ///
    /// ```
    /// use prosaic_core::{Context, Engine, ProsaicError, Value};
    /// use prosaic_grammar_en::English;
    ///
    /// let mut engine = Engine::new(English::new())
    ///     .with_faithfulness_gate(1.0);
    ///
    /// engine.register_template("t", "{name} was modified").unwrap();
    ///
    /// let mut ctx = Context::new();
    /// ctx.insert("name", Value::String("UserService".into()));
    /// let mut session = prosaic_core::Session::new();
    /// // "modified" is in the template literal — renders faithfully.
    /// assert!(engine.render(&mut session, "t", &ctx).is_ok());
    /// ```
    pub fn with_faithfulness_gate(mut self, threshold: f32) -> Self {
        self.faithfulness_threshold = Some(threshold);
        self
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
    /// use prosaic_core::{Context, Engine, Session, Value};
    /// use prosaic_grammar_en::English;
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
    pub fn register_template(&mut self, key: &str, source: &str) -> Result<(), ProsaicError> {
        self.register_template_at(key, source, Salience::Medium)
    }

    /// Register a template at a specific salience level. The engine selects
    /// templates at the salience matching the rendered event's magnitude.
    pub fn register_template_at(
        &mut self,
        key: &str,
        source: &str,
        salience: Salience,
    ) -> Result<(), ProsaicError> {
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

    /// Check whether a template is registered under the given key.
    ///
    /// Pure read, no mutation. Useful for callers that want to skip
    /// rendering when no template matches (e.g. tracing bridges where
    /// not every event type has a registered template).
    ///
    /// # Example
    ///
    /// ```
    /// use prosaic_core::Engine;
    /// use prosaic_grammar_en::English;
    ///
    /// let mut engine = Engine::new(English::new());
    /// engine.register_template("greet", "Hello {name}").unwrap();
    ///
    /// assert!(engine.has_template("greet"));
    /// assert!(!engine.has_template("farewell"));
    /// ```
    pub fn has_template(&self, key: &str) -> bool {
        self.templates.contains_key(key)
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
    /// use prosaic_core::{Context, Engine, Session, Value, Variation};
    /// use prosaic_grammar_en::English;
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
    ) -> Result<String, ProsaicError> {
        self.render_with_options(session, key, context, RenderOptions::default())
    }

    /// Internal: render with explicit options. Used by batch rendering
    /// paths that need to adjust default behaviours (e.g. suppressing
    /// the automatic discourse connective when an RST marker is being
    /// applied). Mirrors `render`'s snapshot/restore semantics.
    fn render_with_options(
        &self,
        session: &mut Session,
        key: &str,
        context: impl IntoContext,
        options: RenderOptions,
    ) -> Result<String, ProsaicError> {
        let all_alternatives = self
            .templates
            .get(key)
            .ok_or_else(|| ProsaicError::UnknownTemplate(key.to_string()))?;
        let context = context.into_context();

        let snapshot = session.clone();
        match RenderCtx::new(self, session).render_tx_with_options(
            key,
            all_alternatives,
            &context,
            options,
        ) {
            Ok(output) => {
                // Update temporal anchor after a successful render so that
                // render errors don't corrupt state. The anchor is set whenever
                // the event context carries a `timestamp` slot.
                #[cfg(feature = "time")]
                if let Some(Value::Number(ts)) = context.get("timestamp") {
                    session.last_temporal_anchor = Some(*ts);
                }
                Ok(output)
            }
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
    ) -> Result<Vec<VariantScore>, ProsaicError> {
        let all = self
            .templates
            .get(key)
            .ok_or_else(|| ProsaicError::UnknownTemplate(key.to_string()))?;

        let ctx = context.into_context();
        // State is always restored by score_all_variants internally.
        RenderCtx::new(self, session).score_all_variants(key, all, &ctx)
    }

    /// Render a one-off template string (not registered) with the given context.
    ///
    /// Inline templates are **state-isolated**: they do not participate in
    /// discourse tracking (no connectives, no entity mentions, no list-style
    /// cycle advancement, no plural-focus flag changes) and do not consume
    /// template-variant / round-robin counters. The only side effect on the
    /// real session is that output words are recorded for repetition scoring,
    /// and only when the render succeeds — a failed inline render leaves the
    /// session exactly as it was before the call.
    ///
    /// Implementation: render into a cloned session and discard it. On
    /// success, record output words on the caller's session.
    pub fn render_inline(
        &self,
        session: &mut Session,
        source: &str,
        context: impl IntoContext,
    ) -> Result<String, ProsaicError> {
        let template = Template::parse(source)?;
        let context = context.into_context();

        // Render into a scratch session clone so any stateful pipes
        // (list-style cycle, plural `refer` mention_entity calls, etc.)
        // mutate the clone rather than the caller's session.
        let mut scratch = session.clone();
        let mut output = String::with_capacity(128);
        RenderCtx::new(self, &mut scratch).render_template_into(
            &mut output,
            "<inline>",
            &template,
            &context,
        )?;

        // Success: the only mutation allowed to escape is the repetition
        // scoring word history, so callers see inline output in anti-repeat
        // decisions for subsequent registered renders.
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
    /// use prosaic_core::{Context, Engine, Value};
    /// use prosaic_grammar_en::English;
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
    /// let mut session = prosaic_core::Session::new();
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
    ) -> Result<String, ProsaicError> {
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
                let sentence =
                    self.render_aggregated_subjects(session, events[i].0, &events[i..action_end])?;
                sentences.push(sentence);
                i = action_end;
                continue;
            }

            // Look for a gapping opportunity: same template key, different
            // subjects, AND incompatible non-subject context (different objects
            // or complements). Produces "Foo was moved to core, Bar to util,
            // and Baz to api." from three separate events.
            let gap_end = self.find_gapping_run(events, i);
            if gap_end > i + 1 {
                let mut rendered: Vec<String> = Vec::with_capacity(gap_end - i);
                for (key, ctx) in &events[i..gap_end] {
                    rendered.push(self.render(session, key, ctx)?);
                }
                if let Some(gapped) = reduce_gapping(&rendered) {
                    sentences.push(gapped);
                } else {
                    sentences.extend(rendered);
                }
                i = gap_end;
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

    /// Render a batch of events where each event carries an optional RST
    /// relation describing its rhetorical link to the preceding event.
    ///
    /// When a relation is present on `events[i]` (i ≥ 1), the corresponding
    /// discourse marker is prepended to that sentence instead of a plain
    /// space — e.g. "However, the class Foo was modified." — and the leading
    /// determiner of the rendered sentence is lowercased so the marker's
    /// capitalisation leads naturally.
    ///
    /// When **all** relations are `None`, this method delegates to
    /// [`Engine::render_batch`] so aggregation (clause-reduction,
    /// subject-aggregation) still applies.
    pub fn render_batch_with_relations(
        &self,
        session: &mut Session,
        events: &[(&str, Context, Option<crate::rst::RstRelation>)],
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
            // When an explicit RST marker is being applied, suppress the
            // engine's automatic discourse connective at the source —
            // otherwise `connective_history` advances and output words
            // include text ("Similarly,", "However,") that was never
            // emitted, subtly poisoning anti-repetition scoring and
            // later connective selection.
            let options = if i > 0 && relation.is_some() {
                RenderOptions {
                    suppress_auto_connective: true,
                }
            } else {
                RenderOptions::default()
            };
            let sentence = self.render_with_options(session, key, ctx, options)?;

            // If an RST marker was prepended AND the sentence starts with
            // a capitalised determiner, lowercase the first letter so the
            // marker's capitalisation leads naturally.
            if i > 0 && relation.is_some() {
                output.push_str(&lowercase_first_if_determiner(&sentence));
            } else {
                output.push_str(&sentence);
            }
        }

        Ok(output)
    }

    /// Find the end index (exclusive) of a run of consecutive events that
    /// share the same entity (name + entity_type) but potentially differ in
    /// template key. Used by clause-reduction aggregation to turn a series
    /// of same-subject sentences into one conjunction-reduced sentence.
    ///
    /// Returns `start + 1` if no multi-event run exists.
    fn find_same_entity_run(&self, events: &[(&str, Context)], start: usize) -> usize {
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
    ) -> Result<RenderExplanation, ProsaicError> {
        let all_alternatives = self
            .templates
            .get(key)
            .ok_or_else(|| ProsaicError::UnknownTemplate(key.to_string()))?;

        let context = context.into_context();
        let target_salience = self.context_salience(&context);
        let alternatives = filter_by_salience(all_alternatives, target_salience);

        // Pre-compute candidate scores for diagnostics when choose-best
        // would apply. Run in a snapshot/restore bubble so main session is
        // untouched until the real render below.
        let candidate_scores = {
            let allow_choose_best =
                matches!(self.variation, Variation::Seeded(_) | Variation::Random);
            let is_first = session.discourse.is_first_render();
            if !allow_choose_best || is_first || alternatives.len() < 2 {
                None
            } else {
                let mut scoring_session = session.clone();
                let snapshot = scoring_session.clone();
                let mut scored: Vec<f64> = Vec::with_capacity(alternatives.len());
                let mut scoring_failed = false;
                let mut scratch = String::with_capacity(128);
                for template in &alternatives {
                    scratch.clear();
                    match RenderCtx::new(self, &mut scoring_session).render_template_into(
                        &mut scratch,
                        key,
                        template,
                        &context,
                    ) {
                        Ok(()) => {
                            let score = scoring_session.discourse.repetition_score(&scratch);
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
        let centering_transition = session.discourse.last_transition();

        #[cfg(feature = "polish")]
        let length_split_applied = self
            .max_sentence_length
            .is_some_and(|max| output.chars().count() > max && output.contains(". "));
        #[cfg(not(feature = "polish"))]
        let length_split_applied = false;

        let connective = detect_leading_connective(&output);

        let list_style = session.discourse.last_list_style_used();
        let cleanup_stripped_tail = session.discourse.last_cleanup_stripped_tail();

        Ok(RenderExplanation {
            output,
            template_key: key.to_string(),
            variant_index,
            variant_source,
            salience: target_salience,
            candidate_scores,
            reference_form,
            connective,
            list_style,
            focus_is_plural,
            length_split_applied,
            cleanup_stripped_tail,
            centering_transition,
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
    /// **Errors are terminal.** If any render inside the run fails, the
    /// iterator yields `Some(Err(_))` exactly once and then returns `None`
    /// on every subsequent call. This keeps semantics predictable even
    /// when the failing event sits inside an aggregated, gapped, or
    /// same-entity run whose earlier sentences already mutated session
    /// state — replaying the run after partial success would compound
    /// pronoun / anti-repetition state in unsafe ways. If you need
    /// error-skipping behaviour, validate templates and contexts up
    /// front (e.g. with [`Engine::score_variants`]) rather than relying
    /// on the iterator to recover.
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
    fn find_same_action_run(&self, events: &[(&str, Context)], start: usize) -> usize {
        if start >= events.len() {
            return start;
        }

        let (first_key, ref first_ctx) = events[start];
        let first_name = entity_name_from_context(first_ctx);

        if first_name.is_none() {
            return start + 1;
        }

        let mut end = start + 1;
        let mut seen_names: crate::collections::HashSet<String> = crate::collections::new_set();
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

    /// Find the end index (exclusive) of a run of consecutive events that
    /// are candidates for gapping reduction:
    /// - Same template key as `events[start]`.
    /// - Each event has a distinct, extractable entity name.
    /// - Each event's context is **incompatible** with the first event's
    ///   context (if it were compatible, `find_same_action_run` would have
    ///   already grabbed it for subject-aggregation).
    ///
    /// Returns `start + 1` when no gapping opportunity exists.
    fn find_gapping_run(&self, events: &[(&str, Context)], start: usize) -> usize {
        if start >= events.len() {
            return start;
        }

        let (first_key, first_ctx) = (events[start].0, &events[start].1);
        let Some(first_name) = entity_name_from_context(first_ctx) else {
            return start + 1;
        };

        let mut end = start + 1;
        let mut seen: crate::collections::HashSet<String> = core::iter::once(first_name).collect();

        while end < events.len() {
            let (k, ctx) = (events[end].0, &events[end].1);
            if k != first_key {
                break;
            }
            let Some(name) = entity_name_from_context(ctx) else {
                break;
            };
            if seen.contains(&name) {
                break;
            }
            // Bail if the contexts are compatible — the aggregated-subjects
            // path must win in that case. We only gap incompatible contexts.
            if contexts_compatible_for_aggregation(first_ctx, ctx) {
                break;
            }
            seen.insert(name);
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
    ) -> Result<String, ProsaicError> {
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
                #[cfg(feature = "std")]
                {
                    let nanos = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .subsec_nanos() as usize;
                    nanos % count
                }
                #[cfg(not(feature = "std"))]
                {
                    let _ = count;
                    0
                }
            }
            // RoundRobin requires mutable state — callers that need RoundRobin
            // must go through RenderCtx::select_variant_index instead.
            Variation::RoundRobin => 0,
        }
    }

    /// Build a *Full form* reference. If the entity is in the registry,
    /// run the configured REG algorithm against registered entities of the
    /// same type and include distinguishing attributes as premodifiers. When
    /// the graph-based algorithm is configured and attributes alone do not
    /// disambiguate, one relation clause is appended as a postmodifier.
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
        // With the `reg` feature: look up by (type, name) and run the
        // selected REG algorithm. Without it: degrade gracefully to
        // "the <type> <name>" / just the name.
        #[cfg(feature = "reg")]
        let (attrs, relation): (Vec<String>, Option<(String, String)>) = {
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
            match self.reg_algorithm {
                RegAlgorithm::DaleReiter => (
                    distinguishing_attributes(&target, &self.entity_registry, &self.reg_preference),
                    None,
                ),
                RegAlgorithm::GraphBased => {
                    let desc = distinguishing_subgraph(
                        &target,
                        &self.entity_registry,
                        &self.reg_preference,
                    );
                    (desc.attributes, desc.relation)
                }
            }
        };

        #[cfg(not(feature = "reg"))]
        let (attrs, relation): (Vec<String>, Option<(String, String)>) = {
            if fallback_type.is_empty() {
                return name.to_string();
            }
            (Vec::new(), None)
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
        let base = if attrs.is_empty() {
            format!("the {lower_type} {name}")
        } else {
            format!("the {} {lower_type} {name}", attrs.join(" "))
        };

        // Append relation clause when the graph-based algorithm selected one.
        if let Some((label, target_name)) = relation {
            format!("{base} {label} {target_name}")
        } else {
            base
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
    /// use prosaic_core::{Context, Engine, Value};
    /// use prosaic_grammar_en::English;
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
                .chain(core::iter::once(remainder.trim()))
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
/// Detect a cycle in the partial graph reachable from `entry_name`.
///
/// Returns `Ok(())` if no cycle exists. Returns `Err(cycle)` on a cycle,
/// where `cycle` is the traversal path from the first cycling node back
/// to itself (e.g. `["a", "b", "a"]` for a `a → b → a` loop).
///
/// Unknown partial references (templates that reference a partial not yet
/// registered) are ignored — they are validated separately at render time.
fn detect_partial_cycle(
    partials: &HashMap<String, Template>,
    entry_name: &str,
) -> Result<(), Vec<String>> {
    // DFS with an on-stack path so we can return the offending cycle.
    let mut path: Vec<String> = Vec::new();
    let mut on_stack: HashSet<String> = new_set();
    let mut fully_explored: HashSet<String> = new_set();

    visit(
        partials,
        entry_name,
        &mut path,
        &mut on_stack,
        &mut fully_explored,
    )
}

fn visit(
    partials: &HashMap<String, Template>,
    name: &str,
    path: &mut Vec<String>,
    on_stack: &mut HashSet<String>,
    fully_explored: &mut HashSet<String>,
) -> Result<(), Vec<String>> {
    if fully_explored.contains(name) {
        return Ok(());
    }
    if on_stack.contains(name) {
        // Build the cycle slice: from first occurrence of `name` in path
        // through the tail, plus `name` again to close the loop visibly.
        let start = path.iter().position(|n| n == name).unwrap_or(0);
        let mut cycle: Vec<String> = path[start..].to_vec();
        cycle.push(name.to_string());
        return Err(cycle);
    }
    let template = match partials.get(name) {
        Some(t) => t,
        // Unknown partial — render-time concern, not a cycle. Skip silently
        // here so that registering a partial that points at a not-yet-declared
        // partial stays valid.
        None => return Ok(()),
    };

    path.push(name.to_string());
    on_stack.insert(name.to_string());

    for child in template.partial_names() {
        visit(partials, &child, path, on_stack, fully_explored)?;
    }

    on_stack.remove(name);
    path.pop();
    fully_explored.insert(name.to_string());
    Ok(())
}

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
        let without_conn_str: &str = &without_conn;
        let body = without_conn_str.trim_end_matches(['.', '!', '?']);

        // Try pronoun form first ("It was …" / "it was …").
        let (aux, predicate) = match strip_it_aux_prefix(body) {
            Some(parsed) => parsed,
            None => {
                // Fallback: full-NP repetition — the head's subject+aux
                // prefix appears verbatim in the follower. This covers the
                // case where Centering Rule 1 demoted the pronoun (e.g.
                // after a session reset or long entity gap). FCR Phase 2.
                let remainder = strip_head_subject_prefix(body, head_subject_aux)?;
                if remainder.is_empty() {
                    return None;
                }
                // aux is implicitly head_aux since we matched head_subject_aux.
                (head_aux, remainder)
            }
        };
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

// ── Gapping (ELLEIPO) ──────────────────────────────────────────────────────

/// Split a rendered sentence into its subject word and the remaining tokens.
///
/// The "subject" is the first word that precedes a known auxiliary verb.
/// The returned `rest_tokens` **include** the auxiliary and everything
/// after it, so the longest-common-prefix search operates on the full
/// post-subject span.
///
/// Returns `None` when no auxiliary is found (can't gap safely).
fn split_subject_and_rest(s: &str) -> Option<(&str, Vec<&str>)> {
    for aux in AUX_PREFIXES {
        let marker = format!(" {aux} ");
        if let Some(pos) = s.find(&marker) {
            let subject = &s[..pos];
            // Skip leading space so rest starts at the aux word.
            let rest_str = &s[pos + 1..];
            let rest_tokens: Vec<&str> = rest_str.split_whitespace().collect();
            return Some((subject, rest_tokens));
        }
    }
    None
}

/// Longest common prefix length across the `rest_tokens` vectors in
/// `parsed`.  Returns 0 when `parsed` is empty.
fn longest_common_prefix_len(parsed: &[(&str, Vec<&str>)]) -> usize {
    if parsed.is_empty() {
        return 0;
    }
    let min_len = parsed.iter().map(|(_, t)| t.len()).min().unwrap_or(0);
    for i in 0..min_len {
        let candidate = parsed[0].1[i];
        if !parsed.iter().all(|(_, t)| t[i] == candidate) {
            return i;
        }
    }
    min_len
}

/// Attempt gapping reduction across a run of same-template renders where
/// every event shares the same verb anchor but differs in object/complement.
///
/// Example input sentences:
/// ```text
/// ["Foo was moved to core", "Bar was moved to util", "Baz was moved to api"]
/// ```
/// Produces:
/// ```text
/// "Foo was moved to core, Bar to util, and Baz to api."
/// ```
///
/// Returns `None` and leaves the caller to emit the sentences separately
/// when any guard fires: anchor too short, duplicate subjects, empty
/// divergent suffix, or embedded subordinate clause.
fn reduce_gapping(sentences: &[String]) -> Option<String> {
    if sentences.len() < 2 {
        return None;
    }

    // No embedded clauses in any sentence.
    if sentences.iter().any(|s| predicate_has_embedded_clause(s)) {
        return None;
    }

    // Strip trailing punctuation and leading connectives, then split each
    // sentence into (subject_word, rest_tokens).
    let parsed: Vec<(&str, Vec<&str>)> = sentences
        .iter()
        .map(|s| {
            let trimmed = s.trim_end();
            let stripped = strip_leading_connective(trimmed.trim_end_matches(['.', '!', '?']));
            // SAFETY: the Cow borrows from `trimmed` which lives as long as
            // this closure scope — but we need to return `&str` referencing
            // the original `s`. We compute the byte offset instead.
            let _ = stripped; // keep for borrow-checker
            // Re-derive without Cow: strip connective from trimmed-punctuation slice.
            let body = trimmed.trim_end_matches(['.', '!', '?']);
            let body_stripped: &str = {
                const CONNECTIVES: &[&str] = &[
                    "Additionally,",
                    "Furthermore,",
                    "Similarly,",
                    "Likewise,",
                    "Meanwhile,",
                    "However,",
                    "On the other hand,",
                ];
                let mut result = body;
                for conn in CONNECTIVES {
                    if let Some(rest) = body.strip_prefix(conn) {
                        result = rest.trim_start();
                        break;
                    }
                }
                // "It also was …" → "It was …" can't be represented as a
                // plain &str rewrite without allocation; treat as no-strip.
                result
            };
            split_subject_and_rest(body_stripped)
        })
        .collect::<Option<Vec<_>>>()?;

    // Subjects must all be distinct.
    {
        let mut seen: crate::collections::HashSet<&str> = crate::collections::new_set();
        for (subj, _) in &parsed {
            if !seen.insert(*subj) {
                return None;
            }
        }
    }

    // Longest common prefix across all rest_tokens vectors = the raw anchor.
    let raw_anchor_len = longest_common_prefix_len(&parsed);

    // Trim trailing prepositions from the anchor so that "was moved to"
    // becomes "was moved" — we want to gap the verbal complex only, keeping
    // any preposition together with the divergent complement.
    // E.g. "Foo was moved to core" + "Bar was moved to util"
    //   → anchor = "was moved", suffixes = "to core" / "to util"
    //   → "Foo was moved to core, and Bar to util."
    const PREPOSITIONS: &[&str] = &[
        "to", "from", "at", "in", "on", "by", "for", "with", "into", "onto", "out", "off", "over",
        "under", "above", "below", "through", "across", "against", "along", "around", "behind",
        "beside", "between", "during", "inside", "outside", "toward", "towards", "upon", "within",
        "without",
    ];
    let anchor_len = {
        let mut len = raw_anchor_len;
        while len > 0 && PREPOSITIONS.contains(&parsed[0].1[len - 1]) {
            len -= 1;
        }
        len
    };

    // Anchor must be at least 2 tokens (e.g. "was moved") to be meaningful.
    if anchor_len < 2 {
        return None;
    }

    // Every divergent suffix must be non-empty (something to gap into).
    if parsed.iter().any(|(_, toks)| toks.len() <= anchor_len) {
        return None;
    }

    let anchor = parsed[0].1[..anchor_len].join(" ");

    // Divergent suffixes (the "objects").
    let suffixes: Vec<String> = parsed
        .iter()
        .map(|(_, toks)| toks[anchor_len..].join(" "))
        .collect();

    // Helper to capitalize the first letter of a subject that the discourse
    // system may have lowercased when prepending a connective.
    let capitalize = |s: &str| -> String {
        let mut cs = s.chars();
        match cs.next() {
            None => String::new(),
            Some(c) => c.to_uppercase().collect::<String>() + cs.as_str(),
        }
    };

    // First full sentence: "Foo was moved to core"
    let first = format!("{} {} {}", capitalize(parsed[0].0), anchor, suffixes[0]);
    // Follower fragments: "Bar to util", "Baz to api"
    let tail: Vec<String> = parsed
        .iter()
        .skip(1)
        .zip(suffixes.iter().skip(1))
        .map(|((subj, _), suf)| format!("{} {suf}", capitalize(subj)))
        .collect();

    let joined = match tail.len() {
        1 => format!("{first}, and {}", tail[0]),
        _ => {
            let (last, rest) = tail.split_last().unwrap();
            format!("{first}, {}, and {last}", rest.join(", "))
        }
    };

    Some(format!("{joined}."))
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
/// original string as `Borrowed` when none of the known connectives
/// match, or an `Owned` rewrite for connectives that require synthesis
/// (currently only `"It also"`, which becomes `"It …"`).
fn strip_leading_connective(s: &str) -> alloc::borrow::Cow<'_, str> {
    const CONNECTIVES: &[&str] = &[
        "Additionally,",
        "Furthermore,",
        "Similarly,",
        "Likewise,",
        "Meanwhile,",
        "However,",
        "On the other hand,",
    ];

    for conn in CONNECTIVES {
        if let Some(rest) = s.strip_prefix(conn) {
            return alloc::borrow::Cow::Borrowed(rest.trim_start());
        }
    }

    // "It also was modified" — rewrite to "It was modified" so the
    // pronoun+aux matcher can find its prefix. This requires an
    // allocation because we are synthesising a new prefix.
    if let Some(rest) = s.strip_prefix("It also ") {
        return alloc::borrow::Cow::Owned(format!("It {}", rest.trim_start()));
    }

    alloc::borrow::Cow::Borrowed(s)
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

/// If `body` begins with `subject_aux` followed by a space, return the
/// remaining predicate. Used as a fallback path in
/// [`reduce_same_entity_clauses`] when the pronoun matcher declines — e.g.
/// when Centering Rule 1 demoted the follower to a full NP or a session
/// reset broke the pronoun chain.
///
/// The match is tried both verbatim and with the first character of
/// `subject_aux` lowercased. The discourse system lowercases the first
/// letter of a rendered sentence when it prepends a comma-style connective
/// (`"Additionally, the class X was …"`), so the verbatim capital-T match
/// would otherwise fail in that path.
///
/// Example: `body = "the class Foo was modified"`, `subject_aux = "The class Foo was"` →
/// returns `Some("modified")`.
fn strip_head_subject_prefix<'a>(body: &'a str, subject_aux: &str) -> Option<&'a str> {
    let with_space = format!("{subject_aux} ");
    if let Some(rest) = body.strip_prefix(with_space.as_str()) {
        return Some(rest.trim_start());
    }
    // The discourse system lowercases the first char of the sentence when
    // prepending a comma-style connective. Try the lowercase-first variant.
    let mut lowercased = subject_aux.to_string();
    if let Some(first) = lowercased.chars().next()
        && first.is_uppercase()
    {
        let first_len = first.len_utf8();
        let lower: String = first.to_lowercase().collect();
        lowercased.replace_range(0..first_len, &lower);
        let with_space_lower = format!("{lowercased} ");
        if let Some(rest) = body.strip_prefix(with_space_lower.as_str()) {
            return Some(rest.trim_start());
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
///
/// Returns `true` when the orphan-tail pass removed any dangling tail
/// words — surfaced via [`RenderExplanation::cleanup_stripped_tail`].
fn cleanup_artifacts_in_place(output: &mut String, strictness: Strictness) -> bool {
    collapse_and_tidy_in_place(output);

    if strictness == Strictness::Silent {
        strip_dangling_tail_words_in_place(output)
    } else {
        false
    }
}

/// Collapse multi-space runs, strip whitespace before closing punctuation,
/// and trim outer whitespace — mutates in place using a single scratch buffer swap.
fn collapse_and_tidy_in_place(output: &mut String) {
    // First pass: collapse whitespace runs, trim leading whitespace, and
    // strip space before closing punctuation — all in one scan.
    let mut scratch = String::with_capacity(output.len());
    let mut last_was_space = false;
    let mut started = false;

    let chars: Vec<char> = output.chars().collect();
    let len = chars.len();
    let mut i = 0;
    while i < len {
        let c = chars[i];
        if c.is_whitespace() {
            if started {
                last_was_space = true;
            }
        } else {
            // If there's a pending space, only emit it if the next non-space
            // char is not a closing-punctuation character.
            if last_was_space && !matches!(c, ',' | '.' | '!' | '?' | ':' | ';' | ')' | ']') {
                scratch.push(' ');
            }
            scratch.push(c);
            last_was_space = false;
            started = true;
        }
        i += 1;
    }

    core::mem::swap(output, &mut scratch);
}

/// Words that are almost always followed by an argument — if they're
/// stranded at the very end of an output (optionally before terminal
/// punctuation), the argument must have been swallowed by Silent mode
/// and we strip the orphan.
const ORPHAN_TAIL_WORDS: &[&str] = &[
    // Prepositions taking an object
    "by", "to", "from", "in", "on", "at", "of", "with", "for", "into", "onto", "upon", "about",
    "between", "among", "through", "across",
    // Coordinating & correlative words that need another clause
    "and", "or", "but", "nor", "yet", // Subordinating words that need a clause
    "because", "since", "while", "when", "where", "whether", "unless", "until", "than",
];

/// Strip trailing words that were left orphaned by omitted slots. Repeats
/// until no more matching tails remain — handles chained gaps like
/// `"modified by in"`.
///
/// Returns `true` if any tail word was stripped.
fn strip_dangling_tail_words_in_place(output: &mut String) -> bool {
    let mut stripped_any = false;
    loop {
        // Consider any trailing punctuation separately — we'll preserve it.
        let (body, _) = split_trailing_punct(output);
        let body_len = body.len();
        let trimmed_body = body.trim_end();

        // Grab the last word
        let last_word_start = match trimmed_body.rfind(char::is_whitespace) {
            Some(idx) => idx + 1,
            None => {
                // Single word output — don't touch.
                return stripped_any;
            }
        };
        let last_word = &trimmed_body[last_word_start..];
        let last_word_lower = last_word.to_lowercase();

        if ORPHAN_TAIL_WORDS.contains(&last_word_lower.as_str()) {
            let new_body_end = trimmed_body[..last_word_start].trim_end().len();
            if new_body_end == 0 {
                // The whole output was orphans — bail out to avoid erasing content.
                return stripped_any;
            }
            // Build the new string: new_body + tail_punct
            // tail_punct starts at byte offset body_len in `output`
            let tail_punct_owned = output[body_len..].to_string();
            output.truncate(new_body_end);
            output.push_str(&tail_punct_owned);
            stripped_any = true;
            continue;
        }

        return stripped_any;
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
fn terminate_sentence_in_place(output: &mut String) {
    let trimmed_end = output.trim_end();
    if trimmed_end.is_empty() {
        return;
    }

    // Already ends with sentence-ending punctuation? Leave alone.
    let last = trimmed_end.chars().last().unwrap();
    if matches!(last, '.' | '!' | '?') {
        return;
    }

    // Looks like a fragment (doesn't start with capital, or is short)?
    let first = trimmed_end.chars().next().unwrap();
    if !first.is_uppercase() {
        return;
    }

    // Count words — single words or very short outputs are likely fragments
    let word_count = trimmed_end.split_whitespace().count();
    if word_count < 3 {
        return;
    }

    // Trim trailing whitespace, then append period.
    let trimmed_len = output.trim_end().len();
    output.truncate(trimmed_len);
    output.push('.');
}

/// Check if a template's first segment is a `refer` pipe, meaning the
/// rendered output may start with a lowercase word that needs capitalization.
fn starts_with_refer_pipe(template: &Template) -> bool {
    match template.segments.first() {
        Some(Segment::Slot { pipes, .. }) => pipes.iter().any(|p| p.name == "refer"),
        _ => false,
    }
}

fn capitalize_first_in_place(output: &mut String) {
    let first = match output.chars().next() {
        Some(c) if c.is_lowercase() => c,
        _ => return,
    };
    let first_len = first.len_utf8();
    let upper: String = first.to_uppercase().collect();
    output.replace_range(0..first_len, &upper);
}

fn lowercase_first_in_place(output: &mut String) {
    let first = match output.chars().next() {
        Some(c) if c.is_uppercase() => c,
        _ => return,
    };
    let first_len = first.len_utf8();
    let lower: String = first.to_lowercase().collect();
    output.replace_range(0..first_len, &lower);
}

/// If `s` starts with a common determiner or article (e.g. "The ", "A ", "El "),
/// lowercase the first character and return the result. Otherwise return `s`
/// unchanged. Used by [`Engine::render_batch_with_relations`] so that a
/// discourse marker ("Furthermore, ") naturally leads a sentence that would
/// otherwise start with a capital article ("The class Foo …" →
/// "Furthermore, the class Foo …").
fn lowercase_first_if_determiner(s: &str) -> String {
    let first_word_end = s.find(char::is_whitespace).unwrap_or(s.len());
    let first = &s[..first_word_end];
    const DETERMINERS: &[&str] = &[
        "The", "A", "An", // English
        "El", "La", "Los", "Las", "Un", "Una", // Spanish
        "Der", "Die", "Das", // German
    ];
    if DETERMINERS.contains(&first) {
        let mut result = String::with_capacity(s.len());
        let mut chars = first.chars();
        if let Some(c) = chars.next() {
            result.extend(c.to_lowercase());
        }
        result.push_str(chars.as_str());
        result.push_str(&s[first_word_end..]);
        result
    } else {
        s.to_string()
    }
}

/// Try to replace "The {type} {name} was ..." with a connective like "It also was ..."
fn prepend_replacing_subject_in_place(
    output: &mut String,
    connective: &str,
    entity_name: Option<&str>,
) {
    // Case 1: full NP subject "The <type> <name> …" — strip the NP and
    // replace with the connective. Only safe when the entity name is a
    // single token: the NP boundary is "The " + one type word + one name
    // word. Multi-word names (e.g. "Login flow") make the boundary
    // ambiguous from the rendered string alone, so we fall through to the
    // comma-style prepending instead of chopping mid-name.
    let name_is_single_token = entity_name
        .map(|n| !n.trim().is_empty() && !n.contains(char::is_whitespace))
        .unwrap_or(false);

    if name_is_single_token && let Some(rest) = output.strip_prefix("The ") {
        // Skip entity_type and name (two words)
        let words: Vec<&str> = rest.splitn(3, ' ').collect();
        if words.len() >= 3 {
            let tail = words[2..].join(" ");
            let mut buf = String::with_capacity(connective.len() + 1 + tail.len());
            buf.push_str(connective);
            buf.push(' ');
            buf.push_str(&tail);
            core::mem::swap(output, &mut buf);
            return;
        }
    }

    // Case 2: the render already emitted a pronoun subject ("it was …").
    // Replace the leading "it " so the connective doesn't duplicate the
    // subject — e.g. "It also " + "it was archived" → "It also was archived".
    if let Some(rest) = output.strip_prefix("it ") {
        let mut buf = String::with_capacity(connective.len() + 1 + rest.len());
        buf.push_str(connective);
        buf.push(' ');
        buf.push_str(rest);
        core::mem::swap(output, &mut buf);
        return;
    }

    // Fallback: lowercase the first char then prepend the connective.
    lowercase_first_in_place(output);
    let mut buf = String::with_capacity(connective.len() + 1 + output.len());
    buf.push_str(connective);
    buf.push(' ');
    buf.push_str(output);
    core::mem::swap(output, &mut buf);
}

/// Filter templates to those matching the target salience level.
///
/// Fallback order:
/// 1. Templates registered at the exact target salience.
/// 2. Templates registered at Medium salience (the default).
/// 3. All registered templates (degrades gracefully).
fn filter_by_salience<'a>(
    alternatives: &'a [SalientTemplate],
    target: Salience,
) -> Vec<&'a Template> {
    let exact: Vec<&'a Template> = alternatives
        .iter()
        .filter(|(s, _)| *s == target)
        .map(|(_, t)| t)
        .collect();
    if !exact.is_empty() {
        return exact;
    }

    let medium: Vec<&'a Template> = alternatives
        .iter()
        .filter(|(s, _)| *s == Salience::Medium)
        .map(|(_, t)| t)
        .collect();
    if !medium.is_empty() {
        return medium;
    }

    alternatives.iter().map(|(_, t)| t).collect()
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
        // An entity is truthy if its name is non-empty.
        Some(Value::Entity { name, .. }) => !name.is_empty(),
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
    let a_keys: Vec<&String> = a
        .keys()
        .filter(|k| !entity_keys.contains(&k.as_str()))
        .collect();
    let b_keys: Vec<&String> = b
        .keys()
        .filter(|k| !entity_keys.contains(&k.as_str()))
        .collect();

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
    let verb_replacements = &[(" was ", " were "), (" has ", " have "), (" is ", " are ")];
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

/// Parse the argument string of a `|choose` pipe into an ordered list of
/// `(key, value)` pairs.
///
/// Grammar: `pair ("," pair)*` where `pair ::= key "=" value`.
/// Keys and values are trimmed of surrounding whitespace.
/// Empty segments (e.g. trailing comma) are silently skipped.
/// A pair missing `=` returns [`ProsaicError::InvalidPipe`].
/// A pair whose key is empty after trimming returns [`ProsaicError::InvalidPipe`].
fn parse_choose_pairs(arg: &str) -> Result<Vec<(String, String)>, ProsaicError> {
    let mut out = Vec::new();
    for raw_pair in arg.split(',') {
        let pair = raw_pair.trim();
        if pair.is_empty() {
            continue;
        }
        let eq = pair.find('=').ok_or_else(|| ProsaicError::InvalidPipe {
            pipe: "choose".to_string(),
            reason: format!("pair `{pair}` is missing `=` separator"),
        })?;
        let key = pair[..eq].trim().to_string();
        let value = pair[eq + 1..].trim().to_string();
        if key.is_empty() {
            return Err(ProsaicError::InvalidPipe {
                pipe: "choose".to_string(),
                reason: format!("pair `{pair}` has empty key"),
            });
        }
        out.push((key, value));
    }
    Ok(out)
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

    // ── Template existence ──────────────────────────────────────────────────

    #[test]
    fn has_template_returns_true_for_registered() {
        let mut engine = test_engine();
        engine.register_template("t", "hello").unwrap();
        assert!(engine.has_template("t"));
        assert!(!engine.has_template("nope"));
    }

    // ── Basic rendering (backward compatibility) ─────────────────────────

    #[test]
    fn render_simple_substitution() {
        let mut engine = test_engine();
        engine.register_template("greet", "Hello {name}!").unwrap();

        let mut ctx = Context::new();
        ctx.insert("name", Value::String("world".into()));

        let mut session = test_session();
        assert_eq!(
            engine.render(&mut session, "greet", &ctx).unwrap(),
            "Hello world!"
        );
    }

    #[test]
    fn render_missing_slot_strict() {
        let mut engine = test_engine();
        engine.register_template("greet", "Hello {name}!").unwrap();
        let ctx = Context::new();

        let mut session = test_session();
        let result = engine.render(&mut session, "greet", &ctx);
        assert!(matches!(result, Err(ProsaicError::MissingSlot { .. })));
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
        assert_eq!(
            engine.render(&mut session, "greet", &ctx).unwrap(),
            "Hello!"
        );
    }

    #[test]
    fn render_unknown_template() {
        let engine = test_engine();
        let ctx = Context::new();

        let mut session = test_session();
        let result = engine.render(&mut session, "nonexistent", &ctx);
        assert!(matches!(result, Err(ProsaicError::UnknownTemplate(_))));
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
        assert_eq!(
            engine.render(&mut session, "count", &ctx).unwrap(),
            "1 item"
        );

        session.reset();
        ctx.insert("n", Value::Number(5));
        assert_eq!(
            engine.render(&mut session, "count", &ctx).unwrap(),
            "5 items"
        );
    }

    #[test]
    fn render_article_pipe() {
        let mut engine = test_engine();
        engine.register_template("a", "{thing|article}").unwrap();

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
        engine.register_template("list", "{items|join}").unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert(
            "items",
            Value::List(vec!["a".into(), "b".into(), "c".into()]),
        );
        assert_eq!(
            engine.render(&mut session, "list", &ctx).unwrap(),
            "a, b, and c"
        );
    }

    #[test]
    fn render_join_or_pipe() {
        let mut engine = test_engine();
        engine.register_template("list", "{items|join:or}").unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert(
            "items",
            Value::List(vec!["a".into(), "b".into(), "c".into()]),
        );
        assert_eq!(
            engine.render(&mut session, "list", &ctx).unwrap(),
            "a, b, or c"
        );
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
        assert_eq!(
            engine.render(&mut session, "t", &ctx).unwrap(),
            "[a, b, and 3 more]"
        );
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
            engine
                .render_inline(&mut session, "Hello {name}!", &ctx)
                .unwrap(),
            "Hello world!"
        );
    }

    #[test]
    fn render_inline_does_not_advance_list_style_cycle() {
        // Regression: `{items|join}` advances session.discourse.last_list_style.
        // An inline render must not leak that mutation into a subsequent
        // registered render — otherwise the caller gets a different list
        // style than it would without the inline render.
        let mut engine = test_engine();
        engine.register_template("t", "{items|join}").unwrap();

        let mut s_ref = test_session();
        let mut ctx = Context::new();
        ctx.insert(
            "items",
            Value::List(vec!["a".into(), "b".into(), "c".into()]),
        );

        // Do a reference render — captures which style cycle picks first.
        let ref_out = engine.render(&mut s_ref, "t", &ctx).unwrap();

        // Now: on a FRESH session, do an inline `{|join}` first, then a
        // registered render. If inline leaked the cycle, the registered
        // render would pick a different style than the reference.
        let mut s_test = test_session();
        engine
            .render_inline(&mut s_test, "{items|join}", &ctx)
            .unwrap();
        let after_inline = engine.render(&mut s_test, "t", &ctx).unwrap();

        assert_eq!(
            ref_out, after_inline,
            "inline render leaked list-style cycle into a later registered render"
        );
    }

    #[test]
    fn render_inline_failure_leaves_session_unchanged() {
        // A failing inline render must not record output words or any
        // other partial mutation on the caller's session.
        let engine = test_engine();
        let mut session = test_session();

        // Before the failure, snapshot the discourse state for comparison.
        let snapshot = session.clone();

        // Strict mode: missing slot → render_template_into returns Err
        // before it would otherwise record output words.
        let result = engine.render_inline(&mut session, "Hello {nope}!", Context::new());
        assert!(result.is_err(), "expected missing-slot error");

        // render_index and focus_entity should match pre-call state.
        // Using visible accessors; if new state is added the snapshot
        // Clone catches it transitively via other tests.
        assert_eq!(
            session.discourse.focus_is_plural(),
            snapshot.discourse.focus_is_plural()
        );
    }

    #[test]
    fn render_inline_does_not_mention_entities_via_plural_refer() {
        // Plural `refer` would ordinarily call mention_entity for each
        // name in the list. Inline renders must not leave those entity
        // mentions on the caller's session.
        let mut engine = test_engine();
        engine.register_template("t", "{name|refer}").unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("class".into()));
        ctx.insert("name", Value::String("Alpha".into()));

        // Inline render that would mention Alpha.
        let _ = engine
            .render_inline(&mut session, "{name|refer}", &ctx)
            .unwrap();

        // Now a registered `refer` on the same name should still use
        // Full form (no prior in-session mention from the inline render).
        let out = engine.render(&mut session, "t", &ctx).unwrap();
        assert!(
            out.contains("The class Alpha"),
            "expected Full form (no leaked entity mention); got: {out}"
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
        assert!(matches!(result, Err(ProsaicError::InvalidPipe { .. })));
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
        engine
            .register_template("t", "alpha distinct tokens")
            .unwrap();
        engine
            .register_template("t", "beta different tokens")
            .unwrap();
        engine
            .register_template("t", "gamma unique tokens")
            .unwrap();

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
        assert!(
            unique.len() >= 3,
            "Expected at least 3 unique list styles, got {}: {:?}",
            unique.len(),
            results
        );
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
        assert!(
            result.starts_with('[') && result.ends_with(']'),
            "Expected bracketed format, got: {result}"
        );
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
        engine
            .register_template("other", "Something else happened")
            .unwrap();

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
        let exp = engine
            .render_explained(&mut session, "t", Context::new())
            .unwrap();
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
    fn explain_reports_centering_transition() {
        let mut engine = test_engine();
        engine
            .register_template("t", "{name|refer} was modified")
            .unwrap();
        let mut s = test_session();

        let mut c = Context::new();
        c.insert("entity_type", Value::String("class".into()));
        c.insert("name", Value::String("Foo".into()));

        // First render: no prior Cb → NoCb.
        let e1 = engine.render_explained(&mut s, "t", &c).unwrap();
        assert_eq!(e1.centering_transition, Transition::NoCb);

        // Second render of same entity: Cb == prev_Cb (Foo) AND Cb == Cp (Foo) → Continue.
        let e2 = engine.render_explained(&mut s, "t", &c).unwrap();
        assert_eq!(e2.centering_transition, Transition::Continue);

        // Third render of a new entity: introduces Bar for the first time.
        // previous_cf = [{Foo,0}], current_cf = [{Bar,0}]. No overlap.
        // Bar is new (mention_count == 1), fallback → new_cb = Foo (previous_focus).
        // classify_transition(Foo, Foo, Bar) → Retain.
        let mut c2 = Context::new();
        c2.insert("entity_type", Value::String("class".into()));
        c2.insert("name", Value::String("Bar".into()));
        let e3 = engine.render_explained(&mut s, "t", &c2).unwrap();
        assert_eq!(e3.centering_transition, Transition::Retain);
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

    #[test]
    fn render_explained_reports_list_style_when_join_fires() {
        let mut engine = test_engine();
        engine
            .register_template("list", "{items|join:bracketed}")
            .unwrap();
        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert(
            "items",
            Value::List(vec!["a".into(), "b".into(), "c".into()]),
        );
        let exp = engine.render_explained(&mut session, "list", &ctx).unwrap();
        assert_eq!(
            exp.list_style,
            Some(ListStyle::Bracketed),
            "render_explained should report the forced list style; got: {:?}",
            exp.list_style
        );
    }

    #[test]
    fn render_explained_list_style_none_when_no_join_fired() {
        let mut engine = test_engine();
        engine
            .register_template("plain", "The {entity_type} {name} was renamed")
            .unwrap();
        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("class".into()));
        ctx.insert("name", Value::String("Foo".into()));
        let exp = engine
            .render_explained(&mut session, "plain", &ctx)
            .unwrap();
        assert_eq!(exp.list_style, None);
    }

    #[test]
    fn render_explained_reports_cleanup_stripped_tail_in_silent_mode() {
        // Silent strictness: omitted `location` slot leaves a dangling " in "
        // that the orphan-tail pass should strip, flipping
        // cleanup_stripped_tail to true.
        let mut engine = test_engine().strictness(Strictness::Silent);
        engine
            .register_template("add", "A new {entity_type} was added in {location}")
            .unwrap();
        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("class".into()));
        // location intentionally omitted
        let exp = engine.render_explained(&mut session, "add", &ctx).unwrap();
        assert!(
            exp.cleanup_stripped_tail,
            "Silent-mode render with dangling tail should report cleanup_stripped_tail=true; got output: {:?}",
            exp.output
        );
    }

    #[test]
    fn render_explained_cleanup_stripped_tail_false_for_clean_render() {
        let mut engine = test_engine();
        engine
            .register_template("plain", "The {entity_type} {name} was renamed")
            .unwrap();
        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("class".into()));
        ctx.insert("name", Value::String("Foo".into()));
        let exp = engine
            .render_explained(&mut session, "plain", &ctx)
            .unwrap();
        assert!(!exp.cleanup_stripped_tail);
    }

    // ── Streaming render iterator ────────────────────────────────────────

    #[test]
    fn render_iter_yields_one_sentence_per_event_when_no_aggregation() {
        let mut engine = test_engine();
        engine.register_template("a", "Alpha was seen").unwrap();
        engine.register_template("b", "Beta was found").unwrap();

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
    fn render_iter_error_is_terminal_for_single_events() {
        // Strict mode with a missing slot errors. After the error, the
        // iterator must report None, not replay the failing event.
        let mut engine = test_engine();
        engine
            .register_template("bad", "{missing_slot} was lost")
            .unwrap();

        let mut session = test_session();
        let events: Vec<(&str, Context)> = vec![("bad", Context::new())];
        let mut iter = engine.render_iter(&mut session, &events);
        let first = iter.next();
        assert!(matches!(first, Some(Err(_))));
        let second = iter.next();
        assert!(
            second.is_none(),
            "iterator must return None after a terminal error"
        );
    }

    #[test]
    fn render_iter_error_is_terminal_inside_aggregated_run() {
        // Two events with the same template key but the second one's
        // context is missing a slot the template references. The first
        // aggregated render call will fail — the iterator must end, not
        // replay the run.
        let mut engine = test_engine();
        engine
            .register_template("saw", "{name} saw {target}")
            .unwrap();

        let mut good = Context::new();
        good.insert("entity_type", Value::String("class".into()));
        good.insert("name", Value::String("Alpha".into()));
        good.insert("target", Value::String("X".into()));

        let mut bad = Context::new();
        bad.insert("entity_type", Value::String("class".into()));
        bad.insert("name", Value::String("Beta".into()));
        // target slot omitted → missing-slot error under Strict.

        let mut session = test_session();
        let events: Vec<(&str, Context)> = vec![("saw", good), ("saw", bad)];
        let mut iter = engine.render_iter(&mut session, &events);
        // The aggregation path renders both subjects at once; the missing
        // slot on the second context surfaces as an error.
        let first = iter.next();
        assert!(
            matches!(first, Some(Err(_))),
            "expected the aggregated render to fail; got: {first:?}"
        );
        assert!(
            iter.next().is_none(),
            "iterator must be terminal after an aggregated-run error"
        );
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
        let events: Vec<(&str, Context)> =
            vec![("renamed", ctx.clone()), ("modified", ctx.clone())];
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
        let scores = engine
            .score_variants(&mut session, "t", Context::new())
            .unwrap();
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
        let scores = engine
            .score_variants(&mut session, "t", Context::new())
            .unwrap();
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
        let _ = engine
            .score_variants(&mut session, "t", Context::new())
            .unwrap();
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
        assert!(matches!(result, Err(ProsaicError::UnknownTemplate(_))));
    }

    // ── Template partials ────────────────────────────────────────────────

    #[test]
    fn partial_expands_inline() {
        let mut engine = test_engine();
        engine
            .register_partial("tail", ", affecting {count} {count|pluralize:consumer}")
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
            .register_partial("tail", ", affecting {count} {count|pluralize:consumer}")
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
            Err(ProsaicError::TemplateParseError { .. })
        ));
    }

    #[test]
    fn direct_recursive_partial_is_rejected() {
        let mut engine = test_engine();
        let result = engine.register_partial("a", "{>a}");
        match result {
            Err(ProsaicError::RecursivePartial { cycle }) => {
                assert_eq!(cycle, vec!["a".to_string(), "a".to_string()]);
            }
            other => panic!("expected RecursivePartial, got {other:?}"),
        }
        // The partial must NOT be stored (otherwise future lookups reach
        // a cyclic definition).
        assert!(!engine.partials.contains_key("a"));
    }

    #[test]
    fn indirect_recursive_partial_is_rejected() {
        let mut engine = test_engine();
        // Register `a` referring to a not-yet-existing `b` — that's fine.
        engine.register_partial("a", "{>b}").unwrap();
        // Now registering `b` that refers back to `a` must fail with the
        // a -> b -> a cycle reported, and `b` must NOT be stored.
        let result = engine.register_partial("b", "{>a}");
        match result {
            Err(ProsaicError::RecursivePartial { cycle }) => {
                assert!(
                    cycle.contains(&"a".to_string()) && cycle.contains(&"b".to_string()),
                    "cycle should include both partials; got {cycle:?}"
                );
                // First and last entries should match (cycle closes).
                assert_eq!(cycle.first(), cycle.last());
            }
            other => panic!("expected RecursivePartial, got {other:?}"),
        }
        assert!(!engine.partials.contains_key("b"));
        // Partial `a` is still present (registered successfully earlier).
        assert!(engine.partials.contains_key("a"));
    }

    #[test]
    fn non_cyclic_partial_chain_is_accepted() {
        let mut engine = test_engine();
        engine.register_partial("inner", "-inner-").unwrap();
        engine.register_partial("middle", "[{>inner}]").unwrap();
        engine.register_partial("outer", "<{>middle}>").unwrap();
        engine
            .register_template("t", "prefix {>outer} suffix")
            .unwrap();

        let mut session = test_session();
        let out = engine.render(&mut session, "t", Context::new()).unwrap();
        assert!(out.contains("<[-inner-]>"), "got: {out}");
    }

    #[test]
    fn updating_partial_to_introduce_cycle_rolls_back() {
        let mut engine = test_engine();
        engine.register_partial("a", "literal-a").unwrap();
        engine.register_partial("b", "{>a}").unwrap();

        // Attempt to overwrite `a` with a reference to `b` — would form
        // a -> b -> a cycle and must be rejected. The previous body must
        // remain intact.
        let result = engine.register_partial("a", "{>b}");
        assert!(matches!(result, Err(ProsaicError::RecursivePartial { .. })));

        // Render via `a` — should still produce the prior literal body.
        engine.register_template("t", "see {>a} here").unwrap();
        let mut session = test_session();
        let out = engine.render(&mut session, "t", Context::new()).unwrap();
        assert!(
            out.contains("literal-a"),
            "expected prior partial body to be restored; got: {out}"
        );
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
        assert!(out.contains("This impacts 6 consumers"), "got: {out}");
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
        engine
            .register_template("t", "The class Foo {p|negated}")
            .unwrap();

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
        engine
            .register_template("t", "The class Foo {p|negated}")
            .unwrap();

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
        engine
            .register_template("t", "The class Foo {p|negated}")
            .unwrap();

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
            Err(ProsaicError::InvalidPipe { .. })
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
        assert_eq!(
            engine.render(&mut session, "t", &ctx).unwrap(),
            "the change"
        );
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
        engine
            .render(&mut session, "prime", Context::new())
            .unwrap();

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
        engine
            .render(&mut session, "prime", Context::new())
            .unwrap();
        session.reset();

        let mut ctx = Context::new();
        ctx.insert("noun", Value::String("change".into()));
        assert_eq!(
            engine.render(&mut session, "t", &ctx).unwrap(),
            "the change"
        );
    }

    // ── Quantify pipe ────────────────────────────────────────────────────

    #[test]
    fn quantify_pipe_natural_defaults() {
        // Uses the crate's own TestLang for number_to_words via
        // "<N>"-style stub. We still exercise the small-number spelling
        // path via QuantifyMode::Natural's language callback.
        let mut engine = test_engine();
        engine
            .register_template("t", "{n|quantify} consumer")
            .unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("n", Value::Number(0));
        assert_eq!(
            engine.render(&mut session, "t", &ctx).unwrap(),
            "no consumer"
        );
        session.reset();

        ctx.insert("n", Value::Number(1));
        assert_eq!(
            engine.render(&mut session, "t", &ctx).unwrap(),
            "a single consumer"
        );
        session.reset();

        ctx.insert("n", Value::Number(300));
        assert_eq!(
            engine.render(&mut session, "t", &ctx).unwrap(),
            "hundreds of consumer"
        );
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
        assert_eq!(
            engine.render(&mut session, "t", &ctx).unwrap(),
            "47 callers"
        );
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
        assert_eq!(
            engine.render(&mut session, "t", &ctx).unwrap(),
            "a few dependents"
        );
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
            Err(ProsaicError::InvalidPipe { .. })
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
        assert!(matches!(result, Err(ProsaicError::InvalidPipe { .. })));
    }

    // ── since_last pipe ──────────────────────────────────────────────────

    #[cfg(feature = "time")]
    #[test]
    fn since_last_first_event_falls_back_to_relative() {
        let now = 1_700_000_000;
        let mut engine = test_engine().reference_time(now);
        engine.register_template("t", "{ts|since_last}").unwrap();
        let mut s = Session::new();
        let mut ctx = Context::new();
        ctx.insert("ts", Value::Number(now - 3 * 86400)); // 3 days ago
        ctx.insert("timestamp", Value::Number(now - 3 * 86400));
        let out = engine.render(&mut s, "t", &ctx).unwrap();
        assert!(out.contains("3 days ago"), "got: {out}");
    }

    #[cfg(feature = "time")]
    #[test]
    fn since_last_subsequent_event_uses_anchor() {
        let now = 1_700_000_000;
        let mut engine = test_engine().reference_time(now);
        engine.register_template("t", "{ts|since_last}").unwrap();
        let mut s = Session::new();

        // First event sets the anchor.
        let mut c1 = Context::new();
        let t1 = now - 3 * 86400;
        c1.insert("ts", Value::Number(t1));
        c1.insert("timestamp", Value::Number(t1));
        engine.render(&mut s, "t", &c1).unwrap();

        // Second event, one day later.
        let mut c2 = Context::new();
        let t2 = t1 + 86400;
        c2.insert("ts", Value::Number(t2));
        c2.insert("timestamp", Value::Number(t2));
        let out = engine.render(&mut s, "t", &c2).unwrap();
        assert!(out.contains("the next day"), "got: {out}");
    }

    #[cfg(feature = "time")]
    #[test]
    fn since_last_survives_session_reset() {
        let now = 1_700_000_000;
        let mut engine = test_engine().reference_time(now);
        engine.register_template("t", "{ts|since_last}").unwrap();
        let mut s = Session::new();

        let mut c1 = Context::new();
        let t1 = now - 3 * 86400;
        c1.insert("ts", Value::Number(t1));
        c1.insert("timestamp", Value::Number(t1));
        engine.render(&mut s, "t", &c1).unwrap();

        s.reset(); // Reset discourse, but NOT temporal anchor.
        assert_eq!(s.last_temporal_anchor, Some(t1));

        let mut c2 = Context::new();
        let t2 = t1 + 86400;
        c2.insert("ts", Value::Number(t2));
        c2.insert("timestamp", Value::Number(t2));
        let out = engine.render(&mut s, "t", &c2).unwrap();
        assert!(out.contains("the next day"), "got: {out}");
    }

    #[cfg(feature = "time")]
    #[test]
    fn since_last_reset_temporal_restarts_narrative() {
        let now = 1_700_000_000;
        let mut engine = test_engine().reference_time(now);
        engine.register_template("t", "{ts|since_last}").unwrap();
        let mut s = Session::new();

        let mut c1 = Context::new();
        let t1 = now - 3 * 86400;
        c1.insert("ts", Value::Number(t1));
        c1.insert("timestamp", Value::Number(t1));
        engine.render(&mut s, "t", &c1).unwrap();
        s.reset_temporal();

        let mut c2 = Context::new();
        let t2 = t1 + 86400;
        c2.insert("ts", Value::Number(t2));
        c2.insert("timestamp", Value::Number(t2));
        let out = engine.render(&mut s, "t", &c2).unwrap();
        // now - t2 = 2 days ago (absolute fallback)
        assert!(out.contains("2 days ago"), "got: {out}");
    }

    #[cfg(feature = "time")]
    #[test]
    fn since_last_anchor_set_after_successful_render() {
        let now = 1_700_000_000;
        let mut engine = test_engine().reference_time(now);
        engine.register_template("t", "{ts|since_last}").unwrap();
        let mut s = Session::new();
        assert_eq!(s.last_temporal_anchor, None);

        let mut ctx = Context::new();
        let ts = now - 86400;
        ctx.insert("ts", Value::Number(ts));
        ctx.insert("timestamp", Value::Number(ts));
        engine.render(&mut s, "t", &ctx).unwrap();
        assert_eq!(s.last_temporal_anchor, Some(ts));
    }

    #[cfg(feature = "time")]
    #[test]
    fn since_last_rejects_non_numeric() {
        let mut engine = test_engine().reference_time(1_700_000_000);
        engine.register_template("t", "{x|since_last}").unwrap();
        let mut s = Session::new();
        let mut ctx = Context::new();
        ctx.insert("x", Value::String("not a number".into()));
        let result = engine.render(&mut s, "t", &ctx);
        assert!(matches!(result, Err(ProsaicError::InvalidPipe { .. })));
    }

    // ── Synonym pipe (elegant variation) ─────────────────────────────────

    #[test]
    fn syn_pipe_passes_through_unregistered_words() {
        let mut engine = test_engine();
        engine.register_template("t", "{word|syn}").unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("word", Value::String("unregistered".into()));
        assert_eq!(
            engine.render(&mut session, "t", &ctx).unwrap(),
            "unregistered"
        );
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
        let reduced = reduce_same_entity_clauses(&["The class Foo was renamed.".to_string()]);
        assert!(reduced.is_none());
    }

    #[test]
    fn reduce_accepts_full_np_repetition_same_entity() {
        // Full-NP repetition is now accepted by FCR Phase 2: the follower
        // repeats the head's subject+aux prefix verbatim (e.g. after a
        // session reset that demoted the pronoun). The two predicates fuse.
        let reduced = reduce_same_entity_clauses(&[
            "The class Foo was renamed.".to_string(),
            "The class Foo was modified.".to_string(),
        ]);
        assert_eq!(
            reduced.as_deref(),
            Some("The class Foo was renamed and modified.")
        );
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

    // ── FCR: "It also" connective handling (Phase 1) ────────────────────

    #[test]
    fn reduce_accepts_it_also_connective() {
        // "It also was modified" should strip to "It was modified" so the
        // pronoun+aux matcher accepts it — FCR Phase 1.
        let reduced = reduce_same_entity_clauses(&[
            "The class UserService was renamed.".to_string(),
            "It also was modified.".to_string(),
        ]);
        assert_eq!(
            reduced.as_deref(),
            Some("The class UserService was renamed and modified.")
        );
    }

    #[test]
    fn prepend_replacing_subject_single_word_name_strips_np() {
        // With a single-token name the "The <type> <name> " prefix is
        // unambiguous and the connective replaces it.
        let mut out = String::from("The class Foo was modified");
        prepend_replacing_subject_in_place(&mut out, "It also", Some("Foo"));
        assert_eq!(out, "It also was modified");
    }

    #[test]
    fn prepend_replacing_subject_multiword_name_falls_back() {
        // With a multi-word name the NP boundary is ambiguous from the
        // rendered string alone; we must NOT chop mid-name. Fall back to
        // the lowercased-first-char + comma-style prepending path.
        let mut out = String::from("The feature Login flow was modified");
        prepend_replacing_subject_in_place(&mut out, "It also", Some("Login flow"));
        // Fallback lowercases leading letter then prepends connective + space.
        assert_eq!(out, "It also the feature Login flow was modified");
        assert!(
            !out.contains("flow was modified") || out.starts_with("It also the feature"),
            "must not chop 'Login' off the subject; got: {out}"
        );
    }

    #[test]
    fn prepend_replacing_subject_unknown_name_falls_back() {
        // No entity name supplied → conservative fallback, not an NP strip.
        let mut out = String::from("The class Foo was modified");
        prepend_replacing_subject_in_place(&mut out, "It also", None);
        assert_eq!(out, "It also the class Foo was modified");
    }

    #[test]
    fn render_sequence_with_multiword_name_produces_valid_prose() {
        // End-to-end: two renders on a multi-word-named entity must not
        // produce a corrupted follower sentence via the "It also" path.
        let mut engine = test_engine();
        engine
            .register_template("renamed", "{name|refer} was renamed")
            .unwrap();
        engine
            .register_template("modified", "{name|refer} was modified")
            .unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("feature".into()));
        ctx.insert("name", Value::String("Login flow".into()));

        let r1 = engine.render(&mut session, "renamed", &ctx).unwrap();
        assert!(r1.contains("Login flow"), "got: {r1}");
        let r2 = engine.render(&mut session, "modified", &ctx).unwrap();
        // Must not produce "flow was modified" (mid-name chop).
        assert!(
            !r2.starts_with("flow ") && !r2.contains("also flow "),
            "follow-up render corrupted multi-word name; got: {r2}"
        );
    }

    #[test]
    fn reduce_accepts_mixed_discourse_connectives() {
        // "Additionally," on one sentence, "It also" on the next — both
        // must be stripped before the pronoun+aux match fires.
        let reduced = reduce_same_entity_clauses(&[
            "The class Foo was renamed.".to_string(),
            "Additionally, it was modified.".to_string(),
            "It also was moved.".to_string(),
        ]);
        assert_eq!(
            reduced.as_deref(),
            Some("The class Foo was renamed, modified, and moved.")
        );
    }

    // ── FCR: full-NP repetition (Phase 2) ───────────────────────────────

    #[test]
    fn reduce_accepts_full_np_repetition() {
        let reduced = reduce_same_entity_clauses(&[
            "The class Foo was renamed.".to_string(),
            "The class Foo was modified.".to_string(),
        ]);
        assert_eq!(
            reduced.as_deref(),
            Some("The class Foo was renamed and modified.")
        );
    }

    #[test]
    fn reduce_accepts_full_np_repetition_three_clauses() {
        let reduced = reduce_same_entity_clauses(&[
            "The class Foo was renamed.".to_string(),
            "The class Foo was modified.".to_string(),
            "The class Foo was moved.".to_string(),
        ]);
        assert_eq!(
            reduced.as_deref(),
            Some("The class Foo was renamed, modified, and moved.")
        );
    }

    #[test]
    fn reduce_mixed_np_and_pronoun_accepted() {
        // Head full NP, follower 1 pronoun, follower 2 full NP — all reduce.
        let reduced = reduce_same_entity_clauses(&[
            "The class Foo was renamed.".to_string(),
            "It was modified.".to_string(),
            "The class Foo was moved.".to_string(),
        ]);
        assert_eq!(
            reduced.as_deref(),
            Some("The class Foo was renamed, modified, and moved.")
        );
    }

    #[test]
    fn reduce_rejects_different_np_repetition() {
        // Head is "The class Foo was renamed", follower is "The class Bar was modified" —
        // different NPs; the subject+aux prefix doesn't match, so no fusion.
        let reduced = reduce_same_entity_clauses(&[
            "The class Foo was renamed.".to_string(),
            "The class Bar was modified.".to_string(),
        ]);
        assert_eq!(reduced, None);
    }

    #[test]
    fn reduce_rejects_full_np_with_embedded_clause() {
        // The predicate_has_embedded_clause safety check must still fire
        // on the full-NP fallback path — FCR Phase 2.
        let reduced = reduce_same_entity_clauses(&[
            "The class Foo was renamed.".to_string(),
            "The class Foo was modified, which affects 6 consumers.".to_string(),
        ]);
        assert_eq!(reduced, None);
    }

    // ── Gapping (ELLEIPO) unit tests ─────────────────────────────────────

    #[test]
    fn reduce_gapping_two_events() {
        let ss = vec![
            "Foo was moved to core".to_string(),
            "Bar was moved to util".to_string(),
        ];
        let out = reduce_gapping(&ss).unwrap();
        assert_eq!(out, "Foo was moved to core, and Bar to util.");
    }

    #[test]
    fn reduce_gapping_three_events() {
        let ss = vec![
            "Foo was moved to core".to_string(),
            "Bar was moved to util".to_string(),
            "Baz was moved to api".to_string(),
        ];
        let out = reduce_gapping(&ss).unwrap();
        assert_eq!(out, "Foo was moved to core, Bar to util, and Baz to api.");
    }

    #[test]
    fn reduce_gapping_rejects_single() {
        let ss = vec!["Foo was moved to core".to_string()];
        assert!(reduce_gapping(&ss).is_none());
    }

    #[test]
    fn reduce_gapping_rejects_short_anchor() {
        // Anchor = ["was"] — length 1, below the 2-token threshold.
        let ss = vec!["Foo was moved".to_string(), "Bar was modified".to_string()];
        assert!(reduce_gapping(&ss).is_none());
    }

    #[test]
    fn reduce_gapping_rejects_embedded_clause() {
        let ss = vec![
            "Foo was moved, affecting 3 consumers, to core".to_string(),
            "Bar was moved to util".to_string(),
        ];
        assert!(reduce_gapping(&ss).is_none());
    }

    #[test]
    fn reduce_gapping_rejects_identical_subjects() {
        // Identical subjects → no gapping opportunity.
        let ss = vec![
            "Foo was moved to core".to_string(),
            "Foo was moved to core".to_string(),
        ];
        assert!(reduce_gapping(&ss).is_none());
    }

    #[test]
    fn reduce_gapping_rejects_empty_suffix() {
        // Anchor consumes everything; no divergent tail.
        let ss = vec!["Foo was moved".to_string(), "Bar was moved".to_string()];
        assert!(reduce_gapping(&ss).is_none());
    }

    // ── Gapping integration tests (render_batch / render_iter) ──────────

    #[test]
    fn render_batch_applies_gapping_when_objects_differ() {
        let mut engine = test_engine();
        engine
            .register_template("code.moved", "{name} was moved to {new_location}")
            .unwrap();

        let make = |name: &str, loc: &str| {
            let mut c = Context::new();
            c.insert("entity_type", Value::String("class".into()));
            c.insert("name", Value::String(name.into()));
            c.insert("new_location", Value::String(loc.into()));
            c
        };

        let events = vec![
            ("code.moved", make("Foo", "core")),
            ("code.moved", make("Bar", "util")),
            ("code.moved", make("Baz", "api")),
        ];

        let mut s = Session::new();
        let out = engine.render_batch(&mut s, &events).unwrap();
        assert_eq!(out, "Foo was moved to core, Bar to util, and Baz to api.");
    }

    #[test]
    fn render_batch_gapping_does_not_apply_when_objects_match() {
        // Same template + same non-subject slots → subject aggregation wins,
        // not gapping. Verifies the aggregated-subjects path is not regressed.
        let mut engine = test_engine();
        engine
            .register_template("code.moved", "{name} was moved to {new_location}")
            .unwrap();

        let make = |name: &str| {
            let mut c = Context::new();
            c.insert("entity_type", Value::String("class".into()));
            c.insert("name", Value::String(name.into()));
            c.insert("new_location", Value::String("core".into()));
            c
        };

        let events = vec![("code.moved", make("Foo")), ("code.moved", make("Bar"))];

        let mut s = Session::new();
        let out = engine.render_batch(&mut s, &events).unwrap();
        // Subject aggregation path: "Foo and Bar were moved to core".
        assert!(
            out.contains("Foo and Bar") && out.contains("core"),
            "got: {out}"
        );
        // NOT gapping-style output.
        assert!(!out.contains(", and Bar to "), "got: {out}");
    }

    #[test]
    fn render_iter_applies_gapping() {
        let mut engine = test_engine();
        engine
            .register_template("code.moved", "{name} was moved to {new_location}")
            .unwrap();

        let make = |name: &str, loc: &str| {
            let mut c = Context::new();
            c.insert("entity_type", Value::String("class".into()));
            c.insert("name", Value::String(name.into()));
            c.insert("new_location", Value::String(loc.into()));
            c
        };

        let events = vec![
            ("code.moved", make("Foo", "core")),
            ("code.moved", make("Bar", "util")),
            ("code.moved", make("Baz", "api")),
        ];

        let mut s = Session::new();
        let collected: Result<Vec<_>, _> = engine.render_iter(&mut s, &events).collect();
        let collected = collected.unwrap();
        // One sentence emitted for the gapped run.
        assert_eq!(collected.len(), 1);
        assert_eq!(
            collected[0],
            "Foo was moved to core, Bar to util, and Baz to api."
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
            crate::EntityDescriptor::new("UserService", "class").with_attribute("layer", "domain"),
        );
        engine.register_entity(
            crate::EntityDescriptor::new("AuthService", "class").with_attribute("layer", "infra"),
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
            crate::EntityDescriptor::new("UserService", "class").with_attribute("layer", "domain"),
        );
        engine.register_entity(
            crate::EntityDescriptor::new("UserModule", "module").with_attribute("layer", "infra"),
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
            crate::EntityDescriptor::new("UserService", "class").with_attribute("layer", "domain"),
        );
        engine.register_entity(
            crate::EntityDescriptor::new("UserService", "trait").with_attribute("scope", "public"),
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
        assert!(
            engine
                .render(&mut session, "ok", &ctx)
                .unwrap()
                .contains("alpha")
        );

        // A failed render between the two should NOT advance the counter
        // for "ok" — the missing slot aborts before commit.
        assert!(engine.render(&mut session, "ok", &empty).is_err());

        // Next successful render must be beta, not gamma.
        assert!(
            engine
                .render(&mut session, "ok", &ctx)
                .unwrap()
                .contains("beta")
        );
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
        engine.register_template("t", "{action|verb:past}").unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("action", Value::String("rename".into()));
        assert_eq!(
            engine.render(&mut session, "t", &ctx).unwrap(),
            "was renameed"
        );
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
        assert_eq!(
            engine.render(&mut session, "t", &ctx).unwrap(),
            "has been renameed"
        );
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
        assert_eq!(
            engine.render(&mut session, "t", &ctx).unwrap(),
            "is being renameed"
        );
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
        assert_eq!(
            engine.render(&mut session, "t", &ctx).unwrap(),
            "has renameed"
        );
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
        assert_eq!(
            engine.render(&mut session, "t", &ctx).unwrap(),
            "would be renameed"
        );
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
        assert!(matches!(result, Err(ProsaicError::InvalidPipe { .. })));
    }

    #[test]
    fn verb_pipe_missing_spec_is_error() {
        let mut engine = test_engine();
        engine.register_template("t", "{action|verb}").unwrap();

        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("action", Value::String("rename".into()));
        let result = engine.render(&mut session, "t", &ctx);
        assert!(matches!(result, Err(ProsaicError::InvalidPipe { .. })));
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
        let styles: std::collections::HashSet<&str> = [r1.as_str(), r2.as_str(), r3.as_str()]
            .into_iter()
            .collect();
        assert_eq!(
            styles.len(),
            3,
            "Expected three distinct list styles across three renders, got: {r1} / {r2} / {r3}"
        );
    }

    // ── choose pipe ──────────────────────────────────────────────────────────

    #[test]
    fn choose_pipe_exact_match() {
        let engine = test_engine();
        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("level", Value::String("critical".into()));
        let out = engine
            .render_inline(
                &mut session,
                "{level|choose: critical=URGENT, warn=WARN, default=INFO}",
                &ctx,
            )
            .unwrap();
        assert_eq!(out, "URGENT");
    }

    #[test]
    fn choose_pipe_case_insensitive_match() {
        let engine = test_engine();
        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("level", Value::String("CRITICAL".into()));
        let out = engine
            .render_inline(
                &mut session,
                "{level|choose: critical=URGENT, default=INFO}",
                &ctx,
            )
            .unwrap();
        assert_eq!(out, "URGENT");
    }

    #[test]
    fn choose_pipe_default_fallback() {
        let engine = test_engine();
        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("level", Value::String("info".into()));
        let out = engine
            .render_inline(
                &mut session,
                "{level|choose: critical=URGENT, default=INFO}",
                &ctx,
            )
            .unwrap();
        assert_eq!(out, "INFO");
    }

    #[test]
    fn choose_pipe_number_slot() {
        let engine = test_engine();
        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("count", Value::Number(1));
        let out = engine
            .render_inline(&mut session, "{count|choose: 1=is, default=are}", &ctx)
            .unwrap();
        assert_eq!(out, "is");

        let mut session2 = test_session();
        let mut ctx2 = Context::new();
        ctx2.insert("count", Value::Number(5));
        let out2 = engine
            .render_inline(&mut session2, "{count|choose: 1=is, default=are}", &ctx2)
            .unwrap();
        assert_eq!(out2, "are");
    }

    #[test]
    fn choose_pipe_chains_with_other_pipes() {
        let engine = test_engine();
        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("action", Value::String("modify".into()));
        let out = engine
            .render_inline(
                &mut session,
                "{action|choose: rename=renamed, modify=modified, default=changed|capitalize}",
                &ctx,
            )
            .unwrap();
        assert!(out.contains("Modified"), "got: {out}");
    }

    #[test]
    fn choose_pipe_strict_no_match_no_default_errors() {
        let engine = test_engine().strictness(Strictness::Strict);
        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("level", Value::String("info".into()));
        let err = engine
            .render_inline(&mut session, "{level|choose: critical=URGENT}", &ctx)
            .unwrap_err();
        assert!(matches!(err, ProsaicError::InvalidPipe { .. }));
    }

    #[test]
    fn choose_pipe_lenient_no_match_returns_placeholder() {
        let engine = test_engine().strictness(Strictness::Lenient);
        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("level", Value::String("info".into()));
        let out = engine
            .render_inline(&mut session, "{level|choose: critical=URGENT}", &ctx)
            .unwrap();
        assert!(out.contains("[choose: no match for info]"), "got: {out}");
    }

    #[test]
    fn choose_pipe_silent_no_match_returns_empty() {
        let engine = test_engine().strictness(Strictness::Silent);
        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("level", Value::String("info".into()));
        let out = engine
            .render_inline(&mut session, "{level|choose: critical=URGENT}", &ctx)
            .unwrap();
        assert_eq!(out, "");
    }

    #[test]
    fn choose_pipe_missing_arg_errors() {
        let engine = test_engine().strictness(Strictness::Strict);
        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("level", Value::String("info".into()));
        let err = engine
            .render_inline(&mut session, "{level|choose}", &ctx)
            .unwrap_err();
        assert!(matches!(err, ProsaicError::InvalidPipe { .. }));
    }

    #[test]
    fn choose_pipe_malformed_arg_errors() {
        let engine = test_engine().strictness(Strictness::Strict);
        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("level", Value::String("info".into()));
        let err = engine
            .render_inline(&mut session, "{level|choose: no_equals_here}", &ctx)
            .unwrap_err();
        assert!(matches!(err, ProsaicError::InvalidPipe { .. }));
    }

    // ── |plural pipe ─────────────────────────────────────────────────────────

    #[test]
    fn plural_pipe_singular_for_one() {
        let engine = test_engine();
        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("count", Value::Number(1));
        let out = engine
            .render_inline(&mut session, "{count|plural:service}", &ctx)
            .unwrap();
        assert!(out.contains("service"));
        assert!(!out.contains("services"));
    }

    #[test]
    fn plural_pipe_plural_for_many() {
        let engine = test_engine();
        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("count", Value::Number(5));
        let out = engine
            .render_inline(&mut session, "{count|plural:service}", &ctx)
            .unwrap();
        assert!(out.contains("services"));
    }

    #[test]
    fn plural_pipe_plural_for_zero() {
        // English: 0 → Other → plural form
        let engine = test_engine();
        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("count", Value::Number(0));
        let out = engine
            .render_inline(&mut session, "{count|plural:service}", &ctx)
            .unwrap();
        assert!(out.contains("services"));
    }

    #[test]
    fn plural_pipe_requires_noun_arg() {
        let engine = test_engine().strictness(Strictness::Strict);
        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("count", Value::Number(3));
        let err = engine
            .render_inline(&mut session, "{count|plural}", &ctx)
            .unwrap_err();
        assert!(matches!(err, ProsaicError::InvalidPipe { .. }));
    }

    #[test]
    fn plural_pipe_requires_numeric_value() {
        let engine = test_engine().strictness(Strictness::Strict);
        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("word", Value::String("hello".into()));
        let err = engine
            .render_inline(&mut session, "{word|plural:service}", &ctx)
            .unwrap_err();
        assert!(matches!(err, ProsaicError::InvalidPipe { .. }));
    }

    #[test]
    fn pronoun_realization_routes_through_language_trait() {
        // This test exists to assert that the refactor preserved the English
        // pronoun output. If it fails, the trait default impl doesn't match
        // the old inline logic.
        let mut engine = test_engine();
        engine
            .register_template("t", "{name|refer} was modified")
            .unwrap();
        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("entity_type", Value::String("class".into()));
        ctx.insert("name", Value::String("Foo".into()));

        // First render: Full form — "The class Foo was modified."
        let r1 = engine.render(&mut session, "t", &ctx).unwrap();
        assert!(r1.contains("The class Foo"), "got: {r1}");

        // Second render: Pronoun — should contain "it" (English default).
        let r2 = engine.render(&mut session, "t", &ctx).unwrap();
        assert!(
            r2.to_lowercase().contains("it was") || r2.to_lowercase().contains("it "),
            "got: {r2}"
        );
    }

    #[test]
    fn plural_pipe_and_pluralize_pipe_coexist() {
        // Both pipes must remain functional — neither replaces the other.
        let engine = test_engine();
        let mut session = test_session();
        let mut ctx = Context::new();
        ctx.insert("count", Value::Number(2));
        let plural_out = engine
            .render_inline(&mut session, "{count|plural:item}", &ctx)
            .unwrap();
        session.reset();
        let pluralize_out = engine
            .render_inline(&mut session, "{count|pluralize:item}", &ctx)
            .unwrap();
        // Under English (TestLang) both pipes should produce "items" for count=2.
        assert_eq!(plural_out, pluralize_out);
    }
}

#[cfg(test)]
mod render_batch_with_relations_tests {
    use super::*;
    use crate::language::{Conjunction, Language, Person, Tense};
    use crate::rst::RstRelation;

    struct SimpleLang;

    impl Language for SimpleLang {
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
            "the"
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

    fn make_engine() -> Engine {
        Engine::new(SimpleLang)
            .strictness(Strictness::Strict)
            .variation(Variation::Fixed)
    }

    fn ctx_with_name(name: &str) -> Context {
        let mut c = Context::new();
        c.insert("name", Value::String(name.into()));
        c
    }

    #[test]
    fn render_batch_with_relations_inserts_marker() {
        let mut engine = make_engine();
        engine
            .register_template("t", "The class {name} was modified")
            .unwrap();
        let mut s = Session::new();
        let ctx = ctx_with_name("Foo");
        let events = vec![
            ("t", ctx.clone(), None),
            ("t", ctx, Some(RstRelation::Elaboration)),
        ];
        let out = engine.render_batch_with_relations(&mut s, &events).unwrap();
        assert!(out.contains("Furthermore, "), "got: {out}");
    }

    #[test]
    fn render_batch_with_relations_lowercases_determiner_after_marker() {
        let mut engine = make_engine();
        engine
            .register_template("t", "The class {name} was modified")
            .unwrap();
        let mut s = Session::new();
        let ctx = ctx_with_name("Foo");
        let events = vec![
            ("t", ctx.clone(), None),
            ("t", ctx, Some(RstRelation::Contrast)),
        ];
        let out = engine.render_batch_with_relations(&mut s, &events).unwrap();
        // "However, the class Foo..." — note lowercase "the"
        assert!(out.contains("However, the class"), "got: {out}");
    }

    #[test]
    fn render_batch_with_all_none_delegates_to_render_batch() {
        let mut engine = make_engine();
        engine
            .register_template("t", "{name} was modified")
            .unwrap();
        let mut s = Session::new();
        let ctx = ctx_with_name("Foo");
        let triples = vec![("t", ctx.clone(), None), ("t", ctx.clone(), None)];
        let pairs: Vec<_> = triples.iter().map(|(k, c, _)| (*k, c.clone())).collect();
        let mut s2 = Session::new();
        let from_triples = engine
            .render_batch_with_relations(&mut s, &triples)
            .unwrap();
        let from_pairs = engine.render_batch(&mut s2, &pairs).unwrap();
        assert_eq!(from_triples, from_pairs);
    }

    #[test]
    fn render_batch_with_relations_empty_is_empty_string() {
        let engine = make_engine();
        let mut s = Session::new();
        let events: Vec<(&str, Context, Option<RstRelation>)> = vec![];
        let out = engine.render_batch_with_relations(&mut s, &events).unwrap();
        assert_eq!(out, "");
    }

    #[test]
    fn rst_marker_strips_auto_connective_to_avoid_double_prepend() {
        // Two renders that would normally trigger an automatic "Similarly,"
        // connective (different entity, same action). With an explicit RST
        // Elaboration marker, the output should start with "Furthermore, "
        // — not "Furthermore, Similarly, ".
        let mut engine = make_engine();
        engine
            .register_template("t", "The class {name} was modified")
            .unwrap();
        let mut s = Session::new();
        let events = vec![
            ("t", ctx_with_name("Foo"), None),
            ("t", ctx_with_name("Bar"), Some(RstRelation::Elaboration)),
        ];
        let out = engine.render_batch_with_relations(&mut s, &events).unwrap();
        assert!(out.contains("Furthermore, "), "got: {out}");
        assert!(
            !out.contains("Furthermore, Similarly,") && !out.contains("Furthermore, Likewise,"),
            "RST marker should suppress / strip auto-connective; got: {out}"
        );
    }

    fn ctx_with_entity(name: &str) -> Context {
        // Like ctx_with_name but also sets entity_type so that
        // discourse.mention_entity fires and detect_relation can
        // classify the inter-render link.
        let mut c = Context::new();
        c.insert("entity_type", Value::String("class".into()));
        c.insert("name", Value::String(name.into()));
        c
    }

    #[test]
    fn rst_render_leaves_session_free_of_unemitted_connective() {
        // Regression: render_batch_with_relations previously let the
        // underlying render() select an auto-connective (advancing the
        // no-repeat ring buffer) and record its words, then stripped
        // the connective from the surface. Session state was then
        // inconsistent with emitted prose.
        //
        // Post-fix: a follow-up render that shares the same discourse
        // relation should be free to pick the connective that would
        // have been used during the RST render, because the RST call
        // never advanced the history.
        let mut engine = make_engine();
        engine
            .register_template("t", "The class {name} was modified")
            .unwrap();
        let mut s = Session::new();

        // First two events: the second carries an RST marker that
        // should suppress the auto-connective selection.
        let events = vec![
            ("t", ctx_with_entity("Foo"), None),
            ("t", ctx_with_entity("Bar"), Some(RstRelation::Elaboration)),
        ];
        let _ = engine.render_batch_with_relations(&mut s, &events).unwrap();

        // A subsequent plain render with a different entity on the same
        // template key would classify as DifferentEntitySameAction →
        // connective pool SAME_ACTION_CONNECTIVES = ["Similarly,", "Likewise,"].
        // Because the RST render did NOT consume any connective, the
        // next render must still pick the first available ("Similarly,").
        let exp = engine
            .render_explained(&mut s, "t", &ctx_with_entity("Baz"))
            .unwrap();
        assert_eq!(
            exp.connective,
            Some("Similarly,"),
            "RST render leaked connective history; got connective={:?}, output={}",
            exp.connective,
            exp.output
        );
    }

    #[test]
    fn rst_render_does_not_record_unemitted_connective_words() {
        // Regression: repetition scoring previously saw the stripped
        // auto-connective's words ("Similarly") in word_history. After
        // the fix, a follow-up render's repetition scoring must not be
        // biased against words that never reached the output.
        let mut engine = make_engine().variation(Variation::Seeded(42));
        engine
            .register_template("t", "The class {name} was modified")
            .unwrap();
        let mut s = Session::new();

        let events = vec![
            ("t", ctx_with_entity("Foo"), None),
            ("t", ctx_with_entity("Bar"), Some(RstRelation::Elaboration)),
        ];
        let _ = engine.render_batch_with_relations(&mut s, &events).unwrap();

        // "similarly" should never have been recorded in word history
        // by the RST render — its word_frequency must be zero.
        assert_eq!(
            s.discourse.word_frequency("similarly"),
            0.0,
            "RST render leaked 'similarly' into word history"
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
