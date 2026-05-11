# Binary Assets

The binary release workflow packages the `prosaic` CLI for:

```text
x86_64-unknown-linux-gnu
x86_64-pc-windows-msvc
```

macOS assets are intentionally absent for now. Add macOS targets only after a
self-hosted Wildmason macOS runner is available; do not use `macos-latest` for
routine release work.

## Asset names

Each archive includes the binary, README, both license files, and a small
`PACKAGE.txt` metadata file.

```text
prosaic-v0.6.2-x86_64-unknown-linux-gnu.tar.gz
prosaic-v0.6.2-x86_64-unknown-linux-gnu.tar.gz.sha256
prosaic-v0.6.2-x86_64-pc-windows-msvc.zip
prosaic-v0.6.2-x86_64-pc-windows-msvc.zip.sha256
```

The SHA-256 sidecar format is compatible with `sha256sum -c`:

```text
<sha256>  <archive-name>
```

## Local packaging

Build and package the host target:

```powershell
.\scripts\package-binary.ps1
```

Build a specific target:

```powershell
.\scripts\package-binary.ps1 -Version 0.6.2 -TargetTriple x86_64-pc-windows-msvc
```

The script runs `cargo build --release --locked -p prosaic --target <triple>`,
stages the package under `target/prosaic-package/`, smoke-tests the staged
binary, writes the archive to `target/dist/`, and writes the checksum sidecar.

## GitHub workflow

`.github/workflows/release.yml` runs on `v*.*.*` tag pushes and manual dispatch.
It packages Linux and Windows assets in parallel, downloads the artifacts into a
release job, verifies every checksum, and then creates or updates the GitHub
release.

Manual dispatch can rebuild assets for an existing tag:

```text
workflow: Binary Release
input tag: v0.6.2
```
