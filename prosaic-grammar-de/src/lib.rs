//! German grammar layer for the Prosaic NLG engine.
//!
//! Covers: gender-aware articles (all four cases × three genders × singular/plural),
//! regular and common-irregular noun pluralization, regular weak verb conjugation
//! (present/preterite/future) plus ~10 strong irregulars, past and present
//! participles, German cardinal number spelling (0–999,999), and gendered
//! nominative pronouns/demonstratives.
//!
//! Deliberately out of scope: attributive adjective declension, Konjunktiv,
//! Perfekt compound tenses, full strong-verb tables. See the plan doc for rationale.

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
use conjugate::{conjugate_de, past_participle_de, present_participle_de};
use numbers::number_to_words_de;
use pluralize::{pluralize_de, singularize_de};

/// German language grammar implementation.
///
/// Implements `Language` for German with case-aware article selection,
/// pluralization, and conjugation. See the crate-level documentation for
/// scope notes.
#[derive(Debug, Clone, Default)]
pub struct German;

impl German {
    pub fn new() -> Self {
        Self
    }
}

impl Language for German {
    fn pluralize(&self, word: &str, count: usize) -> String {
        if count == 1 {
            word.to_string()
        } else {
            pluralize_de(word)
        }
    }

    fn singularize(&self, word: &str) -> String {
        singularize_de(word)
    }

    /// Return the nominative definite article inferred from the noun's ending.
    fn article(&self, word: &str) -> &str {
        basic_article(word)
    }

    fn conjugate(&self, verb: &str, tense: Tense, person: Person) -> String {
        conjugate_de(verb, tense, person)
    }

    fn past_participle(&self, verb: &str) -> String {
        past_participle_de(verb)
    }

    fn present_participle(&self, verb: &str) -> String {
        present_participle_de(verb)
    }

    fn join_list(&self, items: &[&str], conjunction: Conjunction) -> String {
        let conj = match conjunction {
            Conjunction::And => "und",
            Conjunction::Or => "oder",
        };
        join_list_de(items, conj)
    }

    fn ordinal(&self, n: usize) -> String {
        match n {
            1 => "erste".into(),
            2 => "zweite".into(),
            3 => "dritte".into(),
            4 => "vierte".into(),
            5 => "fünfte".into(),
            6 => "sechste".into(),
            7 => "siebte".into(),
            8 => "achte".into(),
            9 => "neunte".into(),
            10 => "zehnte".into(),
            _ => format!("{n}."),
        }
    }

    fn number_to_words(&self, n: usize) -> String {
        number_to_words_de(n)
    }

    /// German plural: `n == 1` → One, else Other (CLDR `de`).
    fn plural_category(&self, n: i64) -> PluralCategory {
        match n {
            1 => PluralCategory::One,
            _ => PluralCategory::Other,
        }
    }

    fn realize_reference(
        &self,
        form: ReferenceForm,
        features: &AgreementFeatures,
    ) -> Option<String> {
        match form {
            ReferenceForm::Pronoun => Some(german_pronoun(features)),
            ReferenceForm::Possessive => Some(german_possessive(features)),
            ReferenceForm::Demonstrative => Some(german_demonstrative(features)),
            ReferenceForm::Zero => None,
            ReferenceForm::Full | ReferenceForm::ShortName => None,
        }
    }

    fn plural_description(
        &self,
        entity_type: &str,
        count: usize,
        features: &AgreementFeatures,
    ) -> String {
        match count {
            0 => String::new(),
            1 => format!("{} {}", article_with_features(features), entity_type),
            _ => {
                let plural_features = AgreementFeatures::default()
                    .with_gender(features.gender)
                    .with_case(features.case)
                    .with_number(GrammaticalNumber::Plural);
                format!(
                    "{} {count} {}",
                    article_with_features(&plural_features),
                    self.pluralize(entity_type, count)
                )
            }
        }
    }

    fn proportion_phrase(
        &self,
        matching: i64,
        total: i64,
        noun_singular: Option<&str>,
        features: &AgreementFeatures,
    ) -> String {
        german_proportion(matching, total, noun_singular, features)
    }

