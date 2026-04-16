//! German verb conjugation.
//!
//! Scope: regular weak verbs (present / preterite / future) plus ~10 common
//! strong/modal irregulars: sein, haben, werden, können, müssen, wollen,
//! gehen, kommen, sehen.
//!
//! Regular present: stem + -e / -st / -t
//! Regular preterite: stem + -te / -test / -te
//! Future (simplified): wird/werde/wirst + infinitive
//! Past participle (weak): ge- + stem + -t
//! Present participle: stem + -end

use prosaic_core::{Person, Tense};

// ── Irregular lookup ──────────────────────────────────────────────────────────

/// Returns [ich, du, er/sie/es, wir, ihr, sie/Sie] forms.
/// Only three of these indices are exposed via the trait's Person enum.
fn irregular_lookup(verb: &str, tense: Tense, person: Person) -> Option<&'static str> {
    let forms: Option<[&'static str; 6]> = match (verb, tense) {
        // sein
        ("sein", Tense::Present) => Some(["bin", "bist", "ist", "sind", "seid", "sind"]),
        ("sein", Tense::Past) => Some(["war", "warst", "war", "waren", "wart", "waren"]),
        // haben
        ("haben", Tense::Present) => Some(["habe", "hast", "hat", "haben", "habt", "haben"]),
        ("haben", Tense::Past) => Some(["hatte", "hattest", "hatte", "hatten", "hattet", "hatten"]),
        // werden
        ("werden", Tense::Present) => {
            Some(["werde", "wirst", "wird", "werden", "werdet", "werden"])
        }
        ("werden", Tense::Past) => {
            Some(["wurde", "wurdest", "wurde", "wurden", "wurdet", "wurden"])
        }
        // können
        ("können", Tense::Present) => {
            Some(["kann", "kannst", "kann", "können", "könnt", "können"])
        }
        ("können", Tense::Past) => Some([
            "konnte", "konntest", "konnte", "konnten", "konntet", "konnten",
        ]),
        // müssen
        ("müssen", Tense::Present) => Some(["muss", "musst", "muss", "müssen", "müsst", "müssen"]),
        ("müssen", Tense::Past) => Some([
            "musste", "musstest", "musste", "mussten", "musstet", "mussten",
        ]),
        // sollen
        ("sollen", Tense::Present) => Some(["soll", "sollst", "soll", "sollen", "sollt", "sollen"]),
        ("sollen", Tense::Past) => Some([
            "sollte", "solltest", "sollte", "sollten", "solltet", "sollten",
        ]),
        // wollen
        ("wollen", Tense::Present) => Some(["will", "willst", "will", "wollen", "wollt", "wollen"]),
        ("wollen", Tense::Past) => Some([
            "wollte", "wolltest", "wollte", "wollten", "wolltet", "wollten",
        ]),
        // gehen — strong past only; present is regular
        ("gehen", Tense::Past) => Some(["ging", "gingst", "ging", "gingen", "gingt", "gingen"]),
        // kommen — strong past only
        ("kommen", Tense::Past) => Some(["kam", "kamst", "kam", "kamen", "kamt", "kamen"]),
        // sehen — strong past only
        ("sehen", Tense::Past) => Some(["sah", "sahst", "sah", "sahen", "saht", "sahen"]),
        _ => None,
    };
    forms.map(|f| f[person_index(person)])
}

