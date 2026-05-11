//! Composite scorer for the retrospective refine pass.
//!
//! Computes a single weighted-sum quality score over a [`RenderedDocument`]
//! for a given [`RefineWeights`] and optional [`StyleProfile`]. The
//! scorer is a pure function — no mutation, no side effects. Higher
//! scores are better; the iteration controller compares candidate scores
//! to decide whether each refinement iteration is improving the output.
//!
//! Each component lands in `[0.0, 1.0]` so the weighted sum stays within
//! a predictable range. Components:
//!
//! - **Repetition compliance** — 1 - average per-sentence word
//!   repetition fraction. Higher = more lexical variety across sentences.
//! - **Rhythm compliance** — 1 - normalized cadence flatness. Higher =
//!   more sentence-length variance.
//! - **Connective family balance** — 1 - dominant-family share. Higher
//!   = no single family dominates document-scope emissions.
//! - **Paragraph opener diversity** — distinct openers / total
//!   paragraphs that opened with a connective. Higher = more variety.
//! - **List-style diversity** — distinct styles / total styles emitted.
//! - **RST relation balance** — 1 - dominant-relation share.
//! - **Profile match** — 1 - L1 distance between observed and profile
//!   target length distribution; gated on profile presence.

#[cfg(not(feature = "std"))]
use alloc::string::String;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use crate::discourse::ListStyle;
use crate::refine::{RefineWeights, RenderedDocument};
use crate::rst::RstRelation;
use crate::style::StyleProfile;

/// Compute the composite score for `document` under `weights` and
/// `profile`. Returns a value in `[0.0, sum_of_weights]`. Higher is
/// better.
pub fn score_document(
    document: &RenderedDocument,
    weights: &RefineWeights,
    profile: Option<&StyleProfile>,
) -> f32 {
    weights.repetition * repetition_compliance(document)
        + weights.rhythm * rhythm_compliance(document)
        + weights.connective * connective_family_balance(document)
        + weights.paragraph_opener * paragraph_opener_diversity(document)
        + weights.list_style_diversity * list_style_diversity(document)
        + weights.rst_balance * rst_relation_balance(document)
        + weights.profile_match * profile_match(document, profile)
}

fn repetition_compliance(document: &RenderedDocument) -> f32 {
    if document.sentences.len() < 2 {
        return 1.0;
    }
    // Approximation: 1 minus the average pairwise Jaccard similarity over
    // adjacent sentences. Adjacent-pair similarity is what discourse
    // repetition perceives most strongly.
    let mut total_sim = 0.0_f32;
    let mut pairs = 0_usize;
    for window in document.sentences.windows(2) {
        let a = tokenize(&window[0].text);
        let b = tokenize(&window[1].text);
        if a.is_empty() || b.is_empty() {
            continue;
        }
        let intersection: usize = a.iter().filter(|w| b.contains(w)).count();
        let union: usize = a
            .iter()
            .chain(b.iter())
            .collect::<alloc::collections::BTreeSet<_>>()
            .len();
        if union > 0 {
            total_sim += intersection as f32 / union as f32;
            pairs += 1;
        }
    }
    if pairs == 0 {
        return 1.0;
    }
    1.0 - (total_sim / pairs as f32).clamp(0.0, 1.0)
}

