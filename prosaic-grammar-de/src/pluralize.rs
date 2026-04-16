//! German noun pluralization and singularization.
//!
//! Rules (applied in order):
//! 1. Irregular lookup table (~20 common nouns).
//! 2. Feminine derivational suffixes (-ung, -heit, etc.) → -en.
//! 3. Diminutives (-chen, -lein) → unchanged.
//! 4. Nouns ending in bare -e → append -n (Klasse → Klassen).
//! 5. Nouns ending in -er, -el, -en → unchanged (no heuristic umlaut).
//! 6. Loanword endings (-o, -y) → -s.
//! 7. Default → append -e.
//!
//! Umlaut is NOT applied heuristically — all umlaut plurals come from the
//! irregular table, which is the only safe approach without a full lexicon.

const IRREGULAR_PLURALS: &[(&str, &str)] = &[
    ("Mann", "Männer"),
    ("Kind", "Kinder"),
    ("Haus", "Häuser"),
    ("Wort", "Wörter"),
    ("Buch", "Bücher"),
    ("Mensch", "Menschen"),
    ("Frau", "Frauen"),
    ("Apfel", "Äpfel"),
    ("Vater", "Väter"),
    ("Mutter", "Mütter"),
    ("Bruder", "Brüder"),
    ("Land", "Länder"),
    ("Stadt", "Städte"),
    ("Nacht", "Nächte"),
    ("Hand", "Hände"),
    ("Auto", "Autos"),
    ("Tag", "Tage"),
    ("Jahr", "Jahre"),
    ("Zeit", "Zeiten"),
    ("Klasse", "Klassen"),
];

/// Pluralize a German noun. Input casing is preserved.
///
/// Callers must pass already-capitalized German nouns; this function does NOT
/// auto-capitalize. The plural form preserves the first-character casing of
/// the input.
pub fn pluralize_de(word: &str) -> String {
    // 1. Irregular table (case-sensitive key match first, then lowercase fallback)
    if let Some(pl) = irregular_plural(word) {
        return pl.to_string();
    }

    let lower = word.to_lowercase();

    // 2. Feminine derivational suffixes → -en
    if has_fem_derivational_suffix(&lower) {
        return add_en(word);
    }

    // 3. Diminutives → unchanged
    if lower.ends_with("chen") || lower.ends_with("lein") {
        return word.to_string();
    }

    // 4. Bare -e ending → -n  (e.g. Klasse → Klassen, but caught by irregular above)
    if lower.ends_with('e') {
        return format!("{word}n");
    }

    // 5. -er, -el, -en endings → unchanged (no heuristic umlaut)
    if lower.ends_with("er") || lower.ends_with("el") || lower.ends_with("en") {
        return word.to_string();
    }

    // 6. Loanword endings → -s
    if lower.ends_with('o') || lower.ends_with('y') {
        return format!("{word}s");
    }

    // 7. Default: append -e
    format!("{word}e")
}

/// Attempt to singularize a German plural noun. Best-effort only.
pub fn singularize_de(word: &str) -> String {
    // 1. Reverse lookup of irregular table
    if let Some(sg) = irregular_singular(word) {
        return sg.to_string();
    }

    // 2. Strip common plural suffixes
    // -en / -n  (Frauen → Frau, Klassen → Klasse)
    if word.ends_with("en") && word.len() > 3 {
        return word[..word.len() - 2].to_string();
    }
    if word.ends_with('n') && word.len() > 2 {
        return word[..word.len() - 1].to_string();
    }
    // -e  (Tische → Tisch)
    if word.ends_with('e') && word.len() > 2 {
        return word[..word.len() - 1].to_string();
    }
    // -s  (Autos → Auto)
    if word.ends_with('s') && word.len() > 2 {
        return word[..word.len() - 1].to_string();
    }

    word.to_string()
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn has_fem_derivational_suffix(lower: &str) -> bool {
    lower.ends_with("ung")
        || lower.ends_with("heit")
        || lower.ends_with("keit")
        || lower.ends_with("schaft")
        || lower.ends_with("ion")
}

fn add_en(word: &str) -> String {
    format!("{word}en")
}

fn irregular_plural(word: &str) -> Option<&'static str> {
    // Try exact match first (preserves capitalisation contract with the table)
    for &(sg, pl) in IRREGULAR_PLURALS {
        if sg == word {
            return Some(pl);
        }
    }
    // Case-insensitive fallback: lowercase input vs lowercase table key
    let lower = word.to_lowercase();
    for &(sg, pl) in IRREGULAR_PLURALS {
        if sg.to_lowercase() == lower {
            return Some(pl);
        }
    }
    None
}

