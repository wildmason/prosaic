# Plan: `ctx!` Macro — Ergonomic Context Construction

**Owner:** sonnet agent
**Scope:** `nlg-core` (macro + `IntoValue` trait), no other crates
**Estimated size:** ~100–150 LOC including tests
**Test gate:** all 376 tests pass after every sub-phase; zero warnings
**Branch discipline:** local only, commits at each sub-phase

---

## Why

Every example, demo, and test today builds a `Context` like this:

```rust
let mut ctx = Context::new();
ctx.insert("entity_type", Value::String("class".into()));
ctx.insert("old_name", Value::String("UserService".into()));
ctx.insert("new_name", Value::String("AccountService".into()));
ctx.insert("consumer_count", Value::Number(6));
ctx.insert("consumers", Value::List(vec!["ProfileComponent".into(), "SettingsComponent".into()]));
```

Six lines of boilerplate per render site. The ecosystem agent identified this as the highest-ROI ergonomic win — `#[derive(IntoContext)]` already handles struct-based context building, but ad-hoc call sites (in CLI argument adapters, in tests, in inline scripts, in the demo) still fall back to the verbose form.

The target shape:

```rust
let ctx = ctx! {
    entity_type: "class",
    old_name: "UserService",
    new_name: "AccountService",
    consumer_count: 6,
    consumers: ["ProfileComponent", "SettingsComponent"],
};
```

Implemented via an `IntoValue` trait that dispatches at the type level. Callers pay zero syntax cost for the default case; `AgreementFeatures`-carrying values (not shipping in v1, but reserved in v1.5) drop in through the same trait.

## Non-goals

- **Do not** ship `entity!` helper yet — it depends on `AgreementFeatures` which is v1.5 multilingual work.
- **Do not** change `Context`, `Value`, or `IntoContext`. The macro and trait are purely additive.
- **Do not** export `IntoValue` as a user-extensible trait beyond the concrete impls we ship. `IntoValue` is pragmatic glue, not a public extension surface.
- **Do not** replace existing callers in tests/examples/demo in the same commit — that's a separate mechanical-update commit after the macro lands. Keep the feature-add and the adoption commits separate.
- **Do not** touch `nlg-derive`. The `ctx!` macro is `macro_rules!`, not a proc macro.

## Success criteria

1. `ctx! { k: v, k2: v2, ... }` compiles and returns a `Context`.
2. Value types automatically convert: `&str`, `String` → `Value::String`; integer types → `Value::Number`; array/Vec of `&str`/`String` → `Value::List`.
3. Trailing commas allowed.
4. Empty `ctx! {}` works.
5. Nested calls (`ctx! { name: some_fn_returning_string() }`) work.
6. `AgreementFeatures` / future `Value::Entity` reservation: the `IntoValue` trait is designed so adding a new `Value` variant doesn't require changing the macro.
7. All 376 existing tests still pass. New tests added for the macro's own behaviour. Final test count goes up.
8. `cargo clippy --all-features -- -D warnings` clean.
9. No public API breakage.

---

## Phase 0 — Baseline

```bash
cargo test --all-features
cargo clippy --all-features -- -D warnings
```

Expected: 376 passing, 0 warnings.

**No commit.**

---

## Phase 1 — Add `IntoValue` trait

**File:** `nlg-core/src/context.rs`

### 1.1 Trait definition

Add a new public trait below the existing `Value` impl:

```rust
/// Convert a host value into a [`Value`]. Used by the [`ctx!`] macro to
/// accept strings, integers, and slices without requiring the caller to
/// wrap each in a `Value::*` constructor.
///
/// This trait is intentionally narrow: we ship impls for the common host
/// types and do not expect third parties to extend it. For bespoke
/// types, convert to a supported primitive first (e.g. via `Display`) or
/// use the longer form `Value::String("...".into())`.
pub trait IntoValue {
    fn into_value(self) -> Value;
}
```

