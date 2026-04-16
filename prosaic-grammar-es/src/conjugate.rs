//! Spanish verb conjugation.
//!
//! Scope: regular -ar/-er/-ir verbs in present, simple past (pretérito),
//! and future indicative; past participle; present participle (gerundio).
//! Plus ~10 common irregulars: ser, estar, haber, tener, ir, ver, hacer,
//! decir, poder, querer.
//!
//! NOT in scope: stem-changing verbs, imperfecto, subjunctive, compound
//! tenses beyond the primitives above, full irregular tables.

use prosaic_core::{Person, Tense};

// ── Irregular lookup tables ───────────────────────────────────────────────────

/// (verb, tense) → [yo, tú, él/ella, nosotros, vosotros, ellos/ellas]
fn irregular_lookup(verb: &str, tense: Tense, person: Person) -> Option<&'static str> {
    let forms: Option<[&'static str; 6]> = match (verb, tense) {
        // ser
        ("ser", Tense::Present) => Some(["soy", "eres", "es", "somos", "sois", "son"]),
        ("ser", Tense::Past)    => Some(["fui", "fuiste", "fue", "fuimos", "fuisteis", "fueron"]),
        ("ser", Tense::Future)  => Some(["seré", "serás", "será", "seremos", "seréis", "serán"]),
        // estar
        ("estar", Tense::Present) => Some(["estoy", "estás", "está", "estamos", "estáis", "están"]),
        ("estar", Tense::Past)    => Some(["estuve", "estuviste", "estuvo", "estuvimos", "estuvisteis", "estuvieron"]),
        ("estar", Tense::Future)  => Some(["estaré", "estarás", "estará", "estaremos", "estaréis", "estarán"]),
        // haber
        ("haber", Tense::Present) => Some(["he", "has", "ha", "hemos", "habéis", "han"]),
        ("haber", Tense::Past)    => Some(["hube", "hubiste", "hubo", "hubimos", "hubisteis", "hubieron"]),
        ("haber", Tense::Future)  => Some(["habré", "habrás", "habrá", "habremos", "habréis", "habrán"]),
        // tener
        ("tener", Tense::Present) => Some(["tengo", "tienes", "tiene", "tenemos", "tenéis", "tienen"]),
        ("tener", Tense::Past)    => Some(["tuve", "tuviste", "tuvo", "tuvimos", "tuvisteis", "tuvieron"]),
        ("tener", Tense::Future)  => Some(["tendré", "tendrás", "tendrá", "tendremos", "tendréis", "tendrán"]),
        // ir
        ("ir", Tense::Present) => Some(["voy", "vas", "va", "vamos", "vais", "van"]),
        ("ir", Tense::Past)    => Some(["fui", "fuiste", "fue", "fuimos", "fuisteis", "fueron"]),
        ("ir", Tense::Future)  => Some(["iré", "irás", "irá", "iremos", "iréis", "irán"]),
        // ver
        ("ver", Tense::Present) => Some(["veo", "ves", "ve", "vemos", "veis", "ven"]),
        ("ver", Tense::Past)    => Some(["vi", "viste", "vio", "vimos", "visteis", "vieron"]),
        ("ver", Tense::Future)  => Some(["veré", "verás", "verá", "veremos", "veréis", "verán"]),
        // hacer
        ("hacer", Tense::Present) => Some(["hago", "haces", "hace", "hacemos", "hacéis", "hacen"]),
        ("hacer", Tense::Past)    => Some(["hice", "hiciste", "hizo", "hicimos", "hicisteis", "hicieron"]),
        ("hacer", Tense::Future)  => Some(["haré", "harás", "hará", "haremos", "haréis", "harán"]),
        // decir
        ("decir", Tense::Present) => Some(["digo", "dices", "dice", "decimos", "decís", "dicen"]),
        ("decir", Tense::Past)    => Some(["dije", "dijiste", "dijo", "dijimos", "dijisteis", "dijeron"]),
        ("decir", Tense::Future)  => Some(["diré", "dirás", "dirá", "diremos", "diréis", "dirán"]),
        // poder
        ("poder", Tense::Present) => Some(["puedo", "puedes", "puede", "podemos", "podéis", "pueden"]),
        ("poder", Tense::Past)    => Some(["pude", "pudiste", "pudo", "pudimos", "pudisteis", "pudieron"]),
        ("poder", Tense::Future)  => Some(["podré", "podrás", "podrá", "podremos", "podréis", "podrán"]),
        // querer
        ("querer", Tense::Present) => Some(["quiero", "quieres", "quiere", "queremos", "queréis", "quieren"]),
        ("querer", Tense::Past)    => Some(["quise", "quisiste", "quiso", "quisimos", "quisisteis", "quisieron"]),
        ("querer", Tense::Future)  => Some(["querré", "querrás", "querrá", "querremos", "querréis", "querrán"]),
        _ => None,
    };
    forms.map(|f| f[person_index(person)])
}

