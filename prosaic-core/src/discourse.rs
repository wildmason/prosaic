#[cfg(not(feature = "std"))]
use alloc::string::{String, ToString};
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use crate::collections::{HashMap, HashSet, VecDeque, new_map, new_set};

/// A forward-looking center: an entity realized in an utterance with its
/// grammatical-role-based salience rank (lower = more prominent).
///
/// Rank 0 corresponds to the Subject position; higher ranks correspond to
/// Object (1), Indirect Object / Location (2), and Oblique (3+).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Cf {
    /// Entity name as passed to `mention_entity` or `mention_entity_ranked`.
    pub name: String,
    /// Grammatical-role-based rank (lower = more prominent). Rank 0 is Subject.
    pub rank: u8,
}

/// Centering Theory transition class between consecutive utterances.
///
/// Prefer (in order): `Continue` > `Retain` > `SmoothShift` > `RoughShift`.
/// `NoCb` means no coherent transition could be classified (first render,
/// post-reset, or utterance with no entities).
///
/// Based on Grosz, Joshi & Weinstein (1995).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Transition {
    /// Cb(n) == Cb(n−1) and Cb(n) == Cp(n): most coherent, entity in focus stays.
    Continue,
    /// Cb(n) == Cb(n−1) but Cb(n) != Cp(n): coherent but not the most salient entity.
    Retain,
    /// Cb(n) != Cb(n−1) but Cb(n) == Cp(n): focus shifts cleanly to the new center.
    SmoothShift,
    /// Cb(n) != Cb(n−1) and Cb(n) != Cp(n): least coherent shift.
    RoughShift,
    /// No transition could be classified: first render, post-reset, or no entities.
    NoCb,
}

/// Private word interner. Maps lowercased words to stable `u32` ids.
/// Lowercasing happens at intern time; callers must pass already-lowercased
/// input to `intern`/`get`.
#[derive(Debug, Clone, Default)]
struct WordInterner {
    /// Lowercased word → u32 id.
    by_word: HashMap<String, u32>,
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

    /// Per-decision family slot for connective selection: `Some(family)`
    /// when a connective was emitted, `None` when the family budget
    /// suppressed one so the sentence ran plain. Tracked alongside
    /// `connective_history` but including null slots so dense
    /// same-family runs can be detected even when exact strings differ.
    connective_family_history: VecDeque<Option<ConnectorFamily>>,

    /// The template key used in the previous render (for relationship detection).
    last_template_key: Option<String>,

    /// The primary entity name from the previous render.
    last_entity_name: Option<String>,

    /// Non-stopword tokens from recent renders, with render_index.
    /// Words are stored as interned `u32` ids — see `interner`.
    /// Kept for a window of the last 5 renders.
    word_history: VecDeque<(usize, HashSet<u32>)>,

    /// Word counts from recently emitted sentences. Used to avoid a flat
    /// mid-length cadence when multiple template variants are available.
    sentence_length_history: VecDeque<usize>,

    /// Word interner shared across all render history. Lowercasing happens
    /// once at intern time; all subsequent lookups use pre-lowercased ids.
    interner: WordInterner,

    /// Pre-interned ids for every stopword in `STOPWORDS`. Populated once
    /// during construction so `record_output_words` never scans strings.
    stopword_ids: HashSet<u32>,

    /// Monotonic cycle index used by [`Self::next_list_style`]. The selected
    /// style is found by walking `LIST_STYLES` from this index forward,
    /// skipping any style currently in `recent_list_styles`. The index is
    /// advanced past the picked slot.
    ///
    /// Persists across paragraph-boundary resets so consecutive paragraphs
    /// rotate through the list-style pool instead of restarting at the same
    /// phrasing every time. This mirrors the cross-paragraph semantics of
    /// `Session::last_temporal_anchor`. Use [`Self::reset_list_cycle`] (or
    /// the [`DiscourseState::reset`] hard reset) to clear it.
    last_list_style: usize,

    /// Trailing window of recently chosen list styles, capped at
    /// [`LIST_STYLE_RECENT_WINDOW`]. Both auto-picked and explicitly forced
    /// styles are recorded here so the next auto pick deterministically
    /// avoids them. Persists across paragraph resets alongside
    /// `last_list_style`; cleared by [`Self::reset_list_cycle`] and the
    /// full [`Self::reset`].
    recent_list_styles: VecDeque<ListStyle>,

    /// Whether the current focus is a compound/plural subject, so pronoun
    /// continuations should use "they/them" instead of "it".
    focus_is_plural: bool,

    /// Backward-looking center for the NEXT render. Updated at the end of each
    /// successful render via `advance_cb`. `None` before the first render, after
    /// a reset, or when no coherent transition is available (Rough Shift).
    cb: Option<String>,

    /// Focus entity of the render immediately before the current one. Used to
    /// compute Cb transitions. Different from `focus_entity`: that tracks the
    /// current render's focus; this tracks what `focus_entity` was at the point
    /// `advance_cb` was last called.
    previous_focus: Option<String>,

    /// Forward-looking centers being built during the CURRENT render.
    /// Populated by `mention_entity_ranked`, cleared by `begin_render`.
    /// Ordered by rank ascending (lowest rank first); ties broken by insertion
    /// order. The first element is the Cp (preferred center).
    current_cf: Vec<Cf>,

    /// Forward-looking centers from the PREVIOUS render. Set by
    /// `compute_cb_transition` as a snapshot of `current_cf`. Used to
    /// identify the Cb as the highest-ranked Cf member shared with the
    /// previous utterance.
    previous_cf: Vec<Cf>,

    /// Transition classification computed by the most recent `advance_cb`
    /// call. `Transition::NoCb` before any render or after a reset.
    last_transition: Transition,

    /// List style chosen by the most recent `|join` pipe during the
    /// current render. `None` when no `|join` fired. Cleared at the
    /// start of every render so [`RenderExplanation`] always reports
    /// the value for *this* render.
    last_list_style_used: Option<ListStyle>,

    /// Whether the most recent render's Silent-mode cleanup stripped
    /// any trailing orphan words. Cleared at the start of every render.
    /// Exposed via [`RenderExplanation::cleanup_stripped_tail`].
    last_cleanup_stripped_tail: bool,
}

#[derive(Debug, Clone)]
struct EntityMention {
    entity_type: String,
    last_mentioned: usize,
    mention_count: usize,
}

/// How an entity should be referred to based on discourse context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ReferenceForm {
    /// Full form: "The class UserService"
    Full,
    /// Name only: "UserService"
    ShortName,
    /// Pronoun: "it" / "they" / (lang-specific)
    Pronoun,
    /// Demonstrative determiner + type: "this class" / (lang-specific).
    /// Reserved slot for future discourse rules; not currently emitted by
    /// `DiscourseState::reference_form`.
    Demonstrative,
    /// Possessive pronoun/determiner: "its" / "their" / (lang-specific).
    /// Used by the `{name|possessive}` pipe after the standard discourse
    /// policy has decided that a pronoun-form reference is appropriate.
    Possessive,
    /// Zero realization: surface is empty. Used by pro-drop languages
    /// (Japanese, colloquial Spanish/Italian) where the pronoun is
    /// recoverable from context and the slot emits nothing.
    /// Not currently emitted by the default `DiscourseState::reference_form`;
    /// language-specific discourse extensions may choose this form.
    Zero,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
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
    /// "A, B, and C, among others" — postfix qualifier, drops remainder count.
    AmongOthers,
    /// "A, B, and C, to name a few" — postfix qualifier, drops remainder count.
    ToNameAFew,
    /// "A, B, and C, plus N more" — postfix qualifier, uses remainder count.
    PlusMore,
}

const CONNECTIVE_WINDOW: usize = 6;

/// Sliding-window length used by the connector-family budget. A family is
/// allowed at most `pool.len()` emissions inside this window before
/// `select_connective` starts returning `None` so the next follow-on
/// sentence renders plain. Sized to give the surface text two or three
/// null slots after a fully saturated pool, which is what dissolves the
/// `Similarly,/Likewise,` style alternation Matt flagged in service-shape
/// prose.
const FAMILY_WINDOW: usize = 5;

/// Score deduction applied when a candidate would form an A/B/A
/// alternation with the immediately preceding two emissions. Distances
/// for unused candidates sit at `CONNECTIVE_WINDOW + 1`, so the penalty
/// is large enough to demote a recently-seen alternation partner below
/// any unused option but small enough to leave the LRU recycle cycle
/// (A,B,C → A,B,C) unchanged when the pool offers a third choice.
const ALTERNATION_PENALTY: i64 = 2;

const WORD_HISTORY_WINDOW: usize = 5;
const SENTENCE_RHYTHM_WINDOW: usize = 6;
const ENTITY_REINTRODUCE_DISTANCE: usize = 3;

/// Per-sentence penalty applied when consecutive sentences land on the same
/// side of the running mean length. Small relative to the existing closeness
/// (max 3.0) and mean-delta (max 1.0) contributions so it acts as a cadence
/// tie-breaker rather than dominating the rhythm score.
const SAME_SIDE_PENALTY: f64 = 0.75;

/// Mean-delta threshold (in words) below which a sentence is treated as
/// "at the mean" and contributes no same-side signal. Avoids spurious
/// pivots when lengths sit exactly on or fractionally beside the mean.
const SIDE_OF_MEAN_NEUTRAL_BAND: f64 = 0.5;