    fn discourse_marker(&self, relation: RstRelation) -> Option<&'static str> {
        use RstRelation::*;
        Some(match relation {
            Elaboration => "Außerdem ",
            Contrast => "Allerdings ",
            Cause => "Deshalb ",
            Result => "Folglich ",
            Concession => "Dennoch ",
            Sequence => "Dann ",
            Condition => "Wenn dies geschieht, ",
            Background => "Inzwischen ",
            Summary => "Zusammenfassend ",
        })
    }

    fn is_connective_opener(&self, text: &str) -> bool {
        const GERMAN_OPENERS: &[&str] = &[
            "Außerdem",
            "Darüber hinaus",
            "Zudem",
            "Ebenso",
            "Allerdings",
            "Andererseits",
            "Inzwischen",
            "Deshalb",
            "Folglich",
            "Dennoch",
            "Dann",
            "Wenn dies geschieht,",
            "Zusammenfassend",
        ];
        GERMAN_OPENERS.iter().any(|opener| text.starts_with(opener))
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
            return "zur gleichen Zeit".to_string();
        }
        if diff_secs < 60 {
            return "einen Augenblick später".to_string();
        }
        if diff_secs < HOUR {
            let n = ((diff_secs + MINUTE / 2) / MINUTE).max(1);
            return match n {
                1 => "eine Minute später".to_string(),
                _ => format!("{n} Minuten später"),
            };
        }
        if diff_secs < DAY {
            let n = ((diff_secs + HOUR / 2) / HOUR).max(1);
            if n < 6 {
                return match n {
                    1 => "eine Stunde später".to_string(),
                    _ => format!("{n} Stunden später"),
                };
            }
            return "später am Tag".to_string();
        }
        if diff_secs < 2 * DAY {
            return "am nächsten Tag".to_string();
        }
        if diff_secs < WEEK {
            let n = diff_secs / DAY;
            return format!("{n} Tage später");
        }
        if diff_secs < 2 * WEEK {
            return "in der folgenden Woche".to_string();
        }
        if diff_secs < MONTH {
            let n = diff_secs / WEEK;
            return format!("{n} Wochen später");
        }
        if diff_secs < 2 * MONTH {
            return "im folgenden Monat".to_string();
        }
        if diff_secs < YEAR {
            let n = diff_secs / MONTH;
            return format!("{n} Monate später");
        }
        if diff_secs < 2 * YEAR {
            return "im folgenden Jahr".to_string();
        }
        let n = diff_secs / YEAR;
        format!("{n} Jahre später")
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn german_pronoun(features: &AgreementFeatures) -> String {
    let plural = matches!(
        features.number,
        GrammaticalNumber::Plural | GrammaticalNumber::Dual
    );
    if plural {
        return "sie".into();
    }
    match features.gender {
        Gender::Fem => "sie".into(),
        Gender::Neut => "es".into(),
        _ => "er".into(),
    }
}

fn german_possessive(features: &AgreementFeatures) -> String {
    let plural = matches!(
        features.number,
        GrammaticalNumber::Plural | GrammaticalNumber::Dual
    );
    if plural {
        return "ihre".into();
    }
    match features.gender {
        Gender::Fem => "ihre".into(),
        _ => "sein".into(),
    }
}

fn german_demonstrative(features: &AgreementFeatures) -> String {
    let plural = matches!(
        features.number,
        GrammaticalNumber::Plural | GrammaticalNumber::Dual
    );
    if plural {
        return "diese".into();
    }
    match features.gender {
        Gender::Fem => "diese".into(),
        Gender::Neut => "dieses".into(),
        _ => "dieser".into(),
    }
}

/// Resolve gender for proportion phrasing: explicit features take precedence,
/// then suffix inference on the noun (single-word; multi-word phrases use
/// the first token), then a masculine fallback.
fn resolve_proportion_gender(noun: Option<&str>, features: &AgreementFeatures) -> Gender {
    match features.gender {
        Gender::Unknown => noun
            .and_then(|n| n.split_whitespace().next())
            .map(gender::infer_gender)
            .unwrap_or(Gender::Masc),
        g => g,
    }
}