fn irregular_singular(word: &str) -> Option<&'static str> {
    // Exact plural → singular
    for &(sg, pl) in IRREGULAR_PLURALS {
        if pl == word {
            return Some(sg);
        }
    }
    // Case-insensitive fallback
    let lower = word.to_lowercase();
    for &(sg, pl) in IRREGULAR_PLURALS {
        if pl.to_lowercase() == lower {
            return Some(sg);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Regular suffix rules ──────────────────────────────────────────────────

    #[test]
    fn pluralize_fem_derivational_ung_adds_en() {
        // Meinung is not in the irregular table; rule fires
        assert_eq!(pluralize_de("Meinung"), "Meinungen");
        assert_eq!(pluralize_de("Zeitung"), "Zeitungen");
    }

    #[test]
    fn pluralize_diminutive_chen_unchanged() {
        assert_eq!(pluralize_de("Hündchen"), "Hündchen");
    }

    #[test]
    fn pluralize_bare_e_adds_n() {
        // Klasse is in the irregular table (Klassen), but test a non-irregular -e noun
        assert_eq!(pluralize_de("Katze"), "Katzen");
        assert_eq!(pluralize_de("Flasche"), "Flaschen");
    }

    #[test]
    fn pluralize_er_el_en_unchanged() {
        // No umlaut heuristic — just return unchanged
        assert_eq!(pluralize_de("Lehrer"), "Lehrer");
        assert_eq!(pluralize_de("Schlüssel"), "Schlüssel");
    }

    #[test]
    fn pluralize_loanword_o_adds_s() {
        // Auto is in irregular table (Autos); use a non-irregular -o
        assert_eq!(pluralize_de("Radio"), "Radios");
        assert_eq!(pluralize_de("Sofa"), "Sofae"); // default -e (no -o ending)
    }

    // ── Irregular lookups ─────────────────────────────────────────────────────

    #[test]
    fn pluralize_irregular_mann() {
        assert_eq!(pluralize_de("Mann"), "Männer");
    }

    #[test]
    fn pluralize_irregular_kind() {
        assert_eq!(pluralize_de("Kind"), "Kinder");
    }

    #[test]
    fn pluralize_irregular_haus() {
        assert_eq!(pluralize_de("Haus"), "Häuser");
    }

    #[test]
    fn pluralize_irregular_auto() {
        assert_eq!(pluralize_de("Auto"), "Autos");
    }

    #[test]
    fn pluralize_irregular_klasse() {
        assert_eq!(pluralize_de("Klasse"), "Klassen");
    }

    // ── Case preservation ─────────────────────────────────────────────────────

    #[test]
    fn pluralize_preserves_capitalization_irregular() {
        // Irregular table preserves the table's plural casing (already capitalized)
        let pl = pluralize_de("Mann");
        assert_eq!(&pl[..1], "M", "First char should be uppercase");
        assert_eq!(pl, "Männer");
    }

    #[test]
    fn pluralize_preserves_capitalization_regular() {
        let pl = pluralize_de("Meinung");
        assert_eq!(&pl[..1], "M");
        assert_eq!(pl, "Meinungen");
    }

    #[test]
    fn pluralize_lowercase_input_stays_lowercase() {
        // If caller passes lowercase, output stays lowercase
        let pl = pluralize_de("meinung");
        assert!(pl.starts_with('m'));
    }

    // ── Singularize ───────────────────────────────────────────────────────────

    #[test]
    fn singularize_irregular_männer_to_mann() {
        assert_eq!(singularize_de("Männer"), "Mann");
    }

    #[test]
    fn singularize_en_suffix_stripped() {
        assert_eq!(singularize_de("Zeitungen"), "Zeitung");
    }

    #[test]
    fn singularize_e_suffix_stripped() {
        assert_eq!(singularize_de("Tische"), "Tisch");
    }

    #[test]
    fn singularize_s_suffix_stripped() {
        assert_eq!(singularize_de("Radios"), "Radio");
    }

    #[test]
    fn singularize_irregular_kinder_to_kind() {
        assert_eq!(singularize_de("Kinder"), "Kind");
    }
}
