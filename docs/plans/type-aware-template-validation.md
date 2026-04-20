# Plan: Type-Aware Template Validation (Revised v2)

**Owner:** Gemini CLI
**Scope:** `prosaic-common` (new), `prosaic-derive`, `prosaic-core`
**Estimated size:** ~400 LOC
**Context:** Moves type validation from runtime to compile-time (via `prosaic_template!`) and registration-time (via `Engine`).

---

## Why

Currently, Prosaic validates slot *names* but not their *types*. This leads to runtime errors when a pipe (e.g., `pluralize`) receives an incompatible `Value` variant. This plan establishes a shared type system and validation layer.

## Design

### 1. `prosaic-common` Crate (Shared Source of Truth)
To avoid duplication and drift between the core engine and the proc-macro, we will extract a shared, `no_std`-compatible crate: `prosaic-common`.

- **`ValueType` Enum**: Models the types in `prosaic_core::Value` (`String`, `Number`, `List`, `Entity`) plus `Any`.
- **`PipeSpec` Registry**: A `const` list of all 19 pipes with their input/output contracts.

| Pipe | Input Type | Result Type |
|---|---|---|
| `pluralize`, `plural` | `Number` | `String` |
| `truncate` | `List` | `List` |
| `join` | `List` | `String` |
| `ordinal`, `words`, `quantify` | `Number` | `String` |
| `hedge`, `proportion` | `Number` | `String` |
| `relative`, `since_last` | `Number` | `String` |
| `capitalize`, `verb`, `negated` | `Any` | `String` |
| `article`, `syn`, `demonstrative` | `Any` | `String` |
| `refer`, `choose` | `Any` | `String` |

*Note: Pipe-argument validation (e.g., checking that `truncate` has a numeric arg) is **out of scope** for this phase and remains a runtime check.*

### 2. Compile-Time Validation via `prosaic_template!`
We use a trait-based approach for superior error messages and robustness.

1.  **`HasProsaicSchema` Trait**:
    ```rust
    pub trait HasProsaicSchema {
        const PROSAIC_SCHEMA: &[(&'static str, ValueType)];
    }
    ```
2.  **`#[derive(IntoContext)]`**: Automatically implements `HasProsaicSchema` for the struct.
3.  **`prosaic_template!` Macro**:
    - **Chain Validation**: Always validates that `PipeSpec::output_type` of pipe $n$ matches `input_type` of pipe $n+1$. This works even when no `context` is provided.
    - **Context Validation**: If `context: MyContext` is provided, the macro emits a `const` assertion block. It uses `MyContext::PROSAIC_SCHEMA` to verify each slot.
    - **Error DX**: Instead of a single generic panic, the macro generates targeted assertions: `const _: () = assert_type_match::<MyContext>("count", ValueType::Number);`.

### 3. Type Unification & Conflict Resolution
If a slot is used multiple times in one template:
- **Unification**: `Number` ∩ `Any` → `Number`. `Entity` ∩ `Any` → `Entity`.
- **Conflict**: `Number` ∩ `List` → **Compile Error** ("slot 'x' is used as both Number and List").

## Implementation Phases

### Phase 1: `prosaic-common`
- Create the crate and move `ValueType` and `PIPE_SPECS` into it.
- Update `prosaic-core` and `prosaic-derive` to depend on `prosaic-common`.

### Phase 2: Template Type Inference (`prosaic-core`)
- Implement `Template::infer_types()` using the shared `PIPE_SPECS`.
- Support type unification for multi-mention slots.

### Phase 3: Macro & Trait Enhancements (`prosaic-derive`)
- Define `HasProsaicSchema` in `prosaic-core` (re-exported).
- Update `derive_into_context` to implement the trait.
- Update `prosaic_template!` to generate per-slot type assertions when a context is provided.
- **Verification**: Add `trybuild` tests for:
    - Pipe chain mismatch (e.g., `capitalize | pluralize`).
    - Context mismatch (e.g., `String` field passed to `Number` pipe).
    - Multi-mention conflict.

### Phase 4: Runtime Safety (`prosaic-core`)
- Update `Engine::register_template` to perform a "sanity check" against `PIPE_SPECS`.
- Introduce `Engine::register_template_with_schema<T: HasProsaicSchema>(...)` for users who want runtime enforcement in `Strict` mode even for dynamically loaded templates.

---

## Safety & Invariants
- **Backward Compatibility**: Templates without explicit type requirements or contexts default to `ValueType::Any`.
- **Partial Opaque-ness**: `{>partial}` inclusions are resolved and validated at registration-time, not compile-time.
- **No-Std**: `prosaic-common` must remain `no_std` compatible.
