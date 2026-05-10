//! Per-render-sequence mutable state. See module docs.
//!
//! A `Session` owns the `DiscourseState` (focus stack, template history,
//! word-frequency log, list-style cycle) and any other runtime-mutable
//! counters associated with a render sequence. Callers create one per
//! logical "document" — a batch, a DocumentPlan, a page of output — and
//! pass `&mut Session` into render calls.
//!
//! A fresh session = a fresh narrative. Calling `reset()` on an existing
//! session clears state without deallocating.

use core::sync::atomic::{AtomicUsize, Ordering};

#[cfg(not(feature = "std"))]
use alloc::string::String;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use crate::collections::{HashMap, map_with_capacity, new_map};

use crate::discourse::DiscourseState;
use crate::salience::Salience;
use crate::style::{LengthDistribution, SalienceBias};

/// Mutable state for a render sequence. See module docs.
#[derive(Debug)]
pub struct Session {
    pub(crate) discourse: DiscourseState,
    /// RoundRobin counters keyed by template key. Stored as AtomicUsize
    /// so a future `&Session`-only code path (e.g. read-only scoring)
    /// can still advance counters atomically without an outer borrow.
    pub(crate) round_robin_counters: HashMap<String, AtomicUsize>,
    /// Unix-seconds timestamp of the most recently-rendered event. Used by
    /// the `{timestamp|since_last}` pipe to compute inter-event deltas
    /// ("the next day", "moments later"). Persists across
    /// [`Session::reset`] so narratives can span paragraphs. Starts as
    /// `None`; set automatically whenever an event's context contains a
    /// `timestamp` slot. Call [`Session::reset_temporal`] to clear it.
    pub(crate) last_temporal_anchor: Option<i64>,
    /// Connectives the engine must skip during the next render(s). Used by
    /// the retrospective refine pass to apply `BlacklistConnective`
    /// constraints without mutating the engine. Empty in normal use.
    pub(crate) refine_blacklist_connectives: Vec<String>,
    /// List styles the engine must skip during the next render(s). Used by
    /// the retrospective refine pass to apply `BlacklistListStyle`
    /// constraints without mutating the engine. Empty in normal use.
    pub(crate) refine_blacklist_list_styles: Vec<crate::discourse::ListStyle>,
    /// Refine-pass override for the active `SalienceBias`. When `Some`,
    /// the engine ignores the active style profile's salience bias and
    /// applies this one instead for the duration of the iteration.
    /// Carries `OverrideSalienceBias` constraints. `None` in normal use.
    pub(crate) refine_salience_bias: Option<SalienceBias>,
    /// Refine-pass override for the active sentence-length distribution.
    /// When `Some`, the candidate-scoring path uses this distribution as
    /// the bias target instead of the active style profile's. Carries
    /// `TightenLengthDistribution` constraints. `None` in normal use.
    pub(crate) refine_length_distribution: Option<LengthDistribution>,
    /// Refine-pass override forcing a specific variant tier per template
    /// key. When a render's template key is present, the engine
    /// short-circuits the salience+verbosity calculation and uses the
    /// listed tier directly. Carries `ForceVariantTier` constraints.
    /// Empty in normal use.
    pub(crate) refine_force_variant_tier: Vec<(String, Salience)>,
}

impl Session {
    pub fn new() -> Self {
        Self {
            discourse: DiscourseState::new(),
            round_robin_counters: new_map(),
            last_temporal_anchor: None,
            refine_blacklist_connectives: Vec::new(),
            refine_blacklist_list_styles: Vec::new(),
            refine_salience_bias: None,
            refine_length_distribution: None,
            refine_force_variant_tier: Vec::new(),
        }
    }

    /// Set the connectives + list styles the next render(s) must skip.
    /// Called by the retrospective refine pass before each iteration; not
    /// part of the public engine API.
    pub(crate) fn set_refine_blacklists(
        &mut self,
        connectives: Vec<String>,
        list_styles: Vec<crate::discourse::ListStyle>,
    ) {
        self.refine_blacklist_connectives = connectives;
        self.refine_blacklist_list_styles = list_styles;
    }

    /// Push phantom history entries onto the discourse ring buffers so the
    /// next render's anti-repeat treats these connectives / list styles as
    /// recently used. Called by the retrospective refine pass to apply
    /// `PrimeRecencyWindow` constraints. Pushes are bounded by the same
    /// window caps the live emit path uses.
    pub(crate) fn prime_refine_recency(
        &mut self,
        connectives: &[String],
        list_styles: &[crate::discourse::ListStyle],
    ) {
        self.discourse
            .prime_connective_history(connectives);
        self.discourse
            .prime_list_style_history(list_styles);
    }

