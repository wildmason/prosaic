//! Quantifier naturalization.
//!
//! Turn raw integers into natural quantifier phrases. `0` becomes
//! "no", `1` becomes "a single", small counts stay exact (spelled for
//! the smallest values, digits for the rest), and large counts get
//! hedged ("hundreds of", "thousands of", "over N rounded to a
//! natural bucket"). The pipe form is `{count|quantify}` with
//! optional `:exact` and `:hedged` flavours.

use crate::language::Language;

/// How the quantifier should be framed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum QuantifyMode {
    /// Natural default: 0 → "no", 1 → "a single", small numbers spelled
    /// or digit as appropriate, large numbers hedged to natural buckets.
    #[default]
    Natural,
    /// Always emit a digit count. Still handles 0/1 specially ("no",
    /// "a single") because those read better even in exact contexts.
    Exact,
    /// Always hedge, even for small counts ("a few", "a handful of",
    /// "several"). Useful when counts come from noisy measurements.
    Hedged,
}

/// Produce a natural-language quantifier for `count` using the given
/// `mode`. The language is used to spell out small numbers ("two",
/// "three", …) via its `number_to_words` method.
pub fn quantify(count: i64, mode: QuantifyMode, lang: &dyn Language) -> String {
    match mode {
        QuantifyMode::Exact => exact(count),
        QuantifyMode::Hedged => hedged(count),
        QuantifyMode::Natural => natural(count, lang),
    }
}

fn exact(count: i64) -> String {
    match count {
        0 => "no".to_string(),
        1 => "a single".to_string(),
        _ => count.to_string(),
    }
}

fn hedged(count: i64) -> String {
    match count {
        0 => "no".to_string(),
        1 => "a single".to_string(),
        2 => "a couple of".to_string(),
        3..=5 => "a few".to_string(),
        6..=12 => "a handful of".to_string(),
        13..=19 => "a dozen or so".to_string(),
        20..=49 => "dozens of".to_string(),
        50..=199 => "scores of".to_string(),
        200..=999 => "hundreds of".to_string(),
        1000..=9999 => "thousands of".to_string(),
        _ if count >= 10_000 => "tens of thousands of".to_string(),
        _ => "no".to_string(), // negative counts treated as none
    }
}

fn natural(count: i64, lang: &dyn Language) -> String {
    match count {
        i64::MIN..=0 => "no".to_string(),
        1 => "a single".to_string(),
        2..=12 => lang.number_to_words(count as usize),
        13..=49 => count.to_string(),
        50..=99 => format!("about {}", round_to(count, 10)),
        100..=199 => "over a hundred".to_string(),
        200..=999 => "hundreds of".to_string(),
        1000..=9999 => "thousands of".to_string(),
        _ => "tens of thousands of".to_string(),
    }
}

fn round_to(n: i64, bucket: i64) -> i64 {
    if bucket <= 0 {
        return n;
    }
    let half = bucket / 2;
    ((n + half) / bucket) * bucket
}

/// Parse a quantify spec argument into a mode.
pub fn parse_mode(spec: &str) -> Option<QuantifyMode> {
    match spec {
        "natural" => Some(QuantifyMode::Natural),
        "exact" => Some(QuantifyMode::Exact),
        "hedged" => Some(QuantifyMode::Hedged),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::language::{Conjunction, Language, Person, Tense};

    /// A minimal language that spells out only 2..=12 as English words;
    /// anything else falls through to digits. Enough for these tests.
    struct MiniLang;

    impl Language for MiniLang {
        fn pluralize(&self, word: &str, count: usize) -> String {
            if count == 1 {
                word.to_string()
            } else {
                format!("{word}s")
            }
        }
        fn singularize(&self, word: &str) -> String {
            word.to_string()
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
            let words = [
                "zero", "one", "two", "three", "four", "five", "six",
                "seven", "eight", "nine", "ten", "eleven", "twelve",
            ];
            if n < words.len() {
                words[n].to_string()
            } else {
                n.to_string()
            }
        }
    }

    fn lang() -> MiniLang {
        MiniLang
    }

    #[test]
    fn natural_zero_is_no() {
        assert_eq!(quantify(0, QuantifyMode::Natural, &lang()), "no");
    }

    #[test]
    fn natural_one_is_a_single() {
        assert_eq!(quantify(1, QuantifyMode::Natural, &lang()), "a single");
    }

    #[test]
    fn natural_small_is_spelled() {
        assert_eq!(quantify(3, QuantifyMode::Natural, &lang()), "three");
        assert_eq!(quantify(12, QuantifyMode::Natural, &lang()), "twelve");
    }

    #[test]
    fn natural_medium_is_digit() {
        assert_eq!(quantify(13, QuantifyMode::Natural, &lang()), "13");
        assert_eq!(quantify(47, QuantifyMode::Natural, &lang()), "47");
    }

    #[test]
    fn natural_50s_are_hedged_about_rounded() {
        assert_eq!(quantify(55, QuantifyMode::Natural, &lang()), "about 60");
        assert_eq!(quantify(77, QuantifyMode::Natural, &lang()), "about 80");
    }

    #[test]
    fn natural_hundreds() {
        assert_eq!(
            quantify(150, QuantifyMode::Natural, &lang()),
            "over a hundred"
        );
        assert_eq!(
            quantify(473, QuantifyMode::Natural, &lang()),
            "hundreds of"
        );
    }

    #[test]
    fn natural_thousands() {
        assert_eq!(
            quantify(5_000, QuantifyMode::Natural, &lang()),
            "thousands of"
        );
        assert_eq!(
            quantify(25_000, QuantifyMode::Natural, &lang()),
            "tens of thousands of"
        );
    }

    #[test]
    fn exact_mode_uses_digit_for_non_zero_non_one() {
        assert_eq!(quantify(0, QuantifyMode::Exact, &lang()), "no");
        assert_eq!(quantify(1, QuantifyMode::Exact, &lang()), "a single");
        assert_eq!(quantify(47, QuantifyMode::Exact, &lang()), "47");
    }

    #[test]
    fn hedged_mode_uses_buckets_throughout() {
        assert_eq!(quantify(4, QuantifyMode::Hedged, &lang()), "a few");
        assert_eq!(quantify(10, QuantifyMode::Hedged, &lang()), "a handful of");
        assert_eq!(quantify(30, QuantifyMode::Hedged, &lang()), "dozens of");
        assert_eq!(quantify(300, QuantifyMode::Hedged, &lang()), "hundreds of");
    }

    #[test]
    fn negative_counts_treated_as_none() {
        assert_eq!(quantify(-5, QuantifyMode::Natural, &lang()), "no");
    }
}
