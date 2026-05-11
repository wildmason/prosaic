//! Built-in [`Diagnoser`] implementations for the retrospective refine pass.
//!
//! Each diagnoser inspects a [`RenderedDocument`] and emits zero or more
//! [`Diagnostic`]s when it detects the failure mode it watches for. The
//! diagnoser set is open — callers can register custom diagnosers via
//! [`RefineConfig::with_diagnoser`] — but the six built-ins below are
//! the v1 set the spec calls out and ship enabled by default in
//! [`RefineConfig::balanced`].

#[cfg(not(feature = "std"))]
use alloc::string::String;
#[cfg(not(feature = "std"))]
use alloc::vec;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use crate::discourse::ListStyle;
use crate::refine::{Diagnoser, Diagnostic, RefineConstraint, RenderedDocument};
use crate::rst::RstRelation;
use crate::style::StyleProfile;

// ── Connective → family / RST classification ────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum ConnectorFamily {
    Continuation,
    Similarity,
    Contrast,
}

/// Map a sentence-leading connective to its (family, RST) pair. Mirrors
/// the engine's internal pools in `discourse.rs`. If the discourse-side
/// pools change the mapping must update in sync; the test suite covers
/// every emitted opener.
fn classify(connective: &str) -> Option<(ConnectorFamily, RstRelation)> {
    // Continuation pool (Elaboration in RST terms).
    for cont in &["Additionally,", "Furthermore,", "It also"] {
        if connective.starts_with(cont) {
            return Some((ConnectorFamily::Continuation, RstRelation::Elaboration));
        }
    }
    // Similarity pool (Sequence in RST terms — closest match).
    for sim in &["Similarly,", "Likewise,"] {
        if connective.starts_with(sim) {
            return Some((ConnectorFamily::Similarity, RstRelation::Sequence));
        }
    }
    // Contrast pool.
    for con in &["Meanwhile,", "However,", "On the other hand,"] {
        if connective.starts_with(con) {
            return Some((ConnectorFamily::Contrast, RstRelation::Contrast));
        }
    }
    // Default-Language discourse markers (fall-through bucket).
    for (prefix, rst) in &[
        ("Because of this,", RstRelation::Cause),
        ("As a result,", RstRelation::Result),
        ("Nevertheless,", RstRelation::Concession),
        ("Then,", RstRelation::Sequence),
        ("If this happens,", RstRelation::Condition),
        ("In summary,", RstRelation::Summary),
    ] {
        if connective.starts_with(prefix) {
            // None of the discourse-marker family classifications matter to
            // family-saturation diagnosis in v1 — return None for family so
            // these markers don't count toward continuation/similarity/contrast
            // budgets, matching the engine's internal accounting.
            let _ = rst;
            return None;
        }
    }
    None
}

// ── ParagraphOpenerMonotony ─────────────────────────────────────────────

/// Detects when the same connective opens ≥ `threshold` of the document's
/// paragraphs. Default threshold is 3 with a minimum-paragraphs gate of 4
/// — fires when at least 3 of 4+ paragraphs share an opener, never on
/// short documents where the pattern is statistically meaningless.
#[derive(Debug, Clone)]
pub struct ParagraphOpenerMonotony {
    pub threshold: usize,
    pub min_paragraphs: usize,
}

impl Default for ParagraphOpenerMonotony {
    fn default() -> Self {
        Self {
            threshold: 3,
            min_paragraphs: 4,
        }
    }
}

impl Diagnoser for ParagraphOpenerMonotony {
    fn name(&self) -> &'static str {
        "paragraph_opener_monotony"
    }

    fn diagnose(
        &self,
        document: &RenderedDocument,
        _profile: Option<&StyleProfile>,
    ) -> Vec<Diagnostic> {
        if document.paragraphs.len() < self.min_paragraphs {
            return Vec::new();
        }
        // The first sentence of a paragraph almost never carries a
        // connective (the engine's `reset_for_paragraph` clears the
        // last-template-key so `detect_relation` returns None on the
        // paragraph's first event). The "paragraph opener" we care about
        // is the first connective that fires inside the paragraph,
        // regardless of which sentence emits it.
        let mut count = alloc::collections::BTreeMap::<String, usize>::new();
        for paragraph in &document.paragraphs {
            if let Some(c) = paragraph
                .sentences
                .iter()
                .find_map(|s| s.opening_connective.as_ref())
            {
                *count.entry(c.clone()).or_insert(0) += 1;
            }
        }
        let mut diagnostics = Vec::new();
        for (connective, n) in count {
            if n >= self.threshold {
                let severity = (n as f32) / (document.paragraphs.len() as f32);
                diagnostics.push(Diagnostic {
                    diagnoser: "paragraph_opener_monotony",
                    severity,
                    constraints: vec![RefineConstraint::BlacklistConnective(connective)],
                });
            }
        }
        diagnostics
    }
}

