use std::collections::{HashMap, HashSet, VecDeque};

/// Tracks discourse state across multiple render calls for natural output.
///
/// This is the engine's internal memory — it knows what entities were recently
/// mentioned, what templates were recently used, what connectives were recently
/// inserted, and what words appeared in recent output.
#[derive(Debug)]
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
    /// Kept for a window of the last 5 renders.
    word_history: VecDeque<(usize, HashSet<String>)>,

    /// Last list style index used (for cycling).
    last_list_style: usize,
}

#[derive(Debug, Clone)]
struct EntityMention {
    entity_type: String,
    last_mentioned: usize,
    mention_count: usize,
}

/// How an entity should be referred to based on discourse context.
#[derive(Debug, Clone, PartialEq, Eq)]
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
        Self {
            entities: HashMap::new(),
            render_index: 0,
            focus_entity: None,
            template_history: HashMap::new(),
            connective_history: VecDeque::new(),
            last_template_key: None,
            last_entity_name: None,
            word_history: VecDeque::new(),
            last_list_style: 0,
        }
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
        let same_entity = current_entity.is_some() && current_entity == last_entity;
        let same_action = keys_share_action(current_key, last_key);
        let contrasting = keys_contrast(current_key, last_key);

        if same_entity && !same_action {
            DiscourseRelation::SameEntityDifferentAction
        } else if !same_entity && same_action {
            DiscourseRelation::DifferentEntitySameAction
        } else if contrasting {
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
        let words: HashSet<String> = output
            .split_whitespace()
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase())
            .filter(|w| w.len() > 2 && !STOPWORDS.contains(&w.as_str()))
            .collect();

        self.word_history
            .push_back((self.render_index, words));

        // Trim to window
        while self.word_history.len() > WORD_HISTORY_WINDOW {
            self.word_history.pop_front();
        }
    }

    /// Score a candidate output for repetition against recent history.
    /// Lower score = less repetition = better.
    pub fn repetition_score(&self, candidate: &str) -> f64 {
        let candidate_words: HashSet<String> = candidate
            .split_whitespace()
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase())
            .filter(|w| w.len() > 2 && !STOPWORDS.contains(&w.as_str()))
            .collect();

        let mut score = 0.0;
        for (idx, words) in &self.word_history {
            let distance = self.render_index.saturating_sub(*idx);
            let overlap = candidate_words.intersection(words).count();
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