fn person_index(person: Person) -> usize {
    // We map the three-person enum to third-person singular (index 2) by default.
    // Spanish has 6 forms; we use: 0=yo, 2=él/ella, 4=vosotros as stand-ins
    // but since the Language trait only exposes First/Second/Third we map:
    // First → yo (0), Second → tú (1), Third → él/ella (2)
    match person {
        Person::First  => 0,
        Person::Second => 1,
        Person::Third  => 2,
    }
}

// ── Regular conjugation ───────────────────────────────────────────────────────

/// Conjugate a regular Spanish verb.
pub fn conjugate_es(verb: &str, tense: Tense, person: Person) -> String {
    if let Some(form) = irregular_lookup(verb, tense, person) {
        return form.to_string();
    }
    match tense {
        Tense::Present => conjugate_present(verb, person),
        Tense::Past    => conjugate_preterite(verb, person),
        Tense::Future  => conjugate_future(verb, person),
    }
}

fn conjugate_present(verb: &str, person: Person) -> String {
    if let Some(stem) = verb.strip_suffix("ar") {
        let endings = ["o", "as", "a"];
        return format!("{stem}{}", endings[person_index(person)]);
    }
    if let Some(stem) = verb.strip_suffix("er") {
        let endings = ["o", "es", "e"];
        return format!("{stem}{}", endings[person_index(person)]);
    }
    if let Some(stem) = verb.strip_suffix("ir") {
        let endings = ["o", "es", "e"];
        return format!("{stem}{}", endings[person_index(person)]);
    }
    // Fallback: return verb unchanged
    verb.to_string()
}

fn conjugate_preterite(verb: &str, person: Person) -> String {
    if let Some(stem) = verb.strip_suffix("ar") {
        let endings = ["é", "aste", "ó"];
        return format!("{stem}{}", endings[person_index(person)]);
    }
    if let Some(stem) = verb.strip_suffix("er") {
        let endings = ["í", "iste", "ió"];
        return format!("{stem}{}", endings[person_index(person)]);
    }
    if let Some(stem) = verb.strip_suffix("ir") {
        let endings = ["í", "iste", "ió"];
        return format!("{stem}{}", endings[person_index(person)]);
    }
    verb.to_string()
}

fn conjugate_future(verb: &str, person: Person) -> String {
    // Future uses the infinitive as stem for regular verbs
    let endings = ["é", "ás", "á"];
    format!("{verb}{}", endings[person_index(person)])
}

// ── Participles ───────────────────────────────────────────────────────────────

const IRREGULAR_PAST_PARTICIPLES: &[(&str, &str)] = &[
    ("hacer",   "hecho"),
    ("decir",   "dicho"),
    ("ver",     "visto"),
    ("volver",  "vuelto"),
    ("poner",   "puesto"),
    ("romper",  "roto"),
    ("morir",   "muerto"),
    ("escribir","escrito"),
    ("abrir",   "abierto"),
    ("cubrir",  "cubierto"),
];

/// Return the past participle of a Spanish verb.
/// -ar → -ado, -er/-ir → -ido; irregulars override.
pub fn past_participle_es(verb: &str) -> String {
    let lower = verb.to_lowercase();
    for &(v, pp) in IRREGULAR_PAST_PARTICIPLES {
        if lower == v {
            return pp.to_string();
        }
    }
    if let Some(stem) = verb.strip_suffix("ar") {
        return format!("{stem}ado");
    }
    if let Some(stem) = verb.strip_suffix("er") {
        return format!("{stem}ido");
    }
    if let Some(stem) = verb.strip_suffix("ir") {
        return format!("{stem}ido");
    }
    verb.to_string()
}

