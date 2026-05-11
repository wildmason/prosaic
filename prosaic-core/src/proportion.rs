//! Proportional quantification — natural phrasing for "X of Y" ratios.
//!
//! Raw `{n} of {t}` templates produce awkward output when `n` equals `t`:
//! "2 of 2 modified files" reads robotically; humans write "both modified
//! files" or "all 13 modified files" instead. This module owns the
//! full noun phrase so the surface form can collapse whenever the
//! numerator saturates the denominator.
//!
//! The pure function [`english_proportion`] produces the English phrase
//! for a numerator / denominator / optional singular noun triple.
//! Non-English grammars override [`crate::Language::proportion_phrase`]
//! to produce locale-appropriate phrasing.
//!
//! Buckets (noun = "modified file"):
//!
//! | n / t          | Output                                |
//! |----------------|---------------------------------------|
//! | 0 / 0          | `no modified files`                   |
//! | 0 / N (N>0)    | `none of the N modified files`        |
//! | 1 / 1          | `the only modified file`              |
//! | 2 / 2          | `both modified files`                 |
//! | N / N (N>2)    | `all N modified files`                |
//! | 1 / N (N>1)    | `1 of N modified files`               |
//! | n / t (n<t)    | `n of t modified files`               |
//!
//! Without a noun argument, a parallel bare form is produced:
//! "both" / "all 13" / "3 of 13" / "none" / "the only one".

#[cfg(not(feature = "std"))]
use alloc::format;
#[cfg(not(feature = "std"))]
use alloc::string::{String, ToString};

use crate::language::Language;

