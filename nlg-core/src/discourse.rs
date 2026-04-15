use std::collections::{HashMap, HashSet, VecDeque};

use ahash::AHashMap;

/// Private word interner. Maps lowercased words to stable `u32` ids.
/// Lowercasing happens at intern time; callers must pass already-lowercased
/// input to `intern`/`get`.
#[derive(Debug, Clone, Default)]
struct WordInterner {
    /// Lowercased word → u32 id.
    by_word: AHashMap<String, u32>,
    /// Reverse map for debugging. Indexed by id.
    by_id: Vec<String>,
}

impl WordInterner {
    fn intern(&mut self, word: &str) -> u32 {
        if let Some(&id) = self.by_word.get(word) {
            return id;
        }
        let id = self.by_id.len() as u32;
        let owned = word.to_string();
        self.by_word.insert(owned.clone(), id);
        self.by_id.push(owned);
        id
    }

    fn get(&self, word: &str) -> Option<u32> {
        self.by_word.get(word).copied()
    }
}

/// Tracks discourse state across multiple render calls for natural output.
///
/// This is the engine's internal memory — it knows what entities were recently
/// mentioned, what templates were recently used, what connectives were recently
/// inserted, and what words appeared in recent output.
#[derive(Debug, Clone)]
pub struct DiscourseState {
    /// Tracks entities by name → (entity_type, render_index_of_last_mention).
    entities: HashMap<String, EntityMention>,

    /// The current render index (incremented each render call).
    render_index: usize,

    /// The name of the most recently mentioned entity (for pronoun resolution).
    focus_entity: Option<String>,

    /// Last template variant index used per template key (for anti-repeat).
    template_history: HashMap<String, usize>,

    /// Recently used discourse connectives (ring buffer, max 6).
    connective_history: VecDeque<String>,

    /// The template key used in the previous render (for relationship detection).
    last_template_key: Option<String>,

    /// The primary entity name from the previous render.
    last_entity_name: Option<String>,

    /// Non-stopword tokens from recent renders, with render_index.
    /// Words are stored as interned `u32` ids — see `interner`.
    /// Kept for a window of the last 5 renders.
    word_history: VecDeque<(usize, HashSet<u32>)>,

    /// Word interner shared across all render history. Lowercasing happens
    /// once at intern time; all subsequent lookups use pre-lowercased ids.
    interner: WordInterner,

    /// Pre-interned ids for every stopword in `STOPWORDS`. Populated once
    /// during construction so `record_output_words` never scans strings.
    stopword_ids: HashSet<u32>,

    /// Last list style index used (for cycling).
    last_list_style: usize,

    /// Whether the current focus is a compound/plural subject, so pronoun
    /// continuations should use "they/them" instead of "it".
    focus_is_plural: bool,
}

#[derive(Debug, Clone)]
struct EntityMention {
    entity_type: String,
    last_mentioned: usize,
    mention_count: usize,
}

/// How an entity should be referred to based on discourse context.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ReferenceForm {
    /// Full form: "The class UserService"
    Full,
    /// Name only: "UserService"
    ShortName,
    /// Pronoun: "It" / "it"
    Pronoun,
}

/// The relationship detected between consecutive renders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscourseRelation {
    /// Same entity, different action
    SameEntityDifferentAction,
    /// Different entity, same action type
    DifferentEntitySameAction,
    /// Contrasting actions (e.g., add vs delete)
    Contrast,
    /// No detectable relationship
    None,
}

/// List formatting style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ListStyle {
    /// "including A, B, and C among others"
    Including,
    /// "such as A, B, and C"
    SuchAs,
    /// "— notably A, B, and C, plus N others"
    Dash,
    /// "[A, B, and C, and N more]" (original format)
    Bracketed,
}

const CONNECTIVE_WINDOW: usize = 3;
const WORD_HISTORY_WINDOW: usize = 5;
const ENTITY_REINTRODUCE_DISTANCE: usize = 3;

/// Stopwords excluded from the word frequency map.
const STOPWORDS: &[&str] = &[
    "a", "an", "the", "and", "or", "but", "in", "on", "at", "to", "for",
    "of", "with", "by", "from", "is", "was", "are", "were", "be", "been",
    "being", "have", "has", "had", "do", "does", "did", "will", "would",
    "could", "should", "may", "might", "shall", "can", "not", "no",
    "it", "its", "this", "that", "these", "those", "which", "who",
    "what", "where", "when", "how", "if", "then", "than", "so",
    "as", "up", "out", "into", "also", "just", "more", "most",
];

