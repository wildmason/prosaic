/// Uncountable nouns that don't change between singular and plural.
const UNCOUNTABLE: &[&str] = &[
    "access", "adulthood", "advice", "aircraft", "aluminum", "anger",
    "bison", "blood", "bravery", "butter", "cash", "chassis", "chess",
    "clothing", "cod", "commerce", "cooperation", "corn", "countryside",
    "courage", "data", "debris", "deer", "diabetes", "education",
    "electricity", "elk", "emoji", "equipment", "evidence", "evolution",
    "faith", "feedback", "firmware", "fish", "flour", "food", "footwear",
    "furniture", "gold", "golf", "grammar", "gratitude", "grief",
    "grouse", "guilt", "hair", "happiness", "hardware", "headquarters",
    "health", "help", "homework", "honesty", "hope", "hunger",
    "ice", "information", "insurance", "jeans", "jewelry", "journalism",
    "knowledge", "labour", "legislation", "leisure", "lightning",
    "linguistics", "livestock", "love", "luck", "luggage", "machinery",
    "mackerel", "mail", "malware", "manga", "mathematics", "media",
    "metadata", "moose", "mud", "music", "news", "offspring", "oxygen",
    "patience", "physics", "pliers", "police", "pollution", "poverty",
    "premises", "pride", "proceedings", "produce", "progress",
    "rain", "research", "rice", "salmon", "scissors", "series",
    "sewage", "shambles", "sheep", "shrimp", "software", "spam",
    "species", "staff", "swine", "tennis", "thanks", "thunder",
    "timber", "tofu", "traffic", "transportation", "trousers", "trout",
    "tuna", "vinyl", "warfare", "water", "weather", "wheat",
    "whitebait", "wildlife", "wisdom", "you",
];

/// Irregular plural forms: (singular, plural).
///
/// Also includes English words ending in `-ice` whose plurals end in `-ices`
/// (e.g. "service → services"). Without explicit entries these would be
/// mis-singularised by the Latin `ices → ex` rule designed for "index/indices".
const IRREGULARS: &[(&str, &str)] = &[
    ("child", "children"),
    ("criterion", "criteria"),
    ("device", "devices"),
    ("die", "dice"),
    ("foot", "feet"),
    ("goose", "geese"),
    ("invoice", "invoices"),
    ("justice", "justices"),
    ("louse", "lice"),
    ("man", "men"),
    ("mouse", "mice"),
    ("move", "moves"),
    ("notice", "notices"),
    ("office", "offices"),
    ("ox", "oxen"),
    ("person", "people"),
    ("practice", "practices"),
    ("price", "prices"),
    ("service", "services"),
    ("sex", "sexes"),
    ("slice", "slices"),
    ("tooth", "teeth"),
    ("voice", "voices"),
    ("woman", "women"),
];

