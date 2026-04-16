//! German cardinal number spelling.
//!
//! Covers 0–999,999 with German's reversed compound structure:
//! 21 = einundzwanzig (one-and-twenty), 123 = einhundertdreiundzwanzig.
//!
//! Special stems: sech (from sechs) in 16/60, sieb (from sieben) in 17/70.
//! 30 = dreißig (not *dreißzig).

pub fn number_to_words_de(n: usize) -> String {
    match n {
        0 => "null".into(),
        1 => "eins".into(),
        2 => "zwei".into(),
        3 => "drei".into(),
        4 => "vier".into(),
        5 => "fünf".into(),
        6 => "sechs".into(),
        7 => "sieben".into(),
        8 => "acht".into(),
        9 => "neun".into(),
        10 => "zehn".into(),
        11 => "elf".into(),
        12 => "zwölf".into(),
        13 => "dreizehn".into(),
        14 => "vierzehn".into(),
        15 => "fünfzehn".into(),
        16 => "sechzehn".into(),  // sechs → sech
        17 => "siebzehn".into(),  // sieben → sieb
        18 => "achtzehn".into(),
        19 => "neunzehn".into(),
        20..=99 => compound_tens(n),
        100..=999 => compound_hundreds(n),
        1_000..=999_999 => compound_thousands(n),
        _ => n.to_string(),
    }
}

fn tens_word(t: usize) -> &'static str {
    match t {
        2 => "zwanzig",
        3 => "dreißig",
        4 => "vierzig",
        5 => "fünfzig",
        6 => "sechzig",  // sechs → sech
        7 => "siebzig",  // sieben → sieb
        8 => "achtzig",
        9 => "neunzig",
        _ => "",
    }
}

fn compound_tens(n: usize) -> String {
    let tens = n / 10;
    let ones = n % 10;
    if ones == 0 {
        tens_word(tens).into()
    } else {
        // "eins" becomes "ein" in compounds
        let ones_word = if ones == 1 {
            "ein".to_string()
        } else {
            number_to_words_de(ones)
        };
        format!("{}und{}", ones_word, tens_word(tens))
    }
}

fn compound_hundreds(n: usize) -> String {
    let h = n / 100;
    let rest = n % 100;
    let h_prefix = if h == 1 {
        "einhundert".to_string()
    } else {
        format!("{}hundert", number_to_words_de(h))
    };
    if rest == 0 {
        h_prefix
    } else {
        format!("{}{}", h_prefix, number_to_words_de(rest))
    }
}

fn compound_thousands(n: usize) -> String {
    let k = n / 1_000;
    let rest = n % 1_000;
    let k_prefix = if k == 1 {
        "eintausend".to_string()
    } else {
        format!("{}tausend", number_to_words_de(k))
    };
    if rest == 0 {
        k_prefix
    } else {
        format!("{}{}", k_prefix, number_to_words_de(rest))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Unit words ────────────────────────────────────────────────────────────

    #[test]
    fn unit_words() {
        assert_eq!(number_to_words_de(0), "null");
        assert_eq!(number_to_words_de(1), "eins");
        assert_eq!(number_to_words_de(7), "sieben");
        assert_eq!(number_to_words_de(10), "zehn");
        assert_eq!(number_to_words_de(11), "elf");
        assert_eq!(number_to_words_de(12), "zwölf");
    }

    // ── Teen irregulars ───────────────────────────────────────────────────────

    #[test]
    fn teen_irregulars() {
        assert_eq!(number_to_words_de(13), "dreizehn");
        assert_eq!(number_to_words_de(16), "sechzehn");  // sechs → sech
        assert_eq!(number_to_words_de(17), "siebzehn");  // sieben → sieb
        assert_eq!(number_to_words_de(19), "neunzehn");
    }

    // ── Tens and compounds ────────────────────────────────────────────────────

    #[test]
    fn twenty_and_twenty_one() {
        assert_eq!(number_to_words_de(20), "zwanzig");
        assert_eq!(number_to_words_de(21), "einundzwanzig");
    }

    #[test]
    fn dreissig_spelling() {
        assert_eq!(number_to_words_de(30), "dreißig");
    }

    #[test]
    fn sixty_irregular_stem() {
        assert_eq!(number_to_words_de(60), "sechzig");
        assert_eq!(number_to_words_de(70), "siebzig");
    }

    #[test]
    fn ninety_nine() {
        assert_eq!(number_to_words_de(99), "neunundneunzig");
    }

    // ── Hundreds ──────────────────────────────────────────────────────────────

    #[test]
    fn hundred_even() {
        assert_eq!(number_to_words_de(100), "einhundert");
    }

    #[test]
    fn hundred_one() {
        assert_eq!(number_to_words_de(101), "einhunderteins");
    }

    #[test]
    fn hundred_twenty_three() {
        assert_eq!(number_to_words_de(123), "einhundertdreiundzwanzig");
    }

    // ── Thousands ─────────────────────────────────────────────────────────────

    #[test]
    fn one_thousand() {
        assert_eq!(number_to_words_de(1_000), "eintausend");
    }

    #[test]
    fn two_thousand_one() {
        assert_eq!(number_to_words_de(2_001), "zweitausendeins");
    }

    #[test]
    fn large_number_falls_back_to_numeric() {
        // 1_234_567 is beyond 999_999 — falls back to numeric string
        assert_eq!(number_to_words_de(1_234_567), "1234567");
    }
}
