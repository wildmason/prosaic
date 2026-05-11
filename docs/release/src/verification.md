# Verification

There are two release verification lanes.

## Crates and install smoke

```powershell
.\scripts\verify-release.ps1 -Version 1.0.0
```

This checks each exact crate version through the crates.io API, checks docs.rs,
runs `cargo search` and `cargo info`, installs the published `prosaic` CLI into
`target/prosaic-install-smoke`, and renders a real JSON-lines event through the
installed binary.

## GitHub binary assets

```powershell
.\scripts\verify-binary-release.ps1 -Version 1.0.0
```

This downloads assets from `wildmason/prosaic`, verifies SHA-256 sidecars for
all configured targets, and smoke-runs the archive that matches the local Rust
host triple.

To verify downloaded assets without calling GitHub again:

```powershell
.\scripts\verify-binary-release.ps1 -Version 1.0.0 -SkipDownload
```

To verify checksums only:

```powershell
.\scripts\verify-binary-release.ps1 -Version 1.0.0 -SkipSmoke
```

macOS verification is intentionally excluded until macOS assets are produced by
a self-hosted runner.

## Release book

Build this book locally with:

```powershell
cargo install mdbook --locked --root target/mdbook
.\scripts\build-release-book.ps1
```

The generated HTML lands under `docs/release/book/`, which is ignored by git.
