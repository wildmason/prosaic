[CmdletBinding()]
param(
    [string] $Version,
    [string[]] $Crates = @(),
    [switch] $SkipDocs,
    [switch] $SkipInstall,
    [string] $InstallRoot = "target/prosaic-install-smoke",
    [int] $HttpRetries = 6,
    [int] $HttpRetrySeconds = 20
)

. (Join-Path $PSScriptRoot "ProsaicRelease.ps1")

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = Get-ProsaicRepoRoot
Set-Location $repoRoot

if ([string]::IsNullOrWhiteSpace($Version)) {
    $Version = Get-ProsaicWorkspaceVersion
}

$selectedCrates = Resolve-ProsaicCrateSelection -Crates $Crates

Write-Host "Verifying Prosaic $Version release artifacts."

foreach ($crate in $selectedCrates) {
    $confirmed = $false
    for ($attempt = 1; $attempt -le $HttpRetries; $attempt++) {
        try {
            $response = Invoke-ProsaicCratesIoVersion -Crate $crate -Version $Version
            if ($response.version.yanked) {
                throw "$crate $Version is yanked on crates.io."
            }

            Write-Host "crates.io confirms $crate $Version."
            $confirmed = $true
            break
        } catch {
            if ($attempt -ge $HttpRetries) {
                throw
            }

            Write-Warning "Attempt $attempt/$HttpRetries failed for crates.io $crate $Version`: $($_.Exception.Message)"
            Start-Sleep -Seconds $HttpRetrySeconds
        }
    }

    if (-not $confirmed) {
        throw "Could not confirm $crate $Version on crates.io."
    }
}

if (-not $SkipDocs) {
    foreach ($crate in $selectedCrates) {
        $uri = "https://docs.rs/$crate/$Version"
        $null = Invoke-ProsaicUrlWithRetry -Uri $uri -Attempts $HttpRetries -DelaySeconds $HttpRetrySeconds
        Write-Host "docs.rs responds for $crate $Version."
    }
}

Invoke-ProsaicCheckedCommand -FilePath "cargo" -ArgumentList @("search", "prosaic", "--limit", "5")
Invoke-ProsaicCheckedCommand -FilePath "cargo" -ArgumentList @("info", "prosaic", "--registry", "crates-io")

if (-not $SkipInstall) {
    if (-not [System.IO.Path]::IsPathRooted($InstallRoot)) {
        $InstallRoot = Join-Path $repoRoot $InstallRoot
    }

    Invoke-ProsaicCheckedCommand -FilePath "cargo" -ArgumentList @("install", "prosaic", "--version", $Version, "--root", $InstallRoot, "--force")

    $exeName = "prosaic"
    if ([System.Environment]::OSVersion.Platform -eq [System.PlatformID]::Win32NT) {
        $exeName = "prosaic.exe"
    }

    $exePath = Join-Path (Join-Path $InstallRoot "bin") $exeName
    if (-not (Test-Path $exePath)) {
        throw "Expected installed Prosaic binary at $exePath."
    }

    Invoke-ProsaicCheckedCommand -FilePath $exePath -ArgumentList @("--help")

    $smokeInput = '{"key":"code.renamed","entity_type":"class","old_name":"Foo","new_name":"Bar","consumer_count":3}'
    $smokeOutput = $smokeInput | & $exePath @("--vocab", "code", "--strategy", "sequential") 2>&1
    $exitCode = $LASTEXITCODE
    $smokeText = ($smokeOutput | ForEach-Object { $_.ToString() }) -join [Environment]::NewLine

    if ($exitCode -ne 0) {
        throw "Installed Prosaic smoke test failed with exit code $exitCode`: $smokeText"
    }

    if ($smokeText -notmatch "renamed") {
        throw "Installed Prosaic smoke test produced unexpected output: $smokeText"
    }

    Write-Host "Installed Prosaic smoke output: $smokeText"
}

Write-Host "Release verification completed for Prosaic $Version."
