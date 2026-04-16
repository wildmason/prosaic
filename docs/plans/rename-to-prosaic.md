# Plan: Rename `nlg` → `prosaic`

**Owner:** sonnet agent
**Scope:** entire workspace — directory names, crate names, all imports, error types, macro names, docs, license files, git remote
**Estimated size:** ~500 find-replace operations across ~40 files
**Test gate:** all 534 tests pass post-rename; zero warnings
**Branch discipline:** local only, 1 commit (atomic rename)

---

## Why

The project is going open-source at `https://github.com/wildmason/prosaic`. The `nlg` name is too academic and likely taken on crates.io. `prosaic` is the permanent name.

## Rename mapping

### Directories

| Old | New |
|---|---|
| `nlg-core/` | `prosaic-core/` |
| `nlg-grammar-en/` | `prosaic-grammar-en/` |
| `nlg-derive/` | `prosaic-derive/` |
| `nlg-vocab-code/` | `prosaic-vocab-code/` |
| `nlg-vocab-git/` | `prosaic-vocab-git/` |
| `nlg-vocab-release/` | `prosaic-vocab-release/` |
| `nlg-vocab-pr/` | `prosaic-vocab-pr/` |
| `nlg-cli/` | `prosaic-cli/` |
| `nlg-tracing/` | `prosaic-tracing/` |

### Crate names (in Cargo.toml `[package].name`)

| Old | New |
|---|---|
| `nlg-core` | `prosaic-core` |
| `nlg-grammar-en` | `prosaic-grammar-en` |
| `nlg-derive` | `prosaic-derive` |
| `nlg-vocab-code` | `prosaic-vocab-code` |
| `nlg-vocab-git` | `prosaic-vocab-git` |
| `nlg-vocab-release` | `prosaic-vocab-release` |
| `nlg-vocab-pr` | `prosaic-vocab-pr` |
| `nlg-cli` | `prosaic-cli` |
| `nlg-tracing` | `prosaic-tracing` |

### Rust identifiers (underscored form in `use` / `extern crate` / attribute paths)

| Old | New |
|---|---|
| `nlg_core` | `prosaic_core` |
| `nlg_grammar_en` | `prosaic_grammar_en` |
| `nlg_derive` | `prosaic_derive` |
| `nlg_vocab_code` | `prosaic_vocab_code` |
| `nlg_vocab_git` | `prosaic_vocab_git` |
| `nlg_vocab_release` | `prosaic_vocab_release` |
| `nlg_vocab_pr` | `prosaic_vocab_pr` |
| `nlg_tracing` | `prosaic_tracing` |

### Types and macros

| Old | New |
|---|---|
| `NlgError` | `ProsaicError` |
| `nlg_template!` | `prosaic_template!` |

### Other strings

| Old | New |
|---|---|
| `nlg_faithful` (feature name in faithfulness tests) | `prosaic_faithful` |
| `"nlg"` in CLI help text / binary name | `"prosaic"` |
| CLI binary name (`[[bin]] name = "nlg"`) | `name = "prosaic"` |
| `#[macro_export] macro_rules! nlg_template` | `macro_rules! prosaic_template` |
| Module doc comments referencing "nlg" as the project name | Update to "prosaic" |
| `$crate` paths inside macros (these auto-resolve — no change needed) | No change |
| Workspace `repository` URL | `https://github.com/wildmason/prosaic` |

### Explicitly NOT renamed

| Item | Reason |
|---|---|
| `ctx!` macro | Domain-agnostic, not prefixed |
| `assert_faithful!` macro | Domain-agnostic, not prefixed |
| `Context`, `Value`, `Engine`, `Session`, `Template` | Domain-agnostic core types |
| `IntoContext`, `IntoValue`, `IntoContext` derive | Domain-agnostic traits |
| Local variable names like `nlg_ctx` in nlg-tracing | Rename to `prosaic_ctx` for consistency |
| The root directory `nlg/` on disk | Leave as-is; the user can rename the folder separately if desired |

---

## Execution steps (in order)

### Step 1 — Rename directories via `git mv`

```bash
git mv nlg-core prosaic-core
git mv nlg-grammar-en prosaic-grammar-en
git mv nlg-derive prosaic-derive
git mv nlg-vocab-code prosaic-vocab-code
git mv nlg-vocab-git prosaic-vocab-git
git mv nlg-vocab-release prosaic-vocab-release
git mv nlg-vocab-pr prosaic-vocab-pr
git mv nlg-cli prosaic-cli
git mv nlg-tracing prosaic-tracing
```

### Step 2 — Update root `Cargo.toml`

Replace every `nlg-*` member path with `prosaic-*`. Update `repository` to `https://github.com/wildmason/prosaic`.

### Step 3 — Update each crate's `Cargo.toml`

For each of the 9 crates:
- `[package].name` → `prosaic-*`
- `[package].description` → replace "nlg" with "prosaic" where it appears
- Every `[dependencies]` and `[dev-dependencies]` entry referencing another workspace crate: update both the key name AND the `path = "../nlg-*"` to `path = "../prosaic-*"`
- CLI crate: `[[bin]] name = "nlg"` → `name = "prosaic"`

### Step 4 — Bulk rename in all `.rs` files

Use find-and-replace across every `.rs` file in the workspace. Order matters — do longer strings first to avoid partial replacements:

1. `nlg_vocab_release` → `prosaic_vocab_release`
2. `nlg_vocab_code` → `prosaic_vocab_code`
3. `nlg_vocab_git` → `prosaic_vocab_git`
4. `nlg_vocab_pr` → `prosaic_vocab_pr`
5. `nlg_grammar_en` → `prosaic_grammar_en`
6. `nlg_tracing` → `prosaic_tracing`
7. `nlg_derive` → `prosaic_derive`
8. `nlg_core` → `prosaic_core`
9. `NlgError` → `ProsaicError`
10. `nlg_template` → `prosaic_template` (covers the macro name and all references)
11. `nlg_faithful` → `prosaic_faithful` (feature name if used in test cfg)

Then do a second pass for prose/comments/docs:
12. `nlg crate` → `prosaic crate` (in doc comments)
13. `nlg engine` → `prosaic engine` (in doc comments)
14. `nlg` → `prosaic` ONLY in specific contexts: CLI help text, binary references, crate-level doc comments. DO NOT blindly replace all occurrences of "nlg" — it could appear in unrelated contexts.

### Step 5 — Update README.md

Replace `nlg` references with `prosaic` throughout:
- Crate names
- Import paths (`use nlg_core::` → `use prosaic_core::`)
- CLI usage (`nlg --preset=changelog` → `prosaic --preset=changelog`)
- Binary name

### Step 6 — Update examples and bench files

All `.rs` files in `prosaic-core/examples/`, `prosaic-core/benches/`, and all `tests/` directories. The Step 4 bulk replace should have caught the imports; verify no stale `nlg_` references remain.

### Step 7 — Add LICENSE files

Create `LICENSE-MIT` and `LICENSE-APACHE` at the workspace root.

`LICENSE-MIT`:
```
MIT License

Copyright (c) 2026 Matthew MacKinnon / Wildmason

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

`LICENSE-APACHE`: Standard Apache 2.0 boilerplate with `Copyright 2026 Matthew MacKinnon / Wildmason`.

### Step 8 — Add git remote

```bash
git remote add origin https://github.com/wildmason/prosaic.git
```

### Step 9 — Verify

```bash
cargo check --all-features
cargo test --all-features
cargo test --no-default-features
cargo test --features serde
cargo clippy --all-features -- -D warnings
cargo doc --all-features --no-deps
cargo bench --bench engine -- --test
```

All must pass with **534 tests**, 0 warnings. No stale `nlg` references in any `.rs` file (grep to confirm).

Post-verify grep:

```bash
grep -r "nlg_core\|nlg_derive\|nlg_grammar\|nlg_vocab\|nlg_tracing\|nlg_template\|NlgError\|nlg-core\|nlg-derive\|nlg-grammar\|nlg-vocab\|nlg-cli\|nlg-tracing" --include="*.rs" --include="*.toml" .
```

This should return ZERO hits (except possibly in `docs/plans/` or `docs/swarm/` which reference the old name historically — those are fine).

### Step 10 — Commit

One atomic commit:

**Commit:** `Rename nlg → prosaic across the entire workspace`

---

## Risk register

| Risk | Mitigation |
|---|---|
| `$crate` inside macros resolves to the wrong crate after rename | `$crate` is resolved by the compiler based on the crate the macro is defined in — it auto-follows the rename. No manual fix needed. Verify by running the `ctx!` and `prosaic_template!` tests. |
| Cargo.toml dep keys use underscores (`nlg_core`) not hyphens | Cargo accepts both forms; the `path` field is what matters. But convention is hyphens in Cargo.toml (`prosaic-core`) and underscores in Rust code (`prosaic_core`). Match convention. |
| `git mv` on Windows may fail if a file is locked | Close any editor/IDE that might have files open in the workspace before running. |
| The `docs/plans/` and `docs/swarm/` files reference "nlg" extensively | Leave them as historical artifacts. They document decisions about the project when it was named "nlg". Do NOT rename inside plan/swarm docs. |
| `target/` directory has cached artifacts under old crate names | `cargo clean` after the rename, before the verify step. Add to the plan. |
| README code examples reference old import paths | Step 5 covers this explicitly. |
| Binary name changes from `nlg` to `prosaic` — existing users' scripts break | No existing users — the project hasn't been published. Clean slate. |

## What NOT to do

- **Do not** rename the root directory on disk. The user can do that separately.
- **Do not** rename inside `docs/plans/` or `docs/swarm/` — historical docs.
- **Do not** rename `Context`, `Value`, `Engine`, `Session`, `Template`, `IntoContext`, `IntoValue`, `ctx!`, `assert_faithful!` — these are domain-agnostic.
- **Do not** push to remote in this plan. The user will push when ready.
- **Do not** create multiple commits. One atomic rename commit.
- **Do not** amend any prior commit.

## Definition of done

- [ ] All 9 directories renamed via `git mv`
- [ ] All Cargo.toml files updated (package names, dep paths, repository URL, binary name)
- [ ] All `.rs` files updated (imports, type name, macro name)
- [ ] README.md updated
- [ ] LICENSE-MIT and LICENSE-APACHE files present
- [ ] Git remote `origin` set to `https://github.com/wildmason/prosaic.git`
- [ ] `cargo clean` run
- [ ] 534 tests pass on `--all-features`, `--no-default-features`, `--features serde`
- [ ] `cargo clippy --all-features -- -D warnings` clean
- [ ] `cargo doc --all-features --no-deps` zero warnings
- [ ] Post-verify grep for stale `nlg` references returns zero hits in `.rs`/`.toml` files (excluding `docs/plans/` and `docs/swarm/`)
- [ ] 1 commit: `Rename nlg → prosaic across the entire workspace`
