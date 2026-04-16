//! PARENT-style reference-free faithfulness scoring.
//!
//! Given a rendered output, the Context that produced it, and the
//! template's literal tokens, score how faithful the output is to its
//! inputs. Catches two failure modes:
//!
//! 1. **Hallucinated content** — tokens in the output that have no
//!    source in the Context or template literals. Surfaced as a low
//!    `precision` score and a list of unentailed tokens.
//! 2. **Polarity drift** — the hypothesis has a different count of
//!    negation tokens than the source. Surfaced as `polarity_match =
//!    false` and a list of drifted tokens.
//!
//! Morphological tolerance: singular/plural forms match via the
//! Language trait's `singularize` method. Beyond that, matching is
//! exact (after lowercasing and edge-punctuation stripping).
//!
//! **Note:** Polarity tokens (`not`, `never`, `no`, etc.) are NOT
//! treated as stopwords. They are handled separately via a dedicated
//! multiset comparison. This is intentional and differs from the
//! `discourse.rs` stopwords list which includes `no` and `not`.
//!
//! **Note:** This module maintains its own stopwords list, separate
//! from `discourse.rs`. The two serve different purposes and a future
//! refactor can unify them if appropriate.
//!
//! **Limitation — partials:** Template literals retrieved via
//! `Template::literal_tokens()` do not include text inside
//! `{>partial_name}` expansions, since partials are opaque at parse
//! time. Templates that rely heavily on partials for prose may score
//! lower precision than expected. Callers can pre-expand partials or
//! disable the gate for those templates.
//!
//! **Limitation — cross-linguistic polarity:** The polarity token list
//! is English-only. A future extension point would be a
//! `Language::polarity_tokens()` method; for v1 this is hardcoded.
//!
//! See `docs/plans/parent-faithfulness.md` for design rationale.

#[cfg(not(feature = "std"))]
use alloc::string::{String, ToString};
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use crate::collections::{HashSet, new_set};
use crate::context::{Context, Value};
use crate::language::Language;

/// Polarity (negation) tokens treated separately from content tokens.
/// These are NOT in STOPWORDS — they are scored as a distinct multiset gate.
const POLARITY_TOKENS: &[&str] = &[
    "not", "never", "no", "none", "cannot", "won't", "neither", "nor",
];

/// Stopwords excluded from content scoring. Polarity tokens are deliberately
/// absent from this list — they are handled by the polarity gate instead.
const STOPWORDS: &[&str] = &[
    "a", "an", "the", "and", "or", "but", "in", "on", "at", "to", "for", "of", "with", "by",
    "from", "is", "was", "are", "were", "be", "been", "being", "have", "has", "had", "do", "does",
    "did", "will", "would", "could", "should", "may", "might", "shall", "can", "it", "its", "this",
    "that", "these", "those", "which", "who", "what", "where", "when", "how", "if", "then", "than",
    "so", "as", "up", "out", "into", "also", "just", "more", "most",
];

/// A faithfulness score for a rendered hypothesis against its source.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FaithfulnessScore {
    /// Fraction of hypothesis content tokens entailed by source. `[0.0, 1.0]`.
    pub precision: f32,
    /// True iff polarity-token multisets match exactly between source and hypothesis.
    pub polarity_match: bool,
    /// Hypothesis content tokens not entailed by source (hallucinated words).
    pub unentailed: Vec<String>,
    /// Polarity tokens whose counts differ between hypothesis and source.
    /// Empty iff `polarity_match` is true.
    pub polarity_drift: Vec<PolarityDrift>,
}

/// A single polarity token whose count differs between source and hypothesis.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PolarityDrift {
    /// The polarity token (e.g. `"not"`, `"never"`).
    pub token: String,
    /// How many times it appears in the source (context + template literals).
    pub in_source: usize,
    /// How many times it appears in the hypothesis (rendered output).
    pub in_hypothesis: usize,
}

impl FaithfulnessScore {
    /// True iff `precision == 1.0` AND `polarity_match`. Use this for strict
    /// template conformance tests where zero hallucination is required.
    pub fn is_faithful(&self) -> bool {
        self.precision >= 1.0 && self.polarity_match
    }

    /// True iff `precision >= threshold` AND `polarity_match`. Use for
    /// runtime gates that tolerate a small fraction of unentailed tokens —
    /// e.g. hedged phrasings that legitimately introduce words not in the
    /// input context.
    pub fn passes(&self, threshold: f32) -> bool {
        self.precision >= threshold && self.polarity_match
    }
}

