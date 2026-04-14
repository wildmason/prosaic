/// Verb tense for conjugation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tense {
    Past,
    Present,
    Future,
}

/// Grammatical person for conjugation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Person {
    First,
    Second,
    Third,
}

/// Conjunction used when joining lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Conjunction {
    And,
    Or,
}

/// Trait abstracting over a natural language's grammar rules.
///
/// Implement this trait for each language you want to support.
/// The crate ships with an English implementation in `nlg-grammar-en`.
pub trait Language: Send + Sync {
    /// Return the plural form of `word` for the given `count`.
    /// When `count` is 1, return the singular form.
    fn pluralize(&self, word: &str, count: usize) -> String;

    /// Return the singular form of a potentially plural `word`.
    fn singularize(&self, word: &str) -> String;

    /// Return the indefinite article ("a" or "an" in English) for `word`.
    fn article(&self, word: &str) -> &str;

    /// Conjugate `verb` in the given `tense` and `person`.
    fn conjugate(&self, verb: &str, tense: Tense, person: Person) -> String;

    /// Join a list of items with the given conjunction.
    /// Should use the language's standard list format (e.g., Oxford comma in English).
    fn join_list(&self, items: &[&str], conjunction: Conjunction) -> String;

    /// Return the ordinal string for `n` (e.g., "1st", "2nd", "3rd").
    fn ordinal(&self, n: usize) -> String;

    /// Spell out `n` as words (e.g., 42 → "forty-two").
    fn number_to_words(&self, n: usize) -> String;
}