/// Return the present participle (gerundio) of a Spanish verb.
/// -ar → -ando, -er/-ir → -iendo.
pub fn present_participle_es(verb: &str) -> String {
    if let Some(stem) = verb.strip_suffix("ar") {
        return format!("{stem}ando");
    }
    if let Some(stem) = verb.strip_suffix("er") {
        return format!("{stem}iendo");
    }
    if let Some(stem) = verb.strip_suffix("ir") {
        return format!("{stem}iendo");
    }
    verb.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Present tense ────────────────────────────────────────────────────────

    #[test]
    fn regular_ar_present_first() {
        assert_eq!(conjugate_es("hablar", Tense::Present, Person::First), "hablo");
    }

    #[test]
    fn regular_ar_present_second() {
        assert_eq!(conjugate_es("hablar", Tense::Present, Person::Second), "hablas");
    }

    #[test]
    fn regular_ar_present_third() {
        assert_eq!(conjugate_es("hablar", Tense::Present, Person::Third), "habla");
    }

    #[test]
    fn regular_er_present_first() {
        assert_eq!(conjugate_es("comer", Tense::Present, Person::First), "como");
    }

    #[test]
    fn regular_er_present_third() {
        assert_eq!(conjugate_es("comer", Tense::Present, Person::Third), "come");
    }

    #[test]
    fn regular_ir_present_third() {
        assert_eq!(conjugate_es("vivir", Tense::Present, Person::Third), "vive");
    }

    // ── Preterite tense ──────────────────────────────────────────────────────

    #[test]
    fn regular_ar_preterite_first() {
        assert_eq!(conjugate_es("hablar", Tense::Past, Person::First), "hablé");
    }

    #[test]
    fn regular_ar_preterite_third() {
        assert_eq!(conjugate_es("hablar", Tense::Past, Person::Third), "habló");
    }

    #[test]
    fn regular_er_preterite_third() {
        assert_eq!(conjugate_es("comer", Tense::Past, Person::Third), "comió");
    }

    #[test]
    fn regular_ir_preterite_first() {
        assert_eq!(conjugate_es("vivir", Tense::Past, Person::First), "viví");
    }

    // ── Future tense ─────────────────────────────────────────────────────────

    #[test]
    fn regular_ar_future_third() {
        assert_eq!(conjugate_es("hablar", Tense::Future, Person::Third), "hablará");
    }

    #[test]
    fn regular_er_future_first() {
        assert_eq!(conjugate_es("comer", Tense::Future, Person::First), "comeré");
    }

    // ── Irregulars ───────────────────────────────────────────────────────────

    #[test]
    fn irregular_ser_present_first() {
        assert_eq!(conjugate_es("ser", Tense::Present, Person::First), "soy");
    }

    #[test]
    fn irregular_ser_present_third() {
        assert_eq!(conjugate_es("ser", Tense::Present, Person::Third), "es");
    }

    #[test]
    fn irregular_ser_past_third() {
        assert_eq!(conjugate_es("ser", Tense::Past, Person::Third), "fue");
    }

    #[test]
    fn irregular_estar_present_first() {
        assert_eq!(conjugate_es("estar", Tense::Present, Person::First), "estoy");
    }

    #[test]
    fn irregular_tener_present_first() {
        assert_eq!(conjugate_es("tener", Tense::Present, Person::First), "tengo");
    }

    #[test]
    fn irregular_hacer_present_first() {
        assert_eq!(conjugate_es("hacer", Tense::Present, Person::First), "hago");
    }

    #[test]
    fn irregular_ir_present_first() {
        assert_eq!(conjugate_es("ir", Tense::Present, Person::First), "voy");
    }

    #[test]
    fn irregular_poder_present_third() {
        assert_eq!(conjugate_es("poder", Tense::Present, Person::Third), "puede");
    }

    // ── Past participle ──────────────────────────────────────────────────────

    #[test]
    fn past_participle_regular_ar() {
        assert_eq!(past_participle_es("hablar"), "hablado");
        assert_eq!(past_participle_es("abrir"), "abierto"); // irregular
    }

    #[test]
    fn past_participle_regular_er() {
        assert_eq!(past_participle_es("comer"), "comido");
    }

    #[test]
    fn past_participle_regular_ir() {
        assert_eq!(past_participle_es("vivir"), "vivido");
    }

    #[test]
    fn past_participle_irregulars() {
        assert_eq!(past_participle_es("hacer"), "hecho");
        assert_eq!(past_participle_es("decir"), "dicho");
        assert_eq!(past_participle_es("ver"),   "visto");
    }

    // ── Present participle ───────────────────────────────────────────────────

    #[test]
    fn present_participle_ar() {
        assert_eq!(present_participle_es("hablar"), "hablando");
    }

    #[test]
    fn present_participle_er() {
        assert_eq!(present_participle_es("comer"), "comiendo");
    }

    #[test]
    fn present_participle_ir() {
        assert_eq!(present_participle_es("vivir"), "viviendo");
    }
}
