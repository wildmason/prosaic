use nlg_core::{Person, Tense};

/// Common irregular verb forms: (base, past, past_participle).
/// We use the past form for Past tense and base for Present/Future.
const IRREGULAR_VERBS: &[(&str, &str)] = &[
    ("be", "was"),
    ("become", "became"),
    ("begin", "began"),
    ("break", "broke"),
    ("bring", "brought"),
    ("build", "built"),
    ("buy", "bought"),
    ("can", "could"),
    ("catch", "caught"),
    ("choose", "chose"),
    ("come", "came"),
    ("cost", "cost"),
    ("cut", "cut"),
    ("do", "did"),
    ("draw", "drew"),
    ("drive", "drove"),
    ("eat", "ate"),
    ("fall", "fell"),
    ("feel", "felt"),
    ("find", "found"),
    ("fly", "flew"),
    ("forget", "forgot"),
    ("get", "got"),
    ("give", "gave"),
    ("go", "went"),
    ("grow", "grew"),
    ("have", "had"),
    ("hear", "heard"),
    ("hide", "hid"),
    ("hit", "hit"),
    ("hold", "held"),
    ("hurt", "hurt"),
    ("keep", "kept"),
    ("know", "knew"),
    ("lead", "led"),
    ("leave", "left"),
    ("lend", "lent"),
    ("let", "let"),
    ("lose", "lost"),
    ("make", "made"),
    ("mean", "meant"),
    ("meet", "met"),
    ("move", "moved"),
    ("pay", "paid"),
    ("put", "put"),
    ("read", "read"),
    ("remove", "removed"),
    ("rename", "renamed"),
    ("ride", "rode"),
    ("ring", "rang"),
    ("rise", "rose"),
    ("run", "ran"),
    ("say", "said"),
    ("see", "saw"),
    ("sell", "sold"),
    ("send", "sent"),
    ("set", "set"),
    ("show", "showed"),
    ("shut", "shut"),
    ("sing", "sang"),
    ("sit", "sat"),
    ("sleep", "slept"),
    ("speak", "spoke"),
    ("spend", "spent"),
    ("split", "split"),
    ("stand", "stood"),
    ("steal", "stole"),
    ("strike", "struck"),
    ("swim", "swam"),
    ("take", "took"),
    ("teach", "taught"),
    ("tell", "told"),
    ("think", "thought"),
    ("throw", "threw"),
    ("understand", "understood"),
    ("update", "updated"),
    ("wake", "woke"),
    ("wear", "wore"),
    ("win", "won"),
    ("write", "wrote"),
];

/// Conjugate a verb in the given tense and person.
pub fn conjugate(verb: &str, tense: Tense, person: Person) -> String {
    match tense {
        Tense::Past => past_tense(verb),
        Tense::Present => present_tense(verb, person),
        Tense::Future => format!("will {verb}"),
    }
}

fn past_tense(verb: &str) -> String {
    let lower = verb.to_lowercase();

    // Check irregular verbs
    for &(base, past) in IRREGULAR_VERBS {
        if lower == base {
            return past.to_string();
        }
    }

    // Regular past tense rules
    if lower.ends_with('e') {
        return format!("{verb}d");
    }

    // Consonant + y -> ied
    if lower.ends_with('y') {
        let before_y = lower.as_bytes().get(lower.len().wrapping_sub(2)).copied().unwrap_or(0);
        if !is_vowel(before_y) {
            return format!("{}ied", &verb[..verb.len() - 1]);
        }
    }

    // Double final consonant for short words (CVC pattern)
    if should_double_final_consonant(&lower) {
        let last = lower.chars().last().unwrap();
        return format!("{verb}{last}ed");
    }

    format!("{verb}ed")
}

fn present_tense(verb: &str, person: Person) -> String {
    match person {
        Person::Third => third_person_present(verb),
        _ => verb.to_string(),
    }
}

fn third_person_present(verb: &str) -> String {
    let lower = verb.to_lowercase();

    // Special cases
    if lower == "be" {
        return "is".to_string();
    }
    if lower == "have" {
        return "has".to_string();
    }
    if lower == "do" {
        return "does".to_string();
    }
    if lower == "go" {
        return "goes".to_string();
    }

    // Sibilant endings: -s, -x, -z, -ch, -sh -> +es
    if lower.ends_with('s')
        || lower.ends_with('x')
        || lower.ends_with('z')
        || lower.ends_with("ch")
        || lower.ends_with("sh")
    {
        return format!("{verb}es");
    }

    // Consonant + y -> ies
    if lower.ends_with('y') {
        let before_y = lower.as_bytes().get(lower.len().wrapping_sub(2)).copied().unwrap_or(0);
        if !is_vowel(before_y) {
            return format!("{}ies", &verb[..verb.len() - 1]);
        }
    }

    format!("{verb}s")
}