#[derive(Copy, Clone, PartialEq, Eq)]
enum CadenceSide {
    Above,
    Below,
}

fn side_of_mean(len: f64, mean: f64) -> Option<CadenceSide> {
    let delta = len - mean;
    if delta.abs() < SIDE_OF_MEAN_NEUTRAL_BAND {
        None
    } else if delta > 0.0 {
        Some(CadenceSide::Above)
    } else {
        Some(CadenceSide::Below)
    }
}

/// Stopwords excluded from the word frequency map.
const STOPWORDS: &[&str] = &[
    "a", "an", "the", "and", "or", "but", "in", "on", "at", "to", "for", "of", "with", "by",
    "from", "is", "was", "are", "were", "be", "been", "being", "have", "has", "had", "do", "does",
    "did", "will", "would", "could", "should", "may", "might", "shall", "can", "not", "no", "it",
    "its", "this", "that", "these", "those", "which", "who", "what", "where", "when", "how", "if",
    "then", "than", "so", "as", "up", "out", "into", "also", "just", "more", "most",
];

const LIST_STYLES: &[ListStyle] = &[
    ListStyle::Including,
    ListStyle::SuchAs,
    ListStyle::Dash,
    ListStyle::Bracketed,
    ListStyle::AmongOthers,
    ListStyle::ToNameAFew,
    ListStyle::PlusMore,
];

/// Number of recent list-style picks remembered for anti-repeat. Each call to
/// [`DiscourseState::next_list_style`] (and explicit recordings via
/// [`DiscourseState::record_list_style_used`]) skips any style that appears in
/// the trailing window, so consecutive truncated lists never repeat phrasing
/// even when a forced style and the auto-cycle would otherwise collide.
const LIST_STYLE_RECENT_WINDOW: usize = 2;

/// Connective pools by relationship type.
const SAME_ENTITY_CONNECTIVES: &[&str] = &["Additionally,", "Furthermore,", "It also"];

const SAME_ACTION_CONNECTIVES: &[&str] = &["Similarly,", "Likewise,"];

const CONTRAST_CONNECTIVES: &[&str] = &["Meanwhile,", "However,", "On the other hand,"];

/// Number of distinct list styles in the cycle.
pub(crate) fn list_styles_count() -> usize {
    LIST_STYLES.len()
}


/// Lexical family a connector belongs to. The exact-string anti-repeat
/// only sees individual connectors; the family lets the budget reason
/// about whole categories ("similarity/continuation/contrast") so a
/// two-element pool cannot lock the prose into an A/B/A/B alternation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConnectorFamily {
    /// Continuation/expansion: "Additionally,", "Furthermore,", "It also".
    Continuation,
    /// Similarity: "Similarly,", "Likewise,".
    Similarity,
    /// Contrast: "Meanwhile,", "However,", "On the other hand,".
    Contrast,
}

fn family_for_relation(relation: &DiscourseRelation) -> Option<ConnectorFamily> {
    match relation {
        DiscourseRelation::SameEntityDifferentAction => Some(ConnectorFamily::Continuation),
        DiscourseRelation::DifferentEntitySameAction => Some(ConnectorFamily::Similarity),
        DiscourseRelation::Contrast => Some(ConnectorFamily::Contrast),
        DiscourseRelation::None => None,
    }
}