### 1.2 Impls

Required impls:

- `impl IntoValue for &str` → `Value::String(s.to_string())`
- `impl IntoValue for String` → `Value::String(self)`
- `impl IntoValue for &String` → `Value::String(self.clone())`
- `impl IntoValue for i8, i16, i32, i64, isize` → `Value::Number(self as i64)`
- `impl IntoValue for u8, u16, u32, usize` → `Value::Number(self as i64)` (note: `u64` excluded — overflow risk; if a caller has a `u64` they must convert)
- `impl IntoValue for bool` → `Value::Number(if self { 1 } else { 0 })`
- `impl IntoValue for Value` → `self` (identity — for explicit `Value::Foo(...)` literals)
- `impl IntoValue for Vec<String>` → `Value::List(self)`
- `impl IntoValue for Vec<&str>` → `Value::List(self.iter().map(|s| s.to_string()).collect())`
- `impl<const N: usize> IntoValue for [&str; N]` → `Value::List(self.iter().map(|s| s.to_string()).collect())`
- `impl<const N: usize> IntoValue for [String; N]` → `Value::List(self.to_vec())`

The array impls are what lets `ctx! { items: ["a", "b"] }` work without the user writing `vec![...]`.

Use `macro_rules!` internally for the integer impls to avoid a dozen copy-pasted impl blocks:

```rust
macro_rules! impl_into_value_int {
    ($($t:ty),*) => {
        $(impl IntoValue for $t {
            fn into_value(self) -> Value { Value::Number(self as i64) }
        })*
    };
}
impl_into_value_int!(i8, i16, i32, i64, isize, u8, u16, u32, usize);
```

### 1.3 Re-export from `lib.rs`

Add `IntoValue` to the existing `pub use context::{Context, IntoContext, Value};` re-export line.

### 1.4 Tests

Add a `#[cfg(test)]` module inside `context.rs`:

```rust
#[cfg(test)]
mod into_value_tests {
    use super::*;

    #[test]
    fn str_becomes_value_string() {
        let v: Value = "hello".into_value();
        assert_eq!(v, Value::String("hello".into()));
    }

    #[test]
    fn owned_string_becomes_value_string_without_clone() {
        let s = String::from("hello");
        let v: Value = s.into_value();
        assert_eq!(v, Value::String("hello".into()));
    }

    #[test]
    fn i64_becomes_value_number() {
        let v: Value = 42_i64.into_value();
        assert_eq!(v, Value::Number(42));
    }

    #[test]
    fn i32_becomes_value_number() {
        let v: Value = 42_i32.into_value();
        assert_eq!(v, Value::Number(42));
    }

    #[test]
    fn usize_becomes_value_number() {
        let v: Value = 7_usize.into_value();
        assert_eq!(v, Value::Number(7));
    }

    #[test]
    fn bool_becomes_number_zero_or_one() {
        assert_eq!(true.into_value(), Value::Number(1));
        assert_eq!(false.into_value(), Value::Number(0));
    }

    #[test]
    fn vec_of_str_becomes_value_list() {
        let v: Value = vec!["a", "b"].into_value();
        assert_eq!(v, Value::List(vec!["a".into(), "b".into()]));
    }

    #[test]
    fn array_of_str_becomes_value_list() {
        let v: Value = ["a", "b"].into_value();
        assert_eq!(v, Value::List(vec!["a".into(), "b".into()]));
    }

    #[test]
    fn value_passes_through_identity() {
        let v = Value::Number(99);
        assert_eq!(v.clone().into_value(), v);
    }
}
```

### 1.5 Verify

```bash
cargo test --all-features
cargo test --no-default-features
cargo test --features serde
cargo clippy --all-features -- -D warnings
```

**Commit:** `Add IntoValue trait for ergonomic Value construction`

---

## Phase 2 — Add `ctx!` macro

**File:** `nlg-core/src/context.rs` (same file — keeps the machinery together)

