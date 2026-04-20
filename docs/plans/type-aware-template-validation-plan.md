# Type-Aware Template Validation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move Prosaic template type validation from runtime to compile-time (via `prosaic_template!`) and registration-time (via `Engine`), by introducing a shared `ValueType` + `PipeSpec` registry, a `HasProsaicSchema` trait, and per-slot compile-time assertions.

**Architecture:** Extract a minimal `no_std` `prosaic-common` crate that owns `ValueType`, the `PIPE_SPECS` registry, and `const fn` helpers (`schema_lookup`, `types_compatible`, `byte_eq`). `prosaic-core` depends on it and re-exports the public surface, adds a `HasProsaicSchema` trait and a `Template::infer_types()` method, and performs pipe-chain validation inside `register_template_*`. `prosaic-derive` extends `#[derive(IntoContext)]` to also emit `HasProsaicSchema`, and extends `prosaic_template!` with an optional `context:` argument that emits per-slot `const` assertions calling `schema_lookup` + `types_compatible`.

**Tech Stack:** Rust 2024 edition, `no_std`-compatible core + common crates, `syn` / `quote` proc-macro crate, `trybuild` for compile-fail tests.

**Spec:** See `docs/plans/type-aware-template-validation.md` (revised v2) for the design reference.

**Working directory:** This plan touches three crates and adds one. The implementer should create a worktree (e.g. via `git worktree add ../nlg-typeaware feature/type-aware-validation`) before starting, so partial work does not collide with other plans in flight on `main`.

---

## File Structure

### New files
- `prosaic-common/Cargo.toml` — new minimal crate, `no_std`, no alloc.
- `prosaic-common/src/lib.rs` — `ValueType`, `PipeSpec`, `PIPE_SPECS`, `pipe_spec`, `byte_eq`, `types_compatible`, `schema_lookup`, all `const fn`.
- `prosaic-core/tests/trybuild.rs` — `trybuild` entry point.
- `prosaic-core/tests/ui/chain_mismatch.rs` + `.stderr` — compile-fail fixture.
- `prosaic-core/tests/ui/context_mismatch.rs` + `.stderr` — compile-fail fixture.
- `prosaic-core/tests/ui/multi_mention_conflict.rs` + `.stderr` — compile-fail fixture.
- `prosaic-core/tests/ui/missing_slot_in_schema.rs` + `.stderr` — compile-fail fixture.
- `prosaic-core/tests/ui/context_ok.rs` — compile-pass fixture (sanity check).

### Modified files
- `Cargo.toml` (workspace root) — add `prosaic-common` to members.
- `prosaic-core/Cargo.toml` — add `prosaic-common` dep; add `trybuild` dev-dep.
- `prosaic-core/src/lib.rs` — re-export `ValueType`, `PipeSpec`, `PIPE_SPECS`, `HasProsaicSchema`.
- `prosaic-core/src/context.rs` — define `HasProsaicSchema` trait.
- `prosaic-core/src/template.rs` — add `Template::infer_types()`, associated test module.
- `prosaic-core/src/engine.rs` — call `infer_types` from `register_template_with_language_at`; add `register_template_with_schema<T: HasProsaicSchema>`.
- `prosaic-derive/src/lib.rs` — extend `derive_into_context` to also emit `HasProsaicSchema`; extend `prosaic_template!` parser + expansion to accept optional `context: <path>` and emit per-slot `const` assertions.

---

## Task Dependency Map

Phases run roughly sequentially. Within a phase, later tasks depend on earlier ones.

- Phase 1 (Tasks 1–6): `prosaic-common` crate. Must finish before Phase 2.
- Phase 2 (Tasks 7–9): `HasProsaicSchema` trait + derive extension. Depends on Phase 1.
- Phase 3 (Tasks 10–11): `Template::infer_types` + engine register-time check. Depends on Phase 1.
- Phase 4 (Tasks 12–15): `prosaic_template!` macro `context:` arg + trybuild harness. Depends on Phases 2 and 3.
- Phase 5 (Task 16): `register_template_with_schema<T>`. Depends on Phase 3.
- Phase 6 (Task 17): Docs + CHANGELOG.

---

## Phase 1 — `prosaic-common` crate

### Task 1: Create `prosaic-common` crate skeleton

**Files:**
- Create: `prosaic-common/Cargo.toml`
- Create: `prosaic-common/src/lib.rs`
- Modify: `Cargo.toml` (workspace root) — add `prosaic-common` to `[workspace] members`.

- [ ] **Step 1: Add the crate to the workspace**

Edit `Cargo.toml` at the repo root. Inside `[workspace] members = [...]` add `"prosaic-common"` as the first entry:

```toml
[workspace]
resolver = "2"
members = [
    "prosaic-common",
    "prosaic-core",
    "prosaic-grammar-de",
    # ... existing entries unchanged
]
```

- [ ] **Step 2: Create `prosaic-common/Cargo.toml`**

```toml
[package]
name = "prosaic-common"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "Shared type metadata for prosaic-core and prosaic-derive: ValueType, PipeSpec registry, and const-eval schema helpers."

[dependencies]
```

No dependencies. No features. This crate must compile on `no_std` without `alloc`.

- [ ] **Step 3: Create `prosaic-common/src/lib.rs` with `#![no_std]`**

```rust
//! Shared type metadata for the prosaic template engine.
//!
//! Owns `ValueType`, the `PIPE_SPECS` registry, and const-eval helpers used
//! by both `prosaic-core` (at runtime) and `prosaic-derive` (at macro
//! expansion time). This crate is `no_std` and has no dependencies so it
//! can be included anywhere the other two crates run.

#![no_std]
```

- [ ] **Step 4: Verify the crate compiles**

Run: `cargo check -p prosaic-common`
Expected: `Checking prosaic-common v0.4.0` then `Finished` with no warnings.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml prosaic-common/Cargo.toml prosaic-common/src/lib.rs
git commit -m "feat(common): add prosaic-common crate skeleton"
```

---

### Task 2: Add `ValueType` enum

**Files:**
- Modify: `prosaic-common/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Append to `prosaic-common/src/lib.rs`:

```rust
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
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p prosaic-common --lib value_type_tests`
Expected: FAIL with `cannot find type 'ValueType' in this scope`.

- [ ] **Step 3: Implement `ValueType`**

Add above the test module in `prosaic-common/src/lib.rs`:

```rust
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
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p prosaic-common --lib value_type_tests`
Expected: PASS (three tests).

- [ ] **Step 5: Commit**

```bash
git add prosaic-common/src/lib.rs
git commit -m "feat(common): add ValueType enum"
```

---

### Task 3: Add `PipeSpec` struct and `PIPE_SPECS` registry

**Files:**
- Modify: `prosaic-common/src/lib.rs`

- [ ] **Step 1: Write the failing tests**

Append to `prosaic-common/src/lib.rs`:

```rust
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
        assert!(SPEC.is_some());
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p prosaic-common --lib pipe_spec_tests`
Expected: FAIL with `cannot find type 'PipeSpec'` etc.

- [ ] **Step 3: Implement `PipeSpec`, `PIPE_SPECS`, and `pipe_spec`**

Add above the test module in `prosaic-common/src/lib.rs`:

```rust
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
pub const PIPE_SPECS: &[PipeSpec] = &[
    PipeSpec { name: "pluralize",     input: ValueType::Number, output: ValueType::String },
    PipeSpec { name: "plural",        input: ValueType::Number, output: ValueType::String },
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
```

Note: `pipe_spec` calls `byte_eq` which we will add in Task 5. The test will not compile until Task 5 is done, so this task's test run happens *after* Task 5. To keep each task commit green, add `byte_eq` now as a small helper in this task:

```rust
/// Byte-wise equality, usable in `const fn` (unlike `str::eq`).
/// Internal helper — not part of the public contract.
pub const fn byte_eq(a: &[u8], b: &[u8]) -> bool {
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
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p prosaic-common --lib pipe_spec_tests`
Expected: PASS (seven tests).

- [ ] **Step 5: Commit**

```bash
git add prosaic-common/src/lib.rs
git commit -m "feat(common): add PipeSpec registry for 19 pipes"
```

---

### Task 4: Add `types_compatible` const fn

**Files:**
- Modify: `prosaic-common/src/lib.rs`

- [ ] **Step 1: Write the failing tests**

Append to `prosaic-common/src/lib.rs`:

```rust
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
        const OK: bool = types_compatible(ValueType::Number, ValueType::Number);
        const NOT_OK: bool = types_compatible(ValueType::Number, ValueType::List);
        assert!(OK);
        assert!(!NOT_OK);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p prosaic-common --lib types_compatible_tests`
Expected: FAIL with `cannot find function 'types_compatible'`.

- [ ] **Step 3: Implement `types_compatible`**

Add in `prosaic-common/src/lib.rs` after the `PIPE_SPECS` block:

```rust
/// Returns `true` when a value of type `actual` can satisfy a slot or
/// pipe that expects type `expected`. `ValueType::Any` is compatible
/// with every concrete type in either direction; concrete types are
/// compatible only with themselves.
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
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p prosaic-common --lib types_compatible_tests`
Expected: PASS (four tests).

- [ ] **Step 5: Commit**

```bash
git add prosaic-common/src/lib.rs
git commit -m "feat(common): add types_compatible const fn"
```

---

### Task 5: Add `schema_lookup` const fn

**Files:**
- Modify: `prosaic-common/src/lib.rs`

- [ ] **Step 1: Write the failing tests**

Append to `prosaic-common/src/lib.rs`:

```rust
#[cfg(test)]
mod schema_lookup_tests {
    use super::*;

    const FIXTURE: &[(&str, ValueType)] = &[
        ("name", ValueType::String),
        ("count", ValueType::Number),
        ("items", ValueType::List),
    ];

    #[test]
    fn found_key_returns_type() {
        assert_eq!(schema_lookup(FIXTURE, "name"), Some(ValueType::String));
        assert_eq!(schema_lookup(FIXTURE, "count"), Some(ValueType::Number));
        assert_eq!(schema_lookup(FIXTURE, "items"), Some(ValueType::List));
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
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p prosaic-common --lib schema_lookup_tests`
Expected: FAIL with `cannot find function 'schema_lookup'`.

- [ ] **Step 3: Implement `schema_lookup`**

Add in `prosaic-common/src/lib.rs`:

```rust
/// Look up a slot's declared [`ValueType`] in a `HasProsaicSchema`-style
/// schema slice. `const fn` so it is usable inside compile-time assertion
/// blocks emitted by the `prosaic_template!` macro.
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
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p prosaic-common --lib schema_lookup_tests`
Expected: PASS (five tests).

- [ ] **Step 5: Commit**

```bash
git add prosaic-common/src/lib.rs
git commit -m "feat(common): add schema_lookup const fn"
```

---

### Task 6: Wire `prosaic-common` into `prosaic-core`, re-export public surface

**Files:**
- Modify: `prosaic-core/Cargo.toml`
- Modify: `prosaic-core/src/lib.rs`

- [ ] **Step 1: Add the dependency**

In `prosaic-core/Cargo.toml`, under `[dependencies]`, add:

```toml
prosaic-common = { path = "../prosaic-common", default-features = false }
```

- [ ] **Step 2: Add a re-export sanity test**

Append to `prosaic-core/src/lib.rs` (before the existing `#[cfg_attr]` or near the other `pub use` lines):

```rust
pub use prosaic_common::{PipeSpec, ValueType, pipe_spec, schema_lookup, types_compatible, PIPE_SPECS};
```

And add a small test module at the end of `prosaic-core/src/lib.rs`:

```rust
#[cfg(test)]
mod common_reexport_tests {
    use super::*;

    #[test]
    fn value_type_is_reexported() {
        let _ = ValueType::Number;
    }

    #[test]
    fn pipe_specs_is_reexported_with_all_pipes() {
        assert_eq!(PIPE_SPECS.len(), 19);
    }

    #[test]
    fn types_compatible_is_reexported() {
        assert!(types_compatible(ValueType::Any, ValueType::Number));
    }
}
```

- [ ] **Step 3: Run the tests**

Run: `cargo test -p prosaic-core --lib common_reexport_tests`
Expected: PASS (three tests).

- [ ] **Step 4: Confirm the whole workspace still builds**

Run: `cargo check --workspace`
Expected: `Finished` with no errors.

- [ ] **Step 5: Commit**

```bash
git add prosaic-core/Cargo.toml prosaic-core/src/lib.rs
git commit -m "feat(core): depend on prosaic-common and re-export type surface"
```

---

## Phase 2 — `HasProsaicSchema` trait + derive extension

### Task 7: Define `HasProsaicSchema` trait in `prosaic-core`

**Files:**
- Modify: `prosaic-core/src/context.rs`
- Modify: `prosaic-core/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Append to `prosaic-core/src/context.rs`, inside a new test module at the end of the file:

```rust
#[cfg(test)]
mod has_schema_tests {
    use super::*;
    use prosaic_common::{ValueType, schema_lookup};

    struct Manual;

    impl HasProsaicSchema for Manual {
        const PROSAIC_SCHEMA: &'static [(&'static str, ValueType)] = &[
            ("count", ValueType::Number),
            ("name", ValueType::String),
        ];
    }

    #[test]
    fn manual_impl_exposes_schema() {
        assert_eq!(Manual::PROSAIC_SCHEMA.len(), 2);
    }

    #[test]
    fn schema_is_const_queryable() {
        const T: Option<ValueType> =
            schema_lookup(<Manual as HasProsaicSchema>::PROSAIC_SCHEMA, "count");
        assert_eq!(T, Some(ValueType::Number));
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p prosaic-core --lib has_schema_tests`
Expected: FAIL with `cannot find trait 'HasProsaicSchema'`.

- [ ] **Step 3: Define the trait**

Add to `prosaic-core/src/context.rs`, after the existing `IntoContext` definitions:

```rust
use prosaic_common::ValueType;

/// Compile-time schema for a context type.
///
/// Deriving `#[derive(IntoContext)]` automatically implements this trait
/// using the field names and Rust types of the struct. Hand-written impls
/// are also supported for types that need `ValueType::Entity` slots or
/// other mappings the derive does not produce.
///
/// The `PROSAIC_SCHEMA` constant is queryable at const evaluation time, so
/// the `prosaic_template!` macro can emit per-slot assertions against it
/// using [`prosaic_common::schema_lookup`] and
/// [`prosaic_common::types_compatible`].
pub trait HasProsaicSchema {
    const PROSAIC_SCHEMA: &'static [(&'static str, ValueType)];
}
```

- [ ] **Step 4: Re-export the trait**

In `prosaic-core/src/lib.rs`, add to the block that re-exports from `context`:

```rust
pub use context::{Context, EntityValue, HasProsaicSchema, IntoValue, Value, entity};
```

(Replace the existing `pub use context::{Context, EntityValue, IntoValue, Value, entity};` line.)

- [ ] **Step 5: Run the tests**

Run: `cargo test -p prosaic-core --lib has_schema_tests`
Expected: PASS (two tests).

- [ ] **Step 6: Commit**

```bash
git add prosaic-core/src/context.rs prosaic-core/src/lib.rs
git commit -m "feat(core): define HasProsaicSchema trait"
```

---

### Task 8: Extend `#[derive(IntoContext)]` to also implement `HasProsaicSchema`