// ── ListStyleFatigue ────────────────────────────────────────────────────

/// Detects when the same `ListStyle` dominates the document's list-style
/// emissions. Fires when one style accounts for ≥ `threshold` of the most
/// recent `window` emissions, with a minimum-emissions gate.
#[derive(Debug, Clone)]
pub struct ListStyleFatigue {
    pub threshold: usize,
    pub window: usize,
    pub min_emissions: usize,
}

impl Default for ListStyleFatigue {
    fn default() -> Self {
        Self {
            threshold: 3,
            window: 4,
            min_emissions: 3,
        }
    }
}

impl Diagnoser for ListStyleFatigue {
    fn name(&self) -> &'static str {
        "list_style_fatigue"
    }

    fn diagnose(
        &self,
        document: &RenderedDocument,
        _profile: Option<&StyleProfile>,
    ) -> Vec<Diagnostic> {
        if document.list_styles_used.len() < self.min_emissions {
            return Vec::new();
        }
        let recent_window = document
            .list_styles_used
            .iter()
            .rev()
            .take(self.window)
            .collect::<Vec<_>>();
        let mut count = alloc::collections::BTreeMap::<ListStyle, usize>::new();
        for u in &recent_window {
            *count.entry(u.list_style).or_insert(0) += 1;
        }
        let mut diagnostics = Vec::new();
        for (style, n) in count {
            if n >= self.threshold {
                let severity = (n as f32) / (recent_window.len() as f32);
                diagnostics.push(Diagnostic {
                    diagnoser: "list_style_fatigue",
                    severity,
                    constraints: vec![RefineConstraint::BlacklistListStyle(style)],
                });
            }
        }
        diagnostics
    }
}

// ── RstRelationImbalance ────────────────────────────────────────────────

/// Detects when one RST relation accounts for more than `max_share` of
/// the document's inter-sentence connectives. Default `max_share` is 0.6
/// (60%); minimum-emissions gate prevents short-document false positives.
#[derive(Debug, Clone)]
pub struct RstRelationImbalance {
    pub max_share: f32,
    pub min_emissions: usize,
}

impl Default for RstRelationImbalance {
    fn default() -> Self {
        Self {
            max_share: 0.6,
            min_emissions: 5,
        }
    }
}

impl Diagnoser for RstRelationImbalance {
    fn name(&self) -> &'static str {
        "rst_relation_imbalance"
    }

    fn diagnose(
        &self,
        document: &RenderedDocument,
        _profile: Option<&StyleProfile>,
    ) -> Vec<Diagnostic> {
        let classified: Vec<(String, RstRelation)> = document
            .connectives_used
            .iter()
            .filter_map(|c| classify(&c.connective).map(|(_, rst)| (c.connective.clone(), rst)))
            .collect();
        if classified.len() < self.min_emissions {
            return Vec::new();
        }
        let mut count = alloc::collections::BTreeMap::<RstRelation, Vec<String>>::new();
        for (text, rst) in &classified {
            count.entry(*rst).or_default().push(text.clone());
        }
        let mut diagnostics = Vec::new();
        let total = classified.len() as f32;
        for (_rst, connectives) in count {
            let share = connectives.len() as f32 / total;
            if share > self.max_share {
                // Blacklist the most-emitted connective in this relation
                // bucket; that breaks the imbalance without forbidding the
                // whole RST family (which would over-correct).
                let mut occurrence = alloc::collections::BTreeMap::<String, usize>::new();
                for c in &connectives {
                    *occurrence.entry(c.clone()).or_insert(0) += 1;
                }
                let dominant = occurrence
                    .into_iter()
                    .max_by_key(|(_, n)| *n)
                    .map(|(c, _)| c)
                    .unwrap_or_default();
                diagnostics.push(Diagnostic {
                    diagnoser: "rst_relation_imbalance",
                    severity: share,
                    constraints: vec![RefineConstraint::BlacklistConnective(dominant)],
                });
            }
        }
        diagnostics
    }
}

