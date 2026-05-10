//! Self-Refine retrospective pass — types and iteration controller.
//!
//! Document-scope refinement loop that renders a [`DocumentPlan`] once,
//! runs pluggable diagnosers over the structured output, generates
//! adversarial constraints, re-renders with the constraints applied, and
//! iterates until the composite score converges or a bounded iteration
//! ceiling is reached. The loop is the deterministic, audit-friendly
//! Prosaic adaptation of Madaan et al.'s Self-Refine pattern.
//!
//! See `docs/superpowers/specs/2026-05-09-self-refine-retro-pass-design.md`
//! for the full design rationale.
//!
//! v1 surface delivered here: structured intermediate representation
//! ([`RenderedDocument`]), refine configuration scaffolding, the
//! `Diagnoser` trait, the constraint enum, and the iteration controller.
//! The built-in diagnosers and the composite scorer live in companion
//! modules so each can grow independently.
//!
//! Style coupling is intentional and bidirectional with `style.rs`: the
//! [`Diagnoser::diagnose`] hook accepts an optional `&StyleProfile`
//! because the `ProfileDistributionDrift` diagnoser needs profile-aware
//! targets. Other built-in diagnosers ignore the profile.

use alloc::sync::Arc;

#[cfg(not(feature = "std"))]
use alloc::string::{String, ToString};
#[cfg(not(feature = "std"))]
use alloc::vec;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use crate::discourse::{ListStyle, sentence_word_counts};
use crate::salience::Salience;
use crate::style::{LengthDistribution, SalienceBias, StyleProfile};

// ── Structured rendered document ────────────────────────────────────────

/// A structured view of a rendered document.
///
/// Diagnosers and the composite scorer consume this rather than the
/// flat text of [`crate::DocumentPlan::render`]. The `text` field always
/// equals the same flattened-with-`"\n\n"` form `render` returns, so
/// callers can ignore the structured fields when they don't need them.
///
/// Population of the rich fields (`connectives_used`, `list_styles_used`,
/// per-sentence opening connectives) happens via discourse-state
/// introspection at render time. A `RenderedDocument` returned by
/// [`crate::DocumentPlan::render_structured`] is internally consistent;
/// hand-constructed instances (e.g. for diagnoser tests) need to populate
/// matching data.
#[derive(Debug, Clone, Default, PartialEq)]
#[non_exhaustive]
pub struct RenderedDocument {
    /// The flattened paragraph text, joined with `"\n\n"`.
    pub text: String,
    /// One entry per paragraph, in render order.
    pub paragraphs: Vec<RenderedParagraph>,
    /// All sentences from all paragraphs, flattened in render order.
    /// Mirrors the per-paragraph nesting and exists as a convenience for
    /// scorers that don't care about paragraph boundaries.
    pub sentences: Vec<RenderedSentence>,
    /// Connectives emitted across the whole document, in render order.
    /// Each entry tracks which paragraph and sentence index emitted it.
    pub connectives_used: Vec<UsedConnective>,
    /// List styles emitted across the whole document, in render order.
    pub list_styles_used: Vec<UsedListStyle>,
}

