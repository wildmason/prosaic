[CmdletBinding()]
param(
    [string[]] $Crates = @(),
    [switch] $SkipFmt,
    [switch] $SkipTests,
    [switch] $SkipClippy,
    [switch] $PublishDryRun,
    [switch] $SkipPublishDryRun,
    [switch] $AllowDirtyDryRun
)

. (Join-Path $PSScriptRoot "ProsaicRelease.ps1")

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = Get-ProsaicRepoRoot
Set-Location $repoRoot

$version = Get-ProsaicWorkspaceVersion
$selectedCrates = Resolve-ProsaicCrateSelection -Crates $Crates

Write-Host "Running Prosaic release checks for workspace version $version."

if (-not $SkipFmt) {
    Invoke-ProsaicCheckedCommand -FilePath "cargo" -ArgumentList @("fmt", "--all", "--", "--check")
}

if (-not $SkipTests) {
    Invoke-ProsaicCheckedCommand -FilePath "cargo" -ArgumentList @("test", "--workspace", "--locked")
}

if (-not $SkipClippy) {
    Invoke-ProsaicCheckedCommand -FilePath "cargo" -ArgumentList @("clippy", "--workspace", "--all-targets", "--locked", "--", "-D", "warnings")
}

if ($SkipPublishDryRun) {
    Write-Host "Skipping publish dry-runs."
} elseif ($PublishDryRun) {
    foreach ($crate in $selectedCrates) {
        $args = @("publish", "-p", $crate, "--dry-run", "--locked")
        if ($AllowDirtyDryRun) {
            $args += "--allow-dirty"
        }

        Invoke-ProsaicCheckedCommand -FilePath "cargo" -ArgumentList $args
    }
} else {
    Write-Host "Skipping publish dry-runs by default. Pass -PublishDryRun when the selected crate versions are registry-resolvable."
}

Write-Host "Release checks completed for Prosaic $version."
