//! German article selection.
//!
//! `basic_article` returns "der", "die", or "das" for a noun based on gender
//! inference from the noun's ending.
//!
//! `article_with_features` implements the full 4×4 declension table:
//!
//! ```text
//!            Masc    Fem    Neut    Plural
//! Nom        der     die    das     die
//! Acc        den     die    das     die
//! Dat        dem     der    dem     den
//! Gen        des     der    des     der
//! ```

use prosaic_core::{AgreementFeatures, Case, Gender, GrammaticalNumber};

use crate::gender::infer_gender;

/// Return the definite nominative article inferred from the noun ending.
/// Used by `Language::article`.
pub fn basic_article(word: &str) -> &'static str {
    match infer_gender(word) {
        Gender::Fem => "die",
        Gender::Neut => "das",
        _ => "der",
    }
}

/// Return the definite article matching the given agreement features.
/// Implements the complete Nom/Acc/Dat/Gen × Masc/Fem/Neut/Pl table.
pub fn article_with_features(features: &AgreementFeatures) -> &'static str {
    let plural = matches!(
        features.number,
        GrammaticalNumber::Plural | GrammaticalNumber::Dual
    );

    if plural {
        return match features.case {
            Case::Dative => "den",
            Case::Genitive => "der",
            _ => "die", // Nom + Acc plural = die
        };
    }

    match (features.gender, features.case) {
        // ── Masculine ────────────────────────────────────────────────────────
        (Gender::Masc, Case::Nominative) | (Gender::Masc, Case::Unknown) => "der",
        (Gender::Masc, Case::Accusative) => "den",
        (Gender::Masc, Case::Dative) => "dem",
        (Gender::Masc, Case::Genitive) => "des",
        // ── Feminine ─────────────────────────────────────────────────────────
        (Gender::Fem, Case::Dative) | (Gender::Fem, Case::Genitive) => "der",
        (Gender::Fem, _) => "die",
        // ── Neuter ───────────────────────────────────────────────────────────
        (Gender::Neut, Case::Dative) => "dem",
        (Gender::Neut, Case::Genitive) => "des",
        (Gender::Neut, _) => "das",
        // ── Unknown / Common gender fallback (treat as Masc) ─────────────────
        (_, Case::Accusative) => "den",
        (_, Case::Dative) => "dem",
        (_, Case::Genitive) => "des",
        _ => "der",
    }
}