### 2.1 Macro definition

```rust
/// Build a [`Context`] from key/value pairs. Values may be any type that
/// implements [`IntoValue`] — `&str`, `String`, integer types, `bool`,
/// `Vec<&str>`, `[&str; N]`, or an explicit `Value::*`.
///
/// # Example
///
/// ```
/// use nlg_core::{ctx, Context, Value};
///
/// let c: Context = ctx! {
///     entity_type: "class",
///     name: "UserService",
///     consumer_count: 3,
///     consumers: ["ProfileComponent", "SettingsComponent", "AdminModule"],
/// };
///
/// assert_eq!(c.get("entity_type"), Some(&Value::String("class".into())));
/// assert_eq!(c.get("consumer_count"), Some(&Value::Number(3)));
/// ```
#[macro_export]
macro_rules! ctx {
    () => { $crate::Context::new() };
    ( $( $key:ident : $value:expr ),* $(,)? ) => {{
        let mut c = $crate::Context::new();
        $(
            c.insert(stringify!($key), $crate::IntoValue::into_value($value));
        )*
        c
    }};
    // Also accept string-literal keys for callers who want hyphens or
    // dots in their slot names (e.g. "event.key") — rare but supported.
    ( $( $key:literal : $value:expr ),* $(,)? ) => {{
        let mut c = $crate::Context::new();
        $(
            c.insert($key, $crate::IntoValue::into_value($value));
        )*
        c
    }};
}
```

**Note on the dual arm:** `macro_rules!` doesn't overload — it tries arms top-to-bottom and takes the first match. Because `ident` and `literal` differ syntactically, both arms should disambiguate. However, in practice `macro_rules!` can be finicky with `:literal` after `:ident`-receiving arms. If the literal arm doesn't match, ship with ident-only (the common case) and note it as a follow-up.

Test both forms in tests below.

### 2.2 Doctest

The doctest above should compile and pass. Add it to the macro's rustdoc comment. Verify with `cargo test --doc`.

### 2.3 Tests

Add a `#[cfg(test)]` module:

```rust
#[cfg(test)]
mod ctx_macro_tests {
    use crate::{ctx, Context, Value};

    #[test]
    fn empty_ctx_is_empty() {
        let c: Context = ctx! {};
        // Either assert through a public accessor or just use it in an engine
        // call; for now confirm via a fresh Context comparison.
        let baseline = Context::new();
        // Context may not impl PartialEq — check by trying to read a known-
        // missing key.
        assert_eq!(c.get("anything"), None);
        assert_eq!(baseline.get("anything"), None);
    }

    #[test]
    fn single_slot() {
        let c = ctx! { name: "Foo" };
        assert_eq!(c.get("name"), Some(&Value::String("Foo".into())));
    }

    #[test]
    fn multiple_slots_mixed_types() {
        let c = ctx! {
            name: "Foo",
            count: 3,
            flag: true,
        };
        assert_eq!(c.get("name"), Some(&Value::String("Foo".into())));
        assert_eq!(c.get("count"), Some(&Value::Number(3)));
        assert_eq!(c.get("flag"), Some(&Value::Number(1)));
    }

    #[test]
    fn list_slot_from_array() {
        let c = ctx! { items: ["a", "b", "c"] };
        assert_eq!(
            c.get("items"),
            Some(&Value::List(vec!["a".into(), "b".into(), "c".into()]))
        );
    }

    #[test]
    fn trailing_comma_allowed() {
        let c = ctx! { a: 1, b: 2, };
        assert_eq!(c.get("a"), Some(&Value::Number(1)));
        assert_eq!(c.get("b"), Some(&Value::Number(2)));
    }

    #[test]
    fn expression_values_are_evaluated() {
        let s = String::from("dynamic");
        let c = ctx! { name: s };
        assert_eq!(c.get("name"), Some(&Value::String("dynamic".into())));
    }

    #[test]
    fn value_literal_passes_through() {
        let c = ctx! { x: Value::Number(7) };
        assert_eq!(c.get("x"), Some(&Value::Number(7)));
    }

    // If the literal-key arm works, this passes. If macro_rules arm
    // resolution can't disambiguate, mark ignored and leave a TODO.
    #[test]
    fn literal_key_with_dot() {
        let c = ctx! { "event.key": "modified" };
        assert_eq!(c.get("event.key"), Some(&Value::String("modified".into())));
    }
}
```