/// Score the faithfulness of a rendered `output` against the [`Context`]
/// that produced it and the `template_literals` that anchored it.
///
/// The `language` parameter is used for morphological tolerance:
/// singular/plural pairs are treated as equivalent via
/// [`Language::singularize`].
///
/// # Scoring model
///
/// - **Source set:** union of tokens from every Context value plus every
///   template literal token.
/// - **Content token:** lowercase, edge-punctuation-stripped, length ≥ 3,
///   not a stopword, not a polarity token, not purely numeric.
/// - **Entailment:** exact match on the source set, OR the hypothesis token's
///   singular form matches a source token, OR any source token's singular form
///   matches the hypothesis token (bidirectional singularization).
/// - **Precision:** entailed content tokens / total content tokens.
///   Vacuously 1.0 when the hypothesis has no content tokens.
/// - **Polarity gate:** checked separately; counts each polarity token in
///   source and hypothesis and flags mismatches.
pub fn score_faithfulness(
    output: &str,
    context: &Context,
    template_literals: &[&str],
    language: &dyn Language,
) -> FaithfulnessScore {
    let hyp_tokens = tokenize(output);
    let src_tokens: Vec<String> = {
        let mut v = tokens_from_context(context);
        v.extend(tokens_from_literals(template_literals));
        v
    };

    // ── Polarity gate ──────────────────────────────────────────────────
    // Separate from content scoring. Each polarity token's count must
    // match exactly between source and hypothesis.
    let mut polarity_drift = Vec::new();
    let mut polarity_match = true;
    for &tok in POLARITY_TOKENS {
        let s = src_tokens
            .iter()
            .filter(|t| t.as_ref() as &str == tok)
            .count();
        let h = hyp_tokens
            .iter()
            .filter(|t| t.as_ref() as &str == tok)
            .count();
        if s != h {
            polarity_match = false;
            polarity_drift.push(PolarityDrift {
                token: tok.to_string(),
                in_source: s,
                in_hypothesis: h,
            });
        }
    }

    // ── Content precision ──────────────────────────────────────────────
    let hyp_content: Vec<&String> = hyp_tokens.iter().filter(|t| is_content_token(t)).collect();

    if hyp_content.is_empty() {
        return FaithfulnessScore {
            precision: 1.0,
            polarity_match,
            unentailed: Vec::new(),
            polarity_drift,
        };
    }

    // Build a normalised source set: each source token contributes its
    // own form AND its singularized form for bidirectional tolerance.
    let mut src_set: HashSet<String> = new_set();
    for t in &src_tokens {
        src_set.insert(t.clone());
        src_set.insert(language.singularize(t));
    }

    let mut entailed: usize = 0;
    let mut unentailed: Vec<String> = Vec::new();

    for t in &hyp_content {
        let t_sing = language.singularize(t);
        if src_set.contains(t.as_ref() as &str) || src_set.contains(&t_sing) {
            entailed += 1;
        } else {
            unentailed.push((*t).clone());
        }
    }

    let precision = entailed as f32 / hyp_content.len() as f32;

    FaithfulnessScore {
        precision,
        polarity_match,
        unentailed,
        polarity_drift,
    }
}

// ── Tokenization ────────────────────────────────────────────────────────

/// Lowercase, split on whitespace, trim edge punctuation.
/// Inner hyphens and apostrophes (e.g. `user-facing`, `won't`) are preserved.
fn tokenize(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|raw| {
            raw.trim_matches(|c: char| {
                matches!(
                    c,
                    ',' | '.' | ':' | ';' | '!' | '?' | '"' | '\''
                    | '(' | ')' | '[' | ']' | '\u{2014}' // em dash
                    | '\u{2013}' // en dash
                    | '-'
                )
            })
            .to_lowercase()
        })
        .filter(|s| !s.is_empty())
        .collect()
}

// ── Classification predicates ────────────────────────────────────────────

fn is_stopword(tok: &str) -> bool {
    STOPWORDS.contains(&tok)
}

fn is_polarity(tok: &str) -> bool {
    POLARITY_TOKENS.contains(&tok)
}

fn is_numeric(tok: &str) -> bool {
    !tok.is_empty() && tok.chars().all(|c| c.is_ascii_digit())
}

fn is_content_token(tok: &str) -> bool {
    tok.len() >= 3 && !is_stopword(tok) && !is_polarity(tok) && !is_numeric(tok)
}

// ── Source-set extraction ────────────────────────────────────────────────

fn tokens_from_context(ctx: &Context) -> Vec<String> {
    let mut out = Vec::new();
    for (_key, value) in ctx.iter() {
        match value {
            Value::String(s) => out.extend(tokenize(s)),
            Value::Number(n) => out.push(n.to_string()),
            Value::List(items) => {
                for item in items {
                    out.extend(tokenize(item));
                }
            }
            // Entity renders as its name — same token contribution as String.
            Value::Entity { name, .. } => out.extend(tokenize(name)),
        }
    }
    out
}

fn tokens_from_literals(literals: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    for lit in literals {
        out.extend(tokenize(lit));
    }
    out
}

// ── Test-harness macro ───────────────────────────────────────────────────

