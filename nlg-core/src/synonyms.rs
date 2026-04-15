//! Synonym registry for elegant variation.
//!
//! Register groups of equivalent words (e.g. `["class", "type"]`, or
//! `["consumer", "dependent", "caller"]`) and the `{word|syn}` template
//! pipe will pick whichever variant was *least recently used* in the
//! engine's word-frequency history. This cures the "feels robotic
//! because it keeps saying the same word" effect that lingers even when
//! templates themselves already vary.

/// Registry of synonym groups. Each group is an ordered list; ties in
/// recency are broken by registration order (first-registered wins).
#[derive(Debug, Clone, Default)]
pub struct SynonymRegistry {
    groups: Vec<Vec<String>>,
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
        self.groups
            .push(words.iter().map(|w| w.to_string()).collect());
    }

    /// Look up the synonym group a word belongs to.
    ///
    /// Matching is case-insensitive. Returns `None` when the word is not
    /// registered in any group.
    pub fn synonyms_for(&self, word: &str) -> Option<&[String]> {
        let lower = word.to_lowercase();
        self.groups.iter().find_map(|group| {
            if group.iter().any(|w| w.to_lowercase() == lower) {
                Some(group.as_slice())
            } else {
                None
            }
        })
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