**Files:**
- Modify: `prosaic-derive/src/lib.rs`
- Modify: `prosaic-core/src/context.rs` (test only)

- [ ] **Step 1: Write the failing test**

Append to `prosaic-core/src/context.rs`, inside the `has_schema_tests` module:

```rust
    #[test]
    fn derived_schema_matches_fields() {
        use prosaic_derive::IntoContext;

        #[derive(IntoContext)]
        #[allow(dead_code)]
        struct Doc {
            name: String,
            count: i64,
            tags: Vec<String>,
        }

        let schema = <Doc as HasProsaicSchema>::PROSAIC_SCHEMA;
        assert_eq!(schema.len(), 3);
        assert_eq!(schema_lookup(schema, "name"), Some(ValueType::String));
        assert_eq!(schema_lookup(schema, "count"), Some(ValueType::Number));
        assert_eq!(schema_lookup(schema, "tags"), Some(ValueType::List));
    }

    #[test]
    fn derived_schema_maps_wide_numerics_to_number() {
        use prosaic_derive::IntoContext;

        #[derive(IntoContext)]
        #[allow(dead_code)]
        struct Sizes {
            big: u64,
            arch: usize,
            small: u8,
        }

        let schema = <Sizes as HasProsaicSchema>::PROSAIC_SCHEMA;
        assert_eq!(schema_lookup(schema, "big"), Some(ValueType::Number));
        assert_eq!(schema_lookup(schema, "arch"), Some(ValueType::Number));
        assert_eq!(schema_lookup(schema, "small"), Some(ValueType::Number));
    }

    #[test]
    fn derived_schema_handles_str_reference() {
        use prosaic_derive::IntoContext;

        #[derive(IntoContext)]
        #[allow(dead_code)]
        struct Borrowed<'a> {
            label: &'a str,
        }

        let schema = <Borrowed as HasProsaicSchema>::PROSAIC_SCHEMA;
        assert_eq!(schema_lookup(schema, "label"), Some(ValueType::String));
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p prosaic-core --lib has_schema_tests`
Expected: FAIL with `HasProsaicSchema is not implemented for Doc` (or similar).

- [ ] **Step 3: Extend `derive_into_context` to also emit a `HasProsaicSchema` impl**

In `prosaic-derive/src/lib.rs`, modify the `derive_into_context` function. After the existing loop that builds `insertions`, add a parallel loop that builds `schema_entries`, then emit the second impl alongside the existing one.

Replace the body of `derive_into_context` (starting at `let mut insertions = ...`) with:

```rust
    let mut insertions = Vec::with_capacity(fields.len());
    let mut schema_entries = Vec::with_capacity(fields.len());

    for field in fields {
        let field_name = match &field.ident {
            Some(ident) => ident,
            None => continue,
        };
        let key = field_name.to_string();
        let ty = &field.ty;

        // Value-type mapping (for schema) — unwraps Option<T> to T.
        let effective_ty = extract_option_inner(ty).unwrap_or(ty);
        let value_type_tokens = match value_type_for_rust_type(effective_ty) {
            Some(t) => t,
            None => return unsupported_field_error(field_name, ty, false),
        };
        schema_entries.push(quote! { (#key, #value_type_tokens) });

        // IntoContext insertion (unchanged).
        let conversion = if let Some(inner_ty) = extract_option_inner(ty) {
            match value_conversion_for_type(inner_ty, &quote!(val)) {
                Some(conv) => quote! {
                    if let ::core::option::Option::Some(val) = self.#field_name {
                        ctx.insert(#key, #conv);
                    }
                },
                None => return unsupported_field_error(field_name, inner_ty, true),
            }
        } else {
            match value_conversion_for_type(ty, &quote!(self.#field_name)) {
                Some(conv) => quote! {
                    ctx.insert(#key, #conv);
                },
                None => return unsupported_field_error(field_name, ty, false),
            }
        };

        insertions.push(conversion);
    }

    let expanded = quote! {
        impl #impl_generics ::prosaic_core::IntoContext for #name #ty_generics #where_clause {
            fn into_context(self) -> ::prosaic_core::Context {
                let mut ctx = ::prosaic_core::Context::new();
                #(#insertions)*
                ctx
            }
        }

        impl #impl_generics ::prosaic_core::HasProsaicSchema for #name #ty_generics #where_clause {
            const PROSAIC_SCHEMA: &'static [(&'static str, ::prosaic_core::ValueType)] = &[
                #(#schema_entries),*
            ];
        }
    };

    TokenStream::from(expanded)
```

- [ ] **Step 4: Add the `value_type_for_rust_type` helper**

Add at the bottom of `prosaic-derive/src/lib.rs`, alongside the other helpers:

```rust
/// Map a Rust field type to the `ValueType` it projects into when inserted
/// into a `Context`. Returns `None` if the type is unsupported (caller
/// raises `unsupported_field_error`).
fn value_type_for_rust_type(ty: &Type) -> Option<proc_macro2::TokenStream> {
    if is_type(ty, "String") || is_str_reference(ty) {
        Some(quote! { ::prosaic_core::ValueType::String })
    } else if is_safe_numeric_type(ty) || is_wide_numeric_type(ty) || is_type(ty, "bool") {
        Some(quote! { ::prosaic_core::ValueType::Number })
    } else if is_vec_string(ty) {
        Some(quote! { ::prosaic_core::ValueType::List })
    } else {
        None
    }
}
```

Note on `bool`: the existing `IntoValue for bool` impl maps to `Value::Number(0|1)`, so the schema type must match — `ValueType::Number`.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p prosaic-core --lib has_schema_tests`
Expected: PASS (five tests total in this module now).

- [ ] **Step 6: Commit**

```bash
git add prosaic-derive/src/lib.rs prosaic-core/src/context.rs
git commit -m "feat(derive): emit HasProsaicSchema impl from IntoContext derive"
```

---

### Task 9: Regression test — all existing `IntoContext` tests still pass

**Files:**
- No file changes.

- [ ] **Step 1: Run the full `prosaic-core` + `prosaic-derive` test suites**

Run: `cargo test -p prosaic-core -p prosaic-derive`
Expected: All prior tests pass. The schema emission must not regress `IntoContext` behavior.

- [ ] **Step 2: Run the full workspace test suite**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 3: No commit**

This is a verification-only task. If any tests fail, stop and fix before continuing.

---

## Phase 3 — `Template::infer_types` + engine register-time chain check

### Task 10: Add `Template::infer_types()`

**Files:**
- Modify: `prosaic-core/src/template.rs`

- [ ] **Step 1: Write the failing tests**

Append to the existing `#[cfg(test)] mod tests` block at the bottom of `prosaic-core/src/template.rs`:

```rust
    // ── infer_types tests ───────────────────────────────────────────────

    use prosaic_common::ValueType;

    fn types(t: &Template) -> Vec<(String, ValueType)> {
        let mut v = t.infer_types().expect("expected successful inference");
        v.sort_by(|a, b| a.0.cmp(&b.0));
        v
    }

    #[test]
    fn infer_bare_slot_is_any() {
        let t = Template::parse("{x}").unwrap();
        assert_eq!(types(&t), vec![("x".into(), ValueType::Any)]);
    }

    #[test]
    fn infer_slot_with_number_pipe_is_number() {
        let t = Template::parse("{count|pluralize:item}").unwrap();
        assert_eq!(types(&t), vec![("count".into(), ValueType::Number)]);
    }

    #[test]
    fn infer_slot_with_list_chain_is_list() {
        let t = Template::parse("{items|truncate:3|join}").unwrap();
        assert_eq!(types(&t), vec![("items".into(), ValueType::List)]);
    }

    #[test]
    fn infer_chain_mismatch_is_error() {
        let t = Template::parse("{x|capitalize|pluralize}").unwrap();
        let err = t.infer_types().unwrap_err();
        assert!(err.contains("capitalize"), "error was: {err}");
        assert!(err.contains("pluralize"), "error was: {err}");
    }

    #[test]
    fn infer_multi_mention_any_and_number_unifies_to_number() {
        let t = Template::parse("{x|pluralize:item} {x}").unwrap();
        assert_eq!(types(&t), vec![("x".into(), ValueType::Number)]);
    }

    #[test]
    fn infer_multi_mention_conflict_is_error() {
        let t = Template::parse("{x|pluralize:item} {x|join}").unwrap();
        let err = t.infer_types().unwrap_err();
        assert!(err.contains("'x'") || err.contains("`x`"), "error was: {err}");
        assert!(err.contains("Number"), "error was: {err}");
        assert!(err.contains("List"), "error was: {err}");
    }

    #[test]
    fn infer_unknown_pipe_is_error() {
        let t = Template::parse("{x|nonexistent_pipe}").unwrap();
        let err = t.infer_types().unwrap_err();
        assert!(err.contains("nonexistent_pipe"), "error was: {err}");
    }

    #[test]
    fn infer_conditional_guard_slot_is_any() {
        let t = Template::parse("{?count}hello{/?}").unwrap();
        let ts = types(&t);
        assert_eq!(ts, vec![("count".into(), ValueType::Any)]);
    }

    #[test]
    fn infer_pipes_inside_conditional_are_checked() {
        let t = Template::parse("{?count}{count|pluralize:item}{/?}").unwrap();
        assert_eq!(types(&t), vec![("count".into(), ValueType::Number)]);
    }

    #[test]
    fn infer_literal_only_is_empty() {
        let t = Template::parse("no slots").unwrap();
        assert_eq!(types(&t), vec![]);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p prosaic-core --lib template::tests::infer`
Expected: FAIL with `cannot find method 'infer_types'`.

- [ ] **Step 3: Implement `Template::infer_types`**

Add to `prosaic-core/src/template.rs`, inside `impl Template { ... }` (insert near `slot_keys` / `pipe_names`):

```rust
    /// Infer the [`ValueType`] required for each slot, based on pipe-chain flow.
    ///
    /// Walks every slot (including condition keys in `{?key}...{/?}` and
    /// slots nested inside conditionals). For each slot:
    /// - If it is used bare, its inferred type is `Any`.
    /// - If its first pipe has input type `T`, the slot type is `T`.
    /// - Every downstream pipe's input must match the previous pipe's output
    ///   (using [`types_compatible`]); mismatch returns an `Err`.
    /// - When a slot appears multiple times, the inferred types are **unified**
    ///   by intersection: `Any ∩ T → T`, `T ∩ T → T`, and two distinct
    ///   concrete types produce an `Err`.
    ///
    /// Unknown pipe names produce an `Err`. Slots inside `{>partial}` are
    /// skipped (partials are opaque at parse time).
    ///
    /// Returns a `Vec<(slot_name, inferred_type)>` in unspecified order on
    /// success, or a human-readable error string on conflict.
    pub fn infer_types(&self) -> Result<Vec<(String, ValueType)>, String> {
        let mut by_slot: Vec<(String, ValueType)> = Vec::new();
        infer_segments(&self.segments, &mut by_slot)?;
        Ok(by_slot)
    }
```

Also add (at module level, outside the `impl` block):

```rust
use prosaic_common::{PipeSpec, ValueType, pipe_spec, types_compatible};

fn infer_segments(
    segments: &[Segment],
    out: &mut Vec<(String, ValueType)>,
) -> Result<(), String> {
    for seg in segments {
        match seg {
            Segment::Literal(_) | Segment::Partial { .. } => {}
            Segment::Slot { key, pipes } => {
                let slot_ty = slot_type_from_pipes(key, pipes)?;
                unify(out, key, slot_ty)?;
            }
            Segment::Conditional { condition_key, inner } => {
                unify(out, condition_key, ValueType::Any)?;
                infer_segments(inner, out)?;
            }
        }
    }
    Ok(())
}

fn slot_type_from_pipes(key: &str, pipes: &[Pipe]) -> Result<ValueType, String> {
    // Bare slot: unconstrained.
    let Some(first) = pipes.first() else {
        return Ok(ValueType::Any);
    };

    let first_spec = lookup_spec(&first.name)?;
    let slot_ty = first_spec.input;
    let mut current_output = first_spec.output;
    let mut prev_name: &str = &first.name;

    for next in &pipes[1..] {
        let next_spec = lookup_spec(&next.name)?;
        if !types_compatible(current_output, next_spec.input) {
            return Err(format!(
                "pipe chain mismatch on slot `{key}`: \
                 pipe `{prev_name}` outputs {current_output:?} but pipe `{cur}` expects {expected:?}",
                cur = next.name,
                expected = next_spec.input,
            ));
        }
        current_output = next_spec.output;
        prev_name = &next.name;
    }

    Ok(slot_ty)
}

fn lookup_spec(name: &str) -> Result<&'static PipeSpec, String> {
    pipe_spec(name).ok_or_else(|| format!("unknown pipe `{name}`"))
}

fn unify(
    out: &mut Vec<(String, ValueType)>,
    key: &str,
    ty: ValueType,
) -> Result<(), String> {
    if let Some(entry) = out.iter_mut().find(|(k, _)| k == key) {
        entry.1 = match (entry.1, ty) {
            (ValueType::Any, t) | (t, ValueType::Any) => t,
            (a, b) if a == b => a,
            (a, b) => {
                return Err(format!(
                    "slot `{key}` has conflicting types: used as both {a:?} and {b:?}"
                ));
            }
        };
    } else {
        out.push((key.to_string(), ty));
    }
    Ok(())
}
```

Note: `prev_name` uses `core::ptr::eq`, which requires `use core::ptr;` at the top if not present — `core` is the crate's `no_std` path and already in scope under edition 2024. No explicit `use` needed.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p prosaic-core --lib template::tests`
Expected: PASS — all prior template tests plus the ten new `infer_*` tests.

