# 1.0 Stability Policy

Prosaic 1.0 commits to SemVer for the published Rust crates, the `prosaic`
command-line interface, and the folder-of-files project format.

## Stable Contract

The following surfaces are stable across the 1.x line. Breaking changes require
a 2.0 release.

- Published package names and the lockstep workspace version scheme.
- The `prosaic` crates.io package name and installed `prosaic` binary name.
- CLI render input: newline-delimited JSON objects with a required `key` field
  and string, integer, boolean, null, or string-array slot values.
- CLI render flags: `--preset`, `--vocab`, `--strategy`, `--smart-quotes`,
  `--max-length`, `--style`, `--explain`, `--strict`, `--lenient`, and
  `--silent`.
- CLI project subcommands: `prosaic new`, `prosaic build`, and `prosaic test`,
  including the `blank`, `changelog`, and `vocab-pack` starters and the
  `json`, `rust`, and `both` build targets.
- Template syntax: slots, pipe chains, pipe arguments, conditional sections,
  and partial inclusions.
- Built-in pipe names and their input/output type contract.
- `prosaic.toml`, `templates/*.toml`, `partials/*.toml`, `fixtures/*.json`,
  and `tests/*.toml` project schemas.
- JSON bundle schema version 1 produced by `prosaic build --target=json` and
  loaded by `Engine::load_manifest`.
- Public Rust APIs exported by the workspace crates, including the engine,
  context, template, document-plan, grammar, vocabulary, project, tracing, and
  wasm binding entry points.
- Cargo features documented in the README and cookbook.

## Evolution Within 1.x

The 1.x line may add new crates, flags, fields, enum variants, pipe names,
starter files, style profiles, or vocabulary templates when the addition is
backwards compatible.

The exact prose produced by newly added templates is not itself a SemVer
boundary. Existing golden outputs in the contract test suite are stable.
Additional deterministic phrasing may improve in minor releases when it does not
break documented inputs, schemas, or APIs.

GitHub Release binary assets are part of the distribution story, but the
platform matrix can grow over time. Windows and Linux assets are required for a
normal release. macOS assets remain paused until Wildmason has a self-hosted
macOS runner.

## Release Gate

Before a 1.x release is tagged or published, run the release gate from a clean
worktree:

```powershell
.\scripts\release-check.ps1
```

For an already registry-resolvable version, or a subset whose dependencies are
already live, also run the dry-run publish gate:

```powershell
.\scripts\release-check.ps1 -PublishDryRun
```

Every 1.x release must also keep these contract suites passing:

```powershell
cargo test -p prosaic-core --test stability_contract --locked
cargo test -p prosaic-project --test stability_contract --locked
cargo test -p prosaic --test stability_contract --locked
```

After publishing, verify crates.io, docs.rs, install smoke, binary checksums,
and the matching GitHub Release:

```powershell
.\scripts\verify-release.ps1 -Version 1.0.0
.\scripts\verify-binary-release.ps1 -Version 1.0.0
```

Crates.io versions are immutable. If any published artifact is wrong, fix the
issue and ship a new patch version.
