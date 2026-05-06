//! Spanish grammar layer for the Prosaic NLG engine.
//!
//! Minimal v1.5 scope: gender-aware articles, regular + common-irregular
//! pluralization, regular verb conjugation (present/simple-past/future/
//! past-participle/present-participle), gender agreement on passive
//! participles, gendered pronouns.
//!
//! Uses only pure Rust — no CLDR data. Expansion (subjunctive, imperfecto,
//! full irregular tables, dialectal variation) is planned for follow-up crates.

pub(crate) mod articles;
pub(crate) mod conjugate;
pub mod gender;
pub(crate) mod numbers;
pub(crate) mod pluralize;

use prosaic_core::{
    AgreementFeatures, Conjunction, Gender, GrammaticalNumber, Language, Person, PluralCategory,
    ReferenceForm, RstRelation, Tense,
};

pub use articles::indefinite_article;
use articles::{article_with_features, basic_article};
use conjugate::{conjugate_es, past_participle_es, present_participle_es};
use numbers::number_to_words_es;
use pluralize::{pluralize_es, singularize_es};

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
            Conjunction::Or => "o",
        };
        join_list_es(items, conj)
    }

    fn ordinal(&self, n: usize) -> String {
        // Spanish ordinals beyond 10 are verbose; return numeric form for n >= 11.
        match n {
            1 => "primero".to_string(),
            2 => "segundo".to_string(),
            3 => "tercero".to_string(),
            4 => "cuarto".to_string(),
            5 => "quinto".to_string(),
            6 => "sexto".to_string(),
            7 => "séptimo".to_string(),
            8 => "octavo".to_string(),
            9 => "noveno".to_string(),
            10 => "décimo".to_string(),
            _ => format!("{n}º"),
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
            ReferenceForm::Pronoun => Some(spanish_pronoun(features)),
            ReferenceForm::Possessive => Some(spanish_possessive(features)),
            ReferenceForm::Demonstrative => Some(spanish_demonstrative(features)),
            ReferenceForm::Zero => None,
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
            1 => format!("{} {}", singular_article(features), entity_type),
            _ => format!(
                "{} {count} {}",
                plural_article(features),
                self.pluralize(entity_type, count)
            ),
        }
    }

    fn proportion_phrase(
        &self,
        matching: i64,
        total: i64,
        noun_singular: Option<&str>,
        features: &AgreementFeatures,
    ) -> String {
        spanish_proportion(matching, total, noun_singular, features)
    }

    fn discourse_marker(&self, relation: RstRelation) -> Option<&'static str> {
        use RstRelation::*;
        Some(match relation {
            Elaboration => "Además, ",
            Contrast => "Sin embargo, ",
            Cause => "Debido a esto, ",
            Result => "Como resultado, ",
            Concession => "No obstante, ",
            Sequence => "Luego, ",
            Condition => "Si esto ocurre, ",
            Background => "Mientras tanto, ",
            Summary => "En resumen, ",
        })
    }

    #[cfg(feature = "time")]
    fn since_last_marker(&self, diff_secs: i64) -> String {
        const MINUTE: i64 = 60;
        const HOUR: i64 = 60 * MINUTE;
        const DAY: i64 = 24 * HOUR;
        const WEEK: i64 = 7 * DAY;
        const MONTH: i64 = 30 * DAY;
        const YEAR: i64 = 365 * DAY;

        if diff_secs <= 0 {
            return "al mismo tiempo".to_string();
        }
        if diff_secs < 60 {
            return "momentos después".to_string();
        }
        if diff_secs < HOUR {
            let n = ((diff_secs + MINUTE / 2) / MINUTE).max(1);
            return match n {
                1 => "un minuto después".to_string(),
                _ => format!("{n} minutos después"),
            };
        }
        if diff_secs < DAY {
            let n = ((diff_secs + HOUR / 2) / HOUR).max(1);
            if n < 6 {
                return match n {
                    1 => "una hora después".to_string(),
                    _ => format!("{n} horas después"),
                };
            }
            return "más tarde ese día".to_string();
        }
        if diff_secs < 2 * DAY {
            return "al día siguiente".to_string();
        }
        if diff_secs < WEEK {
            let n = diff_secs / DAY;
            return format!("{n} días después");
        }
        if diff_secs < 2 * WEEK {
            return "la semana siguiente".to_string();
        }
        if diff_secs < MONTH {
            let n = diff_secs / WEEK;
            return format!("{n} semanas después");
        }
        if diff_secs < 2 * MONTH {
            return "el mes siguiente".to_string();
        }
        if diff_secs < YEAR {
            let n = diff_secs / MONTH;
            return format!("{n} meses después");
        }
        if diff_secs < 2 * YEAR {
            return "el año siguiente".to_string();
        }
        let n = diff_secs / YEAR;
        format!("{n} años después")
    }
}

