[CmdletBinding()]
param(
    [string] $Version,
    [string] $TargetTriple,
    [string] $OutDir = "target/dist",
    [switch] $SkipBuild,
    [switch] $SkipSmoke
)

. (Join-Path $PSScriptRoot "ProsaicRelease.ps1")

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = Get-ProsaicRepoRoot
Set-Location $repoRoot

$workspaceVersion = Get-ProsaicWorkspaceVersion
if ([string]::IsNullOrWhiteSpace($Version)) {
    $Version = $workspaceVersion
}

if ($Version -ne $workspaceVersion) {
    throw "Requested binary package version $Version, but the workspace version is $workspaceVersion."
}

if ([string]::IsNullOrWhiteSpace($TargetTriple)) {
    $TargetTriple = Get-ProsaicHostTriple
}

$packageName = Get-ProsaicReleasePackageName -Version $Version -TargetTriple $TargetTriple
$archiveName = Get-ProsaicReleaseArchiveName -Version $Version -TargetTriple $TargetTriple
$binaryName = Get-ProsaicBinaryFileName -TargetTriple $TargetTriple

$targetRoot = ConvertTo-ProsaicAbsolutePath -Path "target" -BasePath $repoRoot
$stageRoot = Assert-ProsaicPathInside -Path (Join-Path $targetRoot "prosaic-package") -ParentPath $targetRoot
$stageDir = Assert-ProsaicPathInside -Path (Join-Path $stageRoot $packageName) -ParentPath $stageRoot
$distDir = ConvertTo-ProsaicAbsolutePath -Path $OutDir -BasePath $repoRoot
$archivePath = Join-Path $distDir $archiveName
$checksumPath = "$archivePath.sha256"

if (-not $SkipBuild) {
    $buildArgs = @("build", "--release", "--locked", "-p", "prosaic", "--target", $TargetTriple)
    Invoke-ProsaicCheckedCommand -FilePath "cargo" -ArgumentList $buildArgs
}

$binaryPath = Join-Path (Join-Path (Join-Path $targetRoot $TargetTriple) "release") $binaryName
if (-not (Test-Path $binaryPath)) {
    throw "Expected built binary at $binaryPath."
}

if (Test-Path $stageDir) {
    Remove-Item -LiteralPath $stageDir -Recurse -Force
}

New-Item -ItemType Directory -Path $stageDir -Force | Out-Null
New-Item -ItemType Directory -Path $distDir -Force | Out-Null

$stagedBinaryPath = Join-Path $stageDir $binaryName
Copy-Item -LiteralPath $binaryPath -Destination $stagedBinaryPath
Copy-Item -LiteralPath (Join-Path $repoRoot "README.md") -Destination (Join-Path $stageDir "README.md")
Copy-Item -LiteralPath (Join-Path $repoRoot "LICENSE-MIT") -Destination (Join-Path $stageDir "LICENSE-MIT")
Copy-Item -LiteralPath (Join-Path $repoRoot "LICENSE-APACHE") -Destination (Join-Path $stageDir "LICENSE-APACHE")

$commit = "unknown"
$commitResult = Invoke-ProsaicCapturedCommand -FilePath "git" -ArgumentList @("rev-parse", "HEAD")
if ($commitResult.ExitCode -eq 0 -and -not [string]::IsNullOrWhiteSpace($commitResult.Output)) {
    $commit = $commitResult.Output.Trim()
}

$packageMetadata = @(
    "Prosaic $Version",
    "Target: $TargetTriple",
    "Command: $binaryName",
    "Repository: https://github.com/wildmason/prosaic",
    "Commit: $commit",
    "",
    "Run `$binaryName --help` for CLI usage."
)
Set-Content -LiteralPath (Join-Path $stageDir "PACKAGE.txt") -Value $packageMetadata -Encoding ASCII

if (-not (Test-ProsaicWindowsPlatform)) {
    Invoke-ProsaicCheckedCommand -FilePath "chmod" -ArgumentList @("755", $stagedBinaryPath)
}

if (-not $SkipSmoke) {
    Invoke-ProsaicCheckedCommand -FilePath $stagedBinaryPath -ArgumentList @("--help")

    $smokeInput = '{"key":"code.renamed","entity_type":"class","old_name":"Foo","new_name":"Bar","consumer_count":3}'
    $smokeOutput = $smokeInput | & $stagedBinaryPath @("--vocab", "code", "--strategy", "sequential") 2>&1
    $exitCode = $LASTEXITCODE
    $smokeText = ($smokeOutput | ForEach-Object { $_.ToString() }) -join [Environment]::NewLine

    if ($exitCode -ne 0) {
        throw "Packaged binary smoke test failed with exit code $exitCode`: $smokeText"
    }

    if ($smokeText -notmatch "renamed") {
        throw "Packaged binary smoke test produced unexpected output: $smokeText"
    }

    Write-Host "Packaged binary smoke output: $smokeText"
}

if (Test-Path $archivePath) {
    Remove-Item -LiteralPath $archivePath -Force
}
if (Test-Path $checksumPath) {
    Remove-Item -LiteralPath $checksumPath -Force
}

if ((Get-ProsaicBinaryArchiveExtension -TargetTriple $TargetTriple) -eq "zip") {
    Compress-Archive -Path $stageDir -DestinationPath $archivePath -Force
} else {
    Push-Location $stageRoot
    try {
        Invoke-ProsaicCheckedCommand -FilePath "tar" -ArgumentList @("-czf", $archivePath, $packageName)
    } finally {
        Pop-Location
    }
}

$hash = (Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash.ToLowerInvariant()
Set-Content -LiteralPath $checksumPath -Value "$hash  $archiveName" -Encoding ASCII

Write-Host "Wrote $archivePath"
Write-Host "Wrote $checksumPath"
