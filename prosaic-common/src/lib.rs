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

/// The type contract of a named pipe: the [`ValueType`] of its input
/// value and the [`ValueType`] of its output.
///
/// Pipe-argument validation (e.g. that `truncate` has a numeric arg) is
/// deliberately **not** modelled here — it remains a runtime check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PipeSpec {
    pub name: &'static str,
    pub input: ValueType,
    pub output: ValueType,
}

/// The complete set of pipes recognised by the Prosaic engine.
///
/// This registry is the single source of truth shared between
/// `prosaic-core::engine::apply_pipe` (dispatch) and
/// `prosaic-derive::prosaic_template!` (compile-time validation).
///
/// Adding a new pipe: add a `PipeSpec` here, add a match arm in
/// `prosaic-core::engine::apply_pipe`, and the `prosaic_template!` macro
/// will automatically recognise it.
///
/// Feature-gated pipes: `relative` and `since_last` are listed
/// unconditionally so templates targeting time-enabled builds still
/// validate. When `prosaic-core` is compiled without the `time` feature,
/// using either pipe produces an `InvalidPipe` error at render time.
// Ordering mirrors `prosaic-derive::VALID_PIPES` so the two tables read
// identically when reviewed side by side. Lookup is a linear scan so
// order has no behavioral impact — this is purely for maintainability.
pub const PIPE_SPECS: &[PipeSpec] = &[
    PipeSpec { name: "plural",        input: ValueType::Number, output: ValueType::String },
    PipeSpec { name: "pluralize",     input: ValueType::Number, output: ValueType::String },
    PipeSpec { name: "article",       input: ValueType::Any,    output: ValueType::String },
    PipeSpec { name: "join",          input: ValueType::List,   output: ValueType::String },
    PipeSpec { name: "ordinal",       input: ValueType::Number, output: ValueType::String },
    PipeSpec { name: "words",         input: ValueType::Number, output: ValueType::String },
    PipeSpec { name: "truncate",      input: ValueType::List,   output: ValueType::List   },
    PipeSpec { name: "capitalize",    input: ValueType::Any,    output: ValueType::String },
    PipeSpec { name: "refer",         input: ValueType::Any,    output: ValueType::String },
    PipeSpec { name: "verb",          input: ValueType::Any,    output: ValueType::String },
    PipeSpec { name: "syn",           input: ValueType::Any,    output: ValueType::String },
    PipeSpec { name: "relative",      input: ValueType::Number, output: ValueType::String },
    PipeSpec { name: "since_last",    input: ValueType::Number, output: ValueType::String },
    PipeSpec { name: "quantify",      input: ValueType::Number, output: ValueType::String },
    PipeSpec { name: "proportion",    input: ValueType::Number, output: ValueType::String },
    PipeSpec { name: "hedge",         input: ValueType::Number, output: ValueType::String },
    PipeSpec { name: "negated",       input: ValueType::Any,    output: ValueType::String },
    PipeSpec { name: "choose",        input: ValueType::Any,    output: ValueType::String },
    PipeSpec { name: "demonstrative", input: ValueType::Any,    output: ValueType::String },
];

/// Returns `true` when a value of type `actual` can satisfy a slot or
/// pipe that expects type `expected`. `ValueType::Any` is compatible
/// with every concrete type in either direction; concrete types are
/// compatible only with themselves.
///
/// Compatibility is symmetric: `types_compatible(a, b) == types_compatible(b, a)`.
/// The narrowing behaviour during unification (e.g. `Number ∩ Any → Number`) is
/// handled by the caller (see `Template::infer_types`) — this function only
/// answers the binary "is this assignment legal?" question.
// Clippy wants `matches!(...)` here, but `matches!` is not stable in `const fn`
// contexts — and this function MUST stay `const fn` because it is called from
// the compile-time assertion blocks emitted by `prosaic_template!`.
#[allow(clippy::match_like_matches_macro)]
pub const fn types_compatible(actual: ValueType, expected: ValueType) -> bool {
    match (actual, expected) {
        (ValueType::Any, _) | (_, ValueType::Any) => true,
        (ValueType::String, ValueType::String) => true,
        (ValueType::Number, ValueType::Number) => true,
        (ValueType::List, ValueType::List) => true,
        (ValueType::Entity, ValueType::Entity) => true,
        _ => false,
    }
}