// ── Helper functions ──────────────────────────────────────────────────────────

fn spanish_pronoun(features: &AgreementFeatures) -> String {
    match (features.gender, features.number) {
        (Gender::Fem, GrammaticalNumber::Plural) | (Gender::Fem, GrammaticalNumber::Dual) => {
            "ellas".to_string()
        }
        (Gender::Fem, _) => "ella".to_string(),
        (_, GrammaticalNumber::Plural) | (_, GrammaticalNumber::Dual) => "ellos".to_string(),
        _ => "él".to_string(),
    }
}

fn spanish_possessive(features: &AgreementFeatures) -> String {
    match features.number {
        GrammaticalNumber::Plural | GrammaticalNumber::Dual => "sus".to_string(),
        _ => "su".to_string(),
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

/// Pluralize a multi-word Spanish noun phrase by pluralizing each whitespace-
/// separated token (e.g. "archivo modificado" → "archivos modificados").
fn pluralize_phrase_es(phrase: &str) -> String {
    phrase
        .split_whitespace()
        .map(pluralize_es)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Resolve the gender to use for proportion phrasing: explicit features take
/// precedence, then inference from the head (first) word of the noun phrase,
/// then a masculine fallback.
fn resolve_proportion_gender(noun: Option<&str>, features: &AgreementFeatures) -> Gender {
    match features.gender {
        Gender::Unknown => noun
            .and_then(|n| n.split_whitespace().next())
            .map(gender::infer_gender)
            .unwrap_or(Gender::Masc),
        g => g,
    }
}

fn spanish_proportion(
    matching: i64,
    total: i64,
    noun_singular: Option<&str>,
    features: &AgreementFeatures,
) -> String {
    let n = matching.max(0);
    let t = total.max(0);
    let gender = resolve_proportion_gender(noun_singular, features);
    let is_fem = matches!(gender, Gender::Fem);

    if t == 0 {
        return match (noun_singular, n) {
            (Some(noun), 0) if is_fem => format!("ninguna {noun}"),
            (Some(noun), 0) => format!("ningún {noun}"),
            (None, 0) if is_fem => "ninguna".to_string(),
            (None, 0) => "ninguno".to_string(),
            // n > 0, t == 0: self-inconsistent; literal fall-through.
            (Some(noun), _) => format!("{n} de 0 {}", pluralize_phrase_es(noun)),
            (None, _) => format!("{n} de 0"),
        };
    }

    if n == 0 {
        let (none_word, plural_article) = if is_fem {
            ("ninguna", "las")
        } else {
            ("ninguno", "los")
        };
        return match noun_singular {
            Some(noun) => format!(
                "{none_word} de {plural_article} {t} {}",
                pluralize_phrase_es(noun)
            ),
            None => format!("{none_word} de {plural_article} {t}"),
        };
    }

    if n >= t {
        return match (noun_singular, t) {
            (Some(noun), 1) if is_fem => format!("la única {noun}"),
            (Some(noun), 1) => format!("el único {noun}"),
            (None, 1) if is_fem => "la única".to_string(),
            (None, 1) => "el único".to_string(),
            (Some(noun), 2) if is_fem => format!("ambas {}", pluralize_phrase_es(noun)),
            (Some(noun), 2) => format!("ambos {}", pluralize_phrase_es(noun)),
            (None, 2) if is_fem => "ambas".to_string(),
            (None, 2) => "ambos".to_string(),
            (Some(noun), _) if is_fem => {
                format!("todas las {t} {}", pluralize_phrase_es(noun))
            }
            (Some(noun), _) => format!("todos los {t} {}", pluralize_phrase_es(noun)),
            (None, _) if is_fem => format!("todas las {t}"),
            (None, _) => format!("todos los {t}"),
        };
    }

    match noun_singular {
        Some(noun) => format!("{n} de {t} {}", pluralize_phrase_es(noun)),
        None => format!("{n} de {t}"),
    }
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
        assert_eq!(
            es.conjugate("hablar", Tense::Present, Person::First),
            "hablo"
        );
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
    fn realize_possessive_singular_and_plural() {
        let es = Spanish::new();
        assert_eq!(
            es.realize_reference(ReferenceForm::Possessive, &AgreementFeatures::default()),
            Some("su".to_string())
        );
        let plural = AgreementFeatures::default().with_number(GrammaticalNumber::Plural);
        assert_eq!(
            es.realize_reference(ReferenceForm::Possessive, &plural),
            Some("sus".to_string())
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

    // ── discourse_marker ──────────────────────────────────────────────────────

    #[test]
    fn discourse_marker_spanish() {
        let es = Spanish::new();
        assert_eq!(
            es.discourse_marker(RstRelation::Elaboration),
            Some("Además, ")
        );
        assert_eq!(
            es.discourse_marker(RstRelation::Contrast),
            Some("Sin embargo, ")
        );
        assert_eq!(
            es.discourse_marker(RstRelation::Result),
            Some("Como resultado, ")
        );
    }

    // ── proportion_phrase ─────────────────────────────────────────────────────

    fn no_features() -> AgreementFeatures {
        AgreementFeatures::default()
    }

    #[test]
    fn proportion_two_of_two_masc_noun_reads_ambos() {
        let es = Spanish::new();
        assert_eq!(
            es.proportion_phrase(2, 2, Some("archivo modificado"), &no_features()),
            "ambos archivos modificados"
        );
    }

    #[test]
    fn proportion_two_of_two_fem_noun_reads_ambas() {
        let es = Spanish::new();
        assert_eq!(
            es.proportion_phrase(2, 2, Some("clase modificada"), &no_features()),
            "ambas clases modificadas"
        );
    }

    #[test]
    fn proportion_all_n_masc_noun_reads_todos_los() {
        let es = Spanish::new();
        assert_eq!(
            es.proportion_phrase(13, 13, Some("archivo modificado"), &no_features()),
            "todos los 13 archivos modificados"
        );
    }

    #[test]
    fn proportion_all_n_fem_noun_reads_todas_las() {
        let es = Spanish::new();
        assert_eq!(
            es.proportion_phrase(13, 13, Some("clase modificada"), &no_features()),
            "todas las 13 clases modificadas"
        );
    }

    #[test]
    fn proportion_one_of_one_masc_reads_el_unico() {
        let es = Spanish::new();
        assert_eq!(
            es.proportion_phrase(1, 1, Some("archivo modificado"), &no_features()),
            "el único archivo modificado"
        );
    }

    #[test]
    fn proportion_one_of_one_fem_reads_la_unica() {
        let es = Spanish::new();
        assert_eq!(
            es.proportion_phrase(1, 1, Some("clase modificada"), &no_features()),
            "la única clase modificada"
        );
    }

    #[test]
    fn proportion_zero_of_n_masc_reads_ninguno_de_los() {
        let es = Spanish::new();
        assert_eq!(
            es.proportion_phrase(0, 5, Some("archivo modificado"), &no_features()),
            "ninguno de los 5 archivos modificados"
        );
    }

    #[test]
    fn proportion_zero_of_n_fem_reads_ninguna_de_las() {
        let es = Spanish::new();
        assert_eq!(
            es.proportion_phrase(0, 5, Some("clase modificada"), &no_features()),
            "ninguna de las 5 clases modificadas"
        );
    }

    #[test]
    fn proportion_zero_zero_masc_reads_ningun() {
        let es = Spanish::new();
        assert_eq!(
            es.proportion_phrase(0, 0, Some("archivo modificado"), &no_features()),
            "ningún archivo modificado"
        );
    }

    #[test]
    fn proportion_zero_zero_fem_reads_ninguna() {
        let es = Spanish::new();
        assert_eq!(
            es.proportion_phrase(0, 0, Some("clase modificada"), &no_features()),
            "ninguna clase modificada"
        );
    }

    #[test]
    fn proportion_partial_keeps_de_form() {
        let es = Spanish::new();
        assert_eq!(
            es.proportion_phrase(3, 13, Some("archivo modificado"), &no_features()),
            "3 de 13 archivos modificados"
        );
    }

    #[test]
    fn proportion_no_noun_two_two_masc_default() {
        let es = Spanish::new();
        // Without noun and without explicit gender, default to masc.
        assert_eq!(
            es.proportion_phrase(2, 2, None, &no_features()),
            "ambos"
        );
    }

    #[test]
    fn proportion_no_noun_two_two_fem_explicit() {
        let es = Spanish::new();
        let f = AgreementFeatures::default().with_gender(Gender::Fem);
        assert_eq!(es.proportion_phrase(2, 2, None, &f), "ambas");
    }

    #[test]
    fn proportion_no_noun_all_n_masc() {
        let es = Spanish::new();
        assert_eq!(
            es.proportion_phrase(7, 7, None, &no_features()),
            "todos los 7"
        );
    }

    #[test]
    fn proportion_no_noun_all_n_fem() {
        let es = Spanish::new();
        let f = AgreementFeatures::default().with_gender(Gender::Fem);
        assert_eq!(es.proportion_phrase(7, 7, None, &f), "todas las 7");
    }

    #[test]
    fn proportion_features_gender_overrides_inference() {
        // Caller explicitly sets fem; the noun's masc ending is overridden.
        let es = Spanish::new();
        let f = AgreementFeatures::default().with_gender(Gender::Fem);
        assert_eq!(
            es.proportion_phrase(2, 2, Some("foo"), &f),
            "ambas foos"
        );
    }

    // ── Send + Sync assertion ─────────────────────────────────────────────────

    #[test]
    fn spanish_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Spanish>();
    }

    // ── since_last_marker ─────────────────────────────────────────────────────

    #[cfg(feature = "time")]
    #[test]
    fn since_last_marker_at_same_time() {
        let es = Spanish::new();
        assert_eq!(es.since_last_marker(0), "al mismo tiempo");
        assert_eq!(es.since_last_marker(-10), "al mismo tiempo");
    }

    #[cfg(feature = "time")]
    #[test]
    fn since_last_marker_moments_later() {
        let es = Spanish::new();
        assert_eq!(es.since_last_marker(30), "momentos después");
    }

    #[cfg(feature = "time")]
    #[test]
    fn since_last_marker_the_next_day() {
        let es = Spanish::new();
        assert_eq!(es.since_last_marker(86_400 + 1), "al día siguiente");
    }

    #[cfg(feature = "time")]
    #[test]
    fn since_last_marker_following_week() {
        let es = Spanish::new();
        assert_eq!(es.since_last_marker(7 * 86_400 + 1), "la semana siguiente");
    }

    #[cfg(feature = "time")]
    #[test]
    fn since_last_marker_months_later() {
        let es = Spanish::new();
        assert_eq!(es.since_last_marker(3 * 30 * 86_400), "3 meses después");
    }
}