- [ ] **Step 5: Commit**

```bash
git add prosaic-core/src/template.rs
git commit -m "feat(core): add Template::infer_types for pipe chain validation"
```

---

### Task 11: Run `infer_types` at register time in the engine

**Files:**
- Modify: `prosaic-core/src/engine.rs`

- [ ] **Step 1: Write the failing test**

Append to the existing test module in `prosaic-core/src/engine.rs` (find `#[cfg(test)] mod tests` — there may be multiple). Add a new module at the end of the file:

```rust
#[cfg(test)]
mod register_template_type_tests {
    use super::*;
    use prosaic_grammar_en::English;

    #[test]
    fn register_template_rejects_chain_mismatch() {
        let mut engine = Engine::new(English::new());
        let err = engine
            .register_template("bad", "{x|capitalize|pluralize}")
            .unwrap_err();
        match err {
            ProsaicError::TemplateParseError { reason, .. } => {
                assert!(
                    reason.contains("chain mismatch"),
                    "unexpected reason: {reason}"
                );
            }
            other => panic!("expected TemplateParseError, got {other:?}"),
        }
    }

    #[test]
    fn register_template_rejects_multi_mention_conflict() {
        let mut engine = Engine::new(English::new());
        let err = engine
            .register_template("bad", "{x|pluralize:item} and {x|join}")
            .unwrap_err();
        assert!(matches!(err, ProsaicError::TemplateParseError { .. }));
    }

    #[test]
    fn register_template_accepts_valid_template() {
        let mut engine = Engine::new(English::new());
        engine
            .register_template("good", "The {name} has {count|pluralize:item}")
            .unwrap();
    }

    #[test]
    fn register_template_accepts_bare_slots() {
        // No pipes => no chain checks to fail.
        let mut engine = Engine::new(English::new());
        engine.register_template("bare", "Hello {name}").unwrap();
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p prosaic-core --lib register_template_type_tests`
Expected: FAIL — `register_template_rejects_chain_mismatch` fails because today `register_template` accepts the template (chain mismatch only surfaces at render time).

- [ ] **Step 3: Update `register_template_with_language_at` to call `infer_types`**

In `prosaic-core/src/engine.rs`, find `register_template_with_language_at` (around line 1858) and change it to:

```rust
    pub fn register_template_with_language_at(
        &mut self,
        key: &str,
        source: &str,
        salience: Salience,
        language: Option<&str>,
    ) -> Result<(), ProsaicError> {
        let template = Template::parse(source)?;

        // Chain-level sanity check: pipe n's output must match pipe n+1's input,
        // and multi-mention slots must unify. This turns what used to be a
        // render-time `InvalidPipe` into a register-time `TemplateParseError`.
        template.infer_types().map_err(|reason| ProsaicError::TemplateParseError {
            template: source.to_string(),
            position: 0,
            reason,
        })?;

        self.templates.entry(key.to_string()).or_default().push(
            SalientTemplate::new(salience, template, language.map(|s| s.to_string())),
        );
        self.rr_initial.entry(key.to_string()).or_insert(0);
        Ok(())
    }
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p prosaic-core --lib register_template_type_tests`
Expected: PASS (four tests).

- [ ] **Step 5: Run the full `prosaic-core` suite**

Run: `cargo test -p prosaic-core`
Expected: PASS — no regressions. If any vocab or grammar test registers an invalid template that was previously tolerated, this task surfaces it. **Treat those as bugs to fix**, not as test churn to suppress — the vocab was silently broken.

- [ ] **Step 6: Commit**

```bash
git add prosaic-core/src/engine.rs
git commit -m "feat(core): validate pipe chains at register_template time"
```

---

## Phase 4 — Macro `context:` argument + trybuild harness

### Task 12: Accept optional `context: <path>` argument in `prosaic_template!`

**Files:**
- Modify: `prosaic-derive/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Add a new file `prosaic-core/src/template_macro_tests.rs` (we register it as a module only for tests):

In `prosaic-core/src/lib.rs`, add near the other `#[cfg(test)]` items at the end:

```rust
#[cfg(test)]
mod template_macro_tests;
```

Create `prosaic-core/src/template_macro_tests.rs`:

```rust
//! Integration tests for the `prosaic_template!` macro, including the
//! `context:` argument added in the type-aware validation plan.

use prosaic_derive::{IntoContext, prosaic_template};

#[derive(IntoContext)]
#[allow(dead_code)]
struct SimpleCtx {
    name: String,
    count: i64,
}

#[test]
fn macro_accepts_context_argument_when_types_align() {
    let tpl = prosaic_template! {
        template: "{name} has {count|pluralize:item}",
        slots: [name, count],
        context: SimpleCtx,
    };
    assert!(tpl.contains("{name}"));
    assert!(tpl.contains("{count|pluralize:item}"));
}

#[test]
fn macro_without_context_still_works() {
    let tpl = prosaic_template! {
        template: "{name} has {count|pluralize:item}",
        slots: [name, count],
    };
    assert!(tpl.contains("{count|pluralize:item}"));
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p prosaic-core --test-threads=1 template_macro_tests`

Actually prefer: `cargo test -p prosaic-core --lib template_macro_tests`
Expected: FAIL with `unknown key 'context'` from the macro parser.

- [ ] **Step 3: Extend the macro input parser to accept `context:`**

In `prosaic-derive/src/lib.rs`, replace the `ProsaicTemplateInput` struct and its `Parse` impl with:

```rust
struct ProsaicTemplateInput {
    template: LitStr,
    slots: Vec<Ident>,
    context: Option<syn::Path>,
}

impl Parse for ProsaicTemplateInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut template: Option<LitStr> = None;
        let mut slots: Option<Vec<Ident>> = None;
        let mut context: Option<syn::Path> = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![:]>()?;
            match key.to_string().as_str() {
                "template" => {
                    template = Some(input.parse::<LitStr>()?);
                }
                "slots" => {
                    let content;
                    syn::bracketed!(content in input);
                    let parsed_idents: Punctuated<Ident, Token![,]> =
                        Punctuated::parse_terminated(&content)?;
                    slots = Some(parsed_idents.into_iter().collect());
                }
                "context" => {
                    context = Some(input.parse::<syn::Path>()?);
                }
                other => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!(
                            "unknown key `{other}` — expected `template`, `slots`, or `context`"
                        ),
                    ));
                }
            }
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }

        let template = template
            .ok_or_else(|| syn::Error::new(input.span(), "missing `template: \"...\"` argument"))?;
        let slots = slots.unwrap_or_default();

        Ok(ProsaicTemplateInput { template, slots, context })
    }
}
```

- [ ] **Step 4: Change the `prosaic_template` expansion to ignore `context` for now**

For this task, the macro must still accept `context:` without using it. `validate_template` remains unchanged (only uses `template` and `slots`). Task 13 will wire up the assertion emission.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p prosaic-core --lib template_macro_tests`
Expected: PASS (two tests).

- [ ] **Step 6: Commit**

```bash
git add prosaic-derive/src/lib.rs prosaic-core/src/lib.rs prosaic-core/src/template_macro_tests.rs
git commit -m "feat(derive): accept optional context argument in prosaic_template!"
```