const LIST_STYLES: &[ListStyle] = &[
    ListStyle::Including,
    ListStyle::SuchAs,
    ListStyle::Dash,
    ListStyle::Bracketed,
];

/// Connective pools by relationship type.
const SAME_ENTITY_CONNECTIVES: &[&str] = &[
    "Additionally,",
    "Furthermore,",
    "It also",
];

const SAME_ACTION_CONNECTIVES: &[&str] = &[
    "Similarly,",
    "Likewise,",
];

const CONTRAST_CONNECTIVES: &[&str] = &[
    "Meanwhile,",
    "However,",
    "On the other hand,",
];

impl DiscourseState {
    pub fn new() -> Self {
        let mut interner = WordInterner::default();
        // Pre-intern all stopwords so membership checks are O(1) u32 lookups.
        let stopword_ids: HashSet<u32> = STOPWORDS
            .iter()
            .map(|&w| interner.intern(w))
            .collect();

        Self {
            entities: HashMap::new(),
            render_index: 0,
            focus_entity: None,
            template_history: HashMap::new(),
            connective_history: VecDeque::new(),
            last_template_key: None,
            last_entity_name: None,
            word_history: VecDeque::new(),
            interner,
            stopword_ids,
            last_list_style: 0,
            focus_is_plural: false,
        }
    }

    /// Mark the current focus as a compound/plural subject so the next
    /// pronoun reference uses "they" rather than "it".
    pub fn set_focus_plural(&mut self, plural: bool) {
        self.focus_is_plural = plural;
    }

    /// Whether the current focus is a plural/compound subject.
    pub fn focus_is_plural(&self) -> bool {
        self.focus_is_plural
    }

    /// Clear all discourse state. Called between unrelated rendering contexts.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Advance to the next render. Must be called at the start of each render.
    pub fn begin_render(&mut self) {
        self.render_index += 1;
    }

    /// Record that an entity was mentioned in the current render.
    /// Resets the focus-plural flag — compound subjects must mark
    /// themselves explicitly via [`Self::set_focus_plural`].
    pub fn mention_entity(&mut self, name: &str, entity_type: &str) {
        let entry = self.entities.entry(name.to_string()).or_insert(EntityMention {
            entity_type: entity_type.to_string(),
            last_mentioned: 0,
            mention_count: 0,
        });
        entry.last_mentioned = self.render_index;
        entry.mention_count += 1;
        entry.entity_type = entity_type.to_string();
        self.focus_entity = Some(name.to_string());
        self.last_entity_name = Some(name.to_string());
        self.focus_is_plural = false;
    }

    /// Determine how to refer to an entity given discourse history.
    pub fn reference_form(&self, name: &str) -> ReferenceForm {
        let mention = match self.entities.get(name) {
            Some(m) => m,
            None => return ReferenceForm::Full,
        };

        let distance = self.render_index.saturating_sub(mention.last_mentioned);

        // If it's been too long, reintroduce with full form
        if distance >= ENTITY_REINTRODUCE_DISTANCE {
            return ReferenceForm::Full;
        }

        // If it's the focus entity and was mentioned in the immediately previous
        // render, and there's no ambiguity (only one recently active entity), use pronoun
        if distance == 1
            && self.focus_entity.as_deref() == Some(name)
            && !self.has_ambiguity(name)
        {
            return ReferenceForm::Pronoun;
        }

        // Otherwise use the short name
        if distance > 0 && distance < ENTITY_REINTRODUCE_DISTANCE {
            return ReferenceForm::ShortName;
        }

        ReferenceForm::Full
    }

    /// Check if there are multiple recently-mentioned entities that could cause
    /// ambiguity when using a pronoun.
    fn has_ambiguity(&self, name: &str) -> bool {
        let recent_count = self
            .entities
            .iter()
            .filter(|(n, m)| {
                n.as_str() != name
                    && self.render_index.saturating_sub(m.last_mentioned) <= 2
            })
            .count();
        recent_count > 0
    }

