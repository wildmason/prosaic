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

use crate::collections::{HashMap, map_with_capacity, new_map};

use crate::discourse::DiscourseState;

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
}

impl Session {
    pub fn new() -> Self {
        Self {
            discourse: DiscourseState::new(),
            round_robin_counters: new_map(),
            last_temporal_anchor: None,
        }
    }

    /// Clear all session state. Equivalent to replacing with `Session::new()`
    /// but preserves allocations.
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

    /// Clear the temporal anchor. Use when starting a temporally-disjoint
    /// narrative in the same session.
    pub fn reset_temporal(&mut self) {
        self.last_temporal_anchor = None;
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