fn person_index(p: Person) -> usize {
    match p {
        Person::First => 0,  // ich
        Person::Second => 1, // du
        Person::Third => 2,  // er/sie/es
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Conjugate a German verb.
pub fn conjugate_de(verb: &str, tense: Tense, person: Person) -> String {
    if let Some(form) = irregular_lookup(verb, tense, person) {
        return form.to_string();
    }

    // Future always delegates to the future helper (even for strong verbs)
    if tense == Tense::Future {
        return conjugate_future(verb, person);
    }

    // Strip -en infinitive suffix to get the stem
    let stem = verb.strip_suffix("en").unwrap_or(verb);

    match tense {
        Tense::Present => conjugate_present_regular(stem, person),
        Tense::Past => conjugate_preterite_regular(stem, person),
        Tense::Future => unreachable!(), // handled above
    }
}

/// Return the past participle (ge- + stem + -t for weak verbs).
pub fn past_participle_de(verb: &str) -> String {
    let stem = verb.strip_suffix("en").unwrap_or(verb);
    format!("ge{stem}t")
}

/// Return the present participle (stem + -end).
pub fn present_participle_de(verb: &str) -> String {
    let stem = verb.strip_suffix("en").unwrap_or(verb);
    format!("{stem}end")
}

// ── Regular helpers ───────────────────────────────────────────────────────────

fn conjugate_present_regular(stem: &str, person: Person) -> String {
    let ending = match person {
        Person::First => "e",
        Person::Second => "st",
        Person::Third => "t",
    };
    format!("{stem}{ending}")
}

fn conjugate_preterite_regular(stem: &str, person: Person) -> String {
    let ending = match person {
        Person::First => "te",
        Person::Second => "test",
        Person::Third => "te",
    };
    format!("{stem}{ending}")
}

fn conjugate_future(verb: &str, person: Person) -> String {
    let aux = match person {
        Person::First => "werde",
        Person::Second => "wirst",
        Person::Third => "wird",
    };
    format!("{aux} {verb}")
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Regular present ───────────────────────────────────────────────────────

    #[test]
    fn regular_present_first_machen() {
        assert_eq!(
            conjugate_de("machen", Tense::Present, Person::First),
            "mache"
        );
    }

    #[test]
    fn regular_present_second_machen() {
        assert_eq!(
            conjugate_de("machen", Tense::Present, Person::Second),
            "machst"
        );
    }

    #[test]
    fn regular_present_third_machen() {
        assert_eq!(
            conjugate_de("machen", Tense::Present, Person::Third),
            "macht"
        );
    }

    // ── Regular preterite ─────────────────────────────────────────────────────

    #[test]
    fn regular_preterite_first_machen() {
        assert_eq!(conjugate_de("machen", Tense::Past, Person::First), "machte");
    }

    #[test]
    fn regular_preterite_second_machen() {
        assert_eq!(
            conjugate_de("machen", Tense::Past, Person::Second),
            "machtest"
        );
    }

    #[test]
    fn regular_preterite_third_machen() {
        assert_eq!(conjugate_de("machen", Tense::Past, Person::Third), "machte");
    }

    // ── Future ────────────────────────────────────────────────────────────────

    #[test]
    fn future_first_person() {
        assert_eq!(
            conjugate_de("machen", Tense::Future, Person::First),
            "werde machen"
        );
    }

    #[test]
    fn future_second_person() {
        assert_eq!(
            conjugate_de("machen", Tense::Future, Person::Second),
            "wirst machen"
        );
    }

    #[test]
    fn future_third_person() {
        assert_eq!(
            conjugate_de("machen", Tense::Future, Person::Third),
            "wird machen"
        );
    }

    // ── Past participle ───────────────────────────────────────────────────────

    #[test]
    fn past_participle_machen() {
        assert_eq!(past_participle_de("machen"), "gemacht");
    }

    #[test]
    fn past_participle_sagen() {
        assert_eq!(past_participle_de("sagen"), "gesagt");
    }

    // ── Present participle ────────────────────────────────────────────────────

    #[test]
    fn present_participle_machen() {
        assert_eq!(present_participle_de("machen"), "machend");
    }

    #[test]
    fn present_participle_laufen() {
        assert_eq!(present_participle_de("laufen"), "laufend");
    }

    // ── Irregulars ────────────────────────────────────────────────────────────

    #[test]
    fn irregular_sein_present_first() {
        assert_eq!(conjugate_de("sein", Tense::Present, Person::First), "bin");
    }

    #[test]
    fn irregular_sein_present_third() {
        assert_eq!(conjugate_de("sein", Tense::Present, Person::Third), "ist");
    }

    #[test]
    fn irregular_sein_past_first() {
        assert_eq!(conjugate_de("sein", Tense::Past, Person::First), "war");
    }

    #[test]
    fn irregular_haben_present_third() {
        assert_eq!(conjugate_de("haben", Tense::Present, Person::Third), "hat");
    }

    #[test]
    fn irregular_haben_past_first() {
        assert_eq!(conjugate_de("haben", Tense::Past, Person::First), "hatte");
    }

    #[test]
    fn irregular_werden_present_third() {
        assert_eq!(
            conjugate_de("werden", Tense::Present, Person::Third),
            "wird"
        );
    }

    #[test]
    fn irregular_können_present_first() {
        assert_eq!(
            conjugate_de("können", Tense::Present, Person::First),
            "kann"
        );
    }

    #[test]
    fn irregular_wollen_present_first() {
        assert_eq!(
            conjugate_de("wollen", Tense::Present, Person::First),
            "will"
        );
    }

    #[test]
    fn irregular_gehen_past_first() {
        assert_eq!(conjugate_de("gehen", Tense::Past, Person::First), "ging");
    }

    #[test]
    fn irregular_sehen_past_third() {
        assert_eq!(conjugate_de("sehen", Tense::Past, Person::Third), "sah");
    }
}
