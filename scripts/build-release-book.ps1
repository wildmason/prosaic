[CmdletBinding()]
param(
    [string] $MdBook = "mdbook"
)

. (Join-Path $PSScriptRoot "ProsaicRelease.ps1")

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = Get-ProsaicRepoRoot
Set-Location $repoRoot

$mdbookCommand = Get-Command $MdBook -ErrorAction SilentlyContinue
if ($null -eq $mdbookCommand) {
    $localName = "mdbook"
    if (Test-ProsaicWindowsPlatform) {
        $localName = "mdbook.exe"
    }

    $localMdBook = Join-Path (Join-Path (Join-Path $repoRoot "target") "mdbook") (Join-Path "bin" $localName)
    if (Test-Path $localMdBook) {
        $MdBook = $localMdBook
    } else {
        throw "mdbook is not installed. Install it with: cargo install mdbook --locked --root target/mdbook"
    }
}

Invoke-ProsaicCheckedCommand -FilePath $MdBook -ArgumentList @("build", "docs/release")