    /// Record which template variant was selected for anti-repeat.
    pub fn record_template_choice(&mut self, key: &str, variant_index: usize) {
        self.template_history.insert(key.to_string(), variant_index);
        self.last_template_key = Some(key.to_string());
    }

    /// Get the last variant index used for a key (to avoid repeating it).
    pub fn last_template_variant(&self, key: &str) -> Option<usize> {
        self.template_history.get(key).copied()
    }

    /// Detect the relationship between the current render and the previous one.
    ///
    /// Both entities must be present (and comparable) to assert a "same
    /// entity" or "different entity" relationship — otherwise the engine
    /// would incorrectly emit e.g. a *Similarly,* connective for a
    /// repeated entity-less template, where no entity comparison is
    /// actually meaningful.
    pub fn detect_relation(
        &self,
        current_key: &str,
        current_entity: Option<&str>,
    ) -> DiscourseRelation {
        let last_key = match &self.last_template_key {
            Some(k) => k.as_str(),
            None => return DiscourseRelation::None,
        };

        let last_entity = self.last_entity_name.as_deref();
        let both_have_entities = current_entity.is_some() && last_entity.is_some();
        let same_entity = both_have_entities && current_entity == last_entity;
        let different_entity = both_have_entities && current_entity != last_entity;

        let same_action = keys_share_action(current_key, last_key);
        let contrasting = keys_contrast(current_key, last_key);

        if same_entity && !same_action {
            DiscourseRelation::SameEntityDifferentAction
        } else if different_entity && same_action {
            DiscourseRelation::DifferentEntitySameAction
        } else if contrasting && both_have_entities {
            DiscourseRelation::Contrast
        } else {
            DiscourseRelation::None
        }
    }

    /// Select a discourse connective for the given relation, respecting the
    /// non-repetition window. Returns None if no suitable connective is available.
    pub fn select_connective(&mut self, relation: &DiscourseRelation) -> Option<&'static str> {
        let pool = match relation {
            DiscourseRelation::SameEntityDifferentAction => SAME_ENTITY_CONNECTIVES,
            DiscourseRelation::DifferentEntitySameAction => SAME_ACTION_CONNECTIVES,
            DiscourseRelation::Contrast => CONTRAST_CONNECTIVES,
            DiscourseRelation::None => return None,
        };

        // Find a connective not recently used
        let selected = pool.iter().find(|&&c| {
            !self
                .connective_history
                .iter()
                .any(|h| h == c)
        });

        if let Some(&connective) = selected {
            self.connective_history.push_back(connective.to_string());
            if self.connective_history.len() > CONNECTIVE_WINDOW {
                self.connective_history.pop_front();
            }
            Some(connective)
        } else {
            // All connectives recently used — skip rather than repeat
            None
        }
    }

    /// Record the words from a rendered output for repetition scoring.
    pub fn record_output_words(&mut self, output: &str) {
        let mut ids: HashSet<u32> = HashSet::new();
        for raw in output.split_whitespace() {
            let w = raw.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
            if w.len() <= 2 {
                continue;
            }
            let id = self.interner.intern(&w);
            if self.stopword_ids.contains(&id) {
                continue;
            }
            ids.insert(id);
        }

        self.word_history.push_back((self.render_index, ids));

        // Trim to window
        while self.word_history.len() > WORD_HISTORY_WINDOW {
            self.word_history.pop_front();
        }
    }

    /// Score a candidate output for repetition against recent history.
    /// Lower score = less repetition = better.
    pub fn repetition_score(&self, candidate: &str) -> f64 {
        // Collect candidate word ids; new words may not be in the interner
        // yet, so use `get` (read-only) and skip unknowns — they have no
        // history so they contribute zero to the score.
        let candidate_ids: HashSet<u32> = candidate
            .split_whitespace()
            .filter_map(|raw| {
                let w = raw.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
                if w.len() <= 2 {
                    return None;
                }
                let id = self.interner.get(&w)?;
                if self.stopword_ids.contains(&id) {
                    return None;
                }
                Some(id)
            })
            .collect();

        let mut score = 0.0;
        for (idx, ids) in &self.word_history {
            let distance = self.render_index.saturating_sub(*idx);
            let overlap = candidate_ids.intersection(ids).count();
            // Closer renders penalized more heavily
            let weight = match distance {
                0 | 1 => 3.0,
                2 => 2.0,
                3 => 1.0,
                _ => 0.5,
            };
            score += overlap as f64 * weight;
        }
        score
    }

    /// Recency-weighted frequency of a specific word in recent output.
    /// Higher numbers mean the word has appeared recently and/or often.
    /// Used to pick the least-recently-used synonym from a registered
    /// group for elegant variation.
    pub fn word_frequency(&self, word: &str) -> f64 {
        let lower = word.to_lowercase();
        // Word must already be interned; if it has never appeared in history
        // its frequency is zero by definition.
        let id = match self.interner.get(&lower) {
            Some(id) => id,
            None => return 0.0,
        };
        let mut score = 0.0;
        for (idx, ids) in &self.word_history {
            if !ids.contains(&id) {
                continue;
            }
            let distance = self.render_index.saturating_sub(*idx);
            let weight = match distance {
                0 | 1 => 3.0,
                2 => 2.0,
                3 => 1.0,
                _ => 0.5,
            };
            score += weight;
        }
        score
    }

    /// Select the next list style, cycling to avoid repetition.
    pub fn next_list_style(&mut self) -> ListStyle {
        let style = LIST_STYLES[self.last_list_style % LIST_STYLES.len()];
        self.last_list_style += 1;
        style
    }

    /// Whether this is the first render (no prior discourse context).
    pub fn is_first_render(&self) -> bool {
        self.render_index <= 1
    }

    /// Whether a prior render happened in this discourse scope, used by
    /// the `{noun|demonstrative}` pipe to decide between "this X" and
    /// "the X". Cleared by `reset()`.
    pub fn has_prior_render(&self) -> bool {
        // begin_render has already bumped render_index for the current
        // render, so strictly greater than 1 means at least one earlier
        // render contributed to discourse state.
        self.render_index > 1
    }
}

