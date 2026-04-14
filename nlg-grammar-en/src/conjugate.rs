use nlg_core::{Person, Tense};

/// Common irregular verb forms: (base, past, past_participle).
/// When past == past_participle, the third element still must be present.
const IRREGULAR_VERBS: &[(&str, &str, &str)] = &[
    ("be", "was", "been"),
    ("become", "became", "become"),
    ("begin", "began", "begun"),
    ("break", "broke", "broken"),
    ("bring", "brought", "brought"),
    ("build", "built", "built"),
    ("buy", "bought", "bought"),
    ("can", "could", "could"),
    ("catch", "caught", "caught"),
    ("choose", "chose", "chosen"),
    ("come", "came", "come"),
    ("cost", "cost", "cost"),
    ("cut", "cut", "cut"),
    ("do", "did", "done"),
    ("draw", "drew", "drawn"),
    ("drive", "drove", "driven"),
    ("eat", "ate", "eaten"),
    ("fall", "fell", "fallen"),
    ("feel", "felt", "felt"),
    ("find", "found", "found"),
    ("fly", "flew", "flown"),
    ("forget", "forgot", "forgotten"),
    ("get", "got", "gotten"),
    ("give", "gave", "given"),
    ("go", "went", "gone"),
    ("grow", "grew", "grown"),
    ("have", "had", "had"),
    ("hear", "heard", "heard"),
    ("hide", "hid", "hidden"),
    ("hit", "hit", "hit"),
    ("hold", "held", "held"),
    ("hurt", "hurt", "hurt"),
    ("keep", "kept", "kept"),
    ("know", "knew", "known"),
    ("lead", "led", "led"),
    ("leave", "left", "left"),
    ("lend", "lent", "lent"),
    ("let", "let", "let"),
    ("lose", "lost", "lost"),
    ("make", "made", "made"),
    ("mean", "meant", "meant"),
    ("meet", "met", "met"),
    ("move", "moved", "moved"),
    ("pay", "paid", "paid"),
    ("put", "put", "put"),
    ("read", "read", "read"),
    ("remove", "removed", "removed"),
    ("rename", "renamed", "renamed"),
    ("ride", "rode", "ridden"),
    ("ring", "rang", "rung"),
    ("rise", "rose", "risen"),
    ("run", "ran", "run"),
    ("say", "said", "said"),
    ("see", "saw", "seen"),
    ("sell", "sold", "sold"),
    ("send", "sent", "sent"),
    ("set", "set", "set"),
    ("show", "showed", "shown"),
    ("shut", "shut", "shut"),
    ("sing", "sang", "sung"),
    ("sit", "sat", "sat"),
    ("sleep", "slept", "slept"),
    ("speak", "spoke", "spoken"),
    ("spend", "spent", "spent"),
    ("split", "split", "split"),
    ("stand", "stood", "stood"),
    ("steal", "stole", "stolen"),
    ("strike", "struck", "struck"),
    ("swim", "swam", "swum"),
    ("take", "took", "taken"),
    ("teach", "taught", "taught"),
    ("tell", "told", "told"),
    ("think", "thought", "thought"),
    ("throw", "threw", "thrown"),
    ("understand", "understood", "understood"),
    ("update", "updated", "updated"),
    ("wake", "woke", "woken"),
    ("wear", "wore", "worn"),
    ("win", "won", "won"),
    ("write", "wrote", "written"),
];

/// Conjugate a verb in the given tense and person.
pub fn conjugate(verb: &str, tense: Tense, person: Person) -> String {
    match tense {
        Tense::Past => past_tense(verb),
        Tense::Present => present_tense(verb, person),
        Tense::Future => format!("will {verb}"),
    }
}

/// Return the past participle of a verb.
/// For regular verbs this is the same as the past tense.
/// For irregular verbs it may differ (e.g., "broke" vs "broken").
pub fn past_participle(verb: &str) -> String {
    let lower = verb.to_lowercase();

    for &(base, _, participle) in IRREGULAR_VERBS {
        if lower == base {
            return participle.to_string();
        }
    }

    // Regular verbs: past participle == past tense
    regular_past(verb)
}

fn past_tense(verb: &str) -> String {
    let lower = verb.to_lowercase();

    // Check irregular verbs
    for &(base, past, _) in IRREGULAR_VERBS {
        if lower == base {
            return past.to_string();
        }
    }

    regular_past(verb)
}

/// Regular past tense / past participle rules (shared by both).
fn regular_past(verb: &str) -> String {
    let lower = verb.to_lowercase();

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

    // Past participle tests

    #[test]
    fn regular_past_participle() {
        assert_eq!(past_participle("walk"), "walked");
        assert_eq!(past_participle("create"), "created");
        assert_eq!(past_participle("rename"), "renamed");
        assert_eq!(past_participle("stop"), "stopped");
    }

    #[test]
    fn irregular_past_participle_differs_from_past() {
        assert_eq!(past_participle("break"), "broken");
        assert_eq!(past_participle("choose"), "chosen");
        assert_eq!(past_participle("drive"), "driven");
        assert_eq!(past_participle("eat"), "eaten");
        assert_eq!(past_participle("forget"), "forgotten");
        assert_eq!(past_participle("give"), "given");
        assert_eq!(past_participle("go"), "gone");
        assert_eq!(past_participle("hide"), "hidden");
        assert_eq!(past_participle("know"), "known");
        assert_eq!(past_participle("see"), "seen");
        assert_eq!(past_participle("show"), "shown");
        assert_eq!(past_participle("speak"), "spoken");
        assert_eq!(past_participle("steal"), "stolen");
        assert_eq!(past_participle("take"), "taken");
        assert_eq!(past_participle("throw"), "thrown");
        assert_eq!(past_participle("write"), "written");
    }

    #[test]
    fn irregular_past_participle_same_as_past() {
        assert_eq!(past_participle("build"), "built");
        assert_eq!(past_participle("buy"), "bought");
        assert_eq!(past_participle("find"), "found");
        assert_eq!(past_participle("keep"), "kept");
        assert_eq!(past_participle("make"), "made");
        assert_eq!(past_participle("sell"), "sold");
        assert_eq!(past_participle("send"), "sent");
        assert_eq!(past_participle("think"), "thought");
    }

    #[test]
    fn past_participle_unchanged_verbs() {
        assert_eq!(past_participle("cut"), "cut");
        assert_eq!(past_participle("hit"), "hit");
        assert_eq!(past_participle("put"), "put");
        assert_eq!(past_participle("set"), "set");
        assert_eq!(past_participle("split"), "split");
    }
}