/// Assert that a rendered output is faithful to its context and template.
///
/// Panics with a detailed diagnostic if the output has unentailed content
/// tokens or a polarity mismatch. Use this in vocab-module tests to
/// ensure templates only produce words entailed by their inputs.
///
/// # Example
///
/// ```
/// use prosaic_core::{assert_faithful, ctx};
/// use prosaic_grammar_en::English;
///
/// let ctx = ctx! { name: "UserService", action: "renamed" };
/// let lits: &[&str] = &["The class ", " was ", "."];
/// assert_faithful!("The class UserService was renamed.", ctx, lits, &English::new());
/// ```
#[macro_export]
macro_rules! assert_faithful {
    ($output:expr, $context:expr, $template_literals:expr, $language:expr $(,)?) => {{
        let score = $crate::score_faithfulness(
            &$output,
            &$context,
            &$template_literals,
            $language,
        );
        if !score.is_faithful() {
            panic!(
                "faithfulness violation:\n  precision: {:.3}\n  polarity_match: {}\n  unentailed: {:?}\n  polarity_drift: {:?}\n  output: {}\n",
                score.precision,
                score.polarity_match,
                score.unentailed,
                score.polarity_drift,
                $output,
            );
        }
    }};
}

// ── Unit tests ───────────────────────────────────────────────────────────
// Tests that require a Language impl (English) live in
// prosaic-core/tests/faithfulness_scorer.rs to avoid the two-crate identity
// issue with dev-dependencies.

#[cfg(test)]
mod tests {
    use super::*;

    // ── Tokenization ─────────────────────────────────────────────────────

    #[test]
    fn tokenize_strips_edge_punctuation() {
        let toks = tokenize("hello, world! (foo)");
        assert_eq!(toks, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn tokenize_lowercases() {
        let toks = tokenize("UserService AccountService");
        assert_eq!(toks, vec!["userservice", "accountservice"]);
    }

    #[test]
    fn tokenize_preserves_inner_hyphens_and_apostrophes() {
        let toks = tokenize("user-facing won't");
        assert_eq!(toks, vec!["user-facing", "won't"]);
    }

    #[test]
    fn tokenize_empty_string_produces_no_tokens() {
        assert!(tokenize("").is_empty());
    }

    #[test]
    fn tokenize_punctuation_only_produces_no_tokens() {
        assert!(tokenize("... ,,, ???").is_empty());
    }

    // ── Classification predicates ─────────────────────────────────────────

    #[test]
    fn content_token_respects_length_threshold() {
        // Length 2 excluded, length 3 included (if not stopword/polarity/numeric)
        assert!(!is_content_token("it"));
        assert!(is_content_token("foo")); // length 3, not excluded
    }

    #[test]
    fn content_token_excludes_stopwords() {
        assert!(!is_content_token("the"));
        assert!(!is_content_token("and"));
        assert!(!is_content_token("was"));
    }

    #[test]
    fn content_token_excludes_polarity() {
        // Polarity tokens are NOT treated as stopwords for scoring
        assert!(!is_content_token("not"));
        assert!(!is_content_token("never"));
        assert!(!is_content_token("nor"));
    }

    #[test]
    fn content_token_excludes_pure_digits() {
        assert!(!is_content_token("123"));
        assert!(!is_content_token("42"));
    }

    #[test]
    fn content_token_admits_alphanumeric_mixed() {
        // "a123" is not pure digits — should pass other checks
        assert!(is_content_token("a123")); // len 4, not stopword, not polarity, not pure numeric
    }

    // ── FaithfulnessScore struct methods ──────────────────────────────────

    #[test]
    fn is_faithful_requires_both_precision_and_polarity() {
        // Precision 1.0 but polarity mismatch → not faithful
        let score_bad_polarity = FaithfulnessScore {
            precision: 1.0,
            polarity_match: false,
            unentailed: vec![],
            polarity_drift: vec![PolarityDrift {
                token: "not".into(),
                in_source: 0,
                in_hypothesis: 1,
            }],
        };
        assert!(!score_bad_polarity.is_faithful());

        // polarity_match true but precision < 1.0 → not faithful
        let score_bad_precision = FaithfulnessScore {
            precision: 0.8,
            polarity_match: true,
            unentailed: vec!["extra".into()],
            polarity_drift: vec![],
        };
        assert!(!score_bad_precision.is_faithful());

        // Both good → faithful
        let score_good = FaithfulnessScore {
            precision: 1.0,
            polarity_match: true,
            unentailed: vec![],
            polarity_drift: vec![],
        };
        assert!(score_good.is_faithful());
    }

    #[test]
    fn passes_threshold_semantics() {
        let score = FaithfulnessScore {
            precision: 0.75,
            polarity_match: true,
            unentailed: vec!["extra".into()],
            polarity_drift: vec![],
        };
        assert!(score.passes(0.5), "0.75 >= 0.5 should pass");
        assert!(score.passes(0.75), "0.75 >= 0.75 should pass (boundary)");
        assert!(!score.passes(0.76), "0.75 < 0.76 should fail");

        // Polarity mismatch always fails regardless of threshold
        let score_polarity_bad = FaithfulnessScore {
            precision: 1.0,
            polarity_match: false,
            unentailed: vec![],
            polarity_drift: vec![PolarityDrift {
                token: "not".into(),
                in_source: 0,
                in_hypothesis: 1,
            }],
        };
        assert!(
            !score_polarity_bad.passes(0.0),
            "polarity mismatch always fails"
        );
    }
}
