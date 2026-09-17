# Run with powershell -NoProfile -File tests/scripts/rust-cache.test.ps1.
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "../../scripts/rust-cache/maintenance.ps1")

function Assert-CacheTest {
    param([bool]$Condition, [string]$Message)
    if (-not $Condition) { throw $Message }
}

# All filesystem fixtures stay under a uniquely owned temporary directory.
$fixture = Join-Path ([IO.Path]::GetTempPath()) ("argusflow-cache-test-" + [guid]::NewGuid())
$target = Join-Path $fixture "target"
New-Item -ItemType Directory -Path (Join-Path $target "debug") -Force | Out-Null
try {
    [IO.File]::WriteAllBytes((Join-Path $target "debug/data"), [byte[]](1, 2, 3))
    Assert-CacheTest ((Get-RustCacheBytes $target) -eq 3) "Nested file sizes must be counted exactly."

    # Simulate a large cache without allocating GiB; intercept the external cleaner.
    $script:cacheBytes = 10GB
    $script:busy = $false
    $script:cleanCalls = 0
    function Get-RustCacheBytes { param([string]$Path) return $script:cacheBytes }
    function Get-Process {
        param($Name, $ErrorAction)
        if ($script:busy) { return [pscustomobject]@{ ProcessName = "cargo" } }
    }
    function cargo {
        Assert-CacheTest ($args[0] -eq "clean") "Must use cargo clean."
        Assert-CacheTest ($args[2] -eq (Join-Path $fixture "Cargo.toml")) "Wrong workspace manifest."
        Assert-CacheTest ($args[4] -eq $target) "Cleanup must stay in the fixture target."
        $script:cleanCalls++
        $global:LASTEXITCODE = 0
    }

    Invoke-RustCacheMaintenance -WorkspaceRoot $fixture -MaxGiB 10
    Assert-CacheTest ($script:cleanCalls -eq 0) "At the limit must retain cache."
    $script:cacheBytes++
    Invoke-RustCacheMaintenance -WorkspaceRoot $fixture -MaxGiB 10 -DryRun
    Assert-CacheTest ($script:cleanCalls -eq 0) "Dry run must not clean."
    $script:busy = $true
    Invoke-RustCacheMaintenance -WorkspaceRoot $fixture -MaxGiB 10
    Assert-CacheTest ($script:cleanCalls -eq 0) "Active Rust processes must defer cleanup."
    $script:busy = $false
    Invoke-RustCacheMaintenance -WorkspaceRoot $fixture -MaxGiB 10
    Assert-CacheTest ($script:cleanCalls -eq 1) "Idle oversized cache must clean once."

    function cargo { $global:LASTEXITCODE = 1 }
    $failed = $false
    try { Invoke-RustCacheMaintenance -WorkspaceRoot $fixture -MaxGiB 10 } catch { $failed = $true }
    Assert-CacheTest $failed "Cargo failure must propagate."
    Write-Host "Passed 6 Rust cache checks."
} finally {
    $resolvedFixture = (Resolve-Path -LiteralPath $fixture).ProviderPath
    $tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\')
    if ((Split-Path -Parent $resolvedFixture) -ne $tempRoot -or
        (Split-Path -Leaf $resolvedFixture) -notlike "argusflow-cache-test-*") {
        throw "Unsafe fixture cleanup path: $resolvedFixture"
    }
    Remove-Item -LiteralPath $resolvedFixture -Recurse -Force
}