/// Look up a pipe by name. `const fn` so it is usable inside `const _: () = { ... }`
/// assertion blocks emitted by the `prosaic_template!` macro.
pub const fn pipe_spec(name: &str) -> Option<&'static PipeSpec> {
    let mut i = 0;
    while i < PIPE_SPECS.len() {
        if byte_eq(PIPE_SPECS[i].name.as_bytes(), name.as_bytes()) {
            return Some(&PIPE_SPECS[i]);
        }
        i += 1;
    }
    None
}

/// Look up a slot's declared [`ValueType`] in a `HasProsaicSchema`-style
/// schema slice. `const fn` so it is usable inside compile-time assertion
/// blocks emitted by the `prosaic_template!` macro.
///
/// Matching is byte-exact and case-sensitive. If the schema contains
/// duplicate keys, the first entry wins.
pub const fn schema_lookup(
    schema: &[(&str, ValueType)],
    slot: &str,
) -> Option<ValueType> {
    let mut i = 0;
    while i < schema.len() {
        if byte_eq(schema[i].0.as_bytes(), slot.as_bytes()) {
            return Some(schema[i].1);
        }
        i += 1;
    }
    None
}

/// Byte-wise equality, usable in `const fn` (unlike `str::eq`).
/// Internal helper — not part of the public contract.
pub(crate) const fn byte_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

#[cfg(test)]
mod pipe_spec_tests {
    use super::*;

    #[test]
    fn all_nineteen_pipes_are_registered() {
        assert_eq!(PIPE_SPECS.len(), 19);
    }

    #[test]
    fn pluralize_is_number_to_string() {
        let p = pipe_spec("pluralize").expect("pluralize must be registered");
        assert_eq!(p.input, ValueType::Number);
        assert_eq!(p.output, ValueType::String);
    }

    #[test]
    fn truncate_is_list_to_list_for_chain_compatibility() {
        let p = pipe_spec("truncate").expect("truncate must be registered");
        assert_eq!(p.input, ValueType::List);
        assert_eq!(p.output, ValueType::List, "truncate must chain into join");
    }

    #[test]
    fn join_is_list_to_string() {
        let p = pipe_spec("join").expect("join must be registered");
        assert_eq!(p.input, ValueType::List);
        assert_eq!(p.output, ValueType::String);
    }

    #[test]
    fn refer_is_any_to_string() {
        let p = pipe_spec("refer").expect("refer must be registered");
        assert_eq!(p.input, ValueType::Any);
        assert_eq!(p.output, ValueType::String);
    }

    #[test]
    fn unknown_pipe_lookup_returns_none() {
        assert!(pipe_spec("nonexistent").is_none());
    }

    #[test]
    fn pipe_spec_lookup_is_const_evaluable() {
        const SPEC: Option<&'static PipeSpec> = pipe_spec("pluralize");
        // Verify both const evaluation worked AND returned the right data.
        // Using matches! lets us destructure inside a const-context assertion
        // without relying on PartialEq on references.
        assert!(matches!(SPEC, Some(s) if s.input == ValueType::Number && s.output == ValueType::String));
    }

    #[test]
    fn pipe_spec_unknown_is_const_evaluable() {
        const MISSING: Option<&'static PipeSpec> = pipe_spec("nonexistent_pipe_xyz");
        assert!(MISSING.is_none());
    }
}

#[cfg(test)]
mod types_compatible_tests {
    use super::*;

    #[test]
    fn any_matches_every_concrete_type() {
        assert!(types_compatible(ValueType::Any, ValueType::Number));
        assert!(types_compatible(ValueType::Number, ValueType::Any));
        assert!(types_compatible(ValueType::Any, ValueType::Any));
    }

