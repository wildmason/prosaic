# Plan: Spanish vocabulary siblings for vocab crates

**Owner:** sonnet agent
**Scope:** Add `es.rs` sibling to `prosaic-vocab-code`, `prosaic-vocab-git`, `prosaic-vocab-release`, `prosaic-vocab-pr`
**Estimated size:** ~800–1,000 LOC including tests
**Test gate:** 852 baseline tests still pass; new tests add ~30–40
**Branch discipline:** local only, one commit per crate (4 commits) OR one bundled commit — choice of bundled is fine

---

## Why

Each vocab crate currently has `templates::en::register` but the `lib.rs` top-level `register()` function delegates only to English. The `en.rs` file header explicitly anticipates `es.rs` siblings: *"When additional locales are added (e.g. `es`, `de`), they will each live in a sibling module and expose the same `register` signature."* This plan realizes that anticipated structure for Spanish, proving the locale pattern works end-to-end with a non-English grammar.

## Design

### Public API

Each vocab crate gains:

```rust
// In lib.rs, already declares `pub mod en;`. Add:
pub mod es;

// New top-level helper, additive (does not break `register()`):
/// Register Spanish templates into an engine that uses a Spanish grammar layer.
pub fn register_es(engine: &mut Engine) -> Result<(), ProsaicError> {
    es::register(engine)
}
```

The existing `register(engine)` continues to delegate to `en::register`. English stays the default.

### Template translation approach

1. **Keep template keys identical** (`code.renamed`, `git.commit`, etc.). Locale selection is purely at registration time.
2. **Keep slot names identical** (`name`, `consumer_count`, `consumers`, `old_name`, `new_name`, `location`, etc.). Application code passes the same context.
3. **Translate the surface text into idiomatic Spanish.** Use natural Spanish phrasing, not word-for-word from English.
4. **Keep the same salience levels and alternative counts** per template key (Low × 2, Medium × 3, High × 1–2, matching English).
5. **Use Spanish pipe arguments.** The `pluralize` pipe takes a word: in English `consumer_count|pluralize:consumer` → "consumer"/"consumers"; in Spanish the same pipe uses the Spanish grammar layer and should receive the Spanish root form, e.g. `consumer_count|pluralize:consumidor` → "consumidor"/"consumidores".

### Spanish phrasing conventions

- **Passive voice:** Spanish uses `ser + past participle` (e.g. "fue renombrado"). Gender agreement on the participle is a real concern but out of scope for v1 vocab — keep participles in masculine form ("renombrado", "eliminado", "modificado") since entity-type names are generic and often opaque. The Spanish grammar layer's `realize_reference` handles gendered pronouns when they're actually used.
- **Accents:** All accents MUST be correct UTF-8 (é, á, í, ó, ú, ñ). Do not use ASCII substitutes.
- **Em-dashes:** Use `\u{2014}` exactly as English templates do — Spanish uses em-dashes too.
- **Oxford comma:** Spanish typically omits it ("X, Y y Z" not "X, Y, y Z"). The Spanish grammar layer's `join_list` already handles this via Oxford-style for consistency; just keep `{|join}` and trust the layer.

### Specific translations (reference)

Approximate guide — the sonnet agent should produce idiomatic Spanish, not mechanical word-for-word:

| English | Spanish |
|---|---|
| "was renamed to" | "fue renombrado a" |
| "is now called" | "ahora se llama" |
| "was removed" | "fue eliminado" |
| "has been deleted" | "ha sido eliminado" |
| "was modified" | "fue modificado" |
| "was moved from X to Y" | "fue movido de X a Y" |
| "has been relocated to" | "ha sido trasladado a" |
| "The signature of X was changed" | "La firma de X fue modificada" |
| "A new X was added in Y" | "Se agregó un nuevo X en Y" |
| "including" | "incluyendo" |
| "notably" | "particularmente" |
| "which impacts" | "lo que afecta a" |
| "may affect" | "podría afectar a" |
| "Thorough review is recommended" | "Se recomienda una revisión exhaustiva" |
| "A significant change" | "Un cambio significativo" |
| "rippling through" | "que repercute en" |
| "All references will need migration" | "Todas las referencias necesitarán migración" |
| "consumer" (noun) | "consumidor" |
| "dependent" (noun) | "dependiente" |
| "caller" (noun) | "invocador" |
| "reference" (noun) | "referencia" |
| "file" (noun) | "archivo" |
| "import" (noun) | "importación" |

### Test strategy

For each crate, the existing `#[cfg(test)] mod tests` block tests English templates. Add a **parallel** `#[cfg(test)] mod tests_es` block that:

1. Uses `prosaic_grammar_es::Spanish` instead of `English`.
2. Calls `register_es(&mut engine)` instead of `register(&mut engine)`.
3. Asserts on Spanish phrases (e.g. `result.contains("fue renombrado a")`).

Mirror ~5–7 tests per vocab crate (rename event, delete event, modify event, salience levels). No need to duplicate every single English test — pick the representative ones.

### Dev-dependencies

Each vocab crate's `Cargo.toml` currently lists `prosaic-grammar-en` as a dev-dependency (used by the English test helpers). Add `prosaic-grammar-es = { path = "../prosaic-grammar-es" }` as a dev-dependency too.

