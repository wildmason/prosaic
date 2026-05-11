# Prosaic Release Book

This book is the operator path for shipping Prosaic. It covers the lockstep
workspace crates, the `prosaic` CLI crate, GitHub release assets, and the checks
that prove a release can be installed and run by users.

The release model is:

- One workspace version for all public crates.
- One git tag per release, named `vMAJOR.MINOR.PATCH`.
- One crates.io publish pass in dependency order.
- One GitHub release with Windows and Linux CLI archives plus SHA-256 sidecars.
- No GitHub-hosted macOS release jobs until Wildmason has a self-hosted macOS
  runner available.

The canonical automation lives in `scripts/`:

```powershell
.\scripts\release-check.ps1
.\scripts\publish-workspace.ps1 -Yes -RequireHeadTag
.\scripts\verify-release.ps1 -Version 0.6.2
.\scripts\verify-binary-release.ps1 -Version 0.6.2
.\scripts\build-release-book.ps1
```

The GitHub binary release workflow lives at `.github/workflows/release.yml`.