---

### Task 13: Emit per-slot `const` assertions when `context:` is provided

**Files:**
- Modify: `prosaic-derive/src/lib.rs`
- Modify: `prosaic-core/src/template_macro_tests.rs`

- [ ] **Step 1: Write the failing pass test**

Append to `prosaic-core/src/template_macro_tests.rs`:

```rust
#[test]
fn macro_with_context_and_matching_types_compiles_and_runs() {
    #[derive(IntoContext)]
    #[allow(dead_code)]
    struct Ok1 {
        count: i64,
        items: Vec<String>,
    }

    let _tpl = prosaic_template! {
        template: "{count|pluralize:item}: {items|truncate:3|join}",
        slots: [count, items],
        context: Ok1,
    };
}

#[test]
fn macro_with_context_unifies_any_and_concrete() {
    // Slot is used once bare and once with a pipe — should unify to Number.
    #[derive(IntoContext)]
    #[allow(dead_code)]
    struct Ok2 {
        x: i64,
    }

    let _tpl = prosaic_template! {
        template: "{x} items ({x|pluralize:item})",
        slots: [x],
        context: Ok2,
    };
}
```

- [ ] **Step 2: Run the tests to verify they currently pass (vacuously — no assertion emitted yet)**

Run: `cargo test -p prosaic-core --lib template_macro_tests`
Expected: PASS. This baseline proves the test harness is set up; the real check is in Step 5 (compile-fail tests) and Task 14 (trybuild).

- [ ] **Step 3: Wire `validate_template` to emit per-slot assertions**

In `prosaic-derive/src/lib.rs`, change `validate_template` to also return a `TokenStream2` of assertions, and change `prosaic_template` to splice them into the expansion.

Replace `validate_template` with:

```rust
fn validate_template(
    input: &ProsaicTemplateInput,
) -> syn::Result<proc_macro2::TokenStream> {
    let template_str = input.template.value();
    let span = input.template.span();

    let parsed = prosaic_core::Template::parse(&template_str)
        .map_err(|e| syn::Error::new(span, format!("invalid template: {e}")))?;

    let declared: std::collections::HashSet<String> =
        input.slots.iter().map(|i| i.to_string()).collect();

    validate_slots(&parsed, &declared, span)?;
    validate_pipes(&parsed, span)?;

    // Infer per-slot types using the shared PIPE_SPECS registry. Chain
    // mismatches and multi-mention conflicts surface here as compile errors.
    let inferred = parsed
        .infer_types()
        .map_err(|reason| syn::Error::new(span, reason))?;

    let assertions = match &input.context {
        Some(ctx_path) => emit_context_assertions(ctx_path, &inferred),
        None => proc_macro2::TokenStream::new(),
    };

    Ok(assertions)
}

fn emit_context_assertions(
    ctx_path: &syn::Path,
    inferred: &[(String, prosaic_common::ValueType)],
) -> proc_macro2::TokenStream {
    use prosaic_common::ValueType;

    let mut stmts = proc_macro2::TokenStream::new();
    for (slot, expected) in inferred {
        let expected_tok = match expected {
            ValueType::String => quote! { ::prosaic_core::ValueType::String },
            ValueType::Number => quote! { ::prosaic_core::ValueType::Number },
            ValueType::List => quote! { ::prosaic_core::ValueType::List },
            ValueType::Entity => quote! { ::prosaic_core::ValueType::Entity },
            ValueType::Any => {
                // A slot inferred as Any imposes no constraint on the context.
                continue;
            }
        };

        let ctx_name_str = quote!(#ctx_path).to_string();
        let missing_msg = format!(
            "prosaic_template: slot `{slot}` is not declared in context `{ctx_name_str}` (no matching field)"
        );
        let mismatch_msg = format!(
            "prosaic_template: slot `{slot}` in context `{ctx_name_str}` has an incompatible type — required by template pipe chain"
        );

        stmts.extend(quote! {
            const _: () = {
                let actual = match ::prosaic_core::schema_lookup(
                    <#ctx_path as ::prosaic_core::HasProsaicSchema>::PROSAIC_SCHEMA,
                    #slot,
                ) {
                    ::core::option::Option::Some(t) => t,
                    ::core::option::Option::None => ::core::panic!(#missing_msg),
                };
                if !::prosaic_core::types_compatible(actual, #expected_tok) {
                    ::core::panic!(#mismatch_msg);
                }
            };
        });
    }
    stmts
}
```

Replace the body of `prosaic_template` with:

```rust
#[proc_macro]
pub fn prosaic_template(input: TokenStream) -> TokenStream {
    let parsed = parse_macro_input!(input as ProsaicTemplateInput);

    match validate_template(&parsed) {
        Ok(assertions) => {
            let lit = &parsed.template;
            quote! { { #assertions #lit } }.into()
        }
        Err(e) => e.to_compile_error().into(),
    }
}
```

The macro's top-level value stays the same (`&'static str` literal), but is wrapped in a block so the `const _: () = ...` items evaluate alongside it.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p prosaic-core --lib template_macro_tests`
Expected: PASS (four tests).

- [ ] **Step 5: Commit**

```bash
git add prosaic-derive/src/lib.rs prosaic-core/src/template_macro_tests.rs
git commit -m "feat(derive): emit per-slot const assertions for context: argument"
```

---

### Task 14: Set up `trybuild` harness + compile-fail fixtures

**Files:**
- Modify: `prosaic-core/Cargo.toml`
- Create: `prosaic-core/tests/trybuild.rs`
- Create: `prosaic-core/tests/ui/chain_mismatch.rs` + `.stderr`
- Create: `prosaic-core/tests/ui/context_mismatch.rs` + `.stderr`
- Create: `prosaic-core/tests/ui/multi_mention_conflict.rs` + `.stderr`
- Create: `prosaic-core/tests/ui/missing_slot_in_schema.rs` + `.stderr`
- Create: `prosaic-core/tests/ui/context_ok.rs`

- [ ] **Step 1: Add `trybuild` as a dev-dep**

In `prosaic-core/Cargo.toml`, under `[dev-dependencies]`, append:

```toml
trybuild = "1"
```

- [ ] **Step 2: Create the harness**

Create `prosaic-core/tests/trybuild.rs`:

```rust
#[test]
fn ui_tests() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*_fail.rs");
    t.pass("tests/ui/*_ok.rs");
}
```

- [ ] **Step 3: Create the pass fixture**

Create `prosaic-core/tests/ui/context_ok.rs`:

```rust
use prosaic_derive::{IntoContext, prosaic_template};

#[derive(IntoContext)]
#[allow(dead_code)]
struct Ctx {
    count: i64,
    items: Vec<String>,
}

fn main() {
    let _tpl = prosaic_template! {
        template: "{count|pluralize:item}: {items|truncate:3|join}",
        slots: [count, items],
        context: Ctx,
    };
}
```

- [ ] **Step 4: Create the chain-mismatch fail fixture**

Create `prosaic-core/tests/ui/chain_mismatch_fail.rs`:

```rust
use prosaic_derive::prosaic_template;

fn main() {
    let _tpl = prosaic_template! {
        template: "{x|capitalize|pluralize}",
        slots: [x],
    };
}
```

(No matching `.stderr` yet — generated in Step 9.)

- [ ] **Step 5: Create the context-mismatch fail fixture**

