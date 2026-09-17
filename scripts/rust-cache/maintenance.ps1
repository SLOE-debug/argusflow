# Measure only real directories; a junction must never expand the cleanup scope.
function Get-RustCacheBytes {
    param([string]$Path)
    $item = Get-Item -LiteralPath $Path -Force
    if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
        throw "Refusing linked cache path: $Path"
    }
    [long]$bytes = 0
    foreach ($entry in Get-ChildItem -LiteralPath $Path -Force) {
        if ($entry.Attributes -band [IO.FileAttributes]::ReparsePoint) {
            throw "Refusing linked cache entry: $($entry.FullName)"
        }
        if ($entry.PSIsContainer) {
            $bytes += Get-RustCacheBytes -Path $entry.FullName
        } else {
            $bytes += $entry.Length
        }
    }
    return $bytes
}

# Clean Cargo artifacts as a coherent set, preserving registry downloads and models.
# Refuse concurrent Rust work conservatively, including work in other repositories.
function Invoke-RustCacheMaintenance {
    param(
        [Parameter(Mandatory)][string]$WorkspaceRoot,
        [Parameter(Mandatory)][ValidateRange(1, 1048576)][double]$MaxGiB,
        [switch]$DryRun
    )
    $root = (Resolve-Path -LiteralPath $WorkspaceRoot).ProviderPath
    $target = [IO.Path]::GetFullPath((Join-Path $root "target"))
    if (-not (Test-Path -LiteralPath $target)) {
        Write-Host "[rust-cache] No target cache. Limit: $MaxGiB GiB."
        return
    }
    # Check the absolute deletion boundary before passing it to Cargo.
    if ((Split-Path -Parent $target) -ne $root -or (Split-Path -Leaf $target) -ne "target") {
        throw "Cache target must be the workspace's direct target directory: $target"
    }
    $bytes = Get-RustCacheBytes -Path $target
    Write-Host ("[rust-cache] {0:N2} GiB / {1} GiB: {2}" -f ($bytes / 1GB), $MaxGiB, $target)
    if ($bytes -le ($MaxGiB * 1GB)) { return }
    if ($DryRun) {
        Write-Host "[rust-cache] Would run cargo clean. The next Rust build will rebuild artifacts."
        return
    }
    if (Get-Process -Name cargo,rustc,rustdoc,argusflow -ErrorAction SilentlyContinue) {
        Write-Warning "[rust-cache] Limit exceeded, but Rust work/app is running. Run cache:prune again when idle."
        return
    }
    Write-Host "[rust-cache] Limit exceeded; cleaning build artifacts. The next Rust build will take longer."
    & cargo clean --manifest-path (Join-Path $root "Cargo.toml") --target-dir $target
    if ($LASTEXITCODE -ne 0) { throw "Rust cache cleanup failed (exit $LASTEXITCODE)." }
}