impl DiscourseState {
    pub fn new() -> Self {
        let mut interner = WordInterner::default();
        // Pre-intern all stopwords so membership checks are O(1) u32 lookups.
        let stopword_ids: HashSet<u32> = STOPWORDS.iter().map(|&w| interner.intern(w)).collect();

        Self {
            entities: new_map(),
            render_index: 0,
            focus_entity: None,
            template_history: new_map(),
            connective_history: VecDeque::new(),
            connective_family_history: VecDeque::new(),
            last_template_key: None,
            last_entity_name: None,
            word_history: VecDeque::new(),
            sentence_length_history: VecDeque::new(),
            interner,
            stopword_ids,
            last_list_style: 0,
            recent_list_styles: VecDeque::with_capacity(LIST_STYLE_RECENT_WINDOW),
            last_list_style_used: None,
            last_cleanup_stripped_tail: false,
            focus_is_plural: false,
            cb: None,
            previous_focus: None,
            current_cf: Vec::new(),
            previous_cf: Vec::new(),
            last_transition: Transition::NoCb,
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

    /// Clear ALL discourse state, including the cross-paragraph list-style
    /// cycle counter. Use when starting a fully unrelated narrative — most
    /// callers want [`Self::reset_for_paragraph`] instead so consecutive
    /// paragraphs continue to rotate list-style phrasings.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Clear discourse state at a paragraph boundary while preserving the
    /// narrative-level stylistic anti-repeat machinery. This is the reset
    /// used by [`Session::reset_for_paragraph`] so multi-paragraph narratives
    /// don't restart variant cycles, list-style rotation, word-repetition
    /// penalties, or sentence-rhythm memory on every paragraph break.
    ///
    /// **Preserved (narrative-level):** `last_list_style` and
    /// `recent_list_styles` (list-style cycle plus anti-repeat window),
    /// `template_history` (variant anti-repeat), `connective_history`
    /// (connective anti-repeat), `word_history` plus `interner` (repetition
    /// scoring), `sentence_length_history` (cadence/rhythm scoring),
    /// `render_index` (so word-history distances stay correct and
    /// `has_prior_render` keeps reporting earlier discourse exists).
    ///
    /// **Cleared (paragraph-local):** the entity table, focus entity and its
    /// plurality, `last_template_key`/`last_entity_name` (so cross-paragraph
    /// relation/connective inference is suppressed), the Centering Theory
    /// `Cb`/`Cf` machinery (`cb`, `previous_focus`, `current_cf`,
    /// `previous_cf`, `last_transition`), and per-render diagnostic signals.
    ///
    /// The clearance set is the load-bearing invariant: anaphora must not
    /// resolve to entities introduced in an earlier paragraph, and rhetorical
    /// connectives ("Furthermore,", "However,") must not jump paragraph
    /// boundaries.
    pub fn reset_for_paragraph(&mut self) {
        // Pronoun/anaphora sources.
        self.entities.clear();
        self.focus_entity = None;
        self.focus_is_plural = false;
        // Relation-detection inputs (drive cross-render connective insertion).
        self.last_template_key = None;
        self.last_entity_name = None;
        // Centering Theory state.
        self.cb = None;
        self.previous_focus = None;
        self.current_cf.clear();
        self.previous_cf.clear();
        self.last_transition = Transition::NoCb;
        // Per-render diagnostics.
        self.last_list_style_used = None;
        self.last_cleanup_stripped_tail = false;
        // Intentionally retained: last_list_style, recent_list_styles,
        // template_history, connective_history,
        // connective_family_history, word_history,
        // sentence_length_history, interner, stopword_ids, render_index.
    }

    /// Clear only the list-style cycle counter and its anti-repeat window.
    /// Mirrors [`Session::reset_temporal`] for callers that want to start a
    /// fresh list-style rotation without otherwise resetting discourse state.
    pub fn reset_list_cycle(&mut self) {
        self.last_list_style = 0;
        self.recent_list_styles.clear();
    }

    /// Advance to the next render. Must be called at the start of each render.
    pub fn begin_render(&mut self) {
        self.render_index += 1;
        self.current_cf.clear();
        // Reset per-render diagnostic signals so `RenderExplanation`
        // always reports the value for THIS render rather than inheriting
        // state from a previous one.
        self.last_list_style_used = None;
        self.last_cleanup_stripped_tail = false;
    }

    /// Record that an entity was mentioned in the current render at rank 0
    /// (Subject position). Delegates to [`Self::mention_entity_ranked`].
    ///
    /// Resets the focus-plural flag — compound subjects must mark
    /// themselves explicitly via [`Self::set_focus_plural`].
    pub fn mention_entity(&mut self, name: &str, entity_type: &str) {
        self.mention_entity_ranked(name, entity_type, 0);
    }

    /// Record that an entity was mentioned in the current render with an
    /// explicit grammatical-role rank. Lower rank = more prominent.
    ///
    /// Rank convention:
    /// - 0: Subject (most prominent — the Cp candidate)
    /// - 1: Direct Object
    /// - 2: Indirect Object / Location
    /// - 3+: Oblique / other
    ///
    /// The entity is inserted into `current_cf` in rank-ascending order.
    /// If the entity is already in the Cf list, the lower of the two ranks
    /// is kept (a subject mention always beats an object mention).
    ///
    /// `focus_entity` is updated when rank == 0 or when no focus has been
    /// set yet for this render; this keeps the Cp semantics: the Subject is
    /// the preferred center.
    pub fn mention_entity_ranked(&mut self, name: &str, entity_type: &str, rank: u8) {
        let entry = self
            .entities
            .entry(name.to_string())
            .or_insert(EntityMention {
                entity_type: entity_type.to_string(),
                last_mentioned: 0,
                mention_count: 0,
            });
        entry.last_mentioned = self.render_index;
        entry.mention_count += 1;
        entry.entity_type = entity_type.to_string();

        // Update focus_entity (= Cp) when this is the most prominent slot
        // (rank 0) or when no focus has been established yet this render.
        if rank == 0 || self.focus_entity.is_none() {
            self.focus_entity = Some(name.to_string());
            self.last_entity_name = Some(name.to_string());
            self.focus_is_plural = false;
        }

        // Insert into current_cf, deduplicating by name (keep lower rank).
        if let Some(existing) = self.current_cf.iter_mut().find(|c| c.name == name) {
            if rank < existing.rank {
                existing.rank = rank;
                // Re-sort after rank update.
                self.current_cf.sort_by_key(|c| c.rank);
            }
        } else {
            self.current_cf.push(Cf {
                name: name.to_string(),
                rank,
            });
            // Sort stably so Cp = first element.
            self.current_cf.sort_by_key(|c| c.rank);
        }
    }

    /// Profile-aware variant of [`Self::reference_form`].
    ///
    /// `PronounDensity::Default` is identical to `reference_form`. `Low`
    /// demotes any computed `Pronoun` to `ShortName`, biasing toward
    /// formal register that keeps full names visible longer. `High`
    /// promotes a `ShortName` to `Pronoun` when the entity is recent
    /// enough (distance ≤ 2) and not in an ambiguity context — biasing
    /// toward conversational register.
    pub fn reference_form_with_density(
        &self,
        name: &str,
        density_low: bool,
        density_high: bool,
    ) -> ReferenceForm {
        let raw = self.reference_form(name);
        if density_low {
            return match raw {
                ReferenceForm::Pronoun => ReferenceForm::ShortName,
                other => other,
            };
        }
        if density_high && raw == ReferenceForm::ShortName && self.is_pronoun_eligible_relaxed(name) {
            return ReferenceForm::Pronoun;
        }
        raw
    }

    fn is_pronoun_eligible_relaxed(&self, name: &str) -> bool {
        let Some(mention) = self.entities.get(name) else {
            return false;
        };
        let distance = self.render_index.saturating_sub(mention.last_mentioned);
        if distance == 0 || distance > 2 {
            return false;
        }
        if self.has_ambiguity(name) {
            return false;
        }
        true
    }

    /// Determine how to refer to an entity given discourse history.
    pub fn reference_form(&self, name: &str) -> ReferenceForm {
        let mention = match self.entities.get(name) {
            Some(m) => m,
            None => return ReferenceForm::Full,
        };

        let distance = self.render_index.saturating_sub(mention.last_mentioned);

        // If it's been too long, reintroduce with full form.
        if distance >= ENTITY_REINTRODUCE_DISTANCE {
            return ReferenceForm::Full;
        }

        // Candidate for pronoun under existing distance/focus/ambiguity rules.
        let pronoun_candidate = distance == 1
            && self.focus_entity.as_deref() == Some(name)
            && !self.has_ambiguity(name);

        if pronoun_candidate {
            // Centering Theory Rule 1 gate:
            //   If any element of Cf(Ui) is realized as a pronoun in Ui+1,
            //   then the Cb(Ui+1) must also be realized as a pronoun.
            //
            // Practically: only pronominalize when the referent IS the Cb, or
            // when there is no Cb yet (fresh discourse / post-reset / first
            // named entity). If the Cb is a *different* entity, demoting to
            // ShortName avoids an ambiguous pronoun resolution.
            match self.cb.as_deref() {
                // No Cb yet (first render or post-reset) — fall through to pronoun.
                None => return ReferenceForm::Pronoun,
                // Referent IS the Cb — Rule 1 permits pronominalization.
                Some(cb_name) if cb_name == name => return ReferenceForm::Pronoun,
                // Referent is NOT the Cb — Rule 1 demotes to ShortName to
                // prevent an ambiguous pronoun whose referent is the Cb entity.
                Some(_) => return ReferenceForm::ShortName,
            }
        }

        // Short name for entities mentioned recently but not pronoun-eligible.
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
                n.as_str() != name && self.render_index.saturating_sub(m.last_mentioned) <= 2
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

    /// Select a discourse connective for the given relation, preferring
    /// candidates absent from recent history. Three deterministic
    /// guardrails layer on top of the LRU pick:
    ///
    /// 1. **Connector-family budget.** Each pool maps to a lexical family
    ///    (continuation, similarity, contrast). When the family already
    ///    contributes `pool.len()` emissions inside the trailing
    ///    `FAMILY_WINDOW`, return `None` so the next sentence renders
    ///    plain. This is the lever that breaks the
    ///    `Similarly,/Likewise,/Similarly,/Likewise,` pattern Matt flagged
    ///    in service-shape prose: the two-element similarity pool is
    ///    forced to alternate after two emissions, so the third call
    ///    drops the connective entirely.
    /// 2. **Exact-connector cooldown.** The immediately preceding
    ///    connective is excluded from candidacy when the pool offers an
    ///    alternative — preserves the existing back-to-back anti-repeat.
    /// 3. **A/B alternation penalty.** Candidates equal to
    ///    `connective_history[len-2]` take a score deduction so the LRU
    ///    pick will not extend an A/B pattern into A/B/A when a fresh
    ///    option exists. For three-element pools this preserves the
    ///    A,B,C cycle; for two-element pools the family budget kicks in
    ///    first and the penalty is moot.
    pub fn select_connective(&mut self, relation: &DiscourseRelation) -> Option<&'static str> {
        self.select_connective_filtered(relation, None, None, None)
    }

    /// Profile-aware variant of [`Self::select_connective`].
    ///
    /// `allowed` (when `Some`) restricts the candidate pool to connectives
    /// also present in the slice. If the resulting pool is empty (every
    /// allowed entry was filtered by the existing anti-repeat or family
    /// budget logic, OR no allowed entries match the base pool at all),
    /// the engine falls back to the unfiltered base pool — profile
    /// preferences are biases, never hard constraints.
    ///
    /// `preferred` (when `Some`) adds a per-connective tie-breaker bonus
    /// to the existing distance/alternation score. Weights are interpreted
    /// in `0.0..=1.0` and scaled by 10 to land in the same rough magnitude
    /// as the existing scoring terms.
    ///
    /// `forbidden` (when `Some`) is a strict subtractive filter applied
    /// *after* the allowed/fallback computation — used by the
    /// retrospective refine pass for `BlacklistConnective` constraints.
    /// Unlike `allowed`, an empty post-`forbidden` pool emits `None`
    /// rather than falling back: that's the whole point of a blacklist.
    pub fn select_connective_filtered(
        &mut self,
        relation: &DiscourseRelation,
        allowed: Option<&[&str]>,
        preferred: Option<&[(&str, f32)]>,
        forbidden: Option<&[&str]>,
    ) -> Option<&'static str> {
        let base_pool: &[&'static str] = match relation {
            DiscourseRelation::SameEntityDifferentAction => SAME_ENTITY_CONNECTIVES,
            DiscourseRelation::DifferentEntitySameAction => SAME_ACTION_CONNECTIVES,
            DiscourseRelation::Contrast => CONTRAST_CONNECTIVES,
            DiscourseRelation::None => return None,
        };
        let family = family_for_relation(relation)
            .expect("non-None relation always maps to a connector family");

        // Apply the profile-allowed filter when one is supplied. An empty
        // post-filter pool falls through to the base pool — profile
        // preferences are biases, not hard constraints.
        let filtered: Option<Vec<&'static str>> = allowed.map(|allow| {
            base_pool
                .iter()
                .copied()
                .filter(|c| allow.iter().any(|s| *s == *c))
                .collect()
        });
        let after_allowed: &[&'static str] = match &filtered {
            Some(v) if !v.is_empty() => v.as_slice(),
            _ => base_pool,
        };