impl Default for DiscourseState {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if two template keys represent the same action type.
/// e.g., "code.renamed" and "code.renamed" → true
/// e.g., "code.renamed" and "code.deleted" → false
fn keys_share_action(a: &str, b: &str) -> bool {
    a == b
}

/// Check if two template keys represent contrasting actions.
fn keys_contrast(a: &str, b: &str) -> bool {
    let contrasts = &[
        ("added", "deleted"),
        ("added", "removed"),
    ];
    let a_action = a.rsplit('.').next().unwrap_or("");
    let b_action = b.rsplit('.').next().unwrap_or("");

    contrasts.iter().any(|&(x, y)| {
        (a_action == x && b_action == y) || (a_action == y && b_action == x)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_mention_is_full() {
        let state = DiscourseState::new();
        assert_eq!(state.reference_form("UserService"), ReferenceForm::Full);
    }

    #[test]
    fn second_mention_is_pronoun_when_focused() {
        let mut state = DiscourseState::new();
        state.begin_render();
        state.mention_entity("UserService", "class");

        state.begin_render();
        assert_eq!(
            state.reference_form("UserService"),
            ReferenceForm::Pronoun
        );
    }

    #[test]
    fn ambiguity_prevents_pronoun() {
        let mut state = DiscourseState::new();
        state.begin_render();
        state.mention_entity("UserService", "class");
        state.mention_entity("AuthService", "class");

        state.begin_render();
        // Both were mentioned recently — ambiguous, use short name
        assert_eq!(
            state.reference_form("UserService"),
            ReferenceForm::ShortName
        );
    }

    #[test]
    fn distant_mention_reintroduces_full() {
        let mut state = DiscourseState::new();
        state.begin_render();
        state.mention_entity("UserService", "class");

        // Advance several renders without mentioning it
        state.begin_render();
        state.begin_render();
        state.begin_render();

        assert_eq!(
            state.reference_form("UserService"),
            ReferenceForm::Full
        );
    }

    #[test]
    fn reset_clears_all_state() {
        let mut state = DiscourseState::new();
        state.begin_render();
        state.mention_entity("UserService", "class");
        state.record_template_choice("code.renamed", 0);

        state.reset();

        assert_eq!(state.reference_form("UserService"), ReferenceForm::Full);
        assert_eq!(state.last_template_variant("code.renamed"), None);
        assert!(state.is_first_render());
    }

    #[test]
    fn template_history_tracks_last_variant() {
        let mut state = DiscourseState::new();
        state.record_template_choice("code.renamed", 2);
        assert_eq!(state.last_template_variant("code.renamed"), Some(2));
    }

    #[test]
    fn connective_avoids_repetition() {
        let mut state = DiscourseState::new();
        let rel = DiscourseRelation::SameEntityDifferentAction;

        let c1 = state.select_connective(&rel).unwrap();
        let c2 = state.select_connective(&rel).unwrap();
        let c3 = state.select_connective(&rel).unwrap();

        assert_ne!(c1, c2);
        assert_ne!(c2, c3);
        assert_ne!(c1, c3);
    }

    #[test]
    fn connective_returns_none_when_exhausted() {
        let mut state = DiscourseState::new();
        let rel = DiscourseRelation::SameEntityDifferentAction;

        // Exhaust all 3 connectives
        state.select_connective(&rel);
        state.select_connective(&rel);
        state.select_connective(&rel);

        // All 3 are in the window — should return None
        assert!(state.select_connective(&rel).is_none());
    }

    #[test]
    fn no_connective_for_none_relation() {
        let mut state = DiscourseState::new();
        assert!(state.select_connective(&DiscourseRelation::None).is_none());
    }

    /// Regression: repeated entity-less templates must not be classified
    /// as DifferentEntitySameAction — that yields spurious "Similarly,"
    /// connectives where no entity comparison is meaningful.
    #[test]
    fn entity_less_repeated_render_produces_no_relation() {
        let mut state = DiscourseState::new();
        state.begin_render();
        state.last_template_key = Some("code.added".to_string());
        state.last_entity_name = None;

        assert_eq!(
            state.detect_relation("code.added", None),
            DiscourseRelation::None
        );
    }

    /// Regression: only one side having an entity is also insufficient to
    /// infer either same-entity or different-entity relationships.
    #[test]
    fn one_sided_entity_presence_produces_no_relation() {
        let mut state = DiscourseState::new();
        state.begin_render();
        state.last_template_key = Some("t".to_string());
        state.last_entity_name = Some("Foo".to_string());

        assert_eq!(
            state.detect_relation("t", None),
            DiscourseRelation::None
        );
    }

    #[test]
    fn detect_same_entity_different_action() {
        let mut state = DiscourseState::new();
        state.begin_render();
        state.last_template_key = Some("code.renamed".to_string());
        state.last_entity_name = Some("Foo".to_string());

        assert_eq!(
            state.detect_relation("code.deleted", Some("Foo")),
            DiscourseRelation::SameEntityDifferentAction
        );
    }

    #[test]
    fn detect_different_entity_same_action() {
        let mut state = DiscourseState::new();
        state.begin_render();
        state.last_template_key = Some("code.renamed".to_string());
        state.last_entity_name = Some("Foo".to_string());

        assert_eq!(
            state.detect_relation("code.renamed", Some("Bar")),
            DiscourseRelation::DifferentEntitySameAction
        );
    }

    #[test]
    fn detect_contrast() {
        let mut state = DiscourseState::new();
        state.begin_render();
        state.last_template_key = Some("code.added".to_string());
        state.last_entity_name = Some("Foo".to_string());

        assert_eq!(
            state.detect_relation("code.deleted", Some("Bar")),
            DiscourseRelation::Contrast
        );
    }

    #[test]
    fn repetition_score_penalizes_recent_overlap() {
        let mut state = DiscourseState::new();
        state.begin_render();
        state.record_output_words("The class UserService was renamed to AccountService");

        state.begin_render();
        let score_high = state.repetition_score(
            "The class UserService was modified affecting AccountService",
        );
        let score_low = state.repetition_score(
            "AuthGuard removed from the application entirely",
        );

        assert!(score_high > score_low);
    }

    #[test]
    fn list_style_cycles() {
        let mut state = DiscourseState::new();
        let s1 = state.next_list_style();
        let s2 = state.next_list_style();
        let s3 = state.next_list_style();
        let s4 = state.next_list_style();

        assert_eq!(s1, ListStyle::Including);
        assert_eq!(s2, ListStyle::SuchAs);
        assert_eq!(s3, ListStyle::Dash);
        assert_eq!(s4, ListStyle::Bracketed);

        // Wraps around
        let s5 = state.next_list_style();
        assert_eq!(s5, ListStyle::Including);
    }
}
