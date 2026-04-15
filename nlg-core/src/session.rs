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

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::discourse::DiscourseState;

/// Mutable state for a render sequence. See module docs.
#[derive(Debug)]
pub struct Session {
    pub(crate) discourse: DiscourseState,
    /// RoundRobin counters keyed by template key. Stored as AtomicUsize
    /// so a future `&Session`-only code path (e.g. read-only scoring)
    /// can still advance counters atomically without an outer borrow.
    pub(crate) round_robin_counters: HashMap<String, AtomicUsize>,
}

impl Session {
    pub fn new() -> Self {
        Self {
            discourse: DiscourseState::new(),
            round_robin_counters: HashMap::new(),
        }
    }

    /// Clear all session state. Equivalent to replacing with `Session::new()`
    /// but preserves allocations.
    pub fn reset(&mut self) {
        self.discourse.reset();
        self.round_robin_counters.clear();
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
    fn clone(&self) -> Self {
        let mut counters = HashMap::with_capacity(self.round_robin_counters.len());
        for (k, v) in &self.round_robin_counters {
            counters.insert(k.clone(), AtomicUsize::new(v.load(Ordering::Relaxed)));
        }
        Self {
            discourse: self.discourse.clone(),
            round_robin_counters: counters,
        }
    }
}