---

## Phase 0 — Baseline

```bash
cargo test --all-features
cargo clippy --all-features -- -D warnings
```

Expected: 852 tests passing, 0 warnings.

---

## Phase 1 — prosaic-vocab-code

1. Read `prosaic-vocab-code/src/en.rs` top-to-bottom.
2. Create `prosaic-vocab-code/src/es.rs` with the same module header docstring (mention `en` sibling and locale dispatch), same function structure (`register_rename_templates`, `register_delete_templates`, etc.), and translated templates.
3. In `lib.rs`:
   - Add `pub mod es;`
   - Add `pub fn register_es(engine: &mut Engine) -> Result<(), ProsaicError>` delegating to `es::register`.
4. In `Cargo.toml`, add:
   ```toml
   prosaic-grammar-es = { path = "../prosaic-grammar-es" }
   ```
   under `[dev-dependencies]`.
5. Add `#[cfg(test)] mod tests_es` block mirroring ~7 of the English tests (rename, delete, add, modify, move, signature_changed, one salience test).

### Translation guidance for this crate

- Keep template **counts and salience levels identical** to English (same number of Low/Medium/High per event).
- Preserve the `{|truncate:3|join}` and `{|truncate:5|join:bracketed}` patterns verbatim — those are engine pipes, locale-independent.
- Spanish pluralize args: "consumidor", "dependiente", "invocador", "referencia", "archivo", "importación", "llamada" (for "call site"), "sitio" (for "site").

### Verify

```bash
cargo test -p prosaic-vocab-code --all-features
cargo clippy -p prosaic-vocab-code --all-features -- -D warnings
```

---

## Phase 2 — prosaic-vocab-git

Same structure. Event keys in this crate: `git.commit`, `git.branch_created`, `git.merged`, `git.reverted`, etc. Read `prosaic-vocab-git/src/en.rs` to get the exact list.

Translation hints:
- "committed" / "commit" → "commit" (loanword, commonly used in Spanish tech contexts) or "confirmación" (more formal, but awkward in tech). **Use "commit" (unchanged) for idiomatic Spanish developer speech.**
- "branch" → "rama"
- "merged" → "fusionado" (verb: "fusionar") or loanword "mergeado" (colloquial tech Spanish). Use "fusionado".
- "reverted" → "revertido"
- "pushed" → "enviado" or loanword. Use "enviado" for formality.
- "pulled" → "obtenido" or loanword. Use "obtenido".
- "tagged" → "etiquetado"

---

## Phase 3 — prosaic-vocab-release

Same structure. Event keys: `release.deployed`, `release.rollback`, `release.canary`, etc.

Translation hints:
- "released" → "publicado" or "lanzado". Use "lanzado" (more common in release contexts).
- "deployed" → "desplegado"
- "rollback" → "revertir" / "reversión"
- "canary" → "canario" or "despliegue canario" (explicit)
- "breaking change" → "cambio disruptivo" or "cambio que rompe compatibilidad". Use "cambio disruptivo".
- "bugfix" → "corrección de error"
- "feature" → "funcionalidad"

---

## Phase 4 — prosaic-vocab-pr

Same structure. Event keys: `pr.opened`, `pr.merged`, `pr.closed`, `pr.review_requested`, etc.

Translation hints:
- "pull request" → "pull request" (loanword, used as-is in Spanish tech) or "solicitud de extracción" (formal). Use "pull request" unchanged for authenticity.
- "opened" → "abierto" / "abrió"
- "review" (noun) → "revisión"
- "approval" → "aprobación"
- "comment" (noun) → "comentario"
- "reviewer" → "revisor"
- "author" → "autor"

---

## Phase 5 — Final verification

```bash
cargo test --all-features
cargo test --no-default-features
cargo test --features serde
cargo clippy --all-features -- -D warnings
cargo doc --all-features --no-deps
```

Total tests up by ~30–40 (roughly 7 tests per vocab crate × 4 crates, minus some shared infra).

**Commit:** `Add Spanish (es) vocabulary siblings for all four vocab crates`

---

## What NOT to do

- **Do NOT** change the English `register()` default or rename existing functions.
- **Do NOT** use Google Translate output verbatim — the translations must be idiomatic Spanish as used by developers. Prefer loanwords where they're the natural choice (e.g., "commit", "pull request").
- **Do NOT** add gender agreement on past participles. Keep them masculine in vocab templates; per-entity gender agreement flows through `realize_reference` and `plural_description`, not through hardcoded template strings.
- **Do NOT** split into 4 commits unless you want to — one bundled commit is fine.
- **Do NOT** amend, rebase, or push to remote.

## Definition of done

- [ ] All 4 vocab crates have `pub mod es` and `register_es` function
- [ ] All 4 `es.rs` files translate the full event-key set from the corresponding `en.rs`
- [ ] All 4 crates have a `tests_es` test module with ~5–7 tests each
- [ ] `cargo test --all-features` passes with test count up by ~30–40
- [ ] `cargo clippy --all-features -- -D warnings` clean
- [ ] `cargo doc --all-features --no-deps` clean
- [ ] One commit with subject: `Add Spanish (es) vocabulary siblings for all four vocab crates`