/// Convert a word to its plural form.
pub fn pluralize(word: &str) -> String {
    let lower = word.to_lowercase();

    // Uncountable nouns don't change
    if UNCOUNTABLE.contains(&lower.as_str()) {
        return word.to_string();
    }

    // Check irregular forms
    for &(singular, plural) in IRREGULARS {
        if lower == singular {
            return match_case(word, plural);
        }
        // Already plural?
        if lower == plural {
            return word.to_string();
        }
    }

    // Rule-based pluralization (ordered by specificity)
    if lower.ends_with("quiz") {
        return format!("{word}zes");
    }
    if lower.ends_with("sis") {
        // analysis -> analyses, basis -> bases
        return format!("{}es", &word[..word.len() - 2]);
    }
    if lower.ends_with("us") && lower.len() > 3 {
        // focus -> foci, stimulus -> stimuli (Latin)
        // But "bus" -> "buses", so check length
        if lower.ends_with("ocus") || lower.ends_with("ulus") || lower.ends_with("ulus") {
            return format!("{}i", &word[..word.len() - 2]);
        }
        return format!("{word}es");
    }
    if lower.ends_with("on") && (lower.ends_with("ion") || lower.ends_with("eon")) {
        // criterion already handled as irregular
        // But phenomenon -> phenomena, automaton -> automata
        if lower.ends_with("enon") || lower.ends_with("aton") {
            return format!("{}a", &word[..word.len() - 2]);
        }
    }
    if lower.ends_with("ix") || lower.ends_with("ex") {
        // index -> indices, matrix -> matrices
        // But "sex" is handled as irregular
        if lower.len() > 3 {
            return format!("{}ices", &word[..word.len() - 2]);
        }
    }
    if lower.ends_with("fe") {
        // knife -> knives, wife -> wives
        return format!("{}ves", &word[..word.len() - 2]);
    }
    if lower.ends_with("lf") || lower.ends_with("rf") || lower.ends_with("af") && lower != "deaf" {
        // half -> halves, scarf -> scarves, leaf -> leaves
        return format!("{}ves", &word[..word.len() - 1]);
    }
    if lower.ends_with("y") {
        let before_y = lower.as_bytes().get(lower.len().wrapping_sub(2)).copied().unwrap_or(0);
        if is_vowel(before_y) {
            // day -> days, key -> keys
            return format!("{word}s");
        }
        // city -> cities, baby -> babies
        return format!("{}ies", &word[..word.len() - 1]);
    }
    if lower.ends_with("o") {
        let before_o = lower.as_bytes().get(lower.len().wrapping_sub(2)).copied().unwrap_or(0);
        if is_vowel(before_o) {
            return format!("{word}s");
        }
        // Special cases: words that take -os
        let os_words = ["photo", "piano", "memo", "solo", "zero", "auto", "euro", "kilo"];
        if os_words.contains(&lower.as_str()) {
            return format!("{word}s");
        }
        // hero -> heroes, potato -> potatoes
        return format!("{word}es");
    }
    if lower.ends_with("s") || lower.ends_with("x") || lower.ends_with("z")
        || lower.ends_with("ch") || lower.ends_with("sh")
    {
        return format!("{word}es");
    }

    // Default: just add -s
    format!("{word}s")
}

/// Convert a word to its singular form.
pub fn singularize(word: &str) -> String {
    let lower = word.to_lowercase();

    // Uncountable nouns don't change
    if UNCOUNTABLE.contains(&lower.as_str()) {
        return word.to_string();
    }

    // Check irregular forms
    for &(singular, plural) in IRREGULARS {
        if lower == plural {
            return match_case(word, singular);
        }
        // Already singular?
        if lower == singular {
            return word.to_string();
        }
    }

    // Rule-based singularization
    if lower.ends_with("ies") && lower.len() > 4 {
        // cities -> city
        return format!("{}y", &word[..word.len() - 3]);
    }
    if lower.ends_with("ves") {
        // knives -> knife
        if lower.ends_with("lves") || lower.ends_with("rves") {
            return format!("{}f", &word[..word.len() - 3]);
        }
        return format!("{}fe", &word[..word.len() - 3]);
    }
    if lower.ends_with("ices") && lower.len() > 5 {
        // indices -> index, vertices -> vertex (Latin ix/ex plurals)
        return format!("{}ex", &word[..word.len() - 4]);
    }
    if lower.ends_with("ses") || lower.ends_with("xes") || lower.ends_with("zes")
        || lower.ends_with("ches") || lower.ends_with("shes")
    {
        return word[..word.len() - 2].to_string();
    }
    if lower.ends_with("s") && !lower.ends_with("ss") {
        return word[..word.len() - 1].to_string();
    }

    word.to_string()
}

fn is_vowel(byte: u8) -> bool {
    matches!(byte, b'a' | b'e' | b'i' | b'o' | b'u' | b'A' | b'E' | b'I' | b'O' | b'U')
}