/// German proportion phrasing.
///
/// Limitations: attributive-adjective declension is out of scope for v1
/// (see crate docs). Pass single-word nouns (`"Datei"`, `"Buch"`,
/// `"Tisch"`) for correct output. Multi-word noun phrases like
/// `"geänderte Datei"` will pluralize the head noun but leave preceding
/// adjectives undeclined — readable but not perfectly correct German.
fn german_proportion(
    matching: i64,
    total: i64,
    noun_singular: Option<&str>,
    features: &AgreementFeatures,
) -> String {
    let n = matching.max(0);
    let t = total.max(0);
    let gender = resolve_proportion_gender(noun_singular, features);

    if t == 0 {
        return match (noun_singular, n, gender) {
            (Some(noun), 0, Gender::Fem) => format!("keine {noun}"),
            (Some(noun), 0, _) => format!("kein {noun}"),
            (None, 0, Gender::Fem) => "keine".to_string(),
            (None, 0, Gender::Neut) => "keines".to_string(),
            (None, 0, _) => "keiner".to_string(),
            // n > 0, t == 0: self-inconsistent; literal fall-through.
            (Some(noun), _, _) => format!("{n} von 0 {}", pluralize_de(noun)),
            (None, _, _) => format!("{n} von 0"),
        };
    }

    if n == 0 {
        let none_word = match gender {
            Gender::Fem => "keine",
            Gender::Neut => "keines",
            _ => "keiner",
        };
        // Genitive plural article "der" is gender-neutral in plural.
        return match noun_singular {
            Some(noun) => format!("{none_word} der {t} {}", pluralize_de(noun)),
            None => format!("{none_word} der {t}"),
        };
    }

    if n >= t {
        return match (noun_singular, t, gender) {
            (Some(noun), 1, Gender::Fem) => format!("die einzige {noun}"),
            (Some(noun), 1, Gender::Neut) => format!("das einzige {noun}"),
            (Some(noun), 1, _) => format!("der einzige {noun}"),
            (None, 1, Gender::Fem) => "die einzige".to_string(),
            (None, 1, Gender::Neut) => "das einzige".to_string(),
            (None, 1, _) => "der einzige".to_string(),
            // "beide" doesn't decline by gender in nominative.
            (Some(noun), 2, _) => format!("beide {}", pluralize_de(noun)),
            (None, 2, _) => "beide".to_string(),
            // "alle N" works across genders in nominative.
            (Some(noun), _, _) => format!("alle {t} {}", pluralize_de(noun)),
            (None, _, _) => format!("alle {t}"),
        };
    }

    match noun_singular {
        Some(noun) => format!("{n} von {t} {}", pluralize_de(noun)),
        None => format!("{n} von {t}"),
    }
}

