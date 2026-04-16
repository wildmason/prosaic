/// Return "a" or "an" for the given word.
///
/// Handles common edge cases:
/// - Words starting with vowel sounds get "an" (apple, hour, honest)
/// - Words starting with consonant sounds get "a" (banana, user, union, one)
/// - Acronyms pronounced letter-by-letter (FBI, HTML, SQL)
pub fn indefinite_article(word: &str) -> &'static str {
    let word = word.trim();
    if word.is_empty() {
        return "a";
    }

    let lower = word.to_lowercase();

    // Special cases: words that start with vowels but sound like consonants
    if starts_with_consonant_sound(&lower) {
        return "a";
    }

    // Special cases: words that start with consonants but sound like vowels
    if starts_with_vowel_sound(&lower) {
        return "an";
    }

    // Acronyms (all uppercase, 2+ chars) — check if first letter sounds like a vowel
    if word.len() >= 2
        && word
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
    {
        return if letter_starts_with_vowel_sound(word.as_bytes()[0]) {
            "an"
        } else {
            "a"
        };
    }

    // Default: check the first character
    if lower.starts_with(|c: char| "aeiou".contains(c)) {
        "an"
    } else {
        "a"
    }
}

/// Words starting with vowel letters that actually have consonant sounds.
fn starts_with_consonant_sound(lower: &str) -> bool {
    // "uni-" prefix pronounced "yoo-" (union, university, uniform, unique, unit, united, universal)
    if lower.starts_with("uni")
        && lower
            .as_bytes()
            .get(3)
            .is_some_and(|&b| matches!(b, b'v' | b'f' | b'q' | b't' | b'o' | b'c' | b'l'))
    {
        return true;
    }

    // "use", "used", "user", "useful" — pronounced "yooz"
    if lower.starts_with("use") || lower == "used" {
        return true;
    }

    // "one" and derivatives — pronounced "wun"
    if lower.starts_with("one") {
        return true;
    }

    // "eu-" prefix pronounced "yoo-" (European, eulogy, euphemism)
    if lower.starts_with("eu") {
        return true;
    }

    // "ew" pronounced "yoo" (ewe, ewer)
    if lower.starts_with("ew") {
        return true;
    }

    false
}

/// Words starting with consonant letters that actually have vowel sounds.
fn starts_with_vowel_sound(lower: &str) -> bool {
    // Silent h: hour, honest, honor, honour, herb, heir
    let silent_h = ["hour", "honest", "honor", "honour", "herb", "heir"];
    for prefix in &silent_h {
        if lower.starts_with(prefix) {
            return true;
        }
    }

    false
}

/// When pronouncing a single letter of the alphabet, does it start with a vowel sound?
/// A="ay", B="bee", C="see", D="dee", E="ee", F="ef", H="aitch", I="eye",
/// L="el", M="em", N="en", O="oh", R="ar", S="es", X="ex"
fn letter_starts_with_vowel_sound(byte: u8) -> bool {
    matches!(
        byte,
        b'A' | b'E' | b'F' | b'H' | b'I' | b'L' | b'M' | b'N' | b'O' | b'R' | b'S' | b'X'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_vowel_words() {
        assert_eq!(indefinite_article("apple"), "an");
        assert_eq!(indefinite_article("elephant"), "an");
        assert_eq!(indefinite_article("igloo"), "an");
        assert_eq!(indefinite_article("orange"), "an");
        assert_eq!(indefinite_article("umbrella"), "an");
    }

    #[test]
    fn basic_consonant_words() {
        assert_eq!(indefinite_article("banana"), "a");
        assert_eq!(indefinite_article("cat"), "a");
        assert_eq!(indefinite_article("dog"), "a");
        assert_eq!(indefinite_article("tree"), "a");
    }

    #[test]
    fn silent_h() {
        assert_eq!(indefinite_article("hour"), "an");
        assert_eq!(indefinite_article("honest"), "an");
        assert_eq!(indefinite_article("honor"), "an");
        assert_eq!(indefinite_article("heir"), "an");
        assert_eq!(indefinite_article("herb"), "an");
    }

    #[test]
    fn aspirated_h() {
        assert_eq!(indefinite_article("house"), "a");
        assert_eq!(indefinite_article("happy"), "a");
        assert_eq!(indefinite_article("horse"), "a");
    }

    #[test]
    fn vowel_letter_consonant_sound() {
        assert_eq!(indefinite_article("user"), "a");
        assert_eq!(indefinite_article("university"), "a");
        assert_eq!(indefinite_article("uniform"), "a");
        assert_eq!(indefinite_article("unique"), "a");
        assert_eq!(indefinite_article("union"), "a");
        assert_eq!(indefinite_article("united"), "a");
        assert_eq!(indefinite_article("one"), "a");
        assert_eq!(indefinite_article("European"), "a");
        assert_eq!(indefinite_article("eulogy"), "a");
    }

    #[test]
    fn acronyms() {
        assert_eq!(indefinite_article("FBI"), "an"); // "ef-bee-eye"
        assert_eq!(indefinite_article("HTML"), "an"); // "aitch-tee-em-el"
        assert_eq!(indefinite_article("SQL"), "an"); // "es-queue-el"
        assert_eq!(indefinite_article("URL"), "a"); // "yoo-ar-el"
        // Note: "NASA" is ambiguous — pronounced as a word it's "a NASA",
        // but as an initialism it's "an N-A-S-A". We default to initialism
        // for all-caps strings, which gives "an NASA". Consumers who know
        // the pronunciation should pass "Nasa" to get word-based article selection.
        assert_eq!(indefinite_article("XML"), "an"); // "ex-em-el"
        assert_eq!(indefinite_article("API"), "an"); // "ay-pee-eye"
    }

    #[test]
    fn empty_string() {
        assert_eq!(indefinite_article(""), "a");
    }

    #[test]
    fn whitespace_handling() {
        assert_eq!(indefinite_article("  apple  "), "an");
    }
}