/// Check if the final consonant should be doubled before adding -ed.
/// Applies to single-syllable CVC words (stop -> stopped, plan -> planned).
fn should_double_final_consonant(word: &str) -> bool {
    let bytes = word.as_bytes();
    if bytes.len() < 3 {
        return false;
    }

    let last = bytes[bytes.len() - 1];
    let second_last = bytes[bytes.len() - 2];
    let third_last = bytes[bytes.len() - 3];

    // Last must be a consonant (not w, x, y)
    if is_vowel(last) || matches!(last, b'w' | b'x' | b'y') {
        return false;
    }
    // Second to last must be a vowel
    if !is_vowel(second_last) {
        return false;
    }
    // Third to last must be a consonant (rough CVC check)
    if is_vowel(third_last) {
        return false;
    }

    // Only for short words (rough single-syllable heuristic)
    bytes.len() <= 4
}

fn is_vowel(byte: u8) -> bool {
    matches!(
        byte,
        b'a' | b'e' | b'i' | b'o' | b'u' | b'A' | b'E' | b'I' | b'O' | b'U'
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use nlg_core::{Person, Tense};

    #[test]
    fn regular_past_tense() {
        assert_eq!(conjugate("walk", Tense::Past, Person::Third), "walked");
        assert_eq!(conjugate("talk", Tense::Past, Person::Third), "talked");
        assert_eq!(conjugate("play", Tense::Past, Person::Third), "played");
    }

    #[test]
    fn past_tense_ending_in_e() {
        assert_eq!(conjugate("create", Tense::Past, Person::Third), "created");
        assert_eq!(conjugate("change", Tense::Past, Person::Third), "changed");
        assert_eq!(conjugate("move", Tense::Past, Person::Third), "moved");
    }

    #[test]
    fn past_tense_consonant_y() {
        assert_eq!(conjugate("carry", Tense::Past, Person::Third), "carried");
        assert_eq!(conjugate("try", Tense::Past, Person::Third), "tried");
    }

    #[test]
    fn past_tense_doubled_consonant() {
        assert_eq!(conjugate("stop", Tense::Past, Person::Third), "stopped");
        assert_eq!(conjugate("plan", Tense::Past, Person::Third), "planned");
        assert_eq!(conjugate("drop", Tense::Past, Person::Third), "dropped");
    }

    #[test]
    fn irregular_past_tense() {
        assert_eq!(conjugate("rename", Tense::Past, Person::Third), "renamed");
        assert_eq!(conjugate("go", Tense::Past, Person::Third), "went");
        assert_eq!(conjugate("make", Tense::Past, Person::Third), "made");
        assert_eq!(conjugate("take", Tense::Past, Person::Third), "took");
        assert_eq!(conjugate("find", Tense::Past, Person::Third), "found");
        assert_eq!(conjugate("build", Tense::Past, Person::Third), "built");
        assert_eq!(conjugate("remove", Tense::Past, Person::Third), "removed");
        assert_eq!(conjugate("update", Tense::Past, Person::Third), "updated");
    }

    #[test]
    fn third_person_present() {
        assert_eq!(conjugate("walk", Tense::Present, Person::Third), "walks");
        assert_eq!(conjugate("run", Tense::Present, Person::Third), "runs");
    }

    #[test]
    fn third_person_present_sibilant() {
        assert_eq!(conjugate("pass", Tense::Present, Person::Third), "passes");
        assert_eq!(conjugate("fix", Tense::Present, Person::Third), "fixes");
        assert_eq!(conjugate("watch", Tense::Present, Person::Third), "watches");
        assert_eq!(conjugate("push", Tense::Present, Person::Third), "pushes");
    }

    #[test]
    fn third_person_present_consonant_y() {
        assert_eq!(conjugate("carry", Tense::Present, Person::Third), "carries");
        assert_eq!(conjugate("fly", Tense::Present, Person::Third), "flies");
    }

    #[test]
    fn third_person_present_vowel_y() {
        assert_eq!(conjugate("play", Tense::Present, Person::Third), "plays");
        assert_eq!(conjugate("stay", Tense::Present, Person::Third), "stays");
    }

    #[test]
    fn third_person_present_irregular() {
        assert_eq!(conjugate("be", Tense::Present, Person::Third), "is");
        assert_eq!(conjugate("have", Tense::Present, Person::Third), "has");
        assert_eq!(conjugate("do", Tense::Present, Person::Third), "does");
        assert_eq!(conjugate("go", Tense::Present, Person::Third), "goes");
    }

    #[test]
    fn future_tense() {
        assert_eq!(conjugate("walk", Tense::Future, Person::Third), "will walk");
        assert_eq!(conjugate("go", Tense::Future, Person::First), "will go");
    }

    #[test]
    fn first_and_second_person_present_unchanged() {
        assert_eq!(conjugate("walk", Tense::Present, Person::First), "walk");
        assert_eq!(conjugate("walk", Tense::Present, Person::Second), "walk");
    }
}
