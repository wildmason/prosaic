# Crates.io Publishing

Prosaic publishes all public crates from one workspace version. The CLI package
is named `prosaic`, and it installs the `prosaic` command.

The publish order is encoded in `scripts/ProsaicRelease.ps1`:

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

Run the publish command from a clean, tagged release commit:

```powershell
.\scripts\publish-workspace.ps1 -Yes -RequireHeadTag
```

The script:

- Reads the workspace version from the root `Cargo.toml`.
- Publishes crates in dependency order.
- Skips exact crate versions already present on crates.io.
- Dry-runs each crate immediately before upload unless
  `-SkipPerCrateDryRun` is set.
- Waits for each just-published crate to become visible in the crates.io API.
- Parses crates.io retry timestamps and sleeps through rate limits.

Crates.io versions are immutable. If a bad artifact is uploaded, yank that
version if needed and ship a new patch version. Do not try to overwrite a
published version.
