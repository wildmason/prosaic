//! Spanish article selection.
//!
//! `basic_article` returns "el" or "la" for a noun based on gender inference
//! from the noun's ending. This is the fallback used when `AgreementFeatures`
//! are not available (e.g., via the bare `Language::article` method).
//!
//! Callers with full `AgreementFeatures` should use `article_with_features`
//! directly (called by `Language::plural_description` and `realize_reference`).

use prosaic_core::{AgreementFeatures, Gender, GrammaticalNumber};

use crate::gender::infer_gender;

/// Return the definite singular article ("el" or "la") inferred from the
/// noun ending. Used by `Language::article`.
pub fn basic_article(word: &str) -> &'static str {
    match infer_gender(word) {
        Gender::Fem => "la",
        _ => "el",
    }
}

/// Return the definite article matching the given agreement features.
pub fn article_with_features(features: &AgreementFeatures) -> &'static str {
    match (features.gender, features.number) {
        (Gender::Fem, GrammaticalNumber::Plural) | (Gender::Fem, GrammaticalNumber::Dual) => "las",
        (Gender::Fem, _) => "la",
        (_, GrammaticalNumber::Plural) | (_, GrammaticalNumber::Dual) => "los",
        _ => "el",
    }
}

/// Return the indefinite article ("un" or "una").
pub fn indefinite_article(word: &str) -> &'static str {
    match infer_gender(word) {
        Gender::Fem => "una",
        _ => "un",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn article_masculine_noun() {
        assert_eq!(basic_article("servicio"), "el");
        assert_eq!(basic_article("libro"), "el");
    }

    #[test]
    fn article_feminine_noun() {
        assert_eq!(basic_article("clase"), "la");
        assert_eq!(basic_article("casa"), "la");
    }

    #[test]
    fn article_with_features_masc_singular() {
        let f = AgreementFeatures::default(); // Unknown gender → Masc fallback
        assert_eq!(article_with_features(&f), "el");
    }

    #[test]
    fn article_with_features_masc_plural() {
        use prosaic_core::GrammaticalNumber;
        let f = AgreementFeatures::default().with_number(GrammaticalNumber::Plural);
        assert_eq!(article_with_features(&f), "los");
    }

    #[test]
    fn article_with_features_fem_singular() {
        let f = AgreementFeatures::default().with_gender(Gender::Fem);
        assert_eq!(article_with_features(&f), "la");
    }

    #[test]
    fn article_with_features_fem_plural() {
        use prosaic_core::GrammaticalNumber;
        let f = AgreementFeatures::default()
            .with_gender(Gender::Fem)
            .with_number(GrammaticalNumber::Plural);
        assert_eq!(article_with_features(&f), "las");
    }

    #[test]
    fn indefinite_article_masc() {
        assert_eq!(indefinite_article("libro"), "un");
    }

    #[test]
    fn indefinite_article_fem() {
        assert_eq!(indefinite_article("casa"), "una");
    }
}