fn rhythm_compliance(document: &RenderedDocument) -> f32 {
    if document.sentences.len() < 3 {
        return 1.0;
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
    // Normalize: stdev of 0 → score 0 (perfectly flat); stdev ≥ 6 → score 1.
    (stdev / 6.0_f32).clamp(0.0, 1.0)
}

/// Newton-Raphson `sqrt` approximation. Used in place of `f32::sqrt` to
/// keep the refine module no_std-compatible (the std `sqrt` impl isn't
/// available in `core` on stable).
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

fn connective_family_balance(document: &RenderedDocument) -> f32 {
    if document.connectives_used.is_empty() {
        return 1.0;
    }
    let total = document.connectives_used.len() as f32;
    let mut count = alloc::collections::BTreeMap::<&'static str, usize>::new();
    for u in &document.connectives_used {
        if let Some(family) = family_for(&u.connective) {
            *count.entry(family).or_insert(0) += 1;
        }
    }
    if count.is_empty() {
        return 1.0;
    }
    let dominant = count.values().copied().max().unwrap_or(0) as f32;
    (1.0 - dominant / total).clamp(0.0, 1.0)
}

fn paragraph_opener_diversity(document: &RenderedDocument) -> f32 {
    let openers: Vec<&String> = document
        .paragraphs
        .iter()
        .filter_map(|p| {
            p.sentences
                .first()
                .and_then(|s| s.opening_connective.as_ref())
        })
        .collect();
    if openers.is_empty() {
        return 1.0;
    }
    let distinct: alloc::collections::BTreeSet<&String> = openers.iter().copied().collect();
    (distinct.len() as f32 / openers.len() as f32).clamp(0.0, 1.0)
}

fn list_style_diversity(document: &RenderedDocument) -> f32 {
    if document.list_styles_used.is_empty() {
        return 1.0;
    }
    let distinct: alloc::collections::BTreeSet<ListStyle> = document
        .list_styles_used
        .iter()
        .map(|u| u.list_style)
        .collect();
    (distinct.len() as f32 / document.list_styles_used.len() as f32).clamp(0.0, 1.0)
}

fn rst_relation_balance(document: &RenderedDocument) -> f32 {
    if document.connectives_used.is_empty() {
        return 1.0;
    }
    let mut count = alloc::collections::BTreeMap::<RstRelation, usize>::new();
    let mut classified_total = 0_usize;
    for u in &document.connectives_used {
        if let Some(rst) = rst_for(&u.connective) {
            *count.entry(rst).or_insert(0) += 1;
            classified_total += 1;
        }
    }
    if classified_total == 0 {
        return 1.0;
    }
    let dominant = count.values().copied().max().unwrap_or(0) as f32;
    (1.0 - dominant / classified_total as f32).clamp(0.0, 1.0)
}

fn profile_match(document: &RenderedDocument, profile: Option<&StyleProfile>) -> f32 {
    let Some(profile) = profile else {
        return 1.0;
    };
    if profile.sentence_length.is_neutral() || document.sentences.is_empty() {
        return 1.0;
    }
    let dist = &profile.sentence_length;
    let mut counts = [0_usize; 3];
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
    if target_sum <= 0.0 {
        return 1.0;
    }
    let target = [
        dist.short / target_sum,
        dist.medium / target_sum,
        dist.long / target_sum,
    ];
    let l1 = (observed[0] - target[0]).abs()
        + (observed[1] - target[1]).abs()
        + (observed[2] - target[2]).abs();
    // L1 distance ranges 0..=2 for normalized distributions.
    (1.0 - l1 / 2.0).clamp(0.0, 1.0)
}

fn tokenize(text: &str) -> Vec<String> {
    text.split_whitespace()
        .filter_map(|w| {
            let cleaned: String = w
                .chars()
                .filter(|c| c.is_alphanumeric())
                .flat_map(|c| c.to_lowercase())
                .collect();
            if cleaned.len() > 2 {
                Some(cleaned)
            } else {
                None
            }
        })
        .collect()
}

fn family_for(connective: &str) -> Option<&'static str> {
    for c in &["Additionally,", "Furthermore,", "It also"] {
        if connective.starts_with(c) {
            return Some("continuation");
        }
    }
    for c in &["Similarly,", "Likewise,"] {
        if connective.starts_with(c) {
            return Some("similarity");
        }
    }
    for c in &["Meanwhile,", "However,", "On the other hand,"] {
        if connective.starts_with(c) {
            return Some("contrast");
        }
    }
    None
}

