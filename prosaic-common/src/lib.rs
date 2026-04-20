//! Shared type metadata for the prosaic template engine.
//!
//! Owns `ValueType`, the `PIPE_SPECS` registry, and const-eval helpers used
//! by both `prosaic-core` (at runtime) and `prosaic-derive` (at macro
//! expansion time). This crate is `no_std` and has no dependencies so it
//! can be included anywhere the other two crates run.

#![no_std]

/// A linguistic type that a template slot or pipe can carry.
///
/// Mirrors the variants of `prosaic_core::Value` plus `Any` as an
/// escape hatch for pipes that accept heterogeneous inputs (such as
/// `capitalize`, `verb`, `refer`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValueType {
    String,
    Number,
    List,
    Entity,
    /// Accepts any slot type. During type unification a concrete type
    /// always wins over `Any` (e.g. `Number ∩ Any → Number`), so marking
    /// a pipe's input `Any` never widens an already-inferred concrete
    /// type.
    Any,
}

#[cfg(test)]
mod value_type_tests {
    use super::*;

    #[test]
    fn variants_are_pairwise_distinct() {
        let all = [
            ValueType::String,
            ValueType::Number,
            ValueType::List,
            ValueType::Entity,
            ValueType::Any,
        ];
        for i in 0..all.len() {
            for j in (i + 1)..all.len() {
                assert_ne!(
                    all[i], all[j],
                    "variants at index {i} and {j} are unexpectedly equal"
                );
            }
        }
    }

    #[test]
    fn value_type_is_copy_and_eq() {
        let a = ValueType::Number;
        let b = a; // Copy
        assert_eq!(a, b);
        assert_ne!(ValueType::Number, ValueType::String);
    }

    #[test]
    fn all_variants_are_const_constructible() {
        const ALL: [ValueType; 5] = [
            ValueType::String,
            ValueType::Number,
            ValueType::List,
            ValueType::Entity,
            ValueType::Any,
        ];
        // The const array proves every variant is const-constructible.
        // Assert the length so this test fails if a variant is removed
        // from the enum without also being removed from this array.
        assert_eq!(ALL.len(), 5);
    }
}