#[derive(Debug, Clone, Default, PartialEq)]
#[non_exhaustive]
pub struct RenderedParagraph {
    /// The full paragraph text.
    pub text: String,
    /// One entry per sentence inside the paragraph.
    pub sentences: Vec<RenderedSentence>,
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct RenderedSentence {
    pub text: String,
    pub word_count: usize,
    /// Connective opening this sentence (if any). Matched against the
    /// engine's known connective set, so abuse-introduced strings won't
    /// false-positive.
    pub opening_connective: Option<String>,
    /// Index of the paragraph this sentence belongs to.
    pub paragraph_index: usize,
    /// Index of this sentence inside its paragraph.
    pub sentence_index_in_paragraph: usize,
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct UsedConnective {
    pub connective: String,
    pub paragraph_index: usize,
    pub sentence_index_in_paragraph: usize,
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct UsedListStyle {
    pub list_style: ListStyle,
    pub paragraph_index: usize,
    pub sentence_index_in_paragraph: usize,
}

impl RenderedDocument {
    /// Build a `RenderedDocument` from a list of paragraph render-results.
    /// Each paragraph result is `(text, list_of_(connective, list_style))`
    /// where the per-sentence metadata lines up with the sentences the
    /// engine emitted for that paragraph.
    pub(crate) fn from_paragraphs(
        rendered: Vec<ParagraphRender>,
    ) -> Self {
        let mut paragraphs = Vec::with_capacity(rendered.len());
        let mut all_sentences: Vec<RenderedSentence> = Vec::new();
        let mut connectives_used: Vec<UsedConnective> = Vec::new();
        let mut list_styles_used: Vec<UsedListStyle> = Vec::new();

        for (p_idx, p) in rendered.iter().enumerate() {
            let mut sentences: Vec<RenderedSentence> = Vec::new();
            let counts = sentence_word_counts(&p.text);
            // Pair sentence boundaries with per-event metadata. When the
            // sentence count and metadata count diverge (e.g. gapping
            // collapsed two events into one sentence), pad with None.
            let split_sentences = split_sentences(&p.text);
            for (s_idx, sentence_text) in split_sentences.iter().enumerate() {
                let meta = p.events.get(s_idx);
                let opening_connective = meta.and_then(|m| m.connective.clone());
                if let Some(c) = &opening_connective {
                    connectives_used.push(UsedConnective {
                        connective: c.clone(),
                        paragraph_index: p_idx,
                        sentence_index_in_paragraph: s_idx,
                    });
                }
                if let Some(ls) = meta.and_then(|m| m.list_style) {
                    list_styles_used.push(UsedListStyle {
                        list_style: ls,
                        paragraph_index: p_idx,
                        sentence_index_in_paragraph: s_idx,
                    });
                }
                let word_count = counts.get(s_idx).copied().unwrap_or_else(|| {
                    sentence_text.split_whitespace().count()
                });
                let s = RenderedSentence {
                    text: sentence_text.clone(),
                    word_count,
                    opening_connective,
                    paragraph_index: p_idx,
                    sentence_index_in_paragraph: s_idx,
                };
                sentences.push(s.clone());
                all_sentences.push(s);
            }
            paragraphs.push(RenderedParagraph {
                text: p.text.clone(),
                sentences,
            });
        }

        let text = paragraphs
            .iter()
            .map(|p| p.text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");

        Self {
            text,
            paragraphs,
            sentences: all_sentences,
            connectives_used,
            list_styles_used,
        }
    }
}

/// Internal carrier from the engine's batch render to RenderedDocument.
/// Each entry is one paragraph's text plus the per-event metadata the
/// discourse state captured.
pub(crate) struct ParagraphRender {
    pub(crate) text: String,
    pub(crate) events: Vec<EventMeta>,
}

#[derive(Default)]
pub(crate) struct EventMeta {
    pub(crate) connective: Option<String>,
    pub(crate) list_style: Option<ListStyle>,
}

/// Split rendered text into sentences. Uses the same heuristic as
/// [`sentence_word_counts`] but returns the substrings instead of just
/// the counts.
fn split_sentences(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut last_was_terminator = false;
    for ch in text.chars() {
        current.push(ch);
        if matches!(ch, '.' | '!' | '?') {
            last_was_terminator = true;
        } else if last_was_terminator && ch.is_whitespace() {
            // Sentence ends at the whitespace following a terminator.
            let trimmed = current.trim().to_string();
            if !trimmed.is_empty() {
                out.push(trimmed);
            }
            current.clear();
            last_was_terminator = false;
        } else if !ch.is_whitespace() {
            last_was_terminator = false;
        }
    }
    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        out.push(trimmed);
    }
    out
}

// ── RefineConfig + Diagnoser + RefineConstraint scaffolding ────────────

/// Configuration for the retrospective refine pass on `DocumentPlan::render`.
///
/// `RefineConfig::off()` is the default and a no-op — engines without
/// `.refine(...)` produce byte-identical output to the no-refine path.
#[derive(Clone)]
#[non_exhaustive]
pub struct RefineConfig {
    pub enabled: bool,
    pub max_iterations: u8,
    pub min_improvement: f32,
    pub weights: RefineWeights,
    pub diagnosers: Vec<Arc<dyn Diagnoser>>,
}

impl core::fmt::Debug for RefineConfig {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("RefineConfig")
            .field("enabled", &self.enabled)
            .field("max_iterations", &self.max_iterations)
            .field("min_improvement", &self.min_improvement)
            .field("weights", &self.weights)
            .field("diagnosers_count", &self.diagnosers.len())
            .finish()
    }
}

impl RefineConfig {
    /// Disabled — render path is unchanged.
    pub fn off() -> Self {
        Self {
            enabled: false,
            max_iterations: 3,
            min_improvement: 0.01,
            weights: RefineWeights::default(),
            diagnosers: Vec::new(),
        }
    }

    /// Default opt-in shape: 3 iterations, balanced weights, no built-in
    /// diagnosers attached yet (built-ins land in a follow-up commit).
    pub fn balanced() -> Self {
        Self {
            enabled: true,
            max_iterations: 3,
            min_improvement: 0.01,
            weights: RefineWeights::default(),
            diagnosers: crate::refine_diagnosers::default_set(),
        }
    }

    pub fn is_off(&self) -> bool {
        !self.enabled
    }

    pub fn with_max_iterations(mut self, n: u8) -> Self {
        self.max_iterations = n;
        self
    }

    pub fn with_min_improvement(mut self, m: f32) -> Self {
        self.min_improvement = m;
        self
    }

    pub fn with_weights(mut self, w: RefineWeights) -> Self {
        self.weights = w;
        self
    }

    pub fn with_diagnoser(mut self, d: Arc<dyn Diagnoser>) -> Self {
        self.diagnosers.push(d);
        self
    }
}

impl Default for RefineConfig {
    fn default() -> Self {
        Self::off()
    }
}

/// Composite-scorer weights. Documented defaults are produced via
/// [`RefineWeights::default`] and serve as the v1 hand-tuned baseline.
/// A future offline tuner (see `docs/plans/refine-scorer-tuner.md`) will
/// emit alternative weight sets for projects that have a reason to
/// deviate.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct RefineWeights {
    pub repetition: f32,
    pub rhythm: f32,
    pub connective: f32,
    pub paragraph_opener: f32,
    pub list_style_diversity: f32,
    pub rst_balance: f32,
    pub profile_match: f32,
}

impl Default for RefineWeights {
    fn default() -> Self {
        Self {
            repetition: 1.0,
            rhythm: 1.0,
            connective: 1.0,
            paragraph_opener: 1.0,
            list_style_diversity: 1.0,
            rst_balance: 1.0,
            profile_match: 1.0,
        }
    }
}

/// A failure pattern detected over a rendered document. Carries a
/// severity for the scorer and a hint about what constraints would
/// address it.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub diagnoser: &'static str,
    pub severity: f32,
    pub constraints: Vec<RefineConstraint>,
}

