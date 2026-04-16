//! Spanish grammar layer for the Prosaic NLG engine.
//!
//! Minimal v1.5 scope: gender-aware articles, regular + common-irregular
//! pluralization, regular verb conjugation (present/simple-past/future/
//! past-participle/present-participle), gender agreement on passive
//! participles, gendered pronouns.
//!
//! Uses only pure Rust — no CLDR data. Expansion (subjunctive, imperfecto,
//! full irregular tables, dialectal variation) is planned for follow-up crates.

pub mod gender;
pub(crate) mod pluralize;
pub(crate) mod articles;
pub(crate) mod conjugate;
pub(crate) mod numbers;

use prosaic_core::{
    AgreementFeatures, Conjunction, Gender, GrammaticalNumber, Language, Person,
    PluralCategory, ReferenceForm, Tense,
};

use articles::{article_with_features, basic_article};
pub use articles::indefinite_article;
use pluralize::{pluralize_es, singularize_es};
use conjugate::{conjugate_es, past_participle_es, present_participle_es};
use numbers::number_to_words_es;

/// Spanish language grammar implementation.
///
/// Covers the most common regular and irregular constructions. It is
/// deliberately minimal for v1.5: no imperfecto, no subjunctive, no
/// full irregular tables. See the crate docs for the exact scope.
#[derive(Debug, Clone, Default)]
pub struct Spanish;

impl Spanish {
    pub fn new() -> Self {
        Self
    }
}

impl Language for Spanish {
    fn pluralize(&self, word: &str, count: usize) -> String {
        if count == 1 {
            word.to_string()
        } else {
            pluralize_es(word)
        }
    }

    fn singularize(&self, word: &str) -> String {
        singularize_es(word)
    }

    /// Return the definite article for `word` inferred from its ending.
    /// Callers with full `AgreementFeatures` should use `plural_description`
    /// or `realize_reference` instead, which use gender-aware logic.
    fn article(&self, word: &str) -> &str {
        basic_article(word)
    }

    fn conjugate(&self, verb: &str, tense: Tense, person: Person) -> String {
        conjugate_es(verb, tense, person)
    }

    fn past_participle(&self, verb: &str) -> String {
        past_participle_es(verb)
    }

    fn present_participle(&self, verb: &str) -> String {
        present_participle_es(verb)
    }

    fn join_list(&self, items: &[&str], conjunction: Conjunction) -> String {
        let conj = match conjunction {
            Conjunction::And => "y",
            Conjunction::Or  => "o",
        };
        join_list_es(items, conj)
    }

    fn ordinal(&self, n: usize) -> String {
        // Spanish ordinals beyond 10 are verbose; return numeric form for n >= 11.
        match n {
            1  => "primero".to_string(),
            2  => "segundo".to_string(),
            3  => "tercero".to_string(),
            4  => "cuarto".to_string(),
            5  => "quinto".to_string(),
            6  => "sexto".to_string(),
            7  => "séptimo".to_string(),
            8  => "octavo".to_string(),
            9  => "noveno".to_string(),
            10 => "décimo".to_string(),
            _  => format!("{n}º"),
        }
    }

    fn number_to_words(&self, n: usize) -> String {
        number_to_words_es(n)
    }

    // ── Category-aware pluralization ──────────────────────────────────────────

    /// Spanish plural rules match English: `n == 1` → One, else Other.
    fn plural_category(&self, n: i64) -> PluralCategory {
        match n {
            1 => PluralCategory::One,
            _ => PluralCategory::Other,
        }
    }

    // `pluralize_with_category` default is correct: One → singular, Other → pluralize(_,2).

    // ── Reference realization ─────────────────────────────────────────────────

    fn realize_reference(
        &self,
        form: ReferenceForm,
        features: &AgreementFeatures,
    ) -> Option<String> {
        match form {
            ReferenceForm::Pronoun      => Some(spanish_pronoun(features)),
            ReferenceForm::Demonstrative => Some(spanish_demonstrative(features)),
            ReferenceForm::Zero         => None,
            ReferenceForm::Full | ReferenceForm::ShortName => None,
        }
    }

    // ── Plural description ────────────────────────────────────────────────────

    fn plural_description(
        &self,
        entity_type: &str,
        count: usize,
        features: &AgreementFeatures,
    ) -> String {
        // Gender-aware: "las 3 clases" (fem), "los 3 servicios" (masc).
        match count {
            0 => String::new(),
            1 => format!(
                "{} {}",
                singular_article(features),
                entity_type
            ),
            _ => format!(
                "{} {count} {}",
                plural_article(features),
                self.pluralize(entity_type, count)
            ),
        }
    }
}

// ── Helper functions ──────────────────────────────────────────────────────────

