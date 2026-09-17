param(
    # GiB threshold for explicit maintenance, not a startup check or live quota.
    [ValidateRange(1, 1048576)]
    [double]$MaxGiB = 10,
    [switch]$DryRun
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "rust-cache/maintenance.ps1")
Invoke-RustCacheMaintenance -WorkspaceRoot (Split-Path -Parent $PSScriptRoot) -MaxGiB $MaxGiB -DryRun:$DryRun
