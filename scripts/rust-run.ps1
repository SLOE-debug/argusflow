<# Start the desktop Rust binary with cache maintenance before and after the run. #>
[CmdletBinding()]
param(
    [ValidateRange(0.001, 1024)]
    [double]$MaxTargetGiB = 8,
    [Parameter(ValueFromRemainingArguments)]
    [string[]]$CargoArguments = @()
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$projectRoot = Split-Path -Parent $PSScriptRoot
$cleanupScript = Join-Path $PSScriptRoot 'clean-rust-cache.ps1'
Push-Location $projectRoot
try {
    & $cleanupScript -MaxTargetGiB $MaxTargetGiB
    & cargo run -p argusflow-desktop @CargoArguments
    if ($LASTEXITCODE -ne 0) {
        throw "Cargo run failed with exit code $LASTEXITCODE."
    }
}
finally {
    try {
        & $cleanupScript -MaxTargetGiB $MaxTargetGiB
    }
    finally {
        Pop-Location
    }
}