fn rst_for(connective: &str) -> Option<RstRelation> {
    for c in &["Additionally,", "Furthermore,", "It also"] {
        if connective.starts_with(c) {
            return Some(RstRelation::Elaboration);
        }
    }
    for c in &["Similarly,", "Likewise,"] {
        if connective.starts_with(c) {
            return Some(RstRelation::Sequence);
        }
    }
    for c in &["Meanwhile,", "However,", "On the other hand,"] {
        if connective.starts_with(c) {
            return Some(RstRelation::Contrast);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::refine::{EventMeta, ParagraphRender, RenderedDocument};

    fn doc_from(paragraphs: Vec<ParagraphRender>) -> RenderedDocument {
        RenderedDocument::from_paragraphs(paragraphs)
    }

    fn one_paragraph(
        text: &str,
        connective: Option<&str>,
        list_style: Option<ListStyle>,
    ) -> ParagraphRender {
        ParagraphRender {
            text: text.to_string(),
            events: vec![EventMeta {
                connective: connective.map(|s| s.to_string()),
                list_style,
            }],
        }
    }

    fn weights() -> RefineWeights {
        RefineWeights::default()
    }

    // ── Pure-function determinism ────────────────────────────────────────

    #[test]
    fn empty_document_scores_at_max() {
        // No sentences = no detected failures. Score should sum to all
        // weights at full value.
        let doc = doc_from(vec![]);
        let s = score_document(&doc, &weights(), None);
        let max = weights().repetition
            + weights().rhythm
            + weights().connective
            + weights().paragraph_opener
            + weights().list_style_diversity
            + weights().rst_balance
            + weights().profile_match;
        assert!((s - max).abs() < 0.001);
    }

    #[test]
    fn score_is_deterministic() {
        let doc = doc_from(vec![
            one_paragraph("First short sentence.", None, None),
            one_paragraph(
                "Additionally, second longer sentence with more words.",
                Some("Additionally,"),
                None,
            ),
        ]);
        let a = score_document(&doc, &weights(), None);
        let b = score_document(&doc, &weights(), None);
        assert_eq!(a, b);
    }

    // ── Monotonicity ─────────────────────────────────────────────────────

    #[test]
    fn rhythm_compliance_higher_with_more_variance() {
        let flat = doc_from(
            (0..6)
                .map(|i| {
                    one_paragraph(
                        &format!(
                            "{} word word word word word word word word word.",
                            "x".repeat(i + 1)
                        ),
                        None,
                        None,
                    )
                })
                .collect(),
        );
        let varied = doc_from(vec![
            one_paragraph("Short.", None, None),
            one_paragraph("A medium length sentence here for context.", None, None),
            one_paragraph(
                "And a much longer sentence with several clauses extending well beyond average length.",
                None,
                None,
            ),
            one_paragraph("Tiny.", None, None),
            one_paragraph(
                "Another medium length sentence with reasonable word count.",
                None,
                None,
            ),
            one_paragraph(
                "Yet another extended one with more words to really push the variance up.",
                None,
                None,
            ),
        ]);
        assert!(rhythm_compliance(&varied) > rhythm_compliance(&flat));
    }

    #[test]
    fn paragraph_opener_diversity_higher_with_distinct_openers() {
        let monotone = doc_from(
            (0..4)
                .map(|_| {
                    one_paragraph(
                        "Additionally, opener text here.",
                        Some("Additionally,"),
                        None,
                    )
                })
                .collect(),
        );
        let diverse = doc_from(vec![
            one_paragraph("Additionally, opener.", Some("Additionally,"), None),
            one_paragraph("Furthermore, opener.", Some("Furthermore,"), None),
            one_paragraph("However, opener.", Some("However,"), None),
            one_paragraph("Similarly, opener.", Some("Similarly,"), None),
        ]);
        assert!(paragraph_opener_diversity(&diverse) > paragraph_opener_diversity(&monotone));
    }

    #[test]
    fn list_style_diversity_higher_with_distinct_styles() {
        let monotone = doc_from(
            (0..4)
                .map(|_| one_paragraph("Sentence with list.", None, Some(ListStyle::Including)))
                .collect(),
        );
        let diverse = doc_from(vec![
            one_paragraph("Sentence.", None, Some(ListStyle::Including)),
            one_paragraph("Sentence.", None, Some(ListStyle::SuchAs)),
            one_paragraph("Sentence.", None, Some(ListStyle::Dash)),
            one_paragraph("Sentence.", None, Some(ListStyle::Bracketed)),
        ]);
        assert!(list_style_diversity(&diverse) > list_style_diversity(&monotone));
    }

    #[test]
    fn rst_relation_balance_higher_when_balanced() {
        let imbalanced = doc_from(
            (0..5)
                .map(|_| one_paragraph("Additionally, sentence.", Some("Additionally,"), None))
                .collect(),
        );
        let balanced = doc_from(vec![
            one_paragraph("Additionally, sentence.", Some("Additionally,"), None),
            one_paragraph("However, sentence.", Some("However,"), None),
            one_paragraph("Similarly, sentence.", Some("Similarly,"), None),
            one_paragraph("Furthermore, sentence.", Some("Furthermore,"), None),
            one_paragraph("Likewise, sentence.", Some("Likewise,"), None),
        ]);
        assert!(rst_relation_balance(&balanced) > rst_relation_balance(&imbalanced));
    }

    #[test]
    fn profile_match_higher_when_distribution_aligns() {
        let target = crate::style::LengthDistribution {
            short: 1.0,
            medium: 0.0,
            long: 0.0,
            short_max_words: 8,
            medium_max_words: 18,
        };
        let p = StyleProfile::builder("short-only")
            .sentence_length(target)
            .build()
            .unwrap();
        let aligned = doc_from(
            (0..6)
                .map(|_| {
                    one_paragraph("Short text here.", None, None) // 3 words → short
                })
                .collect(),
        );
        let misaligned = doc_from(
            (0..6)
                .map(|_| {
                    one_paragraph(
                        "A long sentence with many many words far above the short threshold count.",
                        None,
                        None,
                    )
                })
                .collect(),
        );
        assert!(profile_match(&aligned, Some(&p)) > profile_match(&misaligned, Some(&p)));
    }

    #[test]
    fn full_score_increases_when_one_dimension_strictly_improves() {
        // Same documents except `improved` has more diverse paragraph
        // openers. All other components compute the same — so the full
        // score must rise by ≈ weights.paragraph_opener × delta.
        let mono_openers = doc_from(vec![
            one_paragraph("Additionally, foo.", Some("Additionally,"), None),
            one_paragraph("Additionally, bar.", Some("Additionally,"), None),
            one_paragraph("Additionally, baz.", Some("Additionally,"), None),
            one_paragraph("Additionally, qux.", Some("Additionally,"), None),
        ]);
        let diverse_openers = doc_from(vec![
            one_paragraph("Additionally, foo.", Some("Additionally,"), None),
            one_paragraph("Furthermore, bar.", Some("Furthermore,"), None),
            one_paragraph("However, baz.", Some("However,"), None),
            one_paragraph("Similarly, qux.", Some("Similarly,"), None),
        ]);
        assert!(
            score_document(&diverse_openers, &weights(), None)
                > score_document(&mono_openers, &weights(), None)
        );
    }

    // ── Tokenizer guard ──────────────────────────────────────────────────

    #[test]
    fn tokenize_drops_short_and_punct() {
        let toks = tokenize("a, foo bar! the. baz?");
        assert_eq!(
            toks,
            vec![
                "foo".to_string(),
                "bar".to_string(),
                "the".to_string(),
                "baz".to_string()
            ]
        );
    }
}
