[CmdletBinding()]
param(
    [string] $Version,
    [string] $Tag,
    [string] $Repository = "wildmason/prosaic",
    [string[]] $Targets = @("x86_64-unknown-linux-gnu", "x86_64-pc-windows-msvc"),
    [string] $DownloadRoot = "target/prosaic-binary-release-smoke",
    [switch] $SkipDownload,
    [switch] $SkipSmoke
)

. (Join-Path $PSScriptRoot "ProsaicRelease.ps1")

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = Get-ProsaicRepoRoot
Set-Location $repoRoot

if ([string]::IsNullOrWhiteSpace($Version)) {
    if (-not [string]::IsNullOrWhiteSpace($Tag) -and $Tag -match '^v(.+)$') {
        $Version = $matches[1]
    } else {
        $Version = Get-ProsaicWorkspaceVersion
    }
}

if ([string]::IsNullOrWhiteSpace($Tag)) {
    $Tag = "v$Version"
}

$downloadBase = ConvertTo-ProsaicAbsolutePath -Path $DownloadRoot -BasePath $repoRoot
$safeTag = $Tag -replace '[\\/:*?"<>|]', '_'
$downloadDir = Assert-ProsaicPathInside -Path (Join-Path $downloadBase $safeTag) -ParentPath $downloadBase

if (-not $SkipDownload) {
    if (Test-Path $downloadDir) {
        Remove-Item -LiteralPath $downloadDir -Recurse -Force
    }

    New-Item -ItemType Directory -Path $downloadDir -Force | Out-Null

    $pattern = "prosaic-$Tag-*"
    Invoke-ProsaicCheckedCommand -FilePath "gh" -ArgumentList @(
        "release",
        "download",
        $Tag,
        "--repo",
        $Repository,
        "--dir",
        $downloadDir,
        "--clobber",
        "--pattern",
        $pattern
    )
} elseif (-not (Test-Path $downloadDir)) {
    throw "Download directory does not exist: $downloadDir"
}

$hostTriple = $null
if (-not $SkipSmoke) {
    try {
        $hostTriple = Get-ProsaicHostTriple
    } catch {
        Write-Warning "Could not determine host triple; skipping binary smoke tests. $($_.Exception.Message)"
        $SkipSmoke = $true
    }
}

foreach ($target in $Targets) {
    $packageName = Get-ProsaicReleasePackageName -Version $Version -TargetTriple $target
    $archiveName = Get-ProsaicReleaseArchiveName -Version $Version -TargetTriple $target
    $archivePath = Join-Path $downloadDir $archiveName
    $checksumPath = "$archivePath.sha256"

    if (-not (Test-Path $archivePath)) {
        throw "Missing release asset: $archivePath"
    }
    if (-not (Test-Path $checksumPath)) {
        throw "Missing checksum sidecar: $checksumPath"
    }

    $checksumText = (Get-Content -Raw -LiteralPath $checksumPath).Trim()
    if ($checksumText -notmatch '^([a-fA-F0-9]{64})\s+(.+)$') {
        throw "Checksum sidecar has unexpected format: $checksumPath"
    }

    $expectedHash = $matches[1].ToLowerInvariant()
    $expectedName = $matches[2].Trim()
    if ($expectedName -ne $archiveName) {
        throw "Checksum sidecar names $expectedName, expected $archiveName."
    }

    $actualHash = (Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actualHash -ne $expectedHash) {
        throw "Checksum mismatch for $archiveName. Expected $expectedHash, got $actualHash."
    }

    Write-Host "Checksum verified for $archiveName."

    if ($SkipSmoke -or $target -ne $hostTriple) {
        Write-Host "Skipping smoke test for $target on host $hostTriple."
        continue
    }

    $extractRoot = Assert-ProsaicPathInside -Path (Join-Path $downloadDir "extract-$target") -ParentPath $downloadDir
    if (Test-Path $extractRoot) {
        Remove-Item -LiteralPath $extractRoot -Recurse -Force
    }
    New-Item -ItemType Directory -Path $extractRoot -Force | Out-Null

    if ((Get-ProsaicBinaryArchiveExtension -TargetTriple $target) -eq "zip") {
        Expand-Archive -LiteralPath $archivePath -DestinationPath $extractRoot -Force
    } else {
        Push-Location $extractRoot
        try {
            Invoke-ProsaicCheckedCommand -FilePath "tar" -ArgumentList @("-xzf", $archivePath)
        } finally {
            Pop-Location
        }
    }

    $binaryName = Get-ProsaicBinaryFileName -TargetTriple $target
    $binaryPath = Join-Path (Join-Path $extractRoot $packageName) $binaryName
    if (-not (Test-Path $binaryPath)) {
        $binaryPath = Join-Path $extractRoot $binaryName
    }
    if (-not (Test-Path $binaryPath)) {
        throw "Expected extracted binary for $target under $extractRoot."
    }

    Invoke-ProsaicCheckedCommand -FilePath $binaryPath -ArgumentList @("--help")

    $smokeInput = '{"key":"code.renamed","entity_type":"class","old_name":"Foo","new_name":"Bar","consumer_count":3}'
    $smokeOutput = $smokeInput | & $binaryPath @("--vocab", "code", "--strategy", "sequential") 2>&1
    $exitCode = $LASTEXITCODE
    $smokeText = ($smokeOutput | ForEach-Object { $_.ToString() }) -join [Environment]::NewLine

    if ($exitCode -ne 0) {
        throw "Release binary smoke test failed with exit code $exitCode`: $smokeText"
    }

    if ($smokeText -notmatch "renamed") {
        throw "Release binary smoke test produced unexpected output: $smokeText"
    }

    Write-Host "Release binary smoke output for $target`: $smokeText"
}

Write-Host "Binary release verification completed for $Tag."