fn join_list_de(items: &[&str], conj: &str) -> String {
    match items.len() {
        0 => String::new(),
        1 => items[0].into(),
        2 => format!("{} {} {}", items[0], conj, items[1]),
        _ => {
            let (last, rest) = items.split_last().unwrap();
            // German: "X, Y und Z" — no Oxford comma before und/oder
            format!("{} {} {}", rest.join(", "), conj, last)
        }
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use prosaic_core::Case;

    // ── pluralize ─────────────────────────────────────────────────────────────

    #[test]
    fn pluralize_count_one_returns_unchanged() {
        let de = German::new();
        assert_eq!(de.pluralize("Klasse", 1), "Klasse");
    }

    #[test]
    fn pluralize_count_zero_pluralizes() {
        let de = German::new();
        assert_eq!(de.pluralize("Mann", 0), "Männer");
    }

    #[test]
    fn pluralize_count_two_pluralizes() {
        let de = German::new();
        assert_eq!(de.pluralize("Kind", 2), "Kinder");
    }

    // ── singularize ───────────────────────────────────────────────────────────

    #[test]
    fn singularize_delegates() {
        let de = German::new();
        assert_eq!(de.singularize("Männer"), "Mann");
        assert_eq!(de.singularize("Zeitungen"), "Zeitung");
    }

    // ── article ───────────────────────────────────────────────────────────────

    #[test]
    fn article_masc_inferred() {
        let de = German::new();
        assert_eq!(de.article("Tisch"), "der");
    }

    #[test]
    fn article_fem_inferred() {
        let de = German::new();
        assert_eq!(de.article("Freiheit"), "die");
    }

    #[test]
    fn article_neut_inferred() {
        let de = German::new();
        assert_eq!(de.article("Buch"), "das");
    }

    // ── conjugate ─────────────────────────────────────────────────────────────

    #[test]
    fn conjugate_present_first() {
        let de = German::new();
        assert_eq!(
            de.conjugate("machen", Tense::Present, Person::First),
            "mache"
        );
    }

    #[test]
    fn conjugate_present_third() {
        let de = German::new();
        assert_eq!(
            de.conjugate("machen", Tense::Present, Person::Third),
            "macht"
        );
    }

    #[test]
    fn conjugate_past_first() {
        let de = German::new();
        assert_eq!(de.conjugate("machen", Tense::Past, Person::First), "machte");
    }

    #[test]
    fn conjugate_past_third() {
        let de = German::new();
        assert_eq!(de.conjugate("machen", Tense::Past, Person::Third), "machte");
    }

    // ── participles ───────────────────────────────────────────────────────────

    #[test]
    fn past_participle_machen() {
        let de = German::new();
        assert_eq!(de.past_participle("machen"), "gemacht");
    }

    #[test]
    fn present_participle_machen() {
        let de = German::new();
        assert_eq!(de.present_participle("machen"), "machend");
    }

    // ── join_list ─────────────────────────────────────────────────────────────

    // ── proportion_phrase ─────────────────────────────────────────────────────

    fn no_features() -> AgreementFeatures {
        AgreementFeatures::default()
    }

    #[test]
    fn proportion_two_of_two_fem_noun_reads_beide_plural() {
        let de = German::new();
        assert_eq!(
            de.proportion_phrase(2, 2, Some("Datei"), &no_features()),
            "beide Dateien"
        );
    }

    #[test]
    fn proportion_all_n_with_noun_reads_alle_n_plural() {
        let de = German::new();
        assert_eq!(
            de.proportion_phrase(13, 13, Some("Datei"), &no_features()),
            "alle 13 Dateien"
        );
    }

    #[test]
    fn proportion_one_of_one_masc_reads_der_einzige() {
        let de = German::new();
        assert_eq!(
            de.proportion_phrase(1, 1, Some("Tisch"), &no_features()),
            "der einzige Tisch"
        );
    }

    #[test]
    fn proportion_one_of_one_fem_reads_die_einzige() {
        let de = German::new();
        assert_eq!(
            de.proportion_phrase(1, 1, Some("Datei"), &no_features()),
            "die einzige Datei"
        );
    }

    #[test]
    fn proportion_one_of_one_neut_reads_das_einzige() {
        let de = German::new();
        assert_eq!(
            de.proportion_phrase(1, 1, Some("Buch"), &no_features()),
            "das einzige Buch"
        );
    }

    #[test]
    fn proportion_zero_of_n_masc_reads_keiner() {
        let de = German::new();
        assert_eq!(
            de.proportion_phrase(0, 5, Some("Tisch"), &no_features()),
            "keiner der 5 Tische"
        );
    }

    #[test]
    fn proportion_zero_of_n_fem_reads_keine() {
        let de = German::new();
        assert_eq!(
            de.proportion_phrase(0, 5, Some("Datei"), &no_features()),
            "keine der 5 Dateien"
        );
    }

    #[test]
    fn proportion_zero_of_n_neut_reads_keines() {
        let de = German::new();
        assert_eq!(
            de.proportion_phrase(0, 5, Some("Buch"), &no_features()),
            "keines der 5 Bücher"
        );
    }

    #[test]
    fn proportion_zero_zero_masc_reads_kein() {
        let de = German::new();
        assert_eq!(
            de.proportion_phrase(0, 0, Some("Tisch"), &no_features()),
            "kein Tisch"
        );
    }

    #[test]
    fn proportion_zero_zero_fem_reads_keine() {
        let de = German::new();
        assert_eq!(
            de.proportion_phrase(0, 0, Some("Datei"), &no_features()),
            "keine Datei"
        );
    }

    #[test]
    fn proportion_zero_zero_neut_reads_kein() {
        let de = German::new();
        assert_eq!(
            de.proportion_phrase(0, 0, Some("Buch"), &no_features()),
            "kein Buch"
        );
    }

    #[test]
    fn proportion_partial_with_noun() {
        let de = German::new();
        assert_eq!(
            de.proportion_phrase(3, 13, Some("Datei"), &no_features()),
            "3 von 13 Dateien"
        );
    }

    #[test]
    fn proportion_no_noun_two_two_default_masc() {
        let de = German::new();
        assert_eq!(de.proportion_phrase(2, 2, None, &no_features()), "beide");
    }

    #[test]
    fn proportion_no_noun_all_n() {
        let de = German::new();
        assert_eq!(de.proportion_phrase(7, 7, None, &no_features()), "alle 7");
    }

    #[test]
    fn proportion_no_noun_partial() {
        let de = German::new();
        assert_eq!(de.proportion_phrase(3, 7, None, &no_features()), "3 von 7");
    }

    #[test]
    fn proportion_no_noun_one_of_one_default_masc() {
        let de = German::new();
        assert_eq!(
            de.proportion_phrase(1, 1, None, &no_features()),
            "der einzige"
        );
    }

    #[test]
    fn proportion_no_noun_one_of_one_fem_explicit() {
        let de = German::new();
        let f = AgreementFeatures::default().with_gender(Gender::Fem);
        assert_eq!(de.proportion_phrase(1, 1, None, &f), "die einzige");
    }

    #[test]
    fn proportion_no_noun_one_of_one_neut_explicit() {
        let de = German::new();
        let f = AgreementFeatures::default().with_gender(Gender::Neut);
        assert_eq!(de.proportion_phrase(1, 1, None, &f), "das einzige");
    }

    #[test]
    fn proportion_no_noun_zero_of_n_masc() {
        let de = German::new();
        assert_eq!(
            de.proportion_phrase(0, 5, None, &no_features()),
            "keiner der 5"
        );
    }

    #[test]
    fn proportion_no_noun_zero_of_n_fem() {
        let de = German::new();
        let f = AgreementFeatures::default().with_gender(Gender::Fem);
        assert_eq!(de.proportion_phrase(0, 5, None, &f), "keine der 5");
    }

    #[test]
    fn proportion_no_noun_zero_zero_default_masc() {
        let de = German::new();
        assert_eq!(de.proportion_phrase(0, 0, None, &no_features()), "keiner");
    }

    #[test]
    fn proportion_no_noun_zero_zero_neut() {
        let de = German::new();
        let f = AgreementFeatures::default().with_gender(Gender::Neut);
        assert_eq!(de.proportion_phrase(0, 0, None, &f), "keines");
    }

    #[test]
    fn proportion_features_gender_overrides_inference() {
        let de = German::new();
        // Override fem despite "Tisch" inferring masc.
        let f = AgreementFeatures::default().with_gender(Gender::Fem);
        assert_eq!(
            de.proportion_phrase(0, 0, Some("Tisch"), &f),
            "keine Tisch"
        );
    }

    #[test]
    fn join_list_empty() {
        let de = German::new();
        assert_eq!(de.join_list(&[], Conjunction::And), "");
    }

    #[test]
    fn join_list_single() {
        let de = German::new();
        assert_eq!(de.join_list(&["eins"], Conjunction::And), "eins");
    }

    #[test]
    fn join_list_two_and() {
        let de = German::new();
        assert_eq!(
            de.join_list(&["eins", "zwei"], Conjunction::And),
            "eins und zwei"
        );
    }

    #[test]
    fn join_list_three_and_no_oxford_comma() {
        let de = German::new();
        assert_eq!(
            de.join_list(&["eins", "zwei", "drei"], Conjunction::And),
            "eins, zwei und drei"
        );
    }

    #[test]
    fn join_list_two_or() {
        let de = German::new();
        assert_eq!(
            de.join_list(&["ja", "nein"], Conjunction::Or),
            "ja oder nein"
        );
    }

    // ── ordinal ───────────────────────────────────────────────────────────────

    #[test]
    fn ordinal_first_ten() {
        let de = German::new();
        assert_eq!(de.ordinal(1), "erste");
        assert_eq!(de.ordinal(3), "dritte");
        assert_eq!(de.ordinal(7), "siebte");
        assert_eq!(de.ordinal(10), "zehnte");
    }

    #[test]
    fn ordinal_beyond_ten_numeric() {
        let de = German::new();
        assert_eq!(de.ordinal(11), "11.");
        assert_eq!(de.ordinal(100), "100.");
    }

    // ── number_to_words ───────────────────────────────────────────────────────

    #[test]
    fn number_to_words_spot_checks() {
        let de = German::new();
        assert_eq!(de.number_to_words(3), "drei");
        assert_eq!(de.number_to_words(21), "einundzwanzig");
    }

    // ── plural_category ───────────────────────────────────────────────────────

    #[test]
    fn plural_category_one_is_one() {
        let de = German::new();
        assert_eq!(de.plural_category(1), PluralCategory::One);
    }

    #[test]
    fn plural_category_zero_is_other() {
        let de = German::new();
        assert_eq!(de.plural_category(0), PluralCategory::Other);
    }

    #[test]
    fn plural_category_many_is_other() {
        let de = German::new();
        assert_eq!(de.plural_category(5), PluralCategory::Other);
    }

    // ── realize_reference ─────────────────────────────────────────────────────

    #[test]
    fn pronoun_masc_singular() {
        let de = German::new();
        let f = AgreementFeatures::default().with_gender(Gender::Masc);
        assert_eq!(
            de.realize_reference(ReferenceForm::Pronoun, &f),
            Some("er".to_string())
        );
    }

    #[test]
    fn pronoun_fem_singular() {
        let de = German::new();
        let f = AgreementFeatures::default().with_gender(Gender::Fem);
        assert_eq!(
            de.realize_reference(ReferenceForm::Pronoun, &f),
            Some("sie".to_string())
        );
    }

    #[test]
    fn pronoun_neut_singular() {
        let de = German::new();
        let f = AgreementFeatures::default().with_gender(Gender::Neut);
        assert_eq!(
            de.realize_reference(ReferenceForm::Pronoun, &f),
            Some("es".to_string())
        );
    }

    #[test]
    fn pronoun_plural() {
        let de = German::new();
        let f = AgreementFeatures::default().with_number(GrammaticalNumber::Plural);
        assert_eq!(
            de.realize_reference(ReferenceForm::Pronoun, &f),
            Some("sie".to_string())
        );
    }

    #[test]
    fn possessive_tracks_owner_gender_and_plural() {
        let de = German::new();
        let masc = AgreementFeatures::default().with_gender(Gender::Masc);
        assert_eq!(
            de.realize_reference(ReferenceForm::Possessive, &masc),
            Some("sein".to_string())
        );
        let fem = AgreementFeatures::default().with_gender(Gender::Fem);
        assert_eq!(
            de.realize_reference(ReferenceForm::Possessive, &fem),
            Some("ihre".to_string())
        );
        let plural = AgreementFeatures::default().with_number(GrammaticalNumber::Plural);
        assert_eq!(
            de.realize_reference(ReferenceForm::Possessive, &plural),
            Some("ihre".to_string())
        );
    }

    #[test]
    fn demonstrative_masc_singular() {
        let de = German::new();
        let f = AgreementFeatures::default().with_gender(Gender::Masc);
        assert_eq!(
            de.realize_reference(ReferenceForm::Demonstrative, &f),
            Some("dieser".to_string())
        );
    }

    #[test]
    fn demonstrative_fem_singular() {
        let de = German::new();
        let f = AgreementFeatures::default().with_gender(Gender::Fem);
        assert_eq!(
            de.realize_reference(ReferenceForm::Demonstrative, &f),
            Some("diese".to_string())
        );
    }

    #[test]
    fn demonstrative_neut_singular() {
        let de = German::new();
        let f = AgreementFeatures::default().with_gender(Gender::Neut);
        assert_eq!(
            de.realize_reference(ReferenceForm::Demonstrative, &f),
            Some("dieses".to_string())
        );
    }

    #[test]
    fn demonstrative_plural() {
        let de = German::new();
        let f = AgreementFeatures::default().with_number(GrammaticalNumber::Plural);
        assert_eq!(
            de.realize_reference(ReferenceForm::Demonstrative, &f),
            Some("diese".to_string())
        );
    }

    #[test]
    fn realize_zero_is_none() {
        let de = German::new();
        assert_eq!(
            de.realize_reference(ReferenceForm::Zero, &AgreementFeatures::default()),
            None
        );
    }

    #[test]
    fn realize_full_is_none() {
        let de = German::new();
        assert_eq!(
            de.realize_reference(ReferenceForm::Full, &AgreementFeatures::default()),
            None
        );
    }

    // ── plural_description ────────────────────────────────────────────────────

    #[test]
    fn plural_description_zero_is_empty() {
        let de = German::new();
        assert_eq!(
            de.plural_description("Klasse", 0, &AgreementFeatures::default()),
            ""
        );
    }

    #[test]
    fn plural_description_one_masc() {
        let de = German::new();
        let f = AgreementFeatures::default().with_gender(Gender::Masc);
        assert_eq!(de.plural_description("Tisch", 1, &f), "der Tisch");
    }

    #[test]
    fn plural_description_one_fem() {
        let de = German::new();
        let f = AgreementFeatures::default().with_gender(Gender::Fem);
        assert_eq!(de.plural_description("Klasse", 1, &f), "die Klasse");
    }

    #[test]
    fn plural_description_one_neut() {
        let de = German::new();
        let f = AgreementFeatures::default().with_gender(Gender::Neut);
        assert_eq!(de.plural_description("Haus", 1, &f), "das Haus");
    }

    #[test]
    fn plural_description_many_masc() {
        let de = German::new();
        let f = AgreementFeatures::default().with_gender(Gender::Masc);
        assert_eq!(de.plural_description("Mann", 3, &f), "die 3 Männer");
    }

    #[test]
    fn plural_description_many_fem() {
        let de = German::new();
        let f = AgreementFeatures::default().with_gender(Gender::Fem);
        assert_eq!(de.plural_description("Klasse", 3, &f), "die 3 Klassen");
    }

    #[test]
    fn plural_description_many_neut() {
        let de = German::new();
        let f = AgreementFeatures::default().with_gender(Gender::Neut);
        assert_eq!(de.plural_description("Haus", 3, &f), "die 3 Häuser");
    }

    // ── Case-aware plural_description ─────────────────────────────────────────

    #[test]
    fn plural_description_dative_plural_uses_den() {
        let de = German::new();
        let f = AgreementFeatures::default()
            .with_gender(Gender::Masc)
            .with_case(Case::Dative);
        // plural dative = "den"
        assert_eq!(de.plural_description("Mann", 3, &f), "den 3 Männer");
    }

    #[test]
    fn plural_description_dative_singular_masc_uses_dem() {
        let de = German::new();
        let f = AgreementFeatures::default()
            .with_gender(Gender::Masc)
            .with_case(Case::Dative);
        assert_eq!(de.plural_description("Tisch", 1, &f), "dem Tisch");
    }

    // ── discourse_marker ──────────────────────────────────────────────────────

    #[test]
    fn discourse_marker_german() {
        let de = German::new();
        assert_eq!(
            de.discourse_marker(RstRelation::Elaboration),
            Some("Außerdem ")
        );
        assert_eq!(
            de.discourse_marker(RstRelation::Contrast),
            Some("Allerdings ")
        );
        assert_eq!(de.discourse_marker(RstRelation::Result), Some("Folglich "));
    }

    // ── Send + Sync ───────────────────────────────────────────────────────────

    #[test]
    fn german_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<German>();
    }

    // ── since_last_marker ─────────────────────────────────────────────────────

    #[cfg(feature = "time")]
    #[test]
    fn since_last_marker_at_same_time() {
        let de = German::new();
        assert_eq!(de.since_last_marker(0), "zur gleichen Zeit");
        assert_eq!(de.since_last_marker(-5), "zur gleichen Zeit");
    }

    #[cfg(feature = "time")]
    #[test]
    fn since_last_marker_augenblick_spaeter() {
        let de = German::new();
        assert_eq!(de.since_last_marker(30), "einen Augenblick später");
    }

    #[cfg(feature = "time")]
    #[test]
    fn since_last_marker_am_naechsten_tag() {
        let de = German::new();
        assert_eq!(de.since_last_marker(86_400 + 1), "am nächsten Tag");
    }

    #[cfg(feature = "time")]
    #[test]
    fn since_last_marker_folgende_woche() {
        let de = German::new();
        assert_eq!(
            de.since_last_marker(7 * 86_400 + 1),
            "in der folgenden Woche"
        );
    }

    #[cfg(feature = "time")]
    #[test]
    fn since_last_marker_monate_spaeter() {
        let de = German::new();
        assert_eq!(de.since_last_marker(3 * 30 * 86_400), "3 Monate später");
    }
}
