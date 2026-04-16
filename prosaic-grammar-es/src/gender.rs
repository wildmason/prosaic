//! Gender inference from Spanish noun endings.

use prosaic_core::Gender;

/// Infer grammatical gender from a Spanish noun's ending.
///
/// Rules applied in order:
/// 1. Ends in -ción, -sión, -tión, -xión → Feminine
/// 2. Ends in -dad, -tad, -tud → Feminine
/// 3. Ends in -umbre → Feminine
/// 4. Ends in -a → Feminine (with common exceptions handled later)
/// 5. Ends in -e ending nouns that are historically feminine (-clase, -llave) → Feminine
/// 6. Ends in -o, -or, -aje, -án → Masculine
/// 7. Default → Masculine (most common fallback)
pub fn infer_gender(word: &str) -> Gender {
    let w = word.to_lowercase();
    if w.ends_with("ción") || w.ends_with("sión") || w.ends_with("tión") || w.ends_with("xión")
    {
        return Gender::Fem;
    }
    if w.ends_with("dad") || w.ends_with("tad") || w.ends_with("tud") {
        return Gender::Fem;
    }
    if w.ends_with("umbre") {
        return Gender::Fem;
    }
    if w.ends_with('a') {
        return Gender::Fem;
    }
    // -e endings that are consistently feminine
    if w.ends_with("clase") || w.ends_with("llave") || w.ends_with("noche") || w.ends_with("tarde")
    {
        return Gender::Fem;
    }
    Gender::Masc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gender_a_ending_is_fem() {
        assert_eq!(infer_gender("casa"), Gender::Fem);
        assert_eq!(infer_gender("mesa"), Gender::Fem);
    }

    #[test]
    fn gender_o_ending_is_masc() {
        assert_eq!(infer_gender("libro"), Gender::Masc);
        assert_eq!(infer_gender("servicio"), Gender::Masc);
    }

    #[test]
    fn gender_cion_ending_is_fem() {
        assert_eq!(infer_gender("nación"), Gender::Fem);
        assert_eq!(infer_gender("situación"), Gender::Fem);
    }

    #[test]
    fn gender_dad_ending_is_fem() {
        assert_eq!(infer_gender("ciudad"), Gender::Fem);
        assert_eq!(infer_gender("libertad"), Gender::Fem);
    }

    #[test]
    fn gender_or_ending_is_masc() {
        assert_eq!(infer_gender("color"), Gender::Masc);
        assert_eq!(infer_gender("doctor"), Gender::Masc);
    }

    #[test]
    fn gender_unknown_consonant_defaults_masc() {
        assert_eq!(infer_gender("árbol"), Gender::Masc);
        assert_eq!(infer_gender("papel"), Gender::Masc);
    }

    #[test]
    fn gender_clase_is_fem() {
        assert_eq!(infer_gender("clase"), Gender::Fem);
    }
}