/// Pluggable detector. Built-in diagnosers ship in
/// `prosaic-core::refine_diagnosers`; external diagnosers register via
/// [`RefineConfig::with_diagnoser`].
pub trait Diagnoser: Send + Sync {
    fn name(&self) -> &'static str;
    fn diagnose(
        &self,
        document: &RenderedDocument,
        profile: Option<&StyleProfile>,
    ) -> Vec<Diagnostic>;
}

// ── Iteration controller ────────────────────────────────────────────────

/// Outcome of one refine pass: the final flattened text, the iterations
/// that ran, and the final composite score. Returned by
/// [`crate::DocumentPlan::render_refined`].
#[derive(Debug, Clone)]
pub struct RefineOutcome {
    pub text: String,
    pub iterations_run: u8,
    pub final_score: f32,
    /// `true` when the loop terminated because diagnoses were empty
    /// (the structural happy path). `false` when it stopped via one of
    /// the other termination conditions.
    pub converged_clean: bool,
}

/// Run the retrospective refine loop. Called by
/// [`crate::DocumentPlan::render`] when [`RefineConfig::is_off`] is `false`.
///
/// `render_initial` and `render_with_overrides` are the engine-side
/// callbacks that produce a [`RenderedDocument`]; they are passed in so
/// this module stays free of `Engine` plumbing. The session-side
/// blacklists are applied before each iteration via
/// `apply_constraints_to_session` and cleared after.
pub(crate) fn run_refine_loop<F>(
    config: &RefineConfig,
    profile: Option<&StyleProfile>,
    initial: RenderedDocument,
    initial_session_state: crate::session::Session,
    session: &mut crate::session::Session,
    mut render_with_session: F,
) -> Result<RefineOutcome, crate::error::ProsaicError>
where
    F: FnMut(&mut crate::session::Session) -> Result<RenderedDocument, crate::error::ProsaicError>,
{
    use crate::refine_score::score_document;

    let mut best = initial;
    let mut best_score = score_document(&best, &config.weights, profile);
    let mut best_diagnostics = run_all_diagnosers(&config.diagnosers, &best, profile);

    if best_diagnostics.is_empty() {
        return Ok(RefineOutcome {
            text: best.text,
            iterations_run: 0,
            final_score: best_score,
            converged_clean: true,
        });
    }

    let mut iter = 0_u8;
    let mut prev_diag_signature = diagnosis_signature(&best_diagnostics);

    while iter < config.max_iterations {
        let constraints = aggregate_constraints(&best_diagnostics);
        if constraints.is_empty() {
            break;
        }

        // Restore session to retro-pass entry, then apply constraints.
        *session = initial_session_state.clone();
        apply_constraints_to_session(session, &constraints);

        let candidate = match render_with_session(session) {
            Ok(d) => d,
            Err(e) => {
                // Faithfulness or other render error → reject candidate,
                // advance counter, retry next iteration.
                session.clear_refine_overrides();
                iter += 1;
                if iter >= config.max_iterations {
                    return Ok(RefineOutcome {
                        text: best.text,
                        iterations_run: iter,
                        final_score: best_score,
                        converged_clean: false,
                    });
                }
                let _ = e; // Discarded — we continue with current best.
                continue;
            }
        };
        session.clear_refine_overrides();

        let candidate_score = score_document(&candidate, &config.weights, profile);
        let candidate_diagnostics =
            run_all_diagnosers(&config.diagnosers, &candidate, profile);
        let candidate_signature = diagnosis_signature(&candidate_diagnostics);

        // Cycle halt: same diagnosis signature as before → no progress.
        if candidate_signature == prev_diag_signature && iter > 0 {
            break;
        }

        // Diminishing-returns halt.
        if candidate_score - best_score < config.min_improvement {
            break;
        }

        best = candidate;
        best_score = candidate_score;
        best_diagnostics = candidate_diagnostics;
        prev_diag_signature = candidate_signature;
        iter += 1;

        if best_diagnostics.is_empty() {
            return Ok(RefineOutcome {
                text: best.text,
                iterations_run: iter,
                final_score: best_score,
                converged_clean: true,
            });
        }
    }

    Ok(RefineOutcome {
        text: best.text,
        iterations_run: iter,
        final_score: best_score,
        converged_clean: best_diagnostics.is_empty(),
    })
}