/// Attempt to preserve the case pattern of the original word when replacing.
fn match_case(original: &str, replacement: &str) -> String {
    if original.chars().all(|c| c.is_uppercase()) {
        replacement.to_uppercase()
    } else if original.starts_with(|c: char| c.is_uppercase()) {
        let mut chars = replacement.chars();
        match chars.next() {
            Some(c) => {
                let mut result = c.to_uppercase().to_string();
                result.extend(chars);
                result
            }
            None => String::new(),
        }
    } else {
        replacement.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regular_plurals() {
        assert_eq!(pluralize("cat"), "cats");
        assert_eq!(pluralize("dog"), "dogs");
        assert_eq!(pluralize("car"), "cars");
    }

    #[test]
    fn sibilant_plurals() {
        assert_eq!(pluralize("bus"), "buses");
        assert_eq!(pluralize("box"), "boxes");
        assert_eq!(pluralize("buzz"), "buzzes");
        assert_eq!(pluralize("church"), "churches");
        assert_eq!(pluralize("dish"), "dishes");
    }

    #[test]
    fn y_to_ies() {
        assert_eq!(pluralize("city"), "cities");
        assert_eq!(pluralize("baby"), "babies");
        assert_eq!(pluralize("library"), "libraries");
    }

    #[test]
    fn vowel_y_stays() {
        assert_eq!(pluralize("day"), "days");
        assert_eq!(pluralize("key"), "keys");
        assert_eq!(pluralize("boy"), "boys");
    }

    #[test]
    fn fe_to_ves() {
        assert_eq!(pluralize("knife"), "knives");
        assert_eq!(pluralize("wife"), "wives");
        assert_eq!(pluralize("life"), "lives");
    }

    #[test]
    fn f_to_ves() {
        assert_eq!(pluralize("half"), "halves");
        assert_eq!(pluralize("wolf"), "wolves");
        assert_eq!(pluralize("scarf"), "scarves");
    }

    #[test]
    fn o_plurals() {
        assert_eq!(pluralize("hero"), "heroes");
        assert_eq!(pluralize("potato"), "potatoes");
        assert_eq!(pluralize("photo"), "photos");
        assert_eq!(pluralize("piano"), "pianos");
    }

    #[test]
    fn irregular_plurals() {
        assert_eq!(pluralize("child"), "children");
        assert_eq!(pluralize("person"), "people");
        assert_eq!(pluralize("man"), "men");
        assert_eq!(pluralize("woman"), "women");
        assert_eq!(pluralize("mouse"), "mice");
        assert_eq!(pluralize("foot"), "feet");
        assert_eq!(pluralize("tooth"), "teeth");
        assert_eq!(pluralize("goose"), "geese");
        assert_eq!(pluralize("ox"), "oxen");
    }

    #[test]
    fn uncountable_nouns() {
        assert_eq!(pluralize("sheep"), "sheep");
        assert_eq!(pluralize("fish"), "fish");
        assert_eq!(pluralize("deer"), "deer");
        assert_eq!(pluralize("species"), "species");
        assert_eq!(pluralize("series"), "series");
        assert_eq!(pluralize("software"), "software");
        assert_eq!(pluralize("information"), "information");
        assert_eq!(pluralize("equipment"), "equipment");
    }

    #[test]
    fn sis_to_ses() {
        assert_eq!(pluralize("analysis"), "analyses");
        assert_eq!(pluralize("basis"), "bases");
    }

    #[test]
    fn case_preservation() {
        assert_eq!(pluralize("Child"), "Children");
        assert_eq!(pluralize("CHILD"), "CHILDREN");
    }

    #[test]
    fn already_plural_irregulars() {
        assert_eq!(pluralize("children"), "children");
        assert_eq!(pluralize("people"), "people");
        assert_eq!(pluralize("men"), "men");
    }

    // Singularize tests

    #[test]
    fn singularize_regular() {
        assert_eq!(singularize("cats"), "cat");
        assert_eq!(singularize("dogs"), "dog");
    }

    #[test]
    fn singularize_sibilant() {
        assert_eq!(singularize("buses"), "bus");
        assert_eq!(singularize("boxes"), "box");
        assert_eq!(singularize("churches"), "church");
        assert_eq!(singularize("dishes"), "dish");
    }

    #[test]
    fn singularize_ies() {
        assert_eq!(singularize("cities"), "city");
        assert_eq!(singularize("babies"), "baby");
    }

    #[test]
    fn singularize_ves() {
        assert_eq!(singularize("knives"), "knife");
        assert_eq!(singularize("wolves"), "wolf");
        assert_eq!(singularize("halves"), "half");
    }

    #[test]
    fn singularize_irregular() {
        assert_eq!(singularize("children"), "child");
        assert_eq!(singularize("people"), "person");
        assert_eq!(singularize("men"), "man");
        assert_eq!(singularize("women"), "woman");
        assert_eq!(singularize("mice"), "mouse");
    }

    #[test]
    fn singularize_uncountable() {
        assert_eq!(singularize("sheep"), "sheep");
        assert_eq!(singularize("software"), "software");
    }

    #[test]
    fn singularize_already_singular() {
        assert_eq!(singularize("cat"), "cat");
        assert_eq!(singularize("child"), "child");
    }
}

