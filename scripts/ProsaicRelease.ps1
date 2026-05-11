Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$script:ProsaicCrateOrder = @(
    "prosaic-common",
    "prosaic-core",
    "prosaic-derive",
    "prosaic-grammar-en",
    "prosaic-grammar-es",
    "prosaic-grammar-de",
    "prosaic-vocab-code",
    "prosaic-vocab-git",
    "prosaic-vocab-pr",
    "prosaic-vocab-release",
    "prosaic-project",
    "prosaic-tracing",
    "prosaic-wasm",
    "prosaic"
)

function Get-ProsaicRepoRoot {
    $root = Resolve-Path (Join-Path $PSScriptRoot "..")
    return $root.ProviderPath
}

function Get-ProsaicWorkspaceVersion {
    $cargoTomlPath = Join-Path (Get-ProsaicRepoRoot) "Cargo.toml"
    $content = Get-Content -Raw $cargoTomlPath
    $inWorkspacePackage = $false

    foreach ($line in ($content -split "`r?`n")) {
        $trimmed = $line.Trim()
        if ($trimmed -match '^\[workspace\.package\]$') {
            $inWorkspacePackage = $true
            continue
        }

        if ($trimmed -match '^\[') {
            $inWorkspacePackage = $false
            continue
        }

        if ($inWorkspacePackage -and $trimmed -match '^version\s*=\s*"([^"]+)"') {
            return $matches[1]
        }
    }

    throw "Could not find [workspace.package] version in $cargoTomlPath"
}

function Resolve-ProsaicCrateSelection {
    param(
        [string[]] $Crates = @()
    )

    if ($null -eq $Crates -or $Crates.Count -eq 0) {
        return @($script:ProsaicCrateOrder)
    }

    $requested = @{}
    foreach ($crate in $Crates) {
        if (-not ($script:ProsaicCrateOrder -contains $crate)) {
            throw "Unknown Prosaic crate '$crate'. Known crates: $($script:ProsaicCrateOrder -join ', ')"
        }
        $requested[$crate] = $true
    }

    return @($script:ProsaicCrateOrder | Where-Object { $requested.ContainsKey($_) })
}

function Format-ProsaicCommand {
    param(
        [Parameter(Mandatory = $true)] [string] $FilePath,
        [string[]] $ArgumentList = @()
    )

    $parts = @($FilePath) + $ArgumentList
    return ($parts | ForEach-Object {
        if ($_ -match '\s') {
            '"' + $_ + '"'
        } else {
            $_
        }
    }) -join " "
}

function Invoke-ProsaicCheckedCommand {
    param(
        [Parameter(Mandatory = $true)] [string] $FilePath,
        [string[]] $ArgumentList = @()
    )

    Write-Host "> $(Format-ProsaicCommand -FilePath $FilePath -ArgumentList $ArgumentList)"
    & $FilePath @ArgumentList
    $exitCode = $LASTEXITCODE

    if ($exitCode -ne 0) {
        throw "Command failed with exit code $exitCode`: $(Format-ProsaicCommand -FilePath $FilePath -ArgumentList $ArgumentList)"
    }
}

function Invoke-ProsaicCapturedCommand {
    param(
        [Parameter(Mandatory = $true)] [string] $FilePath,
        [string[]] $ArgumentList = @()
    )

    Write-Host "> $(Format-ProsaicCommand -FilePath $FilePath -ArgumentList $ArgumentList)"
    $previousErrorActionPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $output = & $FilePath @ArgumentList 2>&1
        $exitCode = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $previousErrorActionPreference
    }

    $text = ($output | ForEach-Object { $_.ToString() }) -join [Environment]::NewLine

    return [pscustomobject]@{
        ExitCode = $exitCode
        Output = $text
    }
}

function Get-ProsaicHttpHeaders {
    return @{
        "User-Agent" = "wildmason-prosaic-release-scripts"
    }
}

function Get-ProsaicHttpStatusCode {
    param(
        [Parameter(Mandatory = $true)] $ErrorRecord
    )

    try {
        if ($null -ne $ErrorRecord.Exception.Response) {
            return [int] $ErrorRecord.Exception.Response.StatusCode
        }
    } catch {
        return $null
    }

    return $null
}

function Invoke-ProsaicCratesIoVersion {
    param(
        [Parameter(Mandatory = $true)] [string] $Crate,
        [Parameter(Mandatory = $true)] [string] $Version
    )

    $uri = "https://crates.io/api/v1/crates/$Crate/$Version"
    return Invoke-RestMethod -Uri $uri -Headers (Get-ProsaicHttpHeaders) -TimeoutSec 30
}