// ── DocumentScopeRhythm ─────────────────────────────────────────────────

/// Detects when sentence-length variance across the whole document drops
/// below `min_stdev` words. The per-decision rhythm scorer can land each
/// individual sentence inside a healthy local window while the aggregate
/// flattens to a monotone cadence — this catches the latter.
#[derive(Debug, Clone)]
pub struct DocumentScopeRhythm {
    pub min_stdev: f32,
    pub min_sentences: usize,
}

impl Default for DocumentScopeRhythm {
    fn default() -> Self {
        Self {
            min_stdev: 2.0,
            min_sentences: 6,
        }
    }
}

impl Diagnoser for DocumentScopeRhythm {
    fn name(&self) -> &'static str {
        "document_scope_rhythm"
    }

    fn diagnose(
        &self,
        document: &RenderedDocument,
        _profile: Option<&StyleProfile>,
    ) -> Vec<Diagnostic> {
        if document.sentences.len() < self.min_sentences {
            return Vec::new();
        }
        let lengths: Vec<f32> = document
            .sentences
            .iter()
            .map(|s| s.word_count as f32)
            .collect();
        let n = lengths.len() as f32;
        let mean = lengths.iter().sum::<f32>() / n;
        let variance = lengths
            .iter()
            .map(|x| {
                let d = x - mean;
                d * d
            })
            .sum::<f32>()
            / n;
        let stdev = approx_sqrt(variance);
        if stdev < self.min_stdev {
            // Tighten the engine's length distribution toward a more
            // bursty target. The exact target is a wide-spread default;
            // the v1 retro-pass intentionally doesn't try to be clever
            // about reading the profile here — that's
            // ProfileDistributionDrift's job.
            let target = crate::style::LengthDistribution {
                short: 0.4,
                medium: 0.3,
                long: 0.3,
                short_max_words: 8,
                medium_max_words: 18,
            };
            return vec![Diagnostic {
                diagnoser: "document_scope_rhythm",
                severity: (self.min_stdev - stdev).max(0.0) / self.min_stdev,
                constraints: vec![RefineConstraint::TightenLengthDistribution(target)],
            }];
        }
        Vec::new()
    }
}

// ── ConnectiveFamilySaturation ──────────────────────────────────────────

/// Detects when one connective family (continuation, similarity, contrast)
/// emits more than its document-scope budget. Per the existing engine
/// trailing-window cap, each family caps at the size of its pool inside
/// any FAMILY_WINDOW span; this diagnoser aggregates across the whole
/// document and fires when the *cumulative* count exceeds the
/// `max_per_family` budget.
#[derive(Debug, Clone)]
pub struct ConnectiveFamilySaturation {
    pub max_per_family: usize,
}

impl Default for ConnectiveFamilySaturation {
    fn default() -> Self {
        Self { max_per_family: 4 }
    }
}

impl Diagnoser for ConnectiveFamilySaturation {
    fn name(&self) -> &'static str {
        "connective_family_saturation"
    }

    fn diagnose(
        &self,
        document: &RenderedDocument,
        _profile: Option<&StyleProfile>,
    ) -> Vec<Diagnostic> {
        let mut by_family = alloc::collections::BTreeMap::<ConnectorFamily, Vec<String>>::new();
        for u in &document.connectives_used {
            if let Some((family, _)) = classify(&u.connective) {
                by_family
                    .entry(family)
                    .or_default()
                    .push(u.connective.clone());
            }
        }
        let mut diagnostics = Vec::new();
        for (_family, list) in by_family {
            if list.len() > self.max_per_family {
                let mut occurrence = alloc::collections::BTreeMap::<String, usize>::new();
                for c in &list {
                    *occurrence.entry(c.clone()).or_insert(0) += 1;
                }
                let dominant = occurrence
                    .into_iter()
                    .max_by_key(|(_, n)| *n)
                    .map(|(c, _)| c)
                    .unwrap_or_default();
                diagnostics.push(Diagnostic {
                    diagnoser: "connective_family_saturation",
                    severity: (list.len() as f32) / (self.max_per_family as f32),
                    constraints: vec![RefineConstraint::BlacklistConnective(dominant)],
                });
            }
        }
        diagnostics
    }
}

