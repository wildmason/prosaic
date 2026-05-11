[CmdletBinding()]
param(
    [string] $Version,
    [string[]] $Crates = @(),
    [switch] $DryRun,
    [switch] $SkipGitChecks,
    [switch] $RequireHeadTag,
    [switch] $SkipPerCrateDryRun,
    [switch] $Yes,
    [int] $MaxAttempts = 12,
    [int] $RetryPaddingSeconds = 15
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
    throw "Requested publish version $Version, but the workspace version is $workspaceVersion."
}

$selectedCrates = Resolve-ProsaicCrateSelection -Crates $Crates

if (-not $SkipGitChecks) {
    Assert-ProsaicGitClean
    if ($RequireHeadTag) {
        Assert-ProsaicGitTagAtHead -Tag "v$Version"
    }
}

if ($DryRun) {
    Write-Host "Dry-running publish for Prosaic $Version."
} else {
    Write-Host "Publishing Prosaic $Version to crates.io."
    if (-not $Yes) {
        $confirmation = Read-Host "Type 'publish $Version' to continue"
        if ($confirmation -ne "publish $Version") {
            throw "Publish aborted."
        }
    }
}

foreach ($crate in $selectedCrates) {
    if (-not $DryRun -and (Test-ProsaicCrateVersionPublished -Crate $crate -Version $Version)) {
        Write-Host "Skipping $crate $Version; crates.io already has this exact version."
        continue
    }

    if (-not $DryRun -and -not $SkipPerCrateDryRun) {
        $dryRunArgs = @("publish", "-p", $crate, "--locked", "--dry-run")
        $dryRunResult = Invoke-ProsaicCapturedCommand -FilePath "cargo" -ArgumentList $dryRunArgs
        if ($dryRunResult.Output.Length -gt 0) {
            Write-Host $dryRunResult.Output
        }

        if ($dryRunResult.ExitCode -ne 0) {
            throw "Pre-publish dry-run failed for $crate."
        }
    }

    $args = @("publish", "-p", $crate, "--locked")
    if ($DryRun) {
        $args += "--dry-run"
    }

    $published = $false
    for ($attempt = 1; $attempt -le $MaxAttempts; $attempt++) {
        if ($MaxAttempts -gt 1) {
            Write-Host "Publishing step for $crate, attempt $attempt/$MaxAttempts."
        }

        $result = Invoke-ProsaicCapturedCommand -FilePath "cargo" -ArgumentList $args
        if ($result.Output.Length -gt 0) {
            Write-Host $result.Output
        }

        if ($result.ExitCode -eq 0) {
            $published = $true
            break
        }

        if (-not $DryRun -and $result.Output -match '(?i)(already exists|already uploaded)') {
            Write-Warning "$crate $Version appears to be already published; confirming through crates.io."
            if (Test-ProsaicCrateVersionPublished -Crate $crate -Version $Version) {
                $published = $true
                break
            }
        }

        if (-not $DryRun) {
            $retryAt = Get-ProsaicCratesIoRetryAt -Text $result.Output
            if ($null -ne $retryAt -and $attempt -lt $MaxAttempts) {
                Wait-ProsaicUntil -When $retryAt -PaddingSeconds $RetryPaddingSeconds
                continue
            }

            if ((Test-ProsaicTransientPublishFailure -Text $result.Output) -and $attempt -lt $MaxAttempts) {
                Write-Warning "Transient publish failure for $crate; sleeping 30 seconds before retry."
                Start-Sleep -Seconds 30
                continue
            }
        }

        throw "cargo publish failed for $crate after attempt $attempt."
    }

    if (-not $published) {
        throw "cargo publish did not complete for $crate after $MaxAttempts attempts."
    }

    if (-not $DryRun) {
        Wait-ProsaicCrateVersionPublished -Crate $crate -Version $Version
    }
}

if ($DryRun) {
    Write-Host "Publish dry-run completed for Prosaic $Version."
} else {
    Write-Host "Publishing completed for Prosaic $Version."
}
