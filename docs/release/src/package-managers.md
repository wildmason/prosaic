# Package Managers

Package-manager updates are post-release metadata changes. Publish crates.io,
push the tag, wait for GitHub Release assets, and run binary verification before
updating the package-manager repos.

## Current availability

| Manager | Repository | Install command | Target |
|---------|------------|-----------------|--------|
| Homebrew | `wildmason/homebrew-tap` | `brew install wildmason/tap/prosaic` | Linux x86_64 |
| Scoop | `wildmason/scoop-bucket` | `scoop install prosaic` | Windows x86_64 |

The Homebrew formula intentionally uses the Linux release archive for now.
macOS Homebrew support requires published macOS archives, which should wait for
a self-hosted Wildmason macOS release runner. macOS users can use
`cargo install prosaic` until then.

## Update flow

1. Verify the release assets:

   ```powershell
   .\scripts\verify-binary-release.ps1 -Version 1.0.0
   ```

2. Update `../homebrew-tap/Formula/prosaic.rb`:

   - `version`
   - Linux archive `url`
   - Linux archive `sha256`

3. Update `../scoop-bucket/bucket/prosaic.json`:

   - `version`
   - Windows archive `url`
   - Windows archive `hash`

4. Verify the package metadata:

   ```powershell
   Get-Content ..\scoop-bucket\bucket\prosaic.json -Raw | ConvertFrom-Json | Out-Null
   ```

   Download each URL declared by the package metadata and compare the SHA-256
   to the package hash. On Windows, also extract the Scoop archive and run a
   JSON-lines smoke test through `prosaic.exe`.

5. Run native package-manager checks where available:

   ```sh
   brew audit --strict --online wildmason/tap/prosaic
   brew test wildmason/tap/prosaic
   ```

   ```powershell
   scoop install .\bucket\prosaic.json
   prosaic --help
   scoop uninstall prosaic
   ```

6. Commit and push each package-manager repo independently.
