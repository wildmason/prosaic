# Changelog

## Unreleased

## 0.6.2 - 2026-05-11

- Added release automation scripts for preflight checks, ordered crates.io
  publishing, rate-limit-aware retries, and post-release verification.
- Added GitHub Actions CI for formatting, tests, and clippy across the workspace.
- Tracked `Cargo.lock` so `--locked` release and CI gates are reproducible on
  fresh machines.
- Added Windows/Linux binary release packaging, checksum verification, and a
  GitHub Release workflow for the `prosaic` CLI.
- Added a Prosaic release book covering crates.io publishing, binary assets,
  verification, and recovery.

## 0.6.1 - 2026-05-11

- Initial public crates.io release of the Prosaic workspace.
- Published the CLI package as `prosaic`, installing the `prosaic` command.
- Documented the lockstep workspace versioning scheme and crate publish order.