    #[test]
    fn same_concrete_types_match() {
        assert!(types_compatible(ValueType::Number, ValueType::Number));
        assert!(types_compatible(ValueType::String, ValueType::String));
        assert!(types_compatible(ValueType::List, ValueType::List));
        assert!(types_compatible(ValueType::Entity, ValueType::Entity));
    }

    #[test]
    fn distinct_concrete_types_reject() {
        assert!(!types_compatible(ValueType::Number, ValueType::String));
        assert!(!types_compatible(ValueType::List, ValueType::Number));
        assert!(!types_compatible(ValueType::String, ValueType::Entity));
    }

    #[test]
    fn compat_is_const_evaluable() {
        const {
            assert!(types_compatible(ValueType::Number, ValueType::Number));
            assert!(!types_compatible(ValueType::Number, ValueType::List));
        }
    }

    #[test]
    fn compatibility_is_symmetric() {
        // Compatibility must be commutative: a future regression that adds a
        // directional special case (e.g. "Any on the left but not the right")
        // would violate the unification semantics.
        assert_eq!(
            types_compatible(ValueType::Number, ValueType::String),
            types_compatible(ValueType::String, ValueType::Number),
        );
        assert_eq!(
            types_compatible(ValueType::Number, ValueType::Any),
            types_compatible(ValueType::Any, ValueType::Number),
        );
        assert_eq!(
            types_compatible(ValueType::List, ValueType::Entity),
            types_compatible(ValueType::Entity, ValueType::List),
        );
    }
}

#[cfg(test)]
mod schema_lookup_tests {
    use super::*;

    const FIXTURE: &[(&str, ValueType)] = &[
        ("name", ValueType::String),
        ("count", ValueType::Number),
        ("items", ValueType::List),
        ("actor", ValueType::Entity),
    ];

    #[test]
    fn found_key_returns_type() {
        assert_eq!(schema_lookup(FIXTURE, "name"), Some(ValueType::String));
        assert_eq!(schema_lookup(FIXTURE, "count"), Some(ValueType::Number));
        assert_eq!(schema_lookup(FIXTURE, "items"), Some(ValueType::List));
        assert_eq!(schema_lookup(FIXTURE, "actor"), Some(ValueType::Entity));
    }

    #[test]
    fn missing_key_returns_none() {
        assert_eq!(schema_lookup(FIXTURE, "absent"), None);
    }

    #[test]
    fn empty_schema_returns_none() {
        assert_eq!(schema_lookup(&[], "anything"), None);
    }

    #[test]
    fn lookup_is_const_evaluable() {
        const FOUND: Option<ValueType> = schema_lookup(FIXTURE, "count");
        const MISSING: Option<ValueType> = schema_lookup(FIXTURE, "absent");
        assert_eq!(FOUND, Some(ValueType::Number));
        assert_eq!(MISSING, None);
    }

    #[test]
    fn similar_but_longer_key_does_not_match() {
        // Guards against accidental prefix matching in byte_eq.
        assert_eq!(schema_lookup(FIXTURE, "namer"), None);
        assert_eq!(schema_lookup(FIXTURE, "nam"), None);
    }

    #[test]
    fn duplicate_keys_return_first_match() {
        // Documents the first-match contract — if a schema contains
        // duplicate keys (shouldn't happen with the derive but could with
        // hand-authored impls), the first entry wins.
        const DUPE: &[(&str, ValueType)] = &[
            ("x", ValueType::Number),
            ("x", ValueType::String),
        ];
        assert_eq!(schema_lookup(DUPE, "x"), Some(ValueType::Number));
    }

    #[test]
    fn lookup_is_case_sensitive() {
        // Comparison is byte-exact — case folding is out of scope.
        assert_eq!(schema_lookup(FIXTURE, "Name"), None);
        assert_eq!(schema_lookup(FIXTURE, "NAME"), None);
        assert_eq!(schema_lookup(FIXTURE, "ACTOR"), None);
    }
}