function Test-ProsaicCrateVersionPublished {
    param(
        [Parameter(Mandatory = $true)] [string] $Crate,
        [Parameter(Mandatory = $true)] [string] $Version
    )

    try {
        $null = Invoke-ProsaicCratesIoVersion -Crate $Crate -Version $Version
        return $true
    } catch {
        $statusCode = Get-ProsaicHttpStatusCode -ErrorRecord $_
        if ($statusCode -eq 404) {
            return $false
        }
        throw
    }
}

function Wait-ProsaicCrateVersionPublished {
    param(
        [Parameter(Mandatory = $true)] [string] $Crate,
        [Parameter(Mandatory = $true)] [string] $Version,
        [int] $Attempts = 30,
        [int] $DelaySeconds = 10
    )

    for ($attempt = 1; $attempt -le $Attempts; $attempt++) {
        try {
            if (Test-ProsaicCrateVersionPublished -Crate $Crate -Version $Version) {
                Write-Host "crates.io confirms $Crate $Version."
                return
            }
        } catch {
            Write-Warning "Could not confirm $Crate $Version on crates.io yet: $($_.Exception.Message)"
        }

        if ($attempt -lt $Attempts) {
            Write-Host "Waiting $DelaySeconds seconds for crates.io to expose $Crate $Version..."
            Start-Sleep -Seconds $DelaySeconds
        }
    }

    throw "Timed out waiting for crates.io to expose $Crate $Version."
}

function Assert-ProsaicGitClean {
    $status = & git status --porcelain
    if ($LASTEXITCODE -ne 0) {
        throw "git status failed."
    }

    if ($status.Count -gt 0) {
        throw "Working tree is dirty. Commit or stash changes before publishing.`n$($status -join [Environment]::NewLine)"
    }
}

function Assert-ProsaicGitTagAtHead {
    param(
        [Parameter(Mandatory = $true)] [string] $Tag
    )

    $tags = & git tag --points-at HEAD
    if ($LASTEXITCODE -ne 0) {
        throw "git tag --points-at HEAD failed."
    }

    if (-not ($tags -contains $Tag)) {
        throw "HEAD is not tagged $Tag. Create the release tag or pass -RequireHeadTag:`$false."
    }
}

function Get-ProsaicCratesIoRetryAt {
    param(
        [Parameter(Mandatory = $true)] [string] $Text
    )

    if ($Text -match '(?i)try again after\s+(?<date>[A-Za-z]{3},\s+\d{1,2}\s+[A-Za-z]{3}\s+\d{4}\s+\d{2}:\d{2}:\d{2}\s+GMT)') {
        $culture = [System.Globalization.CultureInfo]::InvariantCulture
        $styles = [System.Globalization.DateTimeStyles]::AssumeUniversal -bor [System.Globalization.DateTimeStyles]::AdjustToUniversal
        return [DateTimeOffset]::Parse($matches["date"], $culture, $styles)
    }

    return $null
}

function Test-ProsaicTransientPublishFailure {
    param(
        [Parameter(Mandatory = $true)] [string] $Text
    )

    return $Text -match '(?i)(rate limited|too many requests|no matching package named|failed to get .*index|timed out|timeout|connection reset|connection refused)'
}

function Wait-ProsaicUntil {
    param(
        [Parameter(Mandatory = $true)] [DateTimeOffset] $When,
        [int] $PaddingSeconds = 15
    )

    $seconds = [Math]::Ceiling(($When - [DateTimeOffset]::UtcNow).TotalSeconds) + $PaddingSeconds
    if ($seconds -lt 1) {
        $seconds = 1
    }

    Write-Host "crates.io asked us to retry at $($When.UtcDateTime.ToString("u")); sleeping $seconds seconds."
    Start-Sleep -Seconds ([int] $seconds)
}

function Invoke-ProsaicUrlWithRetry {
    param(
        [Parameter(Mandatory = $true)] [string] $Uri,
        [int] $Attempts = 6,
        [int] $DelaySeconds = 20
    )

    for ($attempt = 1; $attempt -le $Attempts; $attempt++) {
        try {
            $response = Invoke-WebRequest -Uri $Uri -UseBasicParsing -Method Get -Headers (Get-ProsaicHttpHeaders) -TimeoutSec 30
            if ([int] $response.StatusCode -ge 200 -and [int] $response.StatusCode -lt 400) {
                return $response
            }

            throw "Unexpected HTTP status $($response.StatusCode) from $Uri"
        } catch {
            if ($attempt -ge $Attempts) {
                throw
            }

            Write-Warning "Attempt $attempt/$Attempts failed for $Uri`: $($_.Exception.Message)"
            Start-Sleep -Seconds $DelaySeconds
        }
    }
}
