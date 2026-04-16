//! Spanish pluralization and singularization rules.
//!
//! Rules applied in order (pluralize):
//! 1. Irregular lookup table (~10 common forms)
//! 2. Ends in unstressed vowel (a/e/i/o/u) → add "s"
//! 3. Ends in stressed vowel (á/é/í/ó/ú) → add "es"
//! 4. Ends in "z" → change "z" to "c" + "es"  (luz → luces)
//! 5. Already ends in -s or -x → check syllable stress; if word ends in -és/-ás/-ós/-ús/-ís
//!    treat as stressed → add "es"; otherwise (polysyllabic, unstressed final -s) → unchanged
//! 6. Ends in consonant other than z → add "es"

const IRREGULAR_PLURALS: &[(&str, &str)] = &[
    ("país", "países"),
    ("rey", "reyes"),
    ("ley", "leyes"),
    ("luz", "luces"),
    ("vez", "veces"),
    ("voz", "voces"),
    ("ciudad", "ciudades"),
    ("café", "cafés"),
    ("mamá", "mamás"),
    ("papá", "papás"),
    ("sofá", "sofás"),
];

const UNSTRESSED_FINAL_S: &[&str] = &[
    "lunes", "martes", "miércoles", "jueves", "viernes",
    "atlas", "tesis", "crisis", "análisis", "síntesis",
    "dosis", "paréntesis", "virus", "focus",
];

pub fn pluralize_es(word: &str) -> String {
    // 1. Irregular table lookup (case-insensitive key, preserve capitalisation)
    let lower = word.to_lowercase();
    for &(singular, plural) in IRREGULAR_PLURALS {
        if lower == singular {
            return recase(plural, word);
        }
    }

    // 2. Words that don't change in plural
    if UNSTRESSED_FINAL_S.contains(&lower.as_str()) {
        return word.to_string();
    }

    // Get the last char
    let chars: Vec<char> = word.chars().collect();
    let last = match chars.last() {
        Some(&c) => c,
        None => return word.to_string(),
    };

    match last {
        // Unstressed vowel endings → +s
        'a' | 'e' | 'i' | 'o' | 'u' => format!("{word}s"),

        // Stressed vowel endings → +es
        'á' | 'é' | 'í' | 'ó' | 'ú' => format!("{word}es"),

        // z → ces
        'z' => {
            let stem: String = chars[..chars.len() - 1].iter().collect();
            format!("{stem}ces")
        }

        // Already ends in s or x — only add es if stressed last syllable
        // (like "mes" → "meses"), otherwise leave unchanged ("lunes" already
        // covered above; any remaining -s word we leave unchanged as a safe default)
        's' | 'x' => {
            // "mes" → "meses": single syllable words ending in consonant
            if is_likely_monosyllable(word) {
                format!("{word}es")
            } else {
                word.to_string()
            }
        }

        // Consonant → +es
        _ => format!("{word}es"),
    }
}

pub fn singularize_es(word: &str) -> String {
    let lower = word.to_lowercase();

    // Reverse lookup of irregular table
    for &(singular, plural) in IRREGULAR_PLURALS {
        if lower == plural {
            return recase(singular, word);
        }
    }

    // Words that don't change
    if UNSTRESSED_FINAL_S.contains(&lower.as_str()) {
        return word.to_string();
    }

    // -ces → -z  (luces → luz, veces → vez)
    if lower.ends_with("ces") {
        let stem: String = word.chars().take(word.chars().count() - 3).collect();
        return format!("{stem}z");
    }

    // -es suffix: drop -es when it was added for consonant/stressed-vowel stems
    if lower.ends_with("es") && lower.len() > 3 {
        let candidate = &word[..word.len() - 2];
        // Only strip -es if the remaining form looks like a real stem
        // (i.e., doesn't leave just 1 char, and the penultimate char is consonant-like)
        let cand_chars: Vec<char> = candidate.chars().collect();
        if cand_chars.len() >= 2 {
            let last_cand = *cand_chars.last().unwrap();
            // If stem ends in vowel it was likely -e+s or stressed vowel+es
            match last_cand {
                'a' | 'e' | 'i' | 'o' | 'u' => {
                    // Could be "clase" → "clases" (stem = "clase"), strip the s only
                    if lower.ends_with('s') && !lower.ends_with("es") {
                        return word[..word.len() - 1].to_string();
                    }
                    // "clases" → "clase": strip trailing s
                    return word[..word.len() - 1].to_string();
                }
                _ => {
                    // Consonant stem: strip -es
                    return candidate.to_string();
                }
            }
        }
    }

    // Plain -s suffix: strip it
    if lower.ends_with('s') && lower.len() > 2 {
        return word[..word.len() - 1].to_string();
    }

    word.to_string()
}