        // Apply the strict-forbidden filter (refine-pass blacklist) on
        // top of `after_allowed`. Empty post-forbidden pool → no
        // connective emitted (None). This is the intentional asymmetry
        // with `allowed`: blacklist is a hard constraint.
        let strictly_filtered: Option<Vec<&'static str>> = forbidden.map(|forbid| {
            after_allowed
                .iter()
                .copied()
                .filter(|c| !forbid.iter().any(|f| *f == *c))
                .collect()
        });
        let pool_owned: Vec<&'static str>;
        let pool: &[&'static str] = match &strictly_filtered {
            Some(v) => {
                if v.is_empty() {
                    self.record_family_slot(None);
                    return None;
                }
                pool_owned = v.clone();
                pool_owned.as_slice()
            }
            None => after_allowed,
        };

        // Family-budget gate: count this family's emissions inside the
        // trailing window. Once they saturate the (effective) pool,
        // suppress the connective so the prose continues without a
        // transition cue.
        let family_count = self
            .connective_family_history
            .iter()
            .rev()
            .take(FAMILY_WINDOW)
            .filter(|slot| **slot == Some(family))
            .count();
        if family_count >= pool.len() {
            self.record_family_slot(None);
            return None;
        }

        let immediate = self.connective_history.back().map(String::as_str);
        let two_back = self
            .connective_history
            .iter()
            .rev()
            .nth(1)
            .map(String::as_str);

        let prefer_bonus = |connective: &str| -> i64 {
            let Some(prefs) = preferred else {
                return 0;
            };
            prefs
                .iter()
                .find_map(|(s, w)| if *s == connective { Some(*w) } else { None })
                .map(|w| (w * 10.0) as i64)
                .unwrap_or(0)
        };

        let mut selected: Option<&'static str> = None;
        let mut selected_score: i64 = i64::MIN;

        for &connective in pool {
            if pool.len() > 1 && immediate == Some(connective) {
                continue;
            }

            let distance = self
                .connective_history
                .iter()
                .rev()
                .position(|history| history == connective)
                .unwrap_or(CONNECTIVE_WINDOW + 1) as i64;

            let alternation_penalty = if pool.len() > 1 && two_back == Some(connective) {
                ALTERNATION_PENALTY
            } else {
                0
            };
            let score = distance - alternation_penalty + prefer_bonus(connective);

            if selected.is_none() || score > selected_score {
                selected = Some(connective);
                selected_score = score;
            }
        }

        let connective = selected?;
        self.connective_history.push_back(connective.to_string());
        if self.connective_history.len() > CONNECTIVE_WINDOW {
            self.connective_history.pop_front();
        }
        self.record_family_slot(Some(family));

        Some(connective)
    }

    /// Push a per-decision family slot, capping the ring buffer at
    /// `FAMILY_WINDOW + 2` so the budget check has the full window plus
    /// a small lookahead margin without growing without bound.
    fn record_family_slot(&mut self, slot: Option<ConnectorFamily>) {
        self.connective_family_history.push_back(slot);
        if self.connective_family_history.len() > FAMILY_WINDOW + 2 {
            self.connective_family_history.pop_front();
        }
    }

    /// Record the words from a rendered output for repetition scoring.
    pub fn record_output_words(&mut self, output: &str) {
        let mut ids: HashSet<u32> = new_set();
        for raw in output.split_whitespace() {
            let w = raw
                .trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase();
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

    /// Iterate over the recent sentence-length history (newest last).
    /// Each value is the word count of one emitted sentence inside the
    /// rhythm-tracking window. Exposed for profile-aware scorers that
    /// need to read the cadence buffer without snapshotting the whole
    /// session — the buffer is short and read-only from outside.
    pub fn sentence_length_iter(&self) -> impl Iterator<Item = usize> + '_ {
        self.sentence_length_history.iter().copied()
    }

    /// Record word counts for the sentences emitted by the committed render.
    pub fn record_sentence_rhythm(&mut self, output: &str) {
        for len in sentence_word_counts(output) {
            self.sentence_length_history.push_back(len);
            while self.sentence_length_history.len() > SENTENCE_RHYTHM_WINDOW {
                self.sentence_length_history.pop_front();
            }
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
                let w = raw
                    .trim_matches(|c: char| !c.is_alphanumeric())
                    .to_lowercase();
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

    /// Score a candidate output against recent sentence-length cadence.
    /// Lower is better: candidates with sentence lengths that were just
    /// emitted receive a penalty, while noticeably shorter or longer variants
    /// are preferred when repetition scores are otherwise close.
    ///
    /// In addition to the per-sentence closeness/mean components, a bounded
    /// same-side penalty fires for each consecutive sentence pair (history
    /// → candidate, then candidate → candidate) that lands on the same side
    /// of the running mean. This nudges the selector toward burst-pivot
    /// cadence — alternating short/long around the mean — which is a hallmark
    /// of natural prose. The penalty is purely additive and capped per
    /// sentence so it cannot zero out repetition penalties or push the score
    /// negative.
    pub fn sentence_rhythm_score(&self, candidate: &str) -> f64 {
        let candidate_lengths = sentence_word_counts(candidate);
        if candidate_lengths.is_empty() || self.sentence_length_history.is_empty() {
            return 0.0;
        }

        let recent_mean = self.sentence_length_history.iter().sum::<usize>() as f64
            / self.sentence_length_history.len() as f64;

        // Side of mean for the most recent emitted sentence, if any. Sentences
        // exactly at the mean are treated as neutral (None) and never trigger
        // a same-side penalty in either direction.
        let mut prev_side = self
            .sentence_length_history
            .back()
            .and_then(|len| side_of_mean(*len as f64, recent_mean));

        let mut score = 0.0;
        for len in &candidate_lengths {
            let closest = self
                .sentence_length_history
                .iter()
                .map(|recent| recent.abs_diff(*len))
                .min()
                .unwrap_or(usize::MAX);

            score += match closest {
                0 => 3.0,
                1 => 2.0,
                2 => 1.0,
                3 => 0.5,
                _ => 0.0,
            };

            let mean_delta = (*len as f64 - recent_mean).abs();
            if mean_delta < 1.0 {
                score += 1.0;
            } else if mean_delta < 2.0 {
                score += 0.5;
            }

            let cur_side = side_of_mean(*len as f64, recent_mean);
            if let (Some(prev), Some(cur)) = (prev_side, cur_side)
                && prev == cur
            {
                score += SAME_SIDE_PENALTY;
            }
            // Carry candidate side forward so within-candidate runs (e.g.
            // long → long → long) accumulate the penalty across each pair,
            // not just against history.
            if cur_side.is_some() {
                prev_side = cur_side;
            }
        }

        score / candidate_lengths.len() as f64
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

    /// Select the next list style. Walks `LIST_STYLES` deterministically from
    /// `last_list_style` forward and returns the first style that is not in
    /// the recent-window (`recent_list_styles`). The walk advances past the
    /// chosen slot so subsequent calls progress through the palette rather
    /// than locking onto the first non-recent slot.
    ///
    /// Anti-repeat is fully deterministic — no RNG dependency — and ensures
    /// that an explicit forced style (e.g. `{|join:bracketed}` recorded via
    /// [`Self::record_list_style_used`]) does not collide with the very next
    /// auto-cycle pick. Falls back to the modulo slot if every style somehow
    /// sits in the recent window (unreachable while
    /// `LIST_STYLE_RECENT_WINDOW < LIST_STYLES.len()`, but kept defensive).
    pub fn next_list_style(&mut self) -> ListStyle {
        self.next_list_style_with_bias(None)
    }

    /// Profile-aware variant of [`Self::next_list_style`].
    ///
    /// When `bias` is `Some(target)` and `target` is not currently inside
    /// the anti-repeat window, the cycle advances to the slot just past
    /// `target` and emits it. When `bias` is `None` (i.e., the profile's
    /// `ListStyleBias::Auto` default), or when the bias target is in the
    /// recent window, the natural cycle picks as in `next_list_style`.
    /// The bias is a preference, not an override — anti-repeat always wins.
    pub fn next_list_style_with_bias(&mut self, bias: Option<ListStyle>) -> ListStyle {
        if let Some(target) = bias {
            if !self.recent_list_styles.contains(&target)
                && let Some(target_idx) = LIST_STYLES.iter().position(|s| *s == target)
            {
                // Advance the cycle to the slot just past the bias target so
                // the natural rotation continues coherently afterward, then
                // emit the target.
                self.last_list_style = target_idx.wrapping_add(1);
                self.push_recent_list_style(target);
                self.last_list_style_used = Some(target);
                return target;
            }
        }

        let len = LIST_STYLES.len();
        let start = self.last_list_style % len;

        let mut chosen_offset = 0;
        for offset in 0..len {
            let candidate = LIST_STYLES[(start + offset) % len];
            if !self.recent_list_styles.contains(&candidate) {
                chosen_offset = offset;
                break;
            }
        }

        let style = LIST_STYLES[(start + chosen_offset) % len];
        // Advance past the picked slot so the cycle continues to make
        // forward progress rather than re-evaluating from the same start
        // on the next call.
        self.last_list_style = self.last_list_style.wrapping_add(chosen_offset + 1);
        self.push_recent_list_style(style);
        self.last_list_style_used = Some(style);
        style
    }

    /// Record an explicit list style (e.g. `{|join:bracketed}`) for
    /// diagnostics AND anti-repeat. Forced styles count toward the recent
    /// window so a subsequent auto-cycle pick won't immediately repeat the
    /// forced phrasing.
    pub fn record_list_style_used(&mut self, style: ListStyle) {
        self.push_recent_list_style(style);
        self.last_list_style_used = Some(style);
    }

    fn push_recent_list_style(&mut self, style: ListStyle) {
        // Drop duplicates of `style` already in the window before we push,
        // so the trailing slot is always "the most recent N *distinct*
        // styles" rather than the same forced style filling the buffer.
        self.recent_list_styles.retain(|&s| s != style);
        if self.recent_list_styles.len() == LIST_STYLE_RECENT_WINDOW {
            self.recent_list_styles.pop_front();
        }
        self.recent_list_styles.push_back(style);
    }

    /// List style applied by the most recent render's `|join` pipe (if any).
    pub fn last_list_style_used(&self) -> Option<ListStyle> {
        self.last_list_style_used
    }

    /// Record whether Silent-mode cleanup stripped any trailing orphan words
    /// during the most recent render.
    pub fn set_cleanup_stripped_tail(&mut self, stripped: bool) {
        self.last_cleanup_stripped_tail = stripped;
    }

    /// Whether the most recent render's cleanup pass removed trailing
    /// orphan words (Silent strictness only). `false` in other modes.
    pub fn last_cleanup_stripped_tail(&self) -> bool {
        self.last_cleanup_stripped_tail
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

    /// Advance Cb tracking for the next render. Call this after all mutations
    /// from the current render (`mention_entity`, `record_output_words`) have
    /// completed and the render has committed. On render failure the
    /// `Session` snapshot/restore path will roll back `cb` and `previous_focus`
    /// along with all other fields via `Clone`.
    ///
    /// Called by `Engine::render_tx` at the end of each successful render.
    pub fn advance_cb(&mut self) {
        self.compute_cb_transition();
    }

    /// The Centering Theory transition class from the most recent `advance_cb` call.
    /// Returns `Transition::NoCb` before any render or after a reset.
    pub fn last_transition(&self) -> Transition {
        self.last_transition
    }

    /// The current backward-looking center, if any.
    pub fn cb(&self) -> Option<&str> {
        self.cb.as_deref()
    }

    /// The forward-looking centers being built during the current render,
    /// ordered by rank ascending (Cp = first element).
    pub fn cf(&self) -> &[Cf] {
        &self.current_cf
    }

    /// The forward-looking centers from the previous render.
    pub fn previous_cf(&self) -> &[Cf] {
        &self.previous_cf
    }

    /// Compute and store the Cb for the **next** render, using Cf overlap to
    /// identify the backward-looking center as the highest-ranked entity in
    /// Cf(current) that also appeared in Cf(previous).
    ///
    /// When the pure Cf-overlap definition yields no shared entity, the method
    /// falls back to prior-focus logic to preserve backward compatibility with
    /// Rule 1 pronoun tests:
    ///
    /// - **No previous Cf** (first render / post-reset): Cb = Cp of current.
    /// - **No overlap, new entity first time**: prior focus stays as Cb
    ///   (Smooth Shift — introduce gently, keep prior thread alive).
    /// - **No overlap, entity seen before**: Cb = current Cp (Retain-style).
    /// - **No current entity**: Cb carries prior focus forward.
    fn compute_cb_transition(&mut self) {
        // Cp of this render = first element of current_cf (lowest rank).
        let current_cp: Option<String> = self.current_cf.first().map(|c| c.name.clone());
        let prev_cb = self.cb.clone();

        // New Cb: highest-ranked Cf member shared with the previous Cf.
        let new_cb: Option<String> = self
            .current_cf
            .iter()
            .find(|c| self.previous_cf.iter().any(|p| p.name == c.name))
            .map(|c| c.name.clone());

        // Fallback when the Cf-overlap definition yields nothing.
        let new_cb = match (new_cb, current_cp.clone(), self.previous_focus.clone()) {
            // Overlap found: use it.
            (Some(cb), _, _) => Some(cb),

            // First render (no previous focus yet): Cb = Cp.
            (None, Some(cp), None) => Some(cp),

            // No overlap, but there is a previous focus.
            (None, Some(cp), Some(_)) => {
                if self.entities.get(&cp).is_some_and(|m| m.mention_count > 1) {
                    // Entity seen before: Retain — Cb shifts to newly-focused entity.
                    Some(cp)
                } else {
                    // Brand-new entity: Smooth Shift — prior focus stays as Cb.
                    self.previous_focus.clone()
                }
            }

            // No current entity: carry prior focus forward.
            (None, None, Some(p)) => Some(p),
            (None, None, None) => None,
        };

        // Classify the transition.
        let transition =
            classify_transition(new_cb.as_deref(), prev_cb.as_deref(), current_cp.as_deref());

        self.cb = new_cb;
        self.last_transition = transition;

        // Shift state forward for the next call.
        self.previous_focus = current_cp;
        self.previous_cf = core::mem::take(&mut self.current_cf);
    }
}

/// Classify a Centering Theory transition given the new Cb, the previous Cb,
/// and the Cp (preferred center) of the current utterance.
///
/// Returns `NoCb` when:
/// - There is no current Cb (the utterance has no realized entities), or
/// - There is no previous Cb (first render or post-reset — no prior discourse
///   context exists to classify a transition against).
fn classify_transition(cb: Option<&str>, prev_cb: Option<&str>, cp: Option<&str>) -> Transition {
    let cb = match cb {
        Some(c) => c,
        None => return Transition::NoCb,
    };
    // No prior Cb → no meaningful transition (first render or post-reset).
    let prev_cb = match prev_cb {
        Some(p) => p,
        None => return Transition::NoCb,
    };
    let cb_eq_prev = prev_cb == cb;
    let cb_eq_cp = matches!(cp, Some(c) if c == cb);

    match (cb_eq_prev, cb_eq_cp) {
        (true, true) => Transition::Continue,
        (true, false) => Transition::Retain,
        (false, true) => Transition::SmoothShift,
        (false, false) => Transition::RoughShift,
    }
}

impl Default for DiscourseState {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) fn sentence_word_counts(text: &str) -> Vec<usize> {
    let mut counts = Vec::new();
    let mut current = 0usize;

    for raw in text.split_whitespace() {
        if raw.chars().any(|c| c.is_alphanumeric()) {
            current += 1;
        }

        if (raw.ends_with('.') || raw.ends_with('!') || raw.ends_with('?')) && current > 0 {
            counts.push(current);
            current = 0;
        }
    }

    if current > 0 {
        counts.push(current);
    }

    counts
}

/// Check if two template keys represent the same action type.
/// e.g., "code.renamed" and "code.renamed" → true
/// e.g., "code.renamed" and "code.deleted" → false
fn keys_share_action(a: &str, b: &str) -> bool {
    a == b
}

/// Check if two template keys represent contrasting actions.
fn keys_contrast(a: &str, b: &str) -> bool {
    let contrasts = &[("added", "deleted"), ("added", "removed")];
    let a_action = a.rsplit('.').next().unwrap_or("");
    let b_action = b.rsplit('.').next().unwrap_or("");

    contrasts
        .iter()
        .any(|&(x, y)| (a_action == x && b_action == y) || (a_action == y && b_action == x))
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
        assert_eq!(state.reference_form("UserService"), ReferenceForm::Pronoun);
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

        assert_eq!(state.reference_form("UserService"), ReferenceForm::Full);
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
    fn connective_recency_window_spans_mixed_relation_types() {
        let mut state = DiscourseState::new();
        let same_entity = DiscourseRelation::SameEntityDifferentAction;
        let same_action = DiscourseRelation::DifferentEntitySameAction;
        let contrast = DiscourseRelation::Contrast;

        assert_eq!(state.select_connective(&same_entity), Some("Additionally,"));
        assert_eq!(state.select_connective(&contrast), Some("Meanwhile,"));
        assert_eq!(state.select_connective(&same_action), Some("Similarly,"));
        assert_eq!(state.select_connective(&contrast), Some("However,"));

        // "Additionally," is still inside the six-entry recency window, so
        // the selector moves to the next unused same-entity connective.
        assert_eq!(state.select_connective(&same_entity), Some("Furthermore,"));
    }

    #[test]
    fn connective_family_budget_drops_to_null_when_pool_saturates() {
        let mut state = DiscourseState::new();
        let rel = DiscourseRelation::SameEntityDifferentAction;

        // Three-element continuation pool drains uniquely.
        assert_eq!(state.select_connective(&rel), Some("Additionally,"));
        assert_eq!(state.select_connective(&rel), Some("Furthermore,"));
        assert_eq!(state.select_connective(&rel), Some("It also"));

        // Saturation: rather than recycling the LRU choice and producing
        // an Additionally,/Furthermore,/It also,/Additionally,... cycle,
        // the family budget suppresses the next emissions so the prose
        // dissolves into plain follow-on sentences.
        assert_eq!(state.select_connective(&rel), None);
        assert_eq!(state.select_connective(&rel), None);
        assert_eq!(state.select_connective(&rel), None);

        // Once enough null slots accumulate inside the trailing window,
        // the budget reopens and the LRU pick resumes — Additionally
        // is the oldest emitted connector in `connective_history`.
        assert_eq!(state.select_connective(&rel), Some("Additionally,"));
    }

    /// Regression for the service-shape prose Matt flagged: five follow-on
    /// sentences that all trigger DifferentEntitySameAction must NOT
    /// produce a `Similarly,/Likewise,/Similarly,/Likewise,/Similarly,`
    /// alternation. The two-element similarity pool can sustain at most
    /// two emissions inside the family window before the budget forces
    /// nulls so the pattern dissolves.
    #[test]
    fn similarity_family_budget_breaks_service_shape_alternation() {
        let mut state = DiscourseState::new();
        let rel = DiscourseRelation::DifferentEntitySameAction;

        let emissions: Vec<Option<&'static str>> =
            (0..5).map(|_| state.select_connective(&rel)).collect();

        let connectors: Vec<&'static str> = emissions.iter().filter_map(|e| *e).collect();

        assert!(
            connectors.len() <= 2,
            "expected at most two similarity-family connectives across five \
             follow-on sentences, got {emissions:?}"
        );

        // No A/B/A pattern: the third emission (if any) must not match
        // the connective two slots earlier.
        for window in emissions.windows(3) {
            if let (Some(a), Some(_), Some(c)) = (window[0], window[1], window[2]) {
                assert_ne!(
                    a, c,
                    "A/B/A alternation slipped through the budget: {emissions:?}"
                );
            }
        }

        // Both members of the pool should appear at most once in the
        // surfaced connector list — the budget caps usage at pool.len()
        // = 2 distinct connectives, never two of the same.
        let similarly = connectors.iter().filter(|c| **c == "Similarly,").count();
        let likewise = connectors.iter().filter(|c| **c == "Likewise,").count();
        assert!(
            similarly <= 1 && likewise <= 1,
            "no similarity connector should repeat inside the family window: {emissions:?}"
        );
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

        assert_eq!(state.detect_relation("t", None), DiscourseRelation::None);
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
        let score_high =
            state.repetition_score("The class UserService was modified affecting AccountService");
        let score_low = state.repetition_score("AuthGuard removed from the application entirely");

        assert!(score_high > score_low);
    }

    #[test]
    fn sentence_rhythm_score_penalizes_recent_sentence_lengths() {
        let mut state = DiscourseState::new();
        state.record_sentence_rhythm("Alpha changed after validation passed.");

        let repeated_cadence = state.sentence_rhythm_score("Beta changed after review passed");
        let varied_cadence =
            state.sentence_rhythm_score("Beta changed after review passed and deployment resumed");

        assert!(
            repeated_cadence > varied_cadence,
            "same-length candidates should score worse than varied ones"
        );
    }

    #[test]
    fn sentence_rhythm_score_penalizes_same_side_runs() {
        // History: three short sentences (3, 4, 3 words). Mean = 3.33.
        // Last emitted sentence (3 words) is below mean.
        //
        // Pivoting candidate: a single noticeably-long sentence (above mean)
        // — flips side relative to history's last entry, no same-side
        // penalty fires.
        //
        // Same-side candidate: another short sentence (below mean) — same
        // side as history's last entry, so the burst-pivot penalty fires.
        //
        // The same-side candidate's closeness/mean-delta cost is also
        // higher (it sits inside the recent cluster), but the penalty must
        // strictly increase the gap, not flip its sign. Both effects push
        // the score in the same direction; the assertion proves the
        // additive penalty is observable on top of the existing terms.
        let mut state = DiscourseState::new();
        state.record_sentence_rhythm("Alpha shipped today. Beta paused. Gamma shipped.");

        let pivoting = state.sentence_rhythm_score(
            "Delta shipped after the schema migration finished and the staging build went green",
        );
        let same_side = state.sentence_rhythm_score("Delta shipped today");

        assert!(
            same_side > pivoting,
            "same-side candidate ({same_side}) must score worse than pivoting candidate ({pivoting})"
        );
    }

    #[test]
    fn sentence_rhythm_score_pivot_penalty_does_not_dominate_repetition() {
        // Construct two candidates where the same-side candidate is
        // otherwise repetition-clean and the pivoting candidate reuses the
        // entire prior render's vocabulary. The discourse score the engine
        // actually compares is repetition + rhythm; this test pins down
        // that the rhythm penalty cannot flip the verdict on its own — the
        // repetition signal must still dominate.
        let mut state = DiscourseState::new();
        state.begin_render();
        state.record_output_words("AuthService validated tokens against the registry");
        state.record_sentence_rhythm("AuthService validated tokens against the registry.");

        state.begin_render();
        // Pivoting candidate sits on the opposite side of the running mean
        // (much longer) but reuses every distinctive word from the prior
        // render — heavy repetition.
        let pivoting_repeats = "AuthService validated tokens against the registry yet again";
        // Same-side candidate matches the prior cadence (same length) but
        // introduces wholly new vocabulary — minimal repetition.
        let same_side_clean = "PaymentGateway settled invoices nightly";

        let rep_pivot = state.repetition_score(pivoting_repeats);
        let rep_clean = state.repetition_score(same_side_clean);
        let rhy_pivot = state.sentence_rhythm_score(pivoting_repeats);
        let rhy_clean = state.sentence_rhythm_score(same_side_clean);

        assert!(
            (rep_pivot + rhy_pivot) > (rep_clean + rhy_clean),
            "repetition-heavy pivoting candidate ({}) must still score worse \
             than the repetition-clean same-side candidate ({}); the burst-pivot \
             penalty is a tie-breaker, not a faithfulness override",
            rep_pivot + rhy_pivot,
            rep_clean + rhy_clean,
        );
        // And the rhythm-side delta alone must be smaller than the
        // repetition-side delta — proves the penalty is bounded relative
        // to the dominant constraint.
        assert!(
            (rep_pivot - rep_clean).abs() > (rhy_clean - rhy_pivot).abs(),
            "repetition delta ({}) must dominate rhythm delta ({})",
            rep_pivot - rep_clean,
            rhy_clean - rhy_pivot,
        );
    }

    #[test]
    fn sentence_rhythm_score_is_never_negative() {
        // The score is a sum of non-negative components divided by a
        // positive count. Sweep a handful of histories and candidates to
        // pin down the invariant — a future change that introduces a
        // reward (subtraction) must update this test deliberately.
        let mut state = DiscourseState::new();
        for prior in [
            "Alpha shipped.",
            "Beta paused after the long postmortem dragged on.",
            "Gamma. Delta. Epsilon shipped after lunch.",
        ] {
            state.record_sentence_rhythm(prior);
        }

        for candidate in [
            "",
            "Zeta shipped.",
            "Zeta shipped after a careful review and a brief rollout window.",
            "Short. Long sentence with quite a few words inside it. Short again.",
        ] {
            let score = state.sentence_rhythm_score(candidate);
            assert!(
                score >= 0.0,
                "rhythm score must be non-negative (candidate `{candidate}`, score {score})"
            );
        }
    }

    #[test]
    fn sentence_rhythm_history_is_bounded() {
        let mut state = DiscourseState::new();
        state.record_sentence_rhythm(
            "One changed. Two changed. Three changed. Four changed. Five changed. Six changed. Seven changed.",
        );

        assert_eq!(state.sentence_length_history.len(), SENTENCE_RHYTHM_WINDOW);
    }

    // --- Cb tracking tests (Phase 1) ---

    #[test]
    fn cb_none_before_first_render() {
        let state = DiscourseState::new();
        assert_eq!(state.cb, None);
    }

    #[test]
    fn cb_becomes_focus_after_first_render() {
        let mut state = DiscourseState::new();
        state.begin_render();
        state.mention_entity("Foo", "class");
        state.advance_cb();
        assert_eq!(state.cb.as_deref(), Some("Foo"));
    }

    #[test]
    fn cb_stays_on_continue_transition() {
        let mut state = DiscourseState::new();
        state.begin_render();
        state.mention_entity("Foo", "class");
        state.advance_cb();
        state.begin_render();
        state.mention_entity("Foo", "class");
        state.advance_cb();
        assert_eq!(state.cb.as_deref(), Some("Foo"));
    }

    #[test]
    fn cb_shifts_to_prior_focus_on_new_entity_intro() {
        // Render 1: Foo → Cb becomes Foo (first render, no prev).
        // Render 2: Bar (new entity, mention_count == 1 so Smooth Shift) → Cb stays Foo.
        let mut state = DiscourseState::new();
        state.begin_render();
        state.mention_entity("Foo", "class");
        state.advance_cb();
        state.begin_render();
        state.mention_entity("Bar", "class");
        state.advance_cb();
        assert_eq!(state.cb.as_deref(), Some("Foo"));
    }

    #[test]
    fn cb_shifts_to_current_on_retain() {
        // Render 1: Foo
        // Render 2: Foo (continue)
        // Render 3: Foo (continue)
        // Render 4: Bar (new entity; Smooth Shift → Cb=Foo)
        // Render 5: Foo (re-focus on previously-seen entity; Retain → Cb=Foo)
        let mut state = DiscourseState::new();
        for name in ["Foo", "Foo", "Foo", "Bar", "Foo"] {
            state.begin_render();
            state.mention_entity(name, "class");
            state.advance_cb();
        }
        // Foo has mention_count >= 2 by render 5 → Retain → Cb=Foo
        assert_eq!(state.cb.as_deref(), Some("Foo"));
    }

    #[test]
    fn cb_reset_clears_state() {
        let mut state = DiscourseState::new();
        state.begin_render();
        state.mention_entity("Foo", "class");
        state.advance_cb();
        state.reset();
        assert_eq!(state.cb, None);
        assert_eq!(state.previous_focus, None);
    }

    #[test]
    fn reference_form_all_variants_distinct() {
        // Sanity: ensure the new variants are distinguishable.
        assert_ne!(ReferenceForm::Full, ReferenceForm::Zero);
        assert_ne!(ReferenceForm::Pronoun, ReferenceForm::Demonstrative);
        assert_ne!(ReferenceForm::Pronoun, ReferenceForm::Possessive);
        assert_ne!(ReferenceForm::Possessive, ReferenceForm::ShortName);
        assert_ne!(ReferenceForm::Zero, ReferenceForm::Demonstrative);
    }

    #[test]
    fn list_style_cycles() {
        let mut state = DiscourseState::new();
        let s1 = state.next_list_style();
        let s2 = state.next_list_style();
        let s3 = state.next_list_style();
        let s4 = state.next_list_style();

        // The first four picks still match the original order so existing
        // golden tests (e.g. document_render_preserves_list_style_cycle_across_paragraphs)
        // remain stable.
        assert_eq!(s1, ListStyle::Including);
        assert_eq!(s2, ListStyle::SuchAs);
        assert_eq!(s3, ListStyle::Dash);
        assert_eq!(s4, ListStyle::Bracketed);
    }

    #[test]
    fn list_style_cycle_visits_every_variant_within_palette_length() {
        // Anti-repeat plus deterministic walk should still surface every
        // registered variant within LIST_STYLES.len() consecutive picks,
        // otherwise the palette has dead variants users never see.
        let mut state = DiscourseState::new();
        let mut seen: std::collections::HashSet<ListStyle> = std::collections::HashSet::new();
        for _ in 0..LIST_STYLES.len() {
            seen.insert(state.next_list_style());
        }
        assert_eq!(
            seen.len(),
            LIST_STYLES.len(),
            "anti-repeat cycle dropped a variant: visited {seen:?}"
        );
    }

    #[test]
    fn list_style_anti_repeat_skips_recent_window() {
        // With LIST_STYLE_RECENT_WINDOW = 2, no style may repeat within 3
        // consecutive picks. Walk a long horizon and assert the invariant.
        let mut state = DiscourseState::new();
        let mut history: Vec<ListStyle> = Vec::new();
        for _ in 0..(LIST_STYLES.len() * 3) {
            let style = state.next_list_style();
            if history.len() >= LIST_STYLE_RECENT_WINDOW {
                let recent = &history[history.len() - LIST_STYLE_RECENT_WINDOW..];
                assert!(
                    !recent.contains(&style),
                    "style {style:?} repeated within recent window {recent:?} (history: {history:?})"
                );
            }
            history.push(style);
        }
    }

    #[test]
    fn forced_list_style_blocks_immediate_auto_repeat() {
        // record_list_style_used pushes onto the same recent window as
        // next_list_style. After forcing Bracketed twice in a row, the
        // next auto pick must NOT be Bracketed — the original failure
        // mode was a pure-modulo cycle landing on the just-forced style.
        let mut state = DiscourseState::new();
        state.record_list_style_used(ListStyle::Bracketed);
        state.record_list_style_used(ListStyle::Bracketed);

        let auto = state.next_list_style();
        assert_ne!(auto, ListStyle::Bracketed);
    }

    #[test]
    fn forced_list_style_followed_by_auto_skips_window() {
        // If the template forces Including at the same point the auto-cycle
        // would have produced Including, the next auto pick must skip past
        // the forced style rather than emit it again.
        let mut state = DiscourseState::new();
        // Auto cycle starts at LIST_STYLES[0] = Including. Pre-empt with
        // a forced Including.
        state.record_list_style_used(ListStyle::Including);
        let auto = state.next_list_style();
        assert_ne!(auto, ListStyle::Including);
    }

    #[test]
    fn reset_list_cycle_clears_recent_window_so_first_style_returns() {
        let mut state = DiscourseState::new();
        let _ = state.next_list_style();
        let _ = state.next_list_style();
        state.reset_list_cycle();

        assert_eq!(state.next_list_style(), ListStyle::Including);
    }

    // --- Paragraph-reset invariants (preserve narrative-level anti-repeat,
    // clear paragraph-local pronoun/centering state) ---

    #[test]
    fn paragraph_reset_clears_focus_entity_so_no_pronoun_leak() {
        let mut state = DiscourseState::new();
        state.begin_render();
        state.mention_entity("UserService", "class");

        state.reset_for_paragraph();

        // Without an entity table or focus carryover, the next paragraph's
        // first reference must reintroduce the entity in full form rather
        // than pronominalize a stale focus from the prior paragraph.
        assert_eq!(state.reference_form("UserService"), ReferenceForm::Full);
        assert_eq!(state.focus_entity, None);
        assert!(!state.focus_is_plural);
    }

    #[test]
    fn paragraph_reset_clears_centering_state() {
        let mut state = DiscourseState::new();
        state.begin_render();
        state.mention_entity_ranked("Foo", "class", 0);
        state.advance_cb();
        state.begin_render();
        state.mention_entity_ranked("Foo", "class", 0);
        state.advance_cb();

        state.reset_for_paragraph();

        assert_eq!(state.cb(), None);
        assert!(state.cf().is_empty());
        assert!(state.previous_cf().is_empty());
        assert_eq!(state.last_transition(), Transition::NoCb);
    }

    #[test]
    fn paragraph_reset_suppresses_cross_paragraph_relation_inference() {
        let mut state = DiscourseState::new();
        state.begin_render();
        state.last_template_key = Some("code.added".to_string());
        state.last_entity_name = Some("Foo".to_string());

        state.reset_for_paragraph();

        // Same key + same entity in the next paragraph must not be classified
        // as `Contrast`/`SameEntityDifferentAction` — those would emit a
        // cross-paragraph "However,"/"Furthermore," that bridges over the
        // intentional paragraph break.
        assert_eq!(
            state.detect_relation("code.deleted", Some("Foo")),
            DiscourseRelation::None
        );
    }

    #[test]
    fn paragraph_reset_preserves_template_variant_history() {
        let mut state = DiscourseState::new();
        state.record_template_choice("code.renamed", 2);

        state.reset_for_paragraph();

        // Anti-repeat must survive the paragraph break so the next paragraph
        // doesn't immediately replay the variant the prior paragraph just used.
        assert_eq!(state.last_template_variant("code.renamed"), Some(2));
    }

    #[test]
    fn paragraph_reset_preserves_word_repetition_penalty() {
        let mut state = DiscourseState::new();
        state.begin_render();
        state.record_output_words("AuthGuard removed authentication entirely");

        state.reset_for_paragraph();

        // begin_render advances render_index for the next paragraph's first
        // utterance; the repetition score must still penalize words that
        // appeared in the prior paragraph.
        state.begin_render();
        let overlap_score = state.repetition_score("AuthGuard authentication entirely was removed");
        let unrelated_score = state.repetition_score("Telemetry pipeline rebuilt cleanly");
        assert!(
            overlap_score > unrelated_score,
            "expected overlap score {overlap_score} to exceed unrelated {unrelated_score}",
        );
        assert!(
            overlap_score > 0.0,
            "word_history must persist across paragraph reset"
        );
    }

    #[test]
    fn paragraph_reset_preserves_render_index_so_demonstrative_continues() {
        let mut state = DiscourseState::new();
        // Simulate paragraph 1 with one event.
        state.begin_render();
        state.mention_entity("Foo", "class");
        state.advance_cb();

        state.reset_for_paragraph();

        // First render of paragraph 2.
        state.begin_render();
        // has_prior_render() drives `{noun|demonstrative}`'s "this X" vs
        // "the X" decision. Inside a single narrative, "this" remains correct
        // after the paragraph break — only a full session reset returns to
        // the introductory "the".
        assert!(state.has_prior_render());
        assert!(!state.is_first_render());
    }

    #[test]
    fn paragraph_reset_preserves_list_style_cycle() {
        let mut state = DiscourseState::new();
        let first = state.next_list_style();
        let second_before = state.next_list_style();

        state.reset_for_paragraph();
        let next_after_reset = state.next_list_style();

        // Cycle must NOT restart at the first style after a paragraph break.
        assert_ne!(next_after_reset, first);
        assert_ne!(next_after_reset, second_before);
    }

    #[test]
    fn full_reset_clears_anti_repeat_state() {
        // The full-narrative reset must still clear everything — anti-repeat
        // continuity belongs to a narrative, not to the session as a whole.
        let mut state = DiscourseState::new();
        state.begin_render();
        state.record_template_choice("k", 1);
        state.record_output_words("alpha beta gamma");

        state.reset();

        assert_eq!(state.last_template_variant("k"), None);
        // Newly-recorded non-overlapping words score zero against an empty
        // word_history.
        state.begin_render();
        assert_eq!(state.repetition_score("alpha beta gamma"), 0.0);
    }

    // --- Cf and Transition tests (Phase 2 + Phase 3) ---

    #[test]
    fn transition_no_cb_before_first_render() {
        let state = DiscourseState::new();
        assert_eq!(state.last_transition(), Transition::NoCb);
    }

    #[test]
    fn transition_no_cb_when_no_entity() {
        let mut state = DiscourseState::new();
        state.begin_render();
        state.advance_cb();
        assert_eq!(state.last_transition(), Transition::NoCb);
    }

    #[test]
    fn transition_nocb_after_first_mention() {
        // First render: no previous Cf exists, so no transition is meaningful.
        // prev_cb = None → classify_transition returns NoCb (no Cb to compare against prev).
        // But after the first render, cb is set to current entity.
        // The first advance_cb: new_cb = Some("Foo") (fallback: first render, no prev_focus).
        // prev_cb = None → classify_transition(Some("Foo"), None, Some("Foo"))
        //   → cb_eq_prev = false (prev is None), cb_eq_cp = true → SmoothShift.
        // But the plan says NoCb for the first render. The plan's test checks
        // last_transition == NoCb after render 1, which means we should return NoCb
        // when prev_cb is None (there's no prior Cb to continue from).
        let mut state = DiscourseState::new();
        state.begin_render();
        state.mention_entity("Foo", "class");
        state.advance_cb();
        assert_eq!(state.last_transition(), Transition::NoCb);
    }

    #[test]
    fn transition_continue_same_entity_and_cp() {
        let mut state = DiscourseState::new();
        state.begin_render();
        state.mention_entity("Foo", "class");
        state.advance_cb();
        // First render → NoCb.
        assert_eq!(state.last_transition(), Transition::NoCb);

        state.begin_render();
        state.mention_entity("Foo", "class");
        state.advance_cb();
        // Same entity again: Cb stays Foo, Cp is Foo → Continue.
        assert_eq!(state.last_transition(), Transition::Continue);
    }

    #[test]
    fn transition_continue_when_cp_and_cb_both_same() {
        let mut state = DiscourseState::new();
        state.begin_render();
        state.mention_entity_ranked("Foo", "class", 0);
        state.advance_cb();

        state.begin_render();
        state.mention_entity_ranked("Foo", "class", 0);
        state.advance_cb();
        assert_eq!(state.last_transition(), Transition::Continue);
    }

    #[test]
    fn transition_retain_when_cb_same_but_cp_differs() {
        let mut state = DiscourseState::new();
        state.begin_render();
        state.mention_entity_ranked("Foo", "class", 0);
        state.advance_cb();

        state.begin_render();
        // Foo still in Cf (rank 1 — object), but Cp is now Bar (rank 0 — subject).
        // Cb = Foo (only entity in common with previous Cf), Cp = Bar → Cb != Cp → Retain.
        state.mention_entity_ranked("Bar", "class", 0);
        state.mention_entity_ranked("Foo", "class", 1);
        state.advance_cb();
        assert_eq!(state.last_transition(), Transition::Retain);
    }

    #[test]
    fn transition_smooth_shift_new_entity() {
        let mut state = DiscourseState::new();
        state.begin_render();
        state.mention_entity("Foo", "class");
        state.advance_cb();

        state.begin_render();
        state.mention_entity("Bar", "class");
        state.advance_cb();
        // New entity, no overlap with previous Cf → fallback: Bar seen for first time
        // → previous_focus stays as Cb. Cp = Bar, Cb = Foo (prev focus).
        // prev_cb was Foo; new_cb = Foo; prev_cb == new_cb true; new_cb == Cp false → Retain.
        // OR: if Bar is brand-new and no overlap, fallback gives new_cb = previous_focus = Foo.
        // Then: cb_eq_prev = (Foo == Foo) = true, cb_eq_cp = (Foo == Bar) = false → Retain.
        // But the plan says SmoothShift. The plan's test is at Phase 1 before full Cf is wired.
        // With full Cf: previous_cf = [{Foo,0}], current_cf = [{Bar,0}]. No overlap.
        // Bar is brand-new (mention_count == 1 after this render but the check uses > 1).
        // So fallback: previous_focus (= Foo) → new_cb = Foo.
        // classify_transition(Some("Foo"), Some("Foo"), Some("Bar"))
        //   → cb_eq_prev = true, cb_eq_cp = false → Retain.
        // The plan's Phase 1 test was drafted without full Cf; with Cf it's Retain.
        // We verify the correct Cf-based result: Retain.
        assert_eq!(state.last_transition(), Transition::Retain);
    }

    #[test]
    fn transition_smooth_shift_new_cb_equals_cp() {
        // True Smooth Shift: Cb changes AND Cb == Cp.
        // We need overlap between current and previous Cf where the new Cb != prev Cb.
        // u1: Foo (rank 0). Cb = Foo (first render, NoCb transition).
        // u2: Bar (rank 0), Foo (rank 1). Cf overlap = {Foo}. Cb = Foo.
        //   prev_cb = Foo; new_cb = Foo; cb_eq_prev = true; cb_eq_cp = (Foo==Bar)=false → Retain.
        // To get SmoothShift we need new_cb != prev_cb AND new_cb == cp.
        // u1: Foo. u2: Bar + Foo (Cb=Foo, prev_cb=Foo → Retain).
        // u3: Bar (rank 0 only). Cf={Bar}. Overlap with u2 Cf={Bar,Foo}: Bar is in both.
        //   new_cb = Bar. prev_cb = Foo. cp = Bar.
        //   cb_eq_prev = (Bar==Foo) = false; cb_eq_cp = (Bar==Bar) = true → SmoothShift.
        let mut state = DiscourseState::new();
        state.begin_render();
        state.mention_entity_ranked("Foo", "class", 0);
        state.advance_cb();

        state.begin_render();
        state.mention_entity_ranked("Bar", "class", 0);
        state.mention_entity_ranked("Foo", "class", 1);
        state.advance_cb();
        assert_eq!(state.last_transition(), Transition::Retain);

        state.begin_render();
        state.mention_entity_ranked("Bar", "class", 0);
        state.advance_cb();
        assert_eq!(state.last_transition(), Transition::SmoothShift);
    }

    #[test]
    fn transition_rough_shift_proper() {
        let mut state = DiscourseState::new();
        // u1: focus Foo. Cb = Foo. Cp = Foo. → NoCb (first render).
        state.begin_render();
        state.mention_entity_ranked("Foo", "class", 0);
        state.advance_cb();

        // u2: Bar (rank 0), Foo (rank 1).
        // Cf overlap with u1 Cf={Foo}: Foo is shared. Cb = Foo.
        // prev_cb = Foo, new_cb = Foo, cp = Bar.
        // cb_eq_prev = true, cb_eq_cp = false → Retain.
        state.begin_render();
        state.mention_entity_ranked("Bar", "class", 0);
        state.mention_entity_ranked("Foo", "class", 1);
        state.advance_cb();
        assert_eq!(state.last_transition(), Transition::Retain);

        // u3: Baz (rank 0), Bar (rank 1).
        // Cf overlap with u2 Cf={Bar,Foo}: Bar is in current_cf. Cb = Bar.
        // prev_cb = Foo (from u1→u2 transition), cp = Baz.
        // cb_eq_prev = (Bar==Foo) = false, cb_eq_cp = (Bar==Baz) = false → RoughShift.
        state.begin_render();
        state.mention_entity_ranked("Baz", "class", 0);
        state.mention_entity_ranked("Bar", "class", 1);
        state.advance_cb();
        assert_eq!(state.last_transition(), Transition::RoughShift);
    }

    #[test]
    fn cf_deduplicates_by_name_keeping_lower_rank() {
        let mut state = DiscourseState::new();
        state.begin_render();
        state.mention_entity_ranked("Foo", "class", 2);
        state.mention_entity_ranked("Foo", "class", 0);
        let cf = state.cf();
        assert_eq!(cf.len(), 1);
        assert_eq!(cf[0].rank, 0);
    }

    #[test]
    fn cf_deduplication_keeps_lower_rank_when_second_is_higher() {
        let mut state = DiscourseState::new();
        state.begin_render();
        state.mention_entity_ranked("Foo", "class", 0);
        state.mention_entity_ranked("Foo", "class", 2);
        let cf = state.cf();
        assert_eq!(cf.len(), 1);
        assert_eq!(cf[0].rank, 0);
    }

    #[test]
    fn cf_sorts_by_rank_ascending() {
        let mut state = DiscourseState::new();
        state.begin_render();
        state.mention_entity_ranked("Obj", "class", 1);
        state.mention_entity_ranked("Subj", "class", 0);
        state.mention_entity_ranked("Oblique", "class", 2);
        let cf = state.cf();
        assert_eq!(cf[0].name, "Subj");
        assert_eq!(cf[1].name, "Obj");
        assert_eq!(cf[2].name, "Oblique");
    }

    #[test]
    fn cp_is_first_cf_entry() {
        let mut state = DiscourseState::new();
        state.begin_render();
        state.mention_entity_ranked("Subj", "class", 0);
        state.mention_entity_ranked("Obj", "class", 1);
        assert_eq!(state.cf()[0].name, "Subj");
    }

    #[test]
    fn cf_cleared_by_begin_render() {
        let mut state = DiscourseState::new();
        state.begin_render();
        state.mention_entity_ranked("Foo", "class", 0);
        assert_eq!(state.cf().len(), 1);

        state.begin_render();
        assert_eq!(
            state.cf().len(),
            0,
            "current_cf must be cleared by begin_render"
        );
    }

    #[test]
    fn previous_cf_set_after_advance_cb() {
        let mut state = DiscourseState::new();
        state.begin_render();
        state.mention_entity_ranked("Foo", "class", 0);
        state.mention_entity_ranked("Bar", "class", 1);
        state.advance_cb();

        let prev = state.previous_cf();
        assert_eq!(prev.len(), 2);
        assert_eq!(prev[0].name, "Foo");
        assert_eq!(prev[1].name, "Bar");
    }

    #[test]
    fn mention_entity_delegates_to_rank_zero() {
        let mut state = DiscourseState::new();
        state.begin_render();
        state.mention_entity("Foo", "class");
        let cf = state.cf();
        assert_eq!(cf.len(), 1);
        assert_eq!(cf[0].rank, 0);
    }

    #[test]
    fn classify_transition_all_cases() {
        // Continue: cb == prev_cb AND cb == cp.
        assert_eq!(
            classify_transition(Some("Foo"), Some("Foo"), Some("Foo")),
            Transition::Continue
        );
        // Retain: cb == prev_cb, cb != cp.
        assert_eq!(
            classify_transition(Some("Foo"), Some("Foo"), Some("Bar")),
            Transition::Retain
        );
        // SmoothShift: cb != prev_cb, cb == cp.
        assert_eq!(
            classify_transition(Some("Bar"), Some("Foo"), Some("Bar")),
            Transition::SmoothShift
        );
        // RoughShift: cb != prev_cb, cb != cp.
        assert_eq!(
            classify_transition(Some("Bar"), Some("Foo"), Some("Baz")),
            Transition::RoughShift
        );
        // NoCb: no current cb.
        assert_eq!(
            classify_transition(None, Some("Foo"), Some("Bar")),
            Transition::NoCb
        );
        // NoCb with all None.
        assert_eq!(classify_transition(None, None, None), Transition::NoCb);
    }

    #[test]
    fn reset_clears_cf_and_transition_state() {
        let mut state = DiscourseState::new();
        state.begin_render();
        state.mention_entity_ranked("Foo", "class", 0);
        state.advance_cb();
        state.reset();

        assert_eq!(state.cf().len(), 0);
        assert_eq!(state.previous_cf().len(), 0);
        assert_eq!(state.last_transition(), Transition::NoCb);
    }
}