fn run_all_diagnosers(
    diagnosers: &[Arc<dyn Diagnoser>],
    document: &RenderedDocument,
    profile: Option<&StyleProfile>,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for d in diagnosers {
        out.extend(d.diagnose(document, profile));
    }
    out
}

fn aggregate_constraints(diagnostics: &[Diagnostic]) -> Vec<RefineConstraint> {
    let mut out = Vec::new();
    for d in diagnostics {
        for c in &d.constraints {
            // Deduplicate by structural equality. Constraints are small;
            // a linear scan is fine for v1 set sizes.
            let already = out.iter().any(|existing: &RefineConstraint| match (existing, c) {
                (
                    RefineConstraint::BlacklistConnective(a),
                    RefineConstraint::BlacklistConnective(b),
                ) => a == b,
                (
                    RefineConstraint::BlacklistListStyle(a),
                    RefineConstraint::BlacklistListStyle(b),
                ) => a == b,
                _ => false,
            });
            if !already {
                out.push(c.clone());
            }
        }
    }
    out
}

fn apply_constraints_to_session(
    session: &mut crate::session::Session,
    constraints: &[RefineConstraint],
) {
    let mut blacklist_connectives = Vec::new();
    let mut blacklist_list_styles = Vec::new();
    for c in constraints {
        match c {
            RefineConstraint::BlacklistConnective(s) => blacklist_connectives.push(s.clone()),
            RefineConstraint::BlacklistListStyle(s) => blacklist_list_styles.push(*s),
            // v1 limitation: PrimeRecencyWindow / OverrideSalienceBias /
            // ForceVariantTier / TightenLengthDistribution are accepted
            // but no-op in the iteration loop. Tracked for v0.6.1.
            RefineConstraint::PrimeRecencyWindow { .. }
            | RefineConstraint::OverrideSalienceBias(_)
            | RefineConstraint::ForceVariantTier { .. }
            | RefineConstraint::TightenLengthDistribution(_) => {}
        }
    }
    session.set_refine_blacklists(blacklist_connectives, blacklist_list_styles);
}