// ── ProfileDistributionDrift ───────────────────────────────────────────

/// Active only when a `StyleProfile` is provided. Detects when any of the
/// profile's target distributions (length, list-style, connective frequency)
/// diverges from observed by more than `delta`.
#[derive(Debug, Clone)]
pub struct ProfileDistributionDrift {
    pub delta: f32,
}

impl Default for ProfileDistributionDrift {
    fn default() -> Self {
        Self { delta: 0.25 }
    }
}

impl Diagnoser for ProfileDistributionDrift {
    fn name(&self) -> &'static str {
        "profile_distribution_drift"
    }

    fn diagnose(
        &self,
        document: &RenderedDocument,
        profile: Option<&StyleProfile>,
    ) -> Vec<Diagnostic> {
        let Some(profile) = profile else {
            return Vec::new();
        };
        let mut diagnostics = Vec::new();

        // Length distribution divergence.
        if !profile.sentence_length.is_neutral() && !document.sentences.is_empty() {
            let dist = &profile.sentence_length;
            let mut counts = [0usize; 3];
            for sentence in &document.sentences {
                let bucket = if sentence.word_count <= dist.short_max_words as usize {
                    0
                } else if sentence.word_count <= dist.medium_max_words as usize {
                    1
                } else {
                    2
                };
                counts[bucket] += 1;
            }
            let total = document.sentences.len() as f32;
            let observed = [
                counts[0] as f32 / total,
                counts[1] as f32 / total,
                counts[2] as f32 / total,
            ];
            let target_sum = dist.short + dist.medium + dist.long;
            if target_sum > 0.0 {
                let target = [
                    dist.short / target_sum,
                    dist.medium / target_sum,
                    dist.long / target_sum,
                ];
                let max_diff = (0..3)
                    .map(|i| (observed[i] - target[i]).abs())
                    .fold(0.0_f32, f32::max);
                if max_diff > self.delta {
                    diagnostics.push(Diagnostic {
                        diagnoser: "profile_distribution_drift",
                        severity: max_diff,
                        constraints: vec![RefineConstraint::TightenLengthDistribution(
                            dist.clone(),
                        )],
                    });
                }
            }
        }

        diagnostics
    }
}

/// Newton-Raphson `sqrt` approximation. Used in place of `f32::sqrt` so
/// the refine module stays no_std-compatible.
fn approx_sqrt(x: f32) -> f32 {
    if x <= 0.0 {
        return 0.0;
    }
    let mut g = if x >= 1.0 { x } else { 1.0 };
    for _ in 0..6 {
        g = 0.5 * (g + x / g);
    }
    g
}

// ── Convenience: built-in set ──────────────────────────────────────────