/// Produce the English proportion phrase for `matching / total`, optionally
/// suffixed with a pluralized form of `noun`.
///
/// Negative counts are clamped to zero. When `total <= 0` but `matching > 0`
/// the function falls through to the plain "n of t" form — the input is
/// self-inconsistent and the caller is best served by a literal rendering
/// rather than a silent re-interpretation.
pub fn english_proportion<L: Language + ?Sized>(
    lang: &L,
    matching: i64,
    total: i64,
    noun: Option<&str>,
) -> String {
    let n = matching.max(0);
    let t = total.max(0);

    if t == 0 {
        return match noun {
            Some(noun) if n == 0 => format!("no {}", lang.pluralize(noun, 0)),
            None if n == 0 => "none".to_string(),
            Some(noun) => format!("{n} of 0 {}", lang.pluralize(noun, n as usize)),
            None => format!("{n} of 0"),
        };
    }

    if n == 0 {
        return match noun {
            Some(noun) => format!("none of the {t} {}", lang.pluralize(noun, t as usize)),
            None => format!("none of the {t}"),
        };
    }

    if n >= t {
        return match (noun, t) {
            (Some(noun), 1) => format!("the only {}", lang.pluralize(noun, 1)),
            (None, 1) => "the only one".to_string(),
            (Some(noun), 2) => format!("both {}", lang.pluralize(noun, 2)),
            (None, 2) => "both".to_string(),
            (Some(noun), _) => format!("all {t} {}", lang.pluralize(noun, t as usize)),
            (None, _) => format!("all {t}"),
        };
    }

    match noun {
        Some(noun) => format!("{n} of {t} {}", lang.pluralize(noun, t as usize)),
        None => format!("{n} of {t}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::language::{Conjunction, Language, Person, Tense};

    /// Minimal English-style language for these tests. Just enough
    /// pluralization to exercise the function's noun-suffix path.
    struct MiniLang;

    impl Language for MiniLang {
        fn pluralize(&self, word: &str, count: usize) -> String {
            if count == 1 {
                return word.to_string();
            }
            // Very basic English plural — sufficient for "file" / "service" / "class".
            if word.ends_with("ss")
                || word.ends_with("sh")
                || word.ends_with("ch")
                || word.ends_with('x')
            {
                format!("{word}es")
            } else {
                format!("{word}s")
            }
        }
        fn singularize(&self, word: &str) -> String {
            word.strip_suffix('s').unwrap_or(word).to_string()
        }
        fn article(&self, _word: &str) -> &str {
            "a"
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

    fn lang() -> MiniLang {
        MiniLang
    }

    // ─── With noun argument ───────────────────────────────────────────────

    #[test]
    fn zero_of_zero_with_noun_reads_no_plural() {
        assert_eq!(
            english_proportion(&lang(), 0, 0, Some("modified file")),
            "no modified files"
        );
    }

    #[test]
    fn zero_of_n_with_noun_reads_none_of_the_n() {
        assert_eq!(
            english_proportion(&lang(), 0, 5, Some("modified file")),
            "none of the 5 modified files"
        );
    }

    #[test]
    fn one_of_one_with_noun_reads_the_only_singular() {
        assert_eq!(
            english_proportion(&lang(), 1, 1, Some("modified file")),
            "the only modified file"
        );
    }

    #[test]
    fn two_of_two_with_noun_reads_both_plural() {
        assert_eq!(
            english_proportion(&lang(), 2, 2, Some("modified file")),
            "both modified files"
        );
    }

    #[test]
    fn all_n_with_noun_reads_all_n_plural() {
        assert_eq!(
            english_proportion(&lang(), 13, 13, Some("modified file")),
            "all 13 modified files"
        );
    }

    #[test]
    fn one_of_n_with_noun_reads_one_of_n_plural() {
        assert_eq!(
            english_proportion(&lang(), 1, 5, Some("modified file")),
            "1 of 5 modified files"
        );
    }

    #[test]
    fn partial_with_noun_reads_n_of_t_plural() {
        assert_eq!(
            english_proportion(&lang(), 3, 13, Some("modified file")),
            "3 of 13 modified files"
        );
    }

    // ─── Without noun argument ────────────────────────────────────────────

    #[test]
    fn zero_of_zero_no_noun_reads_none() {
        assert_eq!(english_proportion(&lang(), 0, 0, None), "none");
    }

    #[test]
    fn zero_of_n_no_noun_reads_none_of_the_n() {
        assert_eq!(english_proportion(&lang(), 0, 5, None), "none of the 5");
    }

    #[test]
    fn one_of_one_no_noun_reads_the_only_one() {
        assert_eq!(english_proportion(&lang(), 1, 1, None), "the only one");
    }

    #[test]
    fn two_of_two_no_noun_reads_both() {
        assert_eq!(english_proportion(&lang(), 2, 2, None), "both");
    }

    #[test]
    fn all_n_no_noun_reads_all_n() {
        assert_eq!(english_proportion(&lang(), 13, 13, None), "all 13");
    }

    #[test]
    fn one_of_n_no_noun_reads_one_of_n() {
        assert_eq!(english_proportion(&lang(), 1, 5, None), "1 of 5");
    }

    #[test]
    fn partial_no_noun_reads_n_of_t() {
        assert_eq!(english_proportion(&lang(), 3, 13, None), "3 of 13");
    }

    // ─── Edge cases ───────────────────────────────────────────────────────

    #[test]
    fn numerator_exceeds_denominator_saturates_to_all() {
        // A nonsensical input (n > t) still collapses cleanly — treat as
        // saturated rather than producing "7 of 5".
        assert_eq!(
            english_proportion(&lang(), 7, 5, Some("file")),
            "all 5 files"
        );
    }

    #[test]
    fn negative_numerator_clamps_to_zero() {
        assert_eq!(
            english_proportion(&lang(), -3, 5, Some("file")),
            "none of the 5 files"
        );
    }

    #[test]
    fn negative_denominator_with_zero_numerator_reads_no_plural() {
        assert_eq!(english_proportion(&lang(), 0, -4, Some("file")), "no files");
    }

    #[test]
    fn pluralizes_irregular_noun_suffix() {
        // "class" → "classes" (test uses simplified rules; real English
        // grammar handles this via the Language trait).
        assert_eq!(
            english_proportion(&lang(), 3, 3, Some("class")),
            "all 3 classes"
        );
    }

    #[test]
    fn respects_singular_noun_form_for_one_of_one() {
        // "the only modified file" — NOT "the only modified files".
        let out = english_proportion(&lang(), 1, 1, Some("modified file"));
        assert!(
            !out.contains("files"),
            "expected singular noun for 1/1, got: {out}"
        );
    }

    #[test]
    fn positive_numerator_with_zero_total_falls_through_literally() {
        // Self-inconsistent input; we render it literally rather than
        // re-interpreting. Callers shouldn't send this, but if they do
        // we don't silently correct.
        assert_eq!(
            english_proportion(&lang(), 2, 0, Some("file")),
            "2 of 0 files"
        );
    }
}