Create `prosaic-core/tests/ui/context_mismatch_fail.rs`:

```rust
use prosaic_derive::{IntoContext, prosaic_template};

#[derive(IntoContext)]
#[allow(dead_code)]
struct Ctx {
    count: String, // wrong — pluralize needs Number
}

fn main() {
    let _tpl = prosaic_template! {
        template: "{count|pluralize:item}",
        slots: [count],
        context: Ctx,
    };
}
```

- [ ] **Step 6: Create the multi-mention-conflict fail fixture**

Create `prosaic-core/tests/ui/multi_mention_conflict_fail.rs`:

```rust
use prosaic_derive::prosaic_template;

fn main() {
    let _tpl = prosaic_template! {
        template: "{x|pluralize:item} and {x|join}",
        slots: [x],
    };
}
```

- [ ] **Step 7: Create the missing-slot fail fixture**

Create `prosaic-core/tests/ui/missing_slot_in_schema_fail.rs`:

```rust
use prosaic_derive::{IntoContext, prosaic_template};

#[derive(IntoContext)]
#[allow(dead_code)]
struct Ctx {
    name: String,
}

fn main() {
    let _tpl = prosaic_template! {
        template: "{count|pluralize:item}",
        slots: [count],
        context: Ctx,
    };
}
```

- [ ] **Step 8: Rename the pass fixture to match the glob**

Rename `prosaic-core/tests/ui/context_ok.rs` to `prosaic-core/tests/ui/context_compiles_ok.rs` so it matches the `*_ok.rs` glob from Step 2.

```bash
git mv prosaic-core/tests/ui/context_ok.rs prosaic-core/tests/ui/context_compiles_ok.rs
```

- [ ] **Step 9: Generate the `.stderr` files**

Run: `TRYBUILD=overwrite cargo test -p prosaic-core --test trybuild`

On Windows bash this is:
```bash
TRYBUILD=overwrite cargo test -p prosaic-core --test trybuild
```

Expected: Test fails the first run (no `.stderr` files exist). The `TRYBUILD=overwrite` flag tells `trybuild` to write captured stderr into the missing `.stderr` files.

Inspect each generated file:
- `prosaic-core/tests/ui/chain_mismatch_fail.stderr` — should mention `capitalize` → `pluralize` chain.
- `prosaic-core/tests/ui/context_mismatch_fail.stderr` — should mention `count` and `Ctx`.
- `prosaic-core/tests/ui/multi_mention_conflict_fail.stderr` — should mention `x` with both `Number` and `List`.
- `prosaic-core/tests/ui/missing_slot_in_schema_fail.stderr` — should mention slot `count` not in context `Ctx`.

If any stderr does not contain the expected substrings, stop: the macro's error messages are wrong and Task 13 must be revisited.

- [ ] **Step 10: Rerun `trybuild` normally to confirm it passes against the captured stderr**

Run: `cargo test -p prosaic-core --test trybuild`
Expected: PASS.

- [ ] **Step 11: Commit**

```bash
git add prosaic-core/Cargo.toml prosaic-core/tests/trybuild.rs prosaic-core/tests/ui
git commit -m "test(core): add trybuild compile-fail fixtures for type-aware templates"
```

---

### Task 15: Full-workspace regression verification

**Files:**
- No file changes.

- [ ] **Step 1: Build everything**

Run: `cargo build --workspace --all-features`
Expected: Finished, no errors.

- [ ] **Step 2: Test everything**

Run: `cargo test --workspace --all-features`
Expected: All existing tests still pass, all new tests pass.

- [ ] **Step 3: Build on `no_std` to confirm `prosaic-common` stays `no_std`**

Run: `cargo build -p prosaic-common --no-default-features`

Then confirm `prosaic-core` still builds without default features:
`cargo build -p prosaic-core --no-default-features`

Expected: Finished, no errors.

- [ ] **Step 4: No commit**

Verification only. If anything fails, stop and fix.

---

## Phase 5 — `register_template_with_schema<T>`

### Task 16: Add `Engine::register_template_with_schema<T: HasProsaicSchema>`

**Files:**
- Modify: `prosaic-core/src/engine.rs`

- [ ] **Step 1: Write the failing test**

Append to the `register_template_type_tests` module in `prosaic-core/src/engine.rs`:

```rust
    use prosaic_derive::IntoContext;

    #[derive(IntoContext)]
    #[allow(dead_code)]
    struct GoodCtx {
        count: i64,
    }

    #[derive(IntoContext)]
    #[allow(dead_code)]
    struct BadCtx {
        count: String,
    }

    #[test]
    fn register_template_with_schema_accepts_matching_schema() {
        let mut engine = Engine::new(English::new());
        engine
            .register_template_with_schema::<GoodCtx>(
                "ok",
                "{count|pluralize:item}",
            )
            .unwrap();
    }

    #[test]
    fn register_template_with_schema_rejects_type_mismatch() {
        let mut engine = Engine::new(English::new());
        let err = engine
            .register_template_with_schema::<BadCtx>(
                "bad",
                "{count|pluralize:item}",
            )
            .unwrap_err();
        match err {
            ProsaicError::TemplateParseError { reason, .. } => {
                assert!(reason.contains("count"), "reason: {reason}");
                assert!(
                    reason.contains("Number") || reason.contains("String"),
                    "reason: {reason}"
                );
            }
            other => panic!("expected TemplateParseError, got {other:?}"),
        }
    }

    #[test]
    fn register_template_with_schema_rejects_missing_slot() {
        let mut engine = Engine::new(English::new());
        // GoodCtx has `count` but template uses `unknown_slot`.
        let err = engine
            .register_template_with_schema::<GoodCtx>(
                "missing",
                "{unknown_slot|pluralize:item}",
            )
            .unwrap_err();
        assert!(matches!(err, ProsaicError::TemplateParseError { .. }));
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p prosaic-core --lib register_template_type_tests::register_template_with_schema`
Expected: FAIL with `no method named 'register_template_with_schema'`.

- [ ] **Step 3: Implement the method**

In `prosaic-core/src/engine.rs`, inside `impl Engine` (near the other `register_template_*` methods, around line 1785), add:

```rust
    /// Register a template and cross-check every slot's inferred type
    /// against the static schema of the `T` context type.
    ///
    /// Slot types inferred from pipe chains (e.g. `{count|pluralize:item}`
    /// implies `count: Number`) must be compatible with `T`'s schema. Use
    /// this when loading templates dynamically (from JSON, disk, etc.) and
    /// you want the same strong guarantees the `prosaic_template!` macro
    /// provides at compile time.
    pub fn register_template_with_schema<T>(
        &mut self,
        key: &str,
        source: &str,
    ) -> Result<(), ProsaicError>
    where
        T: crate::HasProsaicSchema,
    {
        let template = Template::parse(source)?;
        let inferred = template
            .infer_types()
            .map_err(|reason| ProsaicError::TemplateParseError {
                template: source.to_string(),
                position: 0,
                reason,
            })?;

        for (slot, expected) in &inferred {
            let actual = crate::schema_lookup(T::PROSAIC_SCHEMA, slot).ok_or_else(|| {
                ProsaicError::TemplateParseError {
                    template: source.to_string(),
                    position: 0,
                    reason: format!(
                        "slot `{slot}` required by template is not declared in context schema"
                    ),
                }
            })?;
            if !crate::types_compatible(actual, *expected) {
                return Err(ProsaicError::TemplateParseError {
                    template: source.to_string(),
                    position: 0,
                    reason: format!(
                        "slot `{slot}` context type {actual:?} is not compatible with template-required {expected:?}"
                    ),
                });
            }
        }

        self.templates.entry(key.to_string()).or_default().push(
            SalientTemplate::new(Salience::Medium, template, None),
        );
        self.rr_initial.entry(key.to_string()).or_insert(0);
        Ok(())
    }
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p prosaic-core --lib register_template_type_tests`
Expected: PASS (all tests in the module, including the three new ones).

