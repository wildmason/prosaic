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
    Any,
}

#[cfg(test)]
mod value_type_tests {
    use super::*;

    #[test]
    fn value_type_variants_compile() {
        let _ = [
            ValueType::String,
            ValueType::Number,
            ValueType::List,
            ValueType::Entity,
            ValueType::Any,
        ];
    }

    #[test]
    fn value_type_is_copy_and_eq() {
        let a = ValueType::Number;
        let b = a; // Copy
        assert_eq!(a, b);
        assert_ne!(ValueType::Number, ValueType::String);
    }

    #[test]
    fn value_type_is_usable_in_const() {
        const _T: ValueType = ValueType::Number;
        assert_eq!(_T, ValueType::Number);
    }
}
