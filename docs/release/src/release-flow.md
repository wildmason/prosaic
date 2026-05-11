# Release Flow

Use this path for a normal patch or minor release.

## 1. Prepare the version

Update the root workspace version and every internal dependency version in
workspace manifests. Keep all public crates in lockstep.

```toml
[workspace.package]
version = "1.0.0"
```

Move `CHANGELOG.md` entries from `Unreleased` into the new dated release
section.

## 2. Run the local gate

```powershell
.\scripts\release-check.ps1
```

This runs formatting, the full workspace test suite, and clippy with warnings as
errors. Do not publish from a commit that has not passed this gate.

For an already-published version, or for selected crates whose dependencies are
already live on crates.io, add the publish dry-run:

```powershell
.\scripts\release-check.ps1 -PublishDryRun
```

For a future lockstep version, the publish script performs per-crate dry-runs
immediately before each upload, after the dependency crates have become visible
in crates.io.

## 3. Commit and tag

```powershell
git status --short
git add .
git commit -m "Release Prosaic 1.0.0"
git tag v1.0.0
```

The publish script can require that HEAD carries the release tag:

```powershell
.\scripts\publish-workspace.ps1 -Yes -RequireHeadTag
```

## 4. Push the release commit and tag

```powershell
git push origin main
git push origin v1.0.0
```

Pushing the tag starts the GitHub binary release workflow. The workflow builds
Windows and Linux CLI archives, verifies their SHA-256 sidecars, and creates or
updates the GitHub release for that tag.

## 5. Verify

```powershell
.\scripts\verify-release.ps1 -Version 1.0.0
.\scripts\verify-binary-release.ps1 -Version 1.0.0
```

The first command proves the crates.io and docs.rs side. The second command
downloads the GitHub release assets, verifies checksums for all configured
binary targets, and smoke-runs the archive matching the local host.
