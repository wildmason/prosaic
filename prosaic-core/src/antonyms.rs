//! Negation naturalization via an antonym registry.
//!
//! Register pairs of (action phrase, positive-framing antonym phrase).
//! The `{phrase|negated}` template pipe looks up the input phrase in
//! the registry; if a registered positive form exists it emits that,
//! otherwise it falls back to inserting "not" at the right spot
//! ("was modified" → "was not modified") so the output stays grammatical
//! regardless of whether the caller thought to register an antonym.

#[cfg(not(feature = "std"))]
use alloc::format;
#[cfg(not(feature = "std"))]
use alloc::string::{String, ToString};

use crate::collections::HashMap;

/// Registry of phrase-level antonym substitutions used for positive
/// framings of negative statements.
#[derive(Debug, Clone, Default)]
pub struct AntonymRegistry {
    map: HashMap<String, String>,
}

impl AntonymRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register that the negative phrase `negative` should be rendered
    /// as the positive phrase `positive` when used via `{phrase|negated}`.
    /// Matching is case-insensitive; the registered positive form keeps
    /// its original casing.
    pub fn register(&mut self, negative: &str, positive: &str) {
        self.map
            .insert(negative.to_lowercase(), positive.to_string());
    }

    /// Look up a positive antonym for the given phrase. Returns `None`
    /// when no antonym has been registered.
    pub fn lookup(&self, negative: &str) -> Option<&str> {
        self.map.get(&negative.to_lowercase()).map(|s| s.as_str())
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }
}

/// Simple single-word auxiliaries. English negation inserts "not"
/// after the *first* aux in a verb phrase regardless of what follows,
/// so for "has been renamed" the "not" slots after "has" ("has not
/// been renamed"), not after "has been". A first-word match is all we
/// need.
const SIMPLE_AUX: &[&str] = &[
    "is", "are", "was", "were", "has", "have", "had", "will", "would", "could", "should", "may",
    "might", "must", "can",
];

/// Fallback negation: split the phrase after its first auxiliary and
/// insert "not". If the first word isn't a recognizable aux, prepend
/// "not " — ungrammatical on its own but better than silently losing
/// the negation (callers should register an antonym for this case).
///
/// Examples:
/// - `"was modified"` → `"was not modified"`
/// - `"has been renamed"` → `"has not been renamed"`
/// - `"will break"` → `"will not break"`
/// - `"broke"` → `"not broke"`
pub fn insert_not(phrase: &str) -> String {
    let mut parts = phrase.splitn(2, ' ');
    let first = match parts.next() {
        Some(w) => w,
        None => return phrase.to_string(),
    };
    let rest = parts.next().unwrap_or("");

    if SIMPLE_AUX.contains(&first.to_lowercase().as_str()) {
        if rest.is_empty() {
            return format!("{first} not");
        }
        return format!("{first} not {rest}");
    }

    format!("not {phrase}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_not_after_was() {
        assert_eq!(insert_not("was modified"), "was not modified");
    }

    #[test]
    fn insert_not_after_has_been() {
        assert_eq!(insert_not("has been renamed"), "has not been renamed");
    }

    #[test]
    fn insert_not_after_modal() {
        assert_eq!(insert_not("will break"), "will not break");
        assert_eq!(insert_not("must fail"), "must not fail");
    }

    #[test]
    fn insert_not_without_aux_falls_back_to_prefix() {
        // Callers who hit this fallback should register an antonym.
        assert_eq!(insert_not("broke"), "not broke");
    }

    #[test]
    fn registry_lookup_case_insensitive() {
        let mut r = AntonymRegistry::new();
        r.register("was modified", "remained unchanged");
        assert_eq!(r.lookup("Was Modified"), Some("remained unchanged"));
    }

    #[test]
    fn registry_unknown_lookup_is_none() {
        let r = AntonymRegistry::new();
        assert_eq!(r.lookup("was modified"), None);
    }
}
