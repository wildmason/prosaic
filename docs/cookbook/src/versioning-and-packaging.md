# Versioning and Packaging

Prosaic publishes multiple Rust crates from one workspace. They move in
lockstep: every public crate uses the version declared in the root
`[workspace.package]` table. Regular internal workspace dependencies carry both
a local `path` and the matching crates.io `version`.

That shape is intentional:

```toml
prosaic-core = { version = "0.6.1", path = "../prosaic-core" }
```

Local development uses the path. Published crates use the version, so the same
manifest works before and after release.

Dev-dependencies are the exception when they are only local test harnesses. For
example, `prosaic-core` tests exercise the derive, grammar, and vocabulary
crates, but those crates depend on `prosaic-core` and cannot exist before it in
the initial publish order. Those bootstrap-only dev-dependencies stay path-only:

```toml
[dev-dependencies]
prosaic-derive = { path = "../prosaic-derive" }
prosaic-grammar-en = { path = "../prosaic-grammar-en" }
prosaic-vocab-code = { path = "../prosaic-vocab-code" }
```

Cargo uses them for local `cargo test --workspace`, but they are not part of
the published dependency graph.

## CLI Package Name

The user-facing command is the nice one:

```sh
cargo install prosaic
prosaic --help
```

The crates.io package is `prosaic`, and it installs a binary named `prosaic`.
The source directory remains `prosaic-cli` because this repository grew the CLI
as a workspace member before public packaging. Directory name and package name
do not need to match.

## Crate Set

The current public set is:

| Crate | Role |
|-------|------|
| `prosaic-common` | Shared no-std metadata for core and derive |
| `prosaic-core` | Engine, templates, discourse, document planning |
| `prosaic-derive` | `IntoContext` and compile-time template macros |
| `prosaic-grammar-en` | English grammar |
| `prosaic-grammar-es` | Spanish grammar |
| `prosaic-grammar-de` | German grammar |
| `prosaic-vocab-code` | Code-change vocabulary |
| `prosaic-vocab-git` | Git/VCS vocabulary |
| `prosaic-vocab-pr` | Pull-request vocabulary |
| `prosaic-vocab-release` | Release-note vocabulary |
| `prosaic-project` | Folder-of-files template project format |
| `prosaic-tracing` | `tracing_subscriber` layer |
| `prosaic-wasm` | `wasm-bindgen` bindings |
| `prosaic` | CLI package and binary |

## Version Rules

- One workspace version is the source of truth.
- Patch releases are for fixes, docs, and non-breaking additions.
- While the line is `0.x`, minor releases may include breaking API changes.
- A release tag uses `vMAJOR.MINOR.PATCH`, for example `v0.6.1`.
- Crates.io versions are immutable; a bad publish requires a new patch version.

## Publish Order

Publish crates in dependency order:

```text
prosaic-common
prosaic-core
prosaic-derive
prosaic-grammar-en
prosaic-grammar-es
prosaic-grammar-de
prosaic-vocab-code
prosaic-vocab-git
prosaic-vocab-pr
prosaic-vocab-release
prosaic-project
prosaic-tracing
prosaic-wasm
prosaic
```

The automation scripts use this order. Keep it in sync with
`scripts/ProsaicRelease.ps1` when adding or removing public crates.

## Release Automation

Run the release gate before publishing:

```powershell
.\scripts\release-check.ps1
```

That runs:

```sh
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

For already-published versions, or for selected crates whose internal
dependencies are already live on crates.io, add `-PublishDryRun`:

```powershell
.\scripts\release-check.ps1 -PublishDryRun
```

That runs `cargo publish --dry-run --locked` in publish order. Use
`-Crates prosaic-common,prosaic-core` to limit the dry-run to a subset while
iterating.

For a future unpublished lockstep version, dependent crates may not be
registry-resolvable until their internal dependencies have been uploaded. The
publish script handles that by dry-running each crate immediately before its
live upload, after the prior crates in the dependency order have become visible
in the crates.io API.

Publish the workspace with:

```powershell
.\scripts\publish-workspace.ps1 -Yes
```

By default the script reads the version from root `[workspace.package]`, requires
a clean git worktree, skips crate versions that are already live on crates.io,
dry-runs each crate immediately before upload, and publishes the remaining
crates in dependency order:

```sh
cargo publish -p <crate> --locked --dry-run
cargo publish -p <crate> --locked
```

If crates.io rate-limits the release, the script parses Cargo's retry timestamp,
sleeps until the requested time with a small safety pad, then resumes on the same
crate. It also waits for each just-published crate to appear in the crates.io API
before attempting dependents.

Use `-SkipPerCrateDryRun` only when the exact dry-run has already been performed
for the same clean release commit.

For a no-upload rehearsal:

```powershell
.\scripts\publish-workspace.ps1 -DryRun -SkipGitChecks
```

After publishing, tag the exact commit and create a GitHub release:

```sh
git tag v0.6.2
git push origin v0.6.2
```

Then verify the release:

```powershell
.\scripts\verify-release.ps1 -Version 0.6.2
```

The verification script checks every exact crate version through the crates.io
API, checks docs.rs, runs `cargo search` and `cargo info`, installs the published
`prosaic` CLI into `target/prosaic-install-smoke`, and runs a real JSON-lines
render through the installed binary.

## CI

GitHub Actions runs the normal Rust gate on every push to `main`, every pull
request, and manual dispatch:

```sh
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

Tests run on Ubuntu, Windows, and macOS. Formatting and clippy run on Ubuntu to
avoid redundant lint work across operating systems.
