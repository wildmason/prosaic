//! Gender inference from German noun endings.
//!
//! Rules applied in order:
//! 1. Exception lookup table (~30 common words that defy suffix rules).
//! 2. Feminine suffixes: -ung, -heit, -keit, -schaft, -ion, -ei, and bare -e.
//! 3. Neuter suffixes: -chen, -lein, -um, -ment.
//! 4. Masculine endings: -er, -ling, -ismus, -ant, -ist.
//! 5. Default: Masculine (the most common fallback in German).

use prosaic_core::Gender;

/// Infer grammatical gender from a German noun's ending.
pub fn infer_gender(word: &str) -> Gender {
    let lower = word.to_lowercase();

    // 1. Exception table (common words that defy suffix rules)
    if let Some(g) = exception_lookup(lower.as_str()) {
        return g;
    }

    // 2. Feminine suffixes
    if lower.ends_with("ung")
        || lower.ends_with("heit")
        || lower.ends_with("keit")
        || lower.ends_with("schaft")
        || lower.ends_with("ion")
        || lower.ends_with("ei")
    {
        return Gender::Fem;
    }
    // -e (but not -chen or -lein, which are neuter diminutives)
    if lower.ends_with('e') && !lower.ends_with("chen") && !lower.ends_with("lein") {
        return Gender::Fem;
    }

    // 3. Neuter suffixes
    if lower.ends_with("chen")
        || lower.ends_with("lein")
        || lower.ends_with("um")
        || lower.ends_with("ment")
    {
        return Gender::Neut;
    }

    // 4. Masculine endings (explicit)
    if lower.ends_with("er")
        || lower.ends_with("ling")
        || lower.ends_with("ismus")
        || lower.ends_with("ant")
        || lower.ends_with("ist")
    {
        return Gender::Masc;
    }

    // 5. Default: masculine
    Gender::Masc
}

fn exception_lookup(lower: &str) -> Option<Gender> {
    match lower {
        // Neuter exceptions
        "mädchen" | "fräulein" | "kind" | "haus" | "buch" | "auto" | "wort" | "bett" | "geld"
        | "jahr" | "land" | "licht" | "meer" | "tier" => Some(Gender::Neut),
        // Masculine exceptions
        "tisch" | "stuhl" | "mann" | "tag" | "monat" | "herbst" | "brief" | "baum" | "berg"
        | "hund" | "zug" => Some(Gender::Masc),
        // Feminine exceptions
        "frau" | "nacht" | "stadt" | "hand" | "welt" | "zeit" => Some(Gender::Fem),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Feminine suffix tests ─────────────────────────────────────────────────

    #[test]
    fn feminine_ung_suffix() {
        assert_eq!(infer_gender("Meinung"), Gender::Fem);
        assert_eq!(infer_gender("Ordnung"), Gender::Fem);
        assert_eq!(infer_gender("Zeitung"), Gender::Fem);
    }

    #[test]
    fn feminine_heit_suffix() {
        assert_eq!(infer_gender("Freiheit"), Gender::Fem);
        assert_eq!(infer_gender("Gesundheit"), Gender::Fem);
    }

    #[test]
    fn feminine_ion_suffix() {
        assert_eq!(infer_gender("Nation"), Gender::Fem);
        assert_eq!(infer_gender("Produktion"), Gender::Fem);
    }

    // ── Neuter suffix tests ───────────────────────────────────────────────────

    #[test]
    fn neuter_chen_suffix() {
        // Mädchen is neuter both via exception AND -chen rule
        assert_eq!(infer_gender("Mädchen"), Gender::Neut);
        assert_eq!(infer_gender("Hündchen"), Gender::Neut);
    }

    #[test]
    fn neuter_um_suffix() {
        assert_eq!(infer_gender("Ministerium"), Gender::Neut);
        assert_eq!(infer_gender("Zentrum"), Gender::Neut);
    }

    #[test]
    fn neuter_exception_buch() {
        assert_eq!(infer_gender("Buch"), Gender::Neut);
    }

    // ── Masculine tests ───────────────────────────────────────────────────────

    #[test]
    fn masculine_exception_tisch() {
        assert_eq!(infer_gender("Tisch"), Gender::Masc);
    }

    #[test]
    fn masculine_er_suffix() {
        assert_eq!(infer_gender("Lehrer"), Gender::Masc);
        assert_eq!(infer_gender("Arbeiter"), Gender::Masc);
    }

    #[test]
    fn masculine_default_no_recognizable_suffix() {
        // A plain noun with no matching suffix defaults to Masc
        assert_eq!(infer_gender("Schrank"), Gender::Masc);
    }

    // ── Exception override tests ──────────────────────────────────────────────

    #[test]
    fn mädchen_is_neut_despite_e_ending() {
        // -chen rule and exception both agree on Neut
        assert_eq!(infer_gender("Mädchen"), Gender::Neut);
    }

    #[test]
    fn frau_is_fem_despite_no_e_ending() {
        assert_eq!(infer_gender("Frau"), Gender::Fem);
    }
}