    /// Set the refine-pass salience-bias override. `None` clears it.
    pub(crate) fn set_refine_salience_bias(&mut self, bias: Option<SalienceBias>) {
        self.refine_salience_bias = bias;
    }

    /// Set the refine-pass sentence-length-distribution override.
    /// `None` clears it.
    pub(crate) fn set_refine_length_distribution(
        &mut self,
        distribution: Option<LengthDistribution>,
    ) {
        self.refine_length_distribution = distribution;
    }

    /// Set the refine-pass forced-variant-tier mapping. Replaces any
    /// existing mapping wholesale.
    pub(crate) fn set_refine_force_variant_tiers(
        &mut self,
        tiers: Vec<(String, Salience)>,
    ) {
        self.refine_force_variant_tier = tiers;
    }

    /// Look up a forced variant tier for the given template key, if any.
    pub(crate) fn refine_forced_tier_for(&self, key: &str) -> Option<Salience> {
        self.refine_force_variant_tier
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, t)| *t)
    }

    /// Clear any active refine-pass overrides. Note: phantom entries
    /// pushed onto discourse ring buffers via `prime_refine_recency` are
    /// not undone — they're indistinguishable from real history once
    /// pushed and decay naturally as new entries arrive. The iteration
    /// controller restores from a clean snapshot before each iteration,
    /// so primes never accumulate across iterations.
    pub(crate) fn clear_refine_overrides(&mut self) {
        self.refine_blacklist_connectives.clear();
        self.refine_blacklist_list_styles.clear();
        self.refine_salience_bias = None;
        self.refine_length_distribution = None;
        self.refine_force_variant_tier.clear();
    }

    /// Clear all session state. Equivalent to replacing with `Session::new()`
    /// but preserves allocations. Use when starting a fully unrelated
    /// narrative in the same session — most multi-paragraph callers want
    /// [`Session::reset_for_paragraph`] instead so style rotation continues.
    ///
    /// NOTE: `last_temporal_anchor` survives so narratives can span paragraphs.
    /// Call [`Session::reset_temporal`] to clear the anchor explicitly when
    /// starting a temporally-disjoint narrative in the same session.
    pub fn reset(&mut self) {
        self.discourse.reset();
        self.round_robin_counters.clear();
        // Intentionally NOT clearing last_temporal_anchor — it must survive
        // paragraph breaks so inter-paragraph temporal phrases ("two weeks later")
        // work correctly.
    }

    /// Reset paragraph-local discourse while keeping narrative-level style
    /// continuity. Pronoun, focus, and Centering Theory state are cleared so
    /// anaphora cannot leak across the paragraph break, but every form of
    /// stylistic anti-repeat — list-style rotation, template-variant history,
    /// connective history, word-repetition scoring, and Round-Robin variant
    /// counters — survives, along with the temporal anchor. Consecutive
    /// paragraphs therefore rotate through `|join` phrasings, avoid replaying
    /// the same template variant or connective, are penalized for repeating
    /// recent vocabulary, and continue to support inter-paragraph temporal
    /// references.
    ///
    /// This is the reset [`crate::DocumentPlan::render`] uses between
    /// paragraphs. Library consumers driving their own paragraph loop should
    /// prefer this over [`Session::reset`].
    pub fn reset_for_paragraph(&mut self) {
        self.discourse.reset_for_paragraph();
        // round_robin_counters are intentionally retained: they back
        // Variation::RoundRobin's variant cycling, and resetting them every
        // paragraph would re-introduce the same opener after each break.
        // See `reset`: temporal anchors intentionally survive paragraph breaks.
    }

    /// Clear the temporal anchor. Use when starting a temporally-disjoint
    /// narrative in the same session.
    pub fn reset_temporal(&mut self) {
        self.last_temporal_anchor = None;
    }

    /// Clear the discourse list-style cycle counter so the next `|join` pipe
    /// starts at the first style in the rotation. Use when starting a
    /// stylistically-disjoint narrative in the same session without doing
    /// a full [`Session::reset`].
    pub fn reset_list_cycle(&mut self) {
        self.discourse.reset_list_cycle();
    }

    /// Mutable access to the underlying discourse state. Use this to call
    /// [`DiscourseState::mention_entity_ranked`] for templates where
    /// grammatical role matters, or to read centering diagnostics such as
    /// [`DiscourseState::cb`], [`DiscourseState::cf`], and
    /// [`DiscourseState::last_transition`].
    pub fn discourse_mut(&mut self) -> &mut DiscourseState {
        &mut self.discourse
    }

    /// Read-only access to the underlying discourse state.
    pub fn discourse(&self) -> &DiscourseState {
        &self.discourse
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for Session {
    /// Deep clone. The RoundRobin counters are cloned by reading each
    /// atomic with `Ordering::Relaxed` — fine because clones are used
    /// as snapshot/restore checkpoints around fallible renders and there
    /// is no concurrent writer during a clone.
    ///
    /// `last_temporal_anchor` is copied so snapshot/restore checkpoints
    /// preserve the temporal state correctly.
    fn clone(&self) -> Self {
        let mut counters = map_with_capacity(self.round_robin_counters.len());
        for (k, v) in &self.round_robin_counters {
            counters.insert(k.clone(), AtomicUsize::new(v.load(Ordering::Relaxed)));
        }
        Self {
            discourse: self.discourse.clone(),
            round_robin_counters: counters,
            last_temporal_anchor: self.last_temporal_anchor,
            refine_blacklist_connectives: self.refine_blacklist_connectives.clone(),
            refine_blacklist_list_styles: self.refine_blacklist_list_styles.clone(),
            refine_salience_bias: self.refine_salience_bias,
            refine_length_distribution: self.refine_length_distribution.clone(),
            refine_force_variant_tier: self.refine_force_variant_tier.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_new_has_no_temporal_anchor() {
        let s = Session::new();
        assert_eq!(s.last_temporal_anchor, None);
    }

    #[test]
    fn session_reset_preserves_temporal_anchor() {
        let mut s = Session::new();
        s.last_temporal_anchor = Some(1_700_000_000);
        s.reset();
        assert_eq!(s.last_temporal_anchor, Some(1_700_000_000));
    }

    #[test]
    fn paragraph_reset_preserves_list_style_cycle() {
        let mut s = Session::new();
        let first = s.discourse.next_list_style();

        s.reset_for_paragraph();
        let second = s.discourse.next_list_style();

        assert_ne!(first, second);
    }

    #[test]
    fn paragraph_reset_preserves_round_robin_counter() {
        // Variation::RoundRobin uses these counters to cycle through template
        // variants. Resetting them every paragraph would replay the same
        // opener after every break — the realism-leak we're closing.
        let mut s = Session::new();
        s.round_robin_counters
            .insert("code.renamed".to_string(), AtomicUsize::new(2));

        s.reset_for_paragraph();

        let counter = s
            .round_robin_counters
            .get("code.renamed")
            .expect("round_robin counter must survive paragraph reset");
        assert_eq!(counter.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn full_reset_clears_round_robin_counters() {
        // Full session resets DO restart the rotation — the counter belongs
        // to the narrative, not the session as a whole.
        let mut s = Session::new();
        s.round_robin_counters
            .insert("code.renamed".to_string(), AtomicUsize::new(2));

        s.reset();

        assert!(s.round_robin_counters.is_empty());
    }

    #[test]
    fn full_reset_restarts_list_style_cycle() {
        let mut s = Session::new();
        let first = s.discourse.next_list_style();

        s.reset();
        let second = s.discourse.next_list_style();

        assert_eq!(first, second);
    }

    #[test]
    fn reset_list_cycle_restarts_rotation_without_full_reset() {
        let mut s = Session::new();
        s.last_temporal_anchor = Some(1_700_000_000);
        let first = s.discourse.next_list_style();
        let _ = s.discourse.next_list_style();

        s.reset_list_cycle();

        // Rotation restarts...
        assert_eq!(s.discourse.next_list_style(), first);
        // ...but the temporal anchor is untouched.
        assert_eq!(s.last_temporal_anchor, Some(1_700_000_000));
    }

    #[test]
    fn session_reset_temporal_clears_anchor() {
        let mut s = Session::new();
        s.last_temporal_anchor = Some(1_700_000_000);
        s.reset_temporal();
        assert_eq!(s.last_temporal_anchor, None);
    }

    #[test]
    fn session_clone_copies_temporal_anchor() {
        let mut s = Session::new();
        s.last_temporal_anchor = Some(1_700_000_000);
        let cloned = s.clone();
        assert_eq!(cloned.last_temporal_anchor, Some(1_700_000_000));
    }

    #[test]
    fn session_clone_is_independent() {
        // Mutating the clone must not affect the original.
        let mut s = Session::new();
        s.last_temporal_anchor = Some(1_700_000_000);
        let mut cloned = s.clone();
        cloned.last_temporal_anchor = Some(9_999_999_999);
        assert_eq!(s.last_temporal_anchor, Some(1_700_000_000));
    }
}