If the literal-key test fails due to macro arm disambiguation, **delete that arm** and the test, ship ident-only, and add a comment noting that hyphenated/dotted keys need `Context::insert` directly.

### 2.4 Verify

```bash
cargo test --all-features
cargo test --no-default-features
cargo test --features serde
cargo clippy --all-features -- -D warnings
```

All must pass. The `cargo test --doc` leg runs doctests — confirm the macro's rustdoc doctest passes.

**Commit:** `Add ctx! macro for ergonomic Context construction`

---

## Phase 3 — Final verification

```bash
cargo test --all-features
cargo test --no-default-features
cargo test --features serde
cargo clippy --all-features -- -D warnings
cargo doc --all-features --no-deps
cargo bench --bench engine -- --test
```

All clean. Test count should be **376 + however-many-tests-you-added** (expect ~16 new tests across both phases, so ~392 total).

**Report:** 2 commit hashes, test count delta, any arm-disambiguation notes from Phase 2.

---

## Risk register

| Risk | Mitigation |
|---|---|
| macro_rules! arm disambiguation between `:ident` and `:literal` fails | Ship ident-only; delete literal arm and test. Document the constraint. |
| `IntoValue for Vec<String>` and `IntoValue for [String; N]` overlap with `IntoValue for Value` via ambiguous trait resolution | Rust's trait system handles these fine — they're concrete types. If you see a "conflicting implementations" error, something deeper is wrong — stop and report. |
| Doctest example in macro fails on `--no-default-features` | If the doctest references feature-gated items, mark it `ignore` or make it feature-aware. The example shown uses only `Context`, `Value`, `ctx` — all default-available. Should work. |
| `Context::get` may not exist or have a different signature | Check `nlg-core/src/context.rs` before writing the tests. Adapt to the actual public API (e.g., if it's `ctx.slots().get(k)` rather than `ctx.get(k)`). The test's purpose is correctness, not API shape. |
| `bool → Value::Number` semantic — is it the right mapping? | The CLI already does this (see `context_from_slots` in nlg-cli/src/main.rs). Consistent. |
| `u64` not included — will someone be surprised? | Document in the `IntoValue` doc comment. Users with u64 must `v as i64` explicitly; prevents silent overflow. |

## What NOT to do

- **Don't** migrate existing tests/examples to use `ctx!` in this plan. Separate concern, separate commit later (optional).
- **Don't** add `#[derive(IntoValue)]`. We have `#[derive(IntoContext)]`; this is different.
- **Don't** make `IntoValue` work across every possible host type (e.g., `chrono::DateTime`, `uuid::Uuid`). Stick to primitives. Users with custom types call `Value::String(v.to_string())` explicitly.
- **Don't** skip doctest verification.
- **Don't** amend commits.

## Definition of done

- [ ] Phase 0 baseline clean
- [ ] 2 commits with specified subject lines
- [ ] `IntoValue` trait exported from `lib.rs`
- [ ] `ctx!` macro exported via `#[macro_export]`
- [ ] All existing 376 tests still pass
- [ ] New tests for `IntoValue` impls and `ctx!` macro all pass
- [ ] Final test count increases by ~16
- [ ] `cargo clippy --all-features -- -D warnings` clean
- [ ] Doctest in the macro's rustdoc passes
- [ ] No public API breakage
- [ ] `Engine: Send + Sync` assert still compiles