/// Return the indefinite article ("ein/eine/…") matching the given features.
/// German has no plural indefinite article — returns "" for plural.
pub fn indefinite_article(features: &AgreementFeatures) -> &'static str {
    if matches!(
        features.number,
        GrammaticalNumber::Plural | GrammaticalNumber::Dual
    ) {
        return "";
    }
    match (features.gender, features.case) {
        // ── Masculine ────────────────────────────────────────────────────────
        (Gender::Masc, Case::Accusative) => "einen",
        (Gender::Masc, Case::Dative) => "einem",
        (Gender::Masc, Case::Genitive) => "eines",
        // ── Feminine ─────────────────────────────────────────────────────────
        (Gender::Fem, Case::Dative) | (Gender::Fem, Case::Genitive) => "einer",
        (Gender::Fem, _) => "eine",
        // ── Neuter ───────────────────────────────────────────────────────────
        (Gender::Neut, Case::Dative) => "einem",
        (Gender::Neut, Case::Genitive) => "eines",
        // ── Default (Masc Nom / Unknown) ─────────────────────────────────────
        _ => "ein",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── basic_article ─────────────────────────────────────────────────────────

    #[test]
    fn basic_article_masc_noun() {
        assert_eq!(basic_article("Tisch"), "der");
        assert_eq!(basic_article("Lehrer"), "der");
    }

    #[test]
    fn basic_article_fem_noun() {
        assert_eq!(basic_article("Freiheit"), "die");
        assert_eq!(basic_article("Klasse"), "die");
    }

    #[test]
    fn basic_article_neut_noun() {
        assert_eq!(basic_article("Buch"), "das");
        assert_eq!(basic_article("Ministerium"), "das");
    }

    // ── Definite article 4×4 table ────────────────────────────────────────────
    // 4 cases × (Masc, Fem, Neut, Plural) = 16 assertions

    #[test]
    fn definite_masc_nominative() {
        let f = AgreementFeatures::default()
            .with_gender(Gender::Masc)
            .with_case(Case::Nominative);
        assert_eq!(article_with_features(&f), "der");
    }

    #[test]
    fn definite_masc_accusative() {
        let f = AgreementFeatures::default()
            .with_gender(Gender::Masc)
            .with_case(Case::Accusative);
        assert_eq!(article_with_features(&f), "den");
    }

    #[test]
    fn definite_masc_dative() {
        let f = AgreementFeatures::default()
            .with_gender(Gender::Masc)
            .with_case(Case::Dative);
        assert_eq!(article_with_features(&f), "dem");
    }

    #[test]
    fn definite_masc_genitive() {
        let f = AgreementFeatures::default()
            .with_gender(Gender::Masc)
            .with_case(Case::Genitive);
        assert_eq!(article_with_features(&f), "des");
    }

    #[test]
    fn definite_fem_nominative() {
        let f = AgreementFeatures::default()
            .with_gender(Gender::Fem)
            .with_case(Case::Nominative);
        assert_eq!(article_with_features(&f), "die");
    }

    #[test]
    fn definite_fem_accusative() {
        let f = AgreementFeatures::default()
            .with_gender(Gender::Fem)
            .with_case(Case::Accusative);
        assert_eq!(article_with_features(&f), "die");
    }

    #[test]
    fn definite_fem_dative() {
        let f = AgreementFeatures::default()
            .with_gender(Gender::Fem)
            .with_case(Case::Dative);
        assert_eq!(article_with_features(&f), "der");
    }

    #[test]
    fn definite_fem_genitive() {
        let f = AgreementFeatures::default()
            .with_gender(Gender::Fem)
            .with_case(Case::Genitive);
        assert_eq!(article_with_features(&f), "der");
    }

    #[test]
    fn definite_neut_nominative() {
        let f = AgreementFeatures::default()
            .with_gender(Gender::Neut)
            .with_case(Case::Nominative);
        assert_eq!(article_with_features(&f), "das");
    }

    #[test]
    fn definite_neut_accusative() {
        let f = AgreementFeatures::default()
            .with_gender(Gender::Neut)
            .with_case(Case::Accusative);
        assert_eq!(article_with_features(&f), "das");
    }

    #[test]
    fn definite_neut_dative() {
        let f = AgreementFeatures::default()
            .with_gender(Gender::Neut)
            .with_case(Case::Dative);
        assert_eq!(article_with_features(&f), "dem");
    }

    #[test]
    fn definite_neut_genitive() {
        let f = AgreementFeatures::default()
            .with_gender(Gender::Neut)
            .with_case(Case::Genitive);
        assert_eq!(article_with_features(&f), "des");
    }

    #[test]
    fn definite_plural_nominative() {
        let f = AgreementFeatures::default()
            .with_number(GrammaticalNumber::Plural)
            .with_case(Case::Nominative);
        assert_eq!(article_with_features(&f), "die");
    }

    #[test]
    fn definite_plural_accusative() {
        let f = AgreementFeatures::default()
            .with_number(GrammaticalNumber::Plural)
            .with_case(Case::Accusative);
        assert_eq!(article_with_features(&f), "die");
    }

    #[test]
    fn definite_plural_dative() {
        let f = AgreementFeatures::default()
            .with_number(GrammaticalNumber::Plural)
            .with_case(Case::Dative);
        assert_eq!(article_with_features(&f), "den");
    }

    #[test]
    fn definite_plural_genitive() {
        let f = AgreementFeatures::default()
            .with_number(GrammaticalNumber::Plural)
            .with_case(Case::Genitive);
        assert_eq!(article_with_features(&f), "der");
    }

    // ── Indefinite article ────────────────────────────────────────────────────

    #[test]
    fn indefinite_masc_nominative() {
        let f = AgreementFeatures::default()
            .with_gender(Gender::Masc)
            .with_case(Case::Nominative);
        assert_eq!(indefinite_article(&f), "ein");
    }

    #[test]
    fn indefinite_masc_accusative() {
        let f = AgreementFeatures::default()
            .with_gender(Gender::Masc)
            .with_case(Case::Accusative);
        assert_eq!(indefinite_article(&f), "einen");
    }

    #[test]
    fn indefinite_masc_dative() {
        let f = AgreementFeatures::default()
            .with_gender(Gender::Masc)
            .with_case(Case::Dative);
        assert_eq!(indefinite_article(&f), "einem");
    }

    #[test]
    fn indefinite_masc_genitive() {
        let f = AgreementFeatures::default()
            .with_gender(Gender::Masc)
            .with_case(Case::Genitive);
        assert_eq!(indefinite_article(&f), "eines");
    }

    #[test]
    fn indefinite_fem_nominative() {
        let f = AgreementFeatures::default()
            .with_gender(Gender::Fem)
            .with_case(Case::Nominative);
        assert_eq!(indefinite_article(&f), "eine");
    }

    #[test]
    fn indefinite_fem_accusative() {
        let f = AgreementFeatures::default()
            .with_gender(Gender::Fem)
            .with_case(Case::Accusative);
        assert_eq!(indefinite_article(&f), "eine");
    }

    #[test]
    fn indefinite_fem_dative() {
        let f = AgreementFeatures::default()
            .with_gender(Gender::Fem)
            .with_case(Case::Dative);
        assert_eq!(indefinite_article(&f), "einer");
    }

    #[test]
    fn indefinite_fem_genitive() {
        let f = AgreementFeatures::default()
            .with_gender(Gender::Fem)
            .with_case(Case::Genitive);
        assert_eq!(indefinite_article(&f), "einer");
    }

    #[test]
    fn indefinite_neut_nominative() {
        let f = AgreementFeatures::default()
            .with_gender(Gender::Neut)
            .with_case(Case::Nominative);
        assert_eq!(indefinite_article(&f), "ein");
    }

    #[test]
    fn indefinite_neut_dative() {
        let f = AgreementFeatures::default()
            .with_gender(Gender::Neut)
            .with_case(Case::Dative);
        assert_eq!(indefinite_article(&f), "einem");
    }

    #[test]
    fn indefinite_neut_genitive() {
        let f = AgreementFeatures::default()
            .with_gender(Gender::Neut)
            .with_case(Case::Genitive);
        assert_eq!(indefinite_article(&f), "eines");
    }

    #[test]
    fn indefinite_plural_is_empty() {
        let f = AgreementFeatures::default().with_number(GrammaticalNumber::Plural);
        assert_eq!(indefinite_article(&f), "");
    }
}