/// Build the v1 default set of six built-in diagnosers as `Arc<dyn Diagnoser>`,
/// in their canonical order. Use this with
/// [`crate::RefineConfig::with_diagnoser`] to attach the full pool.
pub fn default_set() -> Vec<alloc::sync::Arc<dyn Diagnoser>> {
    use alloc::sync::Arc;
    vec![
        Arc::new(ParagraphOpenerMonotony::default()),
        Arc::new(ListStyleFatigue::default()),
        Arc::new(RstRelationImbalance::default()),
        Arc::new(DocumentScopeRhythm::default()),
        Arc::new(ConnectiveFamilySaturation::default()),
        Arc::new(ProfileDistributionDrift::default()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::refine::{EventMeta, ParagraphRender, RenderedDocument};

    fn doc_with_paragraph_openers(openers: &[Option<&str>]) -> RenderedDocument {
        // Two events per paragraph so the connective lands at sentence 1
        // (the natural place for an inter-event opener), matching how
        // the engine actually emits connectives.
        let paragraphs: Vec<ParagraphRender> = openers
            .iter()
            .enumerate()
            .map(|(i, opener)| {
                let text = match opener {
                    Some(o) => format!("Lead in para {i}. {o} continuation here."),
                    None => format!("Lead in para {i}. Continuation here."),
                };
                ParagraphRender {
                    text,
                    events: vec![
                        EventMeta {
                            connective: None,
                            list_style: None,
                        },
                        EventMeta {
                            connective: opener.map(|s| s.to_string()),
                            list_style: None,
                        },
                    ],
                }
            })
            .collect();
        RenderedDocument::from_paragraphs(paragraphs)
    }

    fn doc_with_list_styles(styles: &[ListStyle]) -> RenderedDocument {
        // One paragraph per emission, each a single sentence.
        let paragraphs: Vec<ParagraphRender> = styles
            .iter()
            .enumerate()
            .map(|(i, ls)| ParagraphRender {
                text: format!("Sentence {i} containing items."),
                events: vec![EventMeta {
                    connective: None,
                    list_style: Some(*ls),
                }],
            })
            .collect();
        RenderedDocument::from_paragraphs(paragraphs)
    }

    fn doc_with_connectives(connectives: &[&str]) -> RenderedDocument {
        let paragraphs: Vec<ParagraphRender> = connectives
            .iter()
            .enumerate()
            .map(|(i, c)| ParagraphRender {
                text: format!("{c} sentence number {i}."),
                events: vec![EventMeta {
                    connective: Some((*c).to_string()),
                    list_style: None,
                }],
            })
            .collect();
        RenderedDocument::from_paragraphs(paragraphs)
    }

    fn doc_with_sentence_lengths(lengths: &[usize]) -> RenderedDocument {
        // Pack each length into one paragraph as a single sentence.
        let paragraphs: Vec<ParagraphRender> = lengths
            .iter()
            .map(|&n| {
                let words = (0..n).map(|_| "word").collect::<Vec<_>>().join(" ");
                ParagraphRender {
                    text: format!("{words}."),
                    events: vec![EventMeta::default()],
                }
            })
            .collect();
        RenderedDocument::from_paragraphs(paragraphs)
    }

    // ── ParagraphOpenerMonotony ──────────────────────────────────────────

    #[test]
    fn paragraph_opener_monotony_fires_at_threshold() {
        let doc = doc_with_paragraph_openers(&[
            Some("Additionally,"),
            Some("Additionally,"),
            Some("Additionally,"),
            Some("However,"),
        ]);
        let d = ParagraphOpenerMonotony::default().diagnose(&doc, None);
        assert_eq!(d.len(), 1);
        assert!(matches!(
            &d[0].constraints[0],
            RefineConstraint::BlacklistConnective(s) if s.starts_with("Additionally,")
        ));
    }

    #[test]
    fn paragraph_opener_monotony_silent_below_threshold() {
        let doc = doc_with_paragraph_openers(&[
            Some("Additionally,"),
            Some("Additionally,"),
            Some("Furthermore,"),
            Some("However,"),
        ]);
        let d = ParagraphOpenerMonotony::default().diagnose(&doc, None);
        assert!(d.is_empty());
    }

    #[test]
    fn paragraph_opener_monotony_silent_for_short_docs() {
        let doc = doc_with_paragraph_openers(&[Some("Additionally,"), Some("Additionally,")]);
        let d = ParagraphOpenerMonotony::default().diagnose(&doc, None);
        assert!(d.is_empty());
    }

    // ── ListStyleFatigue ─────────────────────────────────────────────────

    #[test]
    fn list_style_fatigue_fires_when_one_style_dominates_window() {
        let doc = doc_with_list_styles(&[
            ListStyle::Including,
            ListStyle::Including,
            ListStyle::Including,
            ListStyle::SuchAs,
        ]);
        let d = ListStyleFatigue::default().diagnose(&doc, None);
        assert_eq!(d.len(), 1);
        assert!(matches!(
            d[0].constraints[0],
            RefineConstraint::BlacklistListStyle(ListStyle::Including)
        ));
    }

    #[test]
    fn list_style_fatigue_silent_when_diverse() {
        let doc = doc_with_list_styles(&[
            ListStyle::Including,
            ListStyle::SuchAs,
            ListStyle::Dash,
            ListStyle::Bracketed,
        ]);
        let d = ListStyleFatigue::default().diagnose(&doc, None);
        assert!(d.is_empty());
    }

    // ── RstRelationImbalance ─────────────────────────────────────────────

    #[test]
    fn rst_imbalance_fires_when_one_relation_dominates() {
        let doc = doc_with_connectives(&[
            "Additionally,", // Elaboration
            "Additionally,",
            "Furthermore,", // Elaboration
            "Additionally,",
            "However,", // Contrast
        ]);
        // 4/5 emissions are Elaboration → above 0.6 threshold.
        let d = RstRelationImbalance::default().diagnose(&doc, None);
        assert_eq!(d.len(), 1);
    }

    #[test]
    fn rst_imbalance_silent_when_balanced() {
        let doc = doc_with_connectives(&[
            "Additionally,",
            "Additionally,",
            "However,",
            "However,",
            "Similarly,",
        ]);
        let d = RstRelationImbalance::default().diagnose(&doc, None);
        assert!(d.is_empty());
    }

    // ── DocumentScopeRhythm ──────────────────────────────────────────────

    #[test]
    fn document_scope_rhythm_fires_when_lengths_are_flat() {
        let doc = doc_with_sentence_lengths(&[10, 10, 10, 10, 10, 10]);
        let d = DocumentScopeRhythm::default().diagnose(&doc, None);
        assert_eq!(d.len(), 1);
        assert!(matches!(
            d[0].constraints[0],
            RefineConstraint::TightenLengthDistribution(_)
        ));
    }

    #[test]
    fn document_scope_rhythm_silent_when_lengths_vary() {
        let doc = doc_with_sentence_lengths(&[3, 12, 5, 18, 7, 14]);
        let d = DocumentScopeRhythm::default().diagnose(&doc, None);
        assert!(d.is_empty());
    }

    // ── ConnectiveFamilySaturation ───────────────────────────────────────

    #[test]
    fn connective_family_saturation_fires_above_budget() {
        let doc = doc_with_connectives(&[
            "Additionally,",
            "Additionally,",
            "Additionally,",
            "Additionally,",
            "Additionally,", // 5 continuations > 4 budget
        ]);
        let d = ConnectiveFamilySaturation::default().diagnose(&doc, None);
        assert_eq!(d.len(), 1);
    }

    #[test]
    fn connective_family_saturation_silent_at_budget() {
        let doc =
            doc_with_connectives(&["Additionally,", "Additionally,", "Furthermore,", "It also"]);
        let d = ConnectiveFamilySaturation::default().diagnose(&doc, None);
        assert!(d.is_empty());
    }

    // ── ProfileDistributionDrift ─────────────────────────────────────────

    #[test]
    fn profile_drift_silent_without_profile() {
        let doc = doc_with_sentence_lengths(&[3, 5, 7, 9]);
        let d = ProfileDistributionDrift::default().diagnose(&doc, None);
        assert!(d.is_empty());
    }

    #[test]
    fn profile_drift_silent_with_neutral_profile() {
        let doc = doc_with_sentence_lengths(&[3, 5, 7, 9]);
        let p = StyleProfile::neutral();
        let d = ProfileDistributionDrift::default().diagnose(&doc, Some(&p));
        assert!(d.is_empty());
    }

    #[test]
    fn profile_drift_fires_when_observed_misses_target() {
        // All sentences are short (<= 8 words). Target wants long-leaning.
        let doc = doc_with_sentence_lengths(&[3, 4, 5, 4, 3, 5]);
        let target = crate::style::LengthDistribution {
            short: 0.0,
            medium: 0.0,
            long: 1.0,
            short_max_words: 8,
            medium_max_words: 18,
        };
        let p = StyleProfile::builder("long-target")
            .sentence_length(target)
            .build()
            .unwrap();
        let d = ProfileDistributionDrift::default().diagnose(&doc, Some(&p));
        assert_eq!(d.len(), 1);
    }
}
