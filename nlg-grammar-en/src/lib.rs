mod pluralize;
mod articles;
mod conjugate;
mod lists;
mod numbers;

use nlg_core::{Conjunction, Language, Person, Tense};

/// English language grammar implementation.
#[derive(Debug, Clone, Default)]
pub struct English;

impl English {
    pub fn new() -> Self {
        Self
    }
}

impl Language for English {
    fn pluralize(&self, word: &str, count: usize) -> String {
        if count == 1 {
            pluralize::singularize(word)
        } else {
            pluralize::pluralize(word)
        }
    }

    fn singularize(&self, word: &str) -> String {
        pluralize::singularize(word)
    }

    fn article(&self, word: &str) -> &str {
        articles::indefinite_article(word)
    }

    fn conjugate(&self, verb: &str, tense: Tense, person: Person) -> String {
        conjugate::conjugate(verb, tense, person)
    }

    fn past_participle(&self, verb: &str) -> String {
        conjugate::past_participle(verb)
    }

    fn join_list(&self, items: &[&str], conjunction: Conjunction) -> String {
        lists::join_list(items, conjunction)
    }

    fn ordinal(&self, n: usize) -> String {
        numbers::ordinal(n)
    }

    fn number_to_words(&self, n: usize) -> String {
        numbers::to_words(n)
    }
}