- [ ] **Step 5: Commit**

```bash
git add prosaic-core/src/engine.rs
git commit -m "feat(core): add register_template_with_schema for runtime schema enforcement"
```

---

## Phase 6 — Documentation

### Task 17: Update top-level docs to document type-aware validation

**Files:**
- Modify: `prosaic-core/src/lib.rs` (crate-level docs)
- Modify: `README.md` (if present and user-facing)
- Modify: `CHANGELOG.md` (if present)

- [ ] **Step 1: Check for README and CHANGELOG**

Run: `ls README.md CHANGELOG.md 2>/dev/null`
Record which files exist.

- [ ] **Step 2: Update the `prosaic-core` crate-level doc comment**

In `prosaic-core/src/lib.rs`, find the `# Quick start` section. Append a new section after it:

```rust
//! # Type-aware template validation
//!
//! Context types that derive `IntoContext` also get a `HasProsaicSchema`
//! impl for free. Pair it with the `context:` argument of
//! `prosaic_template!` to validate slot types at compile time:
//!
//! ```
//! use prosaic_core::{Context, Engine, Session, Value};
//! use prosaic_derive::{IntoContext, prosaic_template};
//! use prosaic_grammar_en::English;
//!
//! #[derive(IntoContext)]
//! struct RenameCtx {
//!     old_name: String,
//!     new_name: String,
//!     consumer_count: i64,
//! }
//!
//! // Compile error if `consumer_count` were declared as `String`:
//! let tpl = prosaic_template! {
//!     template: "{old_name} → {new_name} ({consumer_count|pluralize:consumer})",
//!     slots: [old_name, new_name, consumer_count],
//!     context: RenameCtx,
//! };
//!
//! let mut engine = Engine::new(English::new());
//! engine.register_template("rename", tpl).unwrap();
//! ```
//!
//! For templates loaded dynamically (JSON manifests, on-disk sources)
//! use `Engine::register_template_with_schema::<T>` to get the same check
//! at registration time.
```

- [ ] **Step 3: Update CHANGELOG.md if it exists**

If `CHANGELOG.md` exists, add a new entry under an `[Unreleased]` section (or create one):

```markdown
## [Unreleased]

### Added
- New `prosaic-common` crate exposing `ValueType`, `PipeSpec`, and the
  19-entry `PIPE_SPECS` registry shared between `prosaic-core` and
  `prosaic-derive`.
- `prosaic_core::HasProsaicSchema` trait; auto-derived by
  `#[derive(IntoContext)]` alongside `IntoContext`.
- `Template::infer_types()` — walks a template and returns per-slot
  inferred `ValueType`s, enforcing pipe-chain compatibility and
  multi-mention unification.
- `prosaic_template!` now accepts an optional `context: <Path>`
  argument that emits per-slot compile-time type assertions.
- `Engine::register_template_with_schema::<T>` — runtime-side sibling
  for dynamically loaded templates.

### Changed
- `Engine::register_template`/`register_template_at`/
  `register_template_with_language`/`register_template_with_language_at`
  now fail with `ProsaicError::TemplateParseError` when a template's
  pipe chain is type-incompatible or a multi-mention slot has
  conflicting requirements. Previously these only surfaced at render
  time as `ProsaicError::InvalidPipe`.
```

If `CHANGELOG.md` does not exist, skip this step.

- [ ] **Step 4: Update README.md if it exists**

If `README.md` has a "Features" section, add a bullet point:

```markdown
- Compile-time template validation: pipe chains and context field types
  are checked by `prosaic_template!` before your code runs.
```

If `README.md` does not exist, skip this step.

- [ ] **Step 5: Run the doc test**

Run: `cargo test -p prosaic-core --doc`
Expected: PASS — the new doc example in Step 2 must compile and run.

- [ ] **Step 6: Commit**

```bash
git add prosaic-core/src/lib.rs CHANGELOG.md README.md
git commit -m "docs: document type-aware template validation"
```

(Drop CHANGELOG.md / README.md from the `git add` if they do not exist.)

---

## Final Verification

### Task 18: Full workspace green-light

**Files:**
- No file changes.

- [ ] **Step 1: Run the full test suite**

Run: `cargo test --workspace --all-features`
Expected: PASS.

- [ ] **Step 2: Run tests without default features**

Run: `cargo test --workspace --no-default-features`
Expected: PASS. (Confirms `prosaic-common` is `no_std`-clean and nothing else broke under the minimal feature set.)

- [ ] **Step 3: Run clippy**

Run: `cargo clippy --workspace --all-features -- -D warnings`
Expected: No warnings. If clippy flags anything in newly-added code, fix in a small follow-up commit on this branch.

- [ ] **Step 4: Run the trybuild suite one more time**

Run: `cargo test -p prosaic-core --test trybuild`
Expected: PASS.

- [ ] **Step 5: Branch summary**

Record commit count and LOC delta:

```bash
git log --oneline main..HEAD
git diff main..HEAD --shortstat
```

Expected: ~18 commits, ~400 LOC net add. Larger is fine if tests are thorough. Smaller likely means placeholder work — investigate.

---

## Notes for the Implementer

1. **Pipe-argument validation is out of scope.** `pluralize:item`, `truncate:3`, etc. — the macro still validates the *pipe name* but not the arg. Runtime continues to return `InvalidPipe` for missing or malformed args. Do not add arg-validation in this plan.

2. **`ValueType::Entity` has no derive mapping.** `#[derive(IntoContext)]` does not emit `ValueType::Entity` for any field type — there is no Rust type that maps to it (users hand-build `Value::Entity` via the `entity()` builder). The variant exists only for hand-written `HasProsaicSchema` impls. Do not add `EntityValue` field support to the derive in this plan.

3. **`const fn` panic messages must be string literals.** Dynamic formatting with `format!` does not work in `const` blocks. The macro emits *one assertion block per slot*, with the slot name and context name baked into the literal — that is the only reason the error messages can be specific.

4. **Pre-existing vocab templates must still pass.** Task 11 may flag real pipe-chain bugs in `prosaic-vocab-*` template sources. If a vocab test fails after Task 11, that is not a regression to suppress — it is a latent bug surfaced by the new check. Investigate and fix the template, not the check.

5. **Do not bypass `trybuild` with `include_str!`.** Compile-fail tests must be real `.rs` files under `tests/ui/` so the harness can invoke the compiler directly. Snapshot-style tests in regular `#[test]` functions cannot catch macro compile errors.

6. **`prosaic-common` must stay dependency-free.** Do not add `serde`, `thiserror`, or anything else. Its entire point is to be safe to include anywhere the other crates go, including `no_std + no alloc` targets.