/// Check if a word is likely monosyllabic (used to decide "mes" → "meses").
/// Simple heuristic: 3 chars or fewer, or ends in consonant after a single vowel.
fn is_likely_monosyllable(word: &str) -> bool {
    let chars: Vec<char> = word.chars().collect();
    if chars.len() <= 3 {
        return true;
    }
    // Count vowel groups as a rough syllable count
    let vowels: &[char] = &['a', 'e', 'i', 'o', 'u', 'á', 'é', 'í', 'ó', 'ú'];
    let vowel_groups: usize = chars
        .windows(2)
        .filter(|w| vowels.contains(&w[0]) && !vowels.contains(&w[1]))
        .count()
        + usize::from(vowels.contains(chars.last().unwrap_or(&' ')));
    vowel_groups <= 1
}

/// Preserve the capitalisation pattern of `original` when applying `form`.
fn recase(form: &str, original: &str) -> String {
    let orig_chars: Vec<char> = original.chars().collect();
    let is_upper = orig_chars.first().map(|c| c.is_uppercase()).unwrap_or(false);
    if is_upper {
        let mut chars = form.chars();
        match chars.next() {
            Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
            None => form.to_string(),
        }
    } else {
        form.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── pluralize ────────────────────────────────────────────────────────────

    #[test]
    fn pluralize_vowel_a_adds_s() {
        assert_eq!(pluralize_es("casa"), "casas");
        assert_eq!(pluralize_es("mesa"), "mesas");
    }

    #[test]
    fn pluralize_vowel_o_adds_s() {
        assert_eq!(pluralize_es("libro"), "libros");
        assert_eq!(pluralize_es("servicio"), "servicios");
    }

    #[test]
    fn pluralize_vowel_e_adds_s() {
        assert_eq!(pluralize_es("nombre"), "nombres");
        assert_eq!(pluralize_es("clase"), "clases");
    }

    #[test]
    fn pluralize_consonant_adds_es() {
        assert_eq!(pluralize_es("color"), "colores");
        assert_eq!(pluralize_es("papel"), "papeles");
        assert_eq!(pluralize_es("mes"), "meses");
    }

    #[test]
    fn pluralize_z_becomes_ces() {
        assert_eq!(pluralize_es("luz"), "luces");
        assert_eq!(pluralize_es("vez"), "veces");
        assert_eq!(pluralize_es("voz"), "voces");
    }

    #[test]
    fn pluralize_irregulars_use_table() {
        assert_eq!(pluralize_es("país"), "países");
        assert_eq!(pluralize_es("rey"), "reyes");
        assert_eq!(pluralize_es("ley"), "leyes");
    }

    #[test]
    fn pluralize_invariant_words_unchanged() {
        assert_eq!(pluralize_es("lunes"), "lunes");
        assert_eq!(pluralize_es("martes"), "martes");
        assert_eq!(pluralize_es("crisis"), "crisis");
    }

    #[test]
    fn pluralize_stressed_vowel_irregular_table_wins() {
        // "café" is in the irregular table: café → cafés (not cafées)
        assert_eq!(pluralize_es("café"), "cafés");
    }

    #[test]
    fn pluralize_stressed_vowel_generic_adds_es() {
        // A stressed final vowel not in the table should get -es
        // e.g. "bisturí" → "bisturíes"
        assert_eq!(pluralize_es("bisturí"), "bisturíes");
    }

    // ── singularize ──────────────────────────────────────────────────────────

    #[test]
    fn singularize_plain_s_stripped() {
        assert_eq!(singularize_es("casas"), "casa");
        assert_eq!(singularize_es("libros"), "libro");
    }

    #[test]
    fn singularize_consonant_es_stripped() {
        assert_eq!(singularize_es("colores"), "color");
        assert_eq!(singularize_es("papeles"), "papel");
    }

    #[test]
    fn singularize_ces_to_z() {
        assert_eq!(singularize_es("luces"), "luz");
        assert_eq!(singularize_es("veces"), "vez");
    }

    #[test]
    fn singularize_irregular_reverse_lookup() {
        assert_eq!(singularize_es("países"), "país");
        assert_eq!(singularize_es("reyes"), "rey");
    }

    #[test]
    fn singularize_invariant_unchanged() {
        assert_eq!(singularize_es("lunes"), "lunes");
        assert_eq!(singularize_es("crisis"), "crisis");
    }
}
