# Versioning and Packaging

Prosaic publishes multiple Rust crates from one workspace. They move in
lockstep: every public crate uses the version declared in the root
`[workspace.package]` table, and every internal workspace dependency carries
both a local `path` and the matching crates.io `version`.

That shape is intentional:

```toml
prosaic-core = { version = "0.6.1", path = "../prosaic-core" }
```

Local development uses the path. Published crates use the version, so the same
manifest works before and after release.

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

Before publishing, run:

```sh
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo publish -p prosaic-common --dry-run --locked
```

Repeat the dry-run for each crate in the publish order, then publish with:

```sh
cargo publish -p <crate> --locked
```

After the last crate is live, tag the exact commit and create a GitHub release.
