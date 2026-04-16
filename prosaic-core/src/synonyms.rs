//! Synonym registry for elegant variation.
//!
//! Register groups of equivalent words (e.g. `["class", "type"]`, or
//! `["consumer", "dependent", "caller"]`) and the `{word|syn}` template
//! pipe will pick whichever variant was *least recently used* in the
//! engine's word-frequency history. This cures the "feels robotic
//! because it keeps saying the same word" effect that lingers even when
//! templates themselves already vary.

use ahash::AHashMap;

/// Registry of synonym groups. Each group is an ordered list; ties in
/// recency are broken by registration order (first-registered wins).
///
/// An inverted index (`index`) maps each lowercased word to its group so
/// that [`synonyms_for`](SynonymRegistry::synonyms_for) is O(1) rather
/// than O(groups × group_size).
#[derive(Debug, Clone, Default)]
pub struct SynonymRegistry {
    groups: Vec<Vec<String>>,
    /// Lowercased word → index into `groups`. Populated on every
    /// `register_group` call so lookups never scan linearly.
    index: AHashMap<String, usize>,
}

impl SynonymRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new synonym group. Order of insertion is preserved and
    /// used to break ties when multiple synonyms have equal recency.
    pub fn register_group(&mut self, words: &[&str]) {
        if words.is_empty() {
            return;
        }
        let group_idx = self.groups.len();
        let group: Vec<String> = words.iter().map(|w| w.to_string()).collect();
        // Pre-lowercase into the index so lookups are allocation-free on
        // the index side (one allocation on the input word remains).
        for w in &group {
            self.index.insert(w.to_lowercase(), group_idx);
        }
        self.groups.push(group);
    }

    /// Look up the synonym group a word belongs to.
    ///
    /// Matching is case-insensitive. Returns `None` when the word is not
    /// registered in any group.
    pub fn synonyms_for(&self, word: &str) -> Option<&[String]> {
        let key = word.to_lowercase();
        self.index.get(&key).map(|&i| self.groups[i].as_slice())
    }

    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }

    pub fn len(&self) -> usize {
        self.groups.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_finds_registered_word() {
        let mut r = SynonymRegistry::new();
        r.register_group(&["class", "type"]);
        let group = r.synonyms_for("class").unwrap();
        assert_eq!(group, &["class".to_string(), "type".to_string()]);
    }

    #[test]
    fn lookup_is_case_insensitive() {
        let mut r = SynonymRegistry::new();
        r.register_group(&["class", "type"]);
        assert!(r.synonyms_for("Class").is_some());
        assert!(r.synonyms_for("TYPE").is_some());
    }

    #[test]
    fn unregistered_word_has_no_group() {
        let r = SynonymRegistry::new();
        assert!(r.synonyms_for("class").is_none());
    }

    #[test]
    fn empty_group_is_ignored() {
        let mut r = SynonymRegistry::new();
        r.register_group(&[]);
        assert!(r.is_empty());
    }
}