fn spanish_pronoun(features: &AgreementFeatures) -> String {
    match (features.gender, features.number) {
        (Gender::Fem, GrammaticalNumber::Plural)
        | (Gender::Fem, GrammaticalNumber::Dual) => "ellas".to_string(),
        (Gender::Fem, _) => "ella".to_string(),
        (_, GrammaticalNumber::Plural)
        | (_, GrammaticalNumber::Dual) => "ellos".to_string(),
        _ => "él".to_string(),
    }
}

fn spanish_demonstrative(features: &AgreementFeatures) -> String {
    match (features.gender, features.number) {
        (Gender::Fem, GrammaticalNumber::Plural) => "estas".to_string(),
        (Gender::Fem, _) => "esta".to_string(),
        (_, GrammaticalNumber::Plural) => "estos".to_string(),
        _ => "este".to_string(),
    }
}

fn singular_article(features: &AgreementFeatures) -> &'static str {
    article_with_features(features)
}

fn plural_article(features: &AgreementFeatures) -> &'static str {
    // Build a temporary plural-number copy of features to get los/las.
    let plural_features = AgreementFeatures::default()
        .with_gender(features.gender)
        .with_number(GrammaticalNumber::Plural);
    article_with_features(&plural_features)
}

fn join_list_es(items: &[&str], conjunction: &str) -> String {
    match items.len() {
        0 => String::new(),
        1 => items[0].to_string(),
        2 => format!("{} {} {}", items[0], conjunction, items[1]),
        _ => {
            let (last, rest) = items.split_last().unwrap();
            format!("{}, {} {}", rest.join(", "), conjunction, last)
        }
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Language trait surface ────────────────────────────────────────────────

    #[test]
    fn pluralize_count_one_returns_word_unchanged() {
        let es = Spanish::new();
        assert_eq!(es.pluralize("clase", 1), "clase");
    }

    #[test]
    fn pluralize_count_zero_pluralizes() {
        let es = Spanish::new();
        assert_eq!(es.pluralize("clase", 0), "clases");
    }

    #[test]
    fn singularize_plain_s_delegates_to_module() {
        let es = Spanish::new();
        // Words pluralized with -s: strip to vowel stem
        assert_eq!(es.singularize("casas"), "casa");
        assert_eq!(es.singularize("libros"), "libro");
    }

    #[test]
    fn singularize_es_delegates_to_module() {
        let es = Spanish::new();
        // Words pluralized with -es on consonant stems
        assert_eq!(es.singularize("colores"), "color");
        assert_eq!(es.singularize("papeles"), "papel");
    }

    #[test]
    fn article_fem_noun() {
        let es = Spanish::new();
        assert_eq!(es.article("casa"), "la");
    }

    #[test]
    fn article_masc_noun() {
        let es = Spanish::new();
        assert_eq!(es.article("libro"), "el");
    }

    #[test]
    fn conjugate_delegates_to_module() {
        let es = Spanish::new();
        assert_eq!(es.conjugate("hablar", Tense::Present, Person::First), "hablo");
    }

    #[test]
    fn past_participle_delegates_to_module() {
        let es = Spanish::new();
        assert_eq!(es.past_participle("hablar"), "hablado");
    }

    #[test]
    fn present_participle_delegates_to_module() {
        let es = Spanish::new();
        assert_eq!(es.present_participle("comer"), "comiendo");
    }

    #[test]
    fn join_list_empty() {
        let es = Spanish::new();
        assert_eq!(es.join_list(&[], Conjunction::And), "");
    }

    #[test]
    fn join_list_single() {
        let es = Spanish::new();
        assert_eq!(es.join_list(&["uno"], Conjunction::And), "uno");
    }

    #[test]
    fn join_list_two_and() {
        let es = Spanish::new();
        assert_eq!(es.join_list(&["uno", "dos"], Conjunction::And), "uno y dos");
    }

    #[test]
    fn join_list_three_and() {
        let es = Spanish::new();
        assert_eq!(
            es.join_list(&["uno", "dos", "tres"], Conjunction::And),
            "uno, dos, y tres"
        );
    }

    #[test]
    fn join_list_two_or() {
        let es = Spanish::new();
        assert_eq!(es.join_list(&["sí", "no"], Conjunction::Or), "sí o no");
    }

    #[test]
    fn ordinal_first_ten() {
        let es = Spanish::new();
        assert_eq!(es.ordinal(1), "primero");
        assert_eq!(es.ordinal(5), "quinto");
        assert_eq!(es.ordinal(10), "décimo");
    }

    #[test]
    fn ordinal_beyond_ten_numeric() {
        let es = Spanish::new();
        assert_eq!(es.ordinal(11), "11º");
        assert_eq!(es.ordinal(100), "100º");
    }

    #[test]
    fn number_to_words_delegates() {
        let es = Spanish::new();
        assert_eq!(es.number_to_words(3), "tres");
        assert_eq!(es.number_to_words(20), "veinte");
    }

    // ── realize_reference ─────────────────────────────────────────────────────

    #[test]
    fn realize_pronoun_masc_singular() {
        let es = Spanish::new();
        let f = AgreementFeatures::default();
        assert_eq!(
            es.realize_reference(ReferenceForm::Pronoun, &f),
            Some("él".to_string())
        );
    }

    #[test]
    fn realize_pronoun_fem_singular() {
        let es = Spanish::new();
        let f = AgreementFeatures::default().with_gender(Gender::Fem);
        assert_eq!(
            es.realize_reference(ReferenceForm::Pronoun, &f),
            Some("ella".to_string())
        );
    }

    #[test]
    fn realize_pronoun_masc_plural() {
        let es = Spanish::new();
        let f = AgreementFeatures::default().with_number(GrammaticalNumber::Plural);
        assert_eq!(
            es.realize_reference(ReferenceForm::Pronoun, &f),
            Some("ellos".to_string())
        );
    }

    #[test]
    fn realize_pronoun_fem_plural() {
        let es = Spanish::new();
        let f = AgreementFeatures::default()
            .with_gender(Gender::Fem)
            .with_number(GrammaticalNumber::Plural);
        assert_eq!(
            es.realize_reference(ReferenceForm::Pronoun, &f),
            Some("ellas".to_string())
        );
    }

    #[test]
    fn realize_demonstrative_masc_singular() {
        let es = Spanish::new();
        let f = AgreementFeatures::default();
        assert_eq!(
            es.realize_reference(ReferenceForm::Demonstrative, &f),
            Some("este".to_string())
        );
    }

    #[test]
    fn realize_demonstrative_fem_singular() {
        let es = Spanish::new();
        let f = AgreementFeatures::default().with_gender(Gender::Fem);
        assert_eq!(
            es.realize_reference(ReferenceForm::Demonstrative, &f),
            Some("esta".to_string())
        );
    }

    #[test]
    fn realize_demonstrative_fem_plural() {
        let es = Spanish::new();
        let f = AgreementFeatures::default()
            .with_gender(Gender::Fem)
            .with_number(GrammaticalNumber::Plural);
        assert_eq!(
            es.realize_reference(ReferenceForm::Demonstrative, &f),
            Some("estas".to_string())
        );
    }

    #[test]
    fn realize_zero_is_none() {
        let es = Spanish::new();
        assert_eq!(
            es.realize_reference(ReferenceForm::Zero, &AgreementFeatures::default()),
            None
        );
    }

    #[test]
    fn realize_full_is_none() {
        let es = Spanish::new();
        assert_eq!(
            es.realize_reference(ReferenceForm::Full, &AgreementFeatures::default()),
            None
        );
    }

    // ── plural_description ────────────────────────────────────────────────────

    #[test]
    fn plural_description_zero_is_empty() {
        let es = Spanish::new();
        assert_eq!(
            es.plural_description("clase", 0, &AgreementFeatures::default()),
            ""
        );
    }

    #[test]
    fn plural_description_one_masc() {
        let es = Spanish::new();
        let f = AgreementFeatures::default(); // Unknown → Masc fallback → "el"
        assert_eq!(es.plural_description("servicio", 1, &f), "el servicio");
    }

    #[test]
    fn plural_description_one_fem() {
        let es = Spanish::new();
        let f = AgreementFeatures::default().with_gender(Gender::Fem);
        assert_eq!(es.plural_description("clase", 1, &f), "la clase");
    }

    #[test]
    fn plural_description_many_masc() {
        let es = Spanish::new();
        let f = AgreementFeatures::default();
        assert_eq!(es.plural_description("servicio", 3, &f), "los 3 servicios");
    }

    #[test]
    fn plural_description_many_fem() {
        let es = Spanish::new();
        let f = AgreementFeatures::default().with_gender(Gender::Fem);
        assert_eq!(es.plural_description("clase", 3, &f), "las 3 clases");
    }

    // ── plural_category ───────────────────────────────────────────────────────

    #[test]
    fn plural_category_one_is_one() {
        let es = Spanish::new();
        assert_eq!(es.plural_category(1), PluralCategory::One);
    }

    #[test]
    fn plural_category_zero_is_other() {
        let es = Spanish::new();
        assert_eq!(es.plural_category(0), PluralCategory::Other);
    }

    #[test]
    fn plural_category_many_is_other() {
        let es = Spanish::new();
        assert_eq!(es.plural_category(5), PluralCategory::Other);
    }

    // ── Send + Sync assertion ─────────────────────────────────────────────────

    #[test]
    fn spanish_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Spanish>();
    }
}
