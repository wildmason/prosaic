# Changelog

## Unreleased

## 1.0.1 - 2026-05-20

- Added package-specific README files and manifest `readme` metadata for all
  published Prosaic crates so crates.io and GitHub show crate-level
  documentation.
- Updated workspace and internal dependency versions for the lockstep 1.0.1
  patch release.

## 1.0.0 - 2026-05-11

- Declared the 1.x stability contract for crates, CLI behavior, template
  syntax, pipe metadata, project schemas, and bundle schema version 1.
- Added semver-facing contract tests for core rendering, the built-in pipe
  registry, project bundling, project validation, CLI rendering, explain JSON,
  and project subcommands.
- Fixed project validation to use the `prosaic-core` pipe registry, eliminating
  a stale duplicate list that missed the supported `possessive` pipe.
- Updated workspace crate versions, lockfile metadata, install snippets, and
  release documentation for the 1.0.0 line.

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