fn diagnosis_signature(diagnostics: &[Diagnostic]) -> Vec<(&'static str, u32)> {
    let mut sig: Vec<(&'static str, u32)> = diagnostics
        .iter()
        .map(|d| (d.diagnoser, (d.severity * 1000.0) as u32))
        .collect();
    sig.sort();
    sig
}

/// An adversarial constraint applied to one refinement iteration.
/// Constraints are additive within an iteration but never persist across
/// iterations — each new iteration re-derives them from the latest
/// diagnosis.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum RefineConstraint {
    /// Forbid this connective for the next render.
    BlacklistConnective(String),
    /// Forbid this list style for the next render.
    BlacklistListStyle(ListStyle),
    /// Prepopulate discourse state with phantom history entries so the
    /// recency window starts already-saturated for these patterns.
    PrimeRecencyWindow {
        connectives: Vec<String>,
        list_styles: Vec<ListStyle>,
    },
    /// Override the salience bias for this render only.
    OverrideSalienceBias(SalienceBias),
    /// Force a particular variant tier for a specific template key.
    ForceVariantTier {
        template_key: String,
        tier: Salience,
    },
    /// Tighten the target sentence-length distribution for this render.
    TightenLengthDistribution(LengthDistribution),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refine_config_off_is_default() {
        let c = RefineConfig::default();
        assert!(c.is_off());
        assert!(!c.enabled);
    }

    #[test]
    fn refine_config_balanced_is_enabled() {
        let c = RefineConfig::balanced();
        assert!(!c.is_off());
        assert_eq!(c.max_iterations, 3);
    }

    #[test]
    fn refine_config_with_max_iterations_overrides_default() {
        let c = RefineConfig::balanced().with_max_iterations(7);
        assert_eq!(c.max_iterations, 7);
    }

    #[test]
    fn weights_default_is_uniform() {
        let w = RefineWeights::default();
        assert_eq!(w.repetition, 1.0);
        assert_eq!(w.profile_match, 1.0);
    }

    #[test]
    fn split_sentences_handles_terminators() {
        let s = split_sentences("First sentence. Second one. Third.");
        assert_eq!(s, vec!["First sentence.", "Second one.", "Third."]);
    }

    #[test]
    fn split_sentences_handles_no_trailing_terminator() {
        let s = split_sentences("First. Trailing");
        assert_eq!(s, vec!["First.", "Trailing"]);
    }

    #[test]
    fn split_sentences_handles_empty() {
        let s = split_sentences("");
        assert!(s.is_empty());
    }

    #[test]
    fn rendered_document_from_paragraphs_aggregates_correctly() {
        let para1 = ParagraphRender {
            text: "Foo was modified. It was renamed.".to_string(),
            events: vec![
                EventMeta {
                    connective: None,
                    list_style: None,
                },
                EventMeta {
                    connective: Some("Additionally,".to_string()),
                    list_style: None,
                },
            ],
        };
        let para2 = ParagraphRender {
            text: "Bar was deleted.".to_string(),
            events: vec![EventMeta::default()],
        };

        let doc = RenderedDocument::from_paragraphs(vec![para1, para2]);
        assert_eq!(doc.paragraphs.len(), 2);
        assert_eq!(doc.sentences.len(), 3);
        assert_eq!(doc.connectives_used.len(), 1);
        assert_eq!(doc.connectives_used[0].connective, "Additionally,");
        assert_eq!(doc.connectives_used[0].paragraph_index, 0);
        assert_eq!(doc.connectives_used[0].sentence_index_in_paragraph, 1);
        assert_eq!(
            doc.text,
            "Foo was modified. It was renamed.\n\nBar was deleted."
        );
    }
}
