# Plan: CLI `--preset` Bundles

**Owner:** sonnet agent
**Scope:** `nlg-cli/` only (Cargo.toml + main.rs)
**Estimated size:** ~150–200 LOC
**Test gate:** all 526 existing tests pass; zero warnings
**Branch discipline:** local only, 1 commit

---

## Why

The CLI currently requires users to manually wire `--vocab`, `--strategy`, and `--max-length` for each use case. A `--preset` flag bundles those into one-command flows:

- `nlg --preset=changelog < events.jsonl` → readable changelog from a stream of release/code/git events
- `nlg --preset=release-notes < events.jsonl` → customer-facing release notes
- `nlg --preset=digest < events.jsonl` → weekly engineering digest with PR summaries

Each preset selects vocab modules, grouping strategy, and optional polish settings. Users can override any preset-selected option with explicit flags (`--preset=changelog --strategy=sequential` overrides the preset's default strategy).

## Design (locked)

### New CLI flag

```
--preset <name>   Apply a named preset configuration (changelog, release-notes, digest)
```

Parsed in `parse_args`. Presets set defaults on the `Config` struct BEFORE other flags are parsed, so explicit flags override preset values. Ensure the preset arg is processed early.

### Preset definitions

| Preset | Vocab modules | Strategy | Max length | Smart quotes |
|---|---|---|---|---|
| `changelog` | code, git, release | by-action | 120 | off |
| `release-notes` | release | by-action | none | on |
| `digest` | code, git, release, pr | by-entity | 100 | off |

### Config extensions

Add two new booleans to `Config`:

```rust
struct Config {
    // ... existing fields ...
    vocab_release: bool,
    vocab_pr: bool,
}
```

Default both to `false`. Extend `--vocab` parsing to accept `"release"` and `"pr"` as additional values (and `"all"` now includes them).

### Cargo.toml deps

Add to `nlg-cli/Cargo.toml`:

```toml
nlg-vocab-release = { path = "../nlg-vocab-release" }
nlg-vocab-pr = { path = "../nlg-vocab-pr" }
```

### `run()` wiring

After existing `if cfg.vocab_code { ... }` / `if cfg.vocab_git { ... }` blocks, add:

```rust
if cfg.vocab_release {
    nlg_vocab_release::register(&mut engine).map_err(|e| format!("vocab-release: {e}"))?;
}
if cfg.vocab_pr {
    nlg_vocab_pr::register(&mut engine).map_err(|e| format!("vocab-pr: {e}"))?;
}
```

### Help text update

Add `--preset` to the help output. Show the three presets and what they configure.

### Override semantics

Presets are applied first. Any explicit flag overrides the preset's default:

```rust
fn parse_args() -> Result<Config, String> {
    let mut cfg = Config::default();

    // First pass: find --preset and apply it.
    // Second pass (or inline): explicit flags override.
    // Simplest: scan for --preset first, apply its defaults, then
    // re-scan all args including non-preset flags in order.
    // ...
}
```

Implementation: scan args in order. When `--preset` is hit, apply its defaults to `cfg`. When any other flag is hit, overwrite the field. Since args are scanned left-to-right, `--preset=changelog --strategy=sequential` applies the preset first, then overrides strategy. This is the standard "preset then override" idiom.

### No tests needed for the CLI binary

The CLI crate is a binary; no `#[test]` tests exist today and the pattern is consistent — `nlg-cli` has zero tests in its current form. The verification step is `cargo build -p nlg-cli` + smoke-test via `echo '...' | cargo run -p nlg-cli -- --preset=changelog`. The sonnet agent should run the smoke test manually via Bash.

### Out of scope

- **Do not** add `--from git-log` ingestor. That reads git log and converts to JSON events; it's a separate concern.
- **Do not** add Markdown output formatting. Presets produce prose; formatting is downstream.
- **Do not** add YAML/TOML config files for preset definitions. They're hardcoded for v1.

---

## Phase 0 — Baseline

```bash
cargo test --all-features
cargo clippy --all-features -- -D warnings
cargo build -p nlg-cli
```

Expected: **526 tests passing**, CLI builds clean.

**No commit.**

---

## Phase 1 — Add `--preset` + vocab extensions to CLI

### 1.1 Update `nlg-cli/Cargo.toml`

Add deps for `nlg-vocab-release` and `nlg-vocab-pr`.

### 1.2 Update `Config` struct

Add `vocab_release: bool`, `vocab_pr: bool`. Default `false`.

### 1.3 Extend `--vocab` parsing

Accept `"release"`, `"pr"` in addition to existing `"code"`, `"git"`. `"all"` / `"both"` now sets all four to true.

### 1.4 Add `--preset` parsing

In `parse_args`, add a match arm for `"--preset"`:

```rust
"--preset" => {
    i += 1;
    let v = args.get(i).ok_or("--preset requires a value")?.clone();
    match v.as_str() {
        "changelog" => {
            cfg.vocab_code = true;
            cfg.vocab_git = true;
            cfg.vocab_release = true;
            cfg.strategy = Strategy::ByAction;
            cfg.max_length = Some(120);
        }
        "release-notes" => {
            cfg.vocab_release = true;
            cfg.strategy = Strategy::ByAction;
            cfg.smart_quotes = true;
        }
        "digest" => {
            cfg.vocab_code = true;
            cfg.vocab_git = true;
            cfg.vocab_release = true;
            cfg.vocab_pr = true;
            cfg.strategy = Strategy::ByEntity;
            cfg.max_length = Some(100);
        }
        other => {
            return Err(format!(
                "unknown preset `{other}` — expected changelog, release-notes, or digest"
            ));
        }
    }
}
```

### 1.5 Wire vocab registration in `run()`

Add the two new registration blocks after existing ones.

### 1.6 Update `print_help()`

Add the `--preset` line and a short explanation of each preset.

### 1.7 Smoke test

```bash
echo '{"key":"release.tagged","version":"1.0.0","title":"Initial release"}' | cargo run -p nlg-cli -- --preset=release-notes
echo '{"key":"release.feature_added","name":"retry pipeline","description":"Automatic retry on transient failures"}' | cargo run -p nlg-cli -- --preset=changelog
echo '{"key":"pr.summary","number":42,"title":"Add retry","author":"Alice","commit_count":5,"files_changed":12}' | cargo run -p nlg-cli -- --preset=digest
```

Each must produce non-empty prose output. Capture and verify.

### 1.8 Verify

```bash
cargo test --all-features
cargo test --no-default-features
cargo test --features serde
cargo clippy --all-features -- -D warnings
cargo build -p nlg-cli
```

All 526 existing tests pass. CLI compiles.

**Commit:** `Add --preset flag to CLI with changelog, release-notes, and digest bundles`

---

## Risk register

| Risk | Mitigation |
|---|---|
| `--preset` before `--vocab` in the arg list means vocab flags don't override | Args are parsed left-to-right. `--preset=changelog` sets `vocab_code=true`; a subsequent `--vocab=none` would reset them. This is intentional override-after-preset semantics. |
| The `"all"` vocab value in `--vocab` should include release + pr | Yes — update the `"all"` / `"both"` branch to set all four. |
| Preset sets `smart_quotes = true` but user wanted it off | User passes `--no-smart-quotes` — oh wait, we don't have that flag. Accept the limitation: if a preset turns on smart quotes, the user can't turn them off. For v1 this only affects `release-notes`; callers who need raw quotes omit `--preset` and configure manually. Document. |
| Smoke test produces mangled output because the event schema doesn't match the vocab crate's expected slots | The event keys and slots in the smoke test must match the registered templates exactly. Read `nlg-vocab-release/src/lib.rs` to confirm slot names before writing the smoke test. |

## What NOT to do

- **Do not** add a `--from` ingestor (git-log → JSON events).
- **Do not** emit Markdown or HTML.
- **Do not** add YAML/TOML config files for presets.
- **Do not** add tests to the CLI crate. It has none today; smoke-test via Bash is sufficient.
- **Do not** amend commits.

## Definition of done

- [ ] Phase 0 baseline clean
- [ ] 1 commit with specified subject line
- [ ] `nlg-cli/Cargo.toml` depends on `nlg-vocab-release` and `nlg-vocab-pr`
- [ ] `Config` has `vocab_release` and `vocab_pr` booleans
- [ ] `--vocab` accepts `release`, `pr`, and `all` sets all four
- [ ] `--preset` accepts `changelog`, `release-notes`, `digest` with correct defaults
- [ ] `print_help()` documents the new flags
- [ ] Smoke tests produce non-empty prose for all three presets
- [ ] All 526 existing tests pass
- [ ] `cargo clippy --all-features -- -D warnings` clean
- [ ] CLI builds clean
