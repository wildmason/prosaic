//! Collection-type aliases for `no_std`-compatible builds.
//!
//! All code inside `prosaic-core` should import collections from this module
//! rather than from `std::collections` or `ahash` directly. This keeps the
//! single `#![cfg_attr(not(feature = "std"), no_std)]` gate working without
//! scattered conditional imports.

pub type HashMap<K, V> = hashbrown::HashMap<K, V, ahash::RandomState>;
pub type HashSet<T> = hashbrown::HashSet<T, ahash::RandomState>;
pub use alloc::collections::{BTreeMap, VecDeque};

/// Construct an empty [`HashMap`].
///
/// Equivalent to `HashMap::new()` from `std::collections`. The custom
/// `ahash::RandomState` hasher requires `with_hasher` rather than `new`,
/// so this helper provides the ergonomic `new`-like constructor.
#[inline]
pub fn new_map<K, V>() -> HashMap<K, V> {
    HashMap::with_hasher(ahash::RandomState::new())
}

/// Construct an empty [`HashSet`].
///
/// Equivalent to `HashSet::new()` from `std::collections`.
#[inline]
pub fn new_set<T>() -> HashSet<T> {
    HashSet::with_hasher(ahash::RandomState::new())
}

/// Construct a [`HashMap`] with at least the given capacity.
///
/// Equivalent to `HashMap::with_capacity(n)` from `std::collections`.
#[inline]
pub fn map_with_capacity<K, V>(capacity: usize) -> HashMap<K, V> {
    HashMap::with_capacity_and_hasher(capacity, ahash::RandomState::new())
}
