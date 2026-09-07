<#
.SYNOPSIS
Limit this repository's rebuildable Cargo target cache at development boundaries.
.DESCRIPTION
The default budget is 8 GiB. Cargo performs the removal under its own build lock.
No registry, installed tool, recording, workflow or other user data is removed.
#>
[CmdletBinding(SupportsShouldProcess)]
param(
    [ValidateRange(0.001, 1024)]
    [double]$MaxTargetGiB = 8
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# Derive the only allowed cleanup destination from this checked-in script.
$projectRoot = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$targetPath = [IO.Path]::GetFullPath((Join-Path $projectRoot 'target'))
$manifestPath = Join-Path $projectRoot 'Cargo.toml'
if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
    throw "Workspace manifest is missing: $manifestPath"
}
if (-not $targetPath.StartsWith($projectRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing to clean outside the workspace: $targetPath"
}
if (-not (Test-Path -LiteralPath $targetPath)) {
    return
}

# Avoid traversing junctions, including one replacing the project itself.
$ancestor = Get-Item -LiteralPath $projectRoot -Force
while ($null -ne $ancestor) {
    if ($ancestor.Attributes -band [IO.FileAttributes]::ReparsePoint) {
        throw "Refusing cache cleanup through a reparse point: $($ancestor.FullName)"
    }
    $ancestor = $ancestor.Parent
}
$target = Get-Item -LiteralPath $targetPath -Force
if (-not $target.PSIsContainer -or ($target.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
    throw "Refusing cache cleanup of a non-directory or reparse point: $targetPath"
}

# Cargo may be using the same files from an editor, another terminal or the app.
$activeProcesses = @(Get-Process -Name cargo, rustc, argusflow -ErrorAction SilentlyContinue)
if ($activeProcesses.Count -gt 0) {
    Write-Host 'Rust cache cleanup deferred while Cargo, rustc or ArgusFlow is running.' -ForegroundColor DarkCyan
    return
}

# Walk explicitly so a nested junction is rejected before its destination is read.
$pendingDirectories = [Collections.Generic.Stack[string]]::new()
$pendingDirectories.Push($targetPath)
[long]$totalBytes = 0
while ($pendingDirectories.Count -gt 0) {
    foreach ($entry in Get-ChildItem -LiteralPath $pendingDirectories.Pop() -Force) {
        if ($entry.Attributes -band [IO.FileAttributes]::ReparsePoint) {
            throw "Refusing cache cleanup with a nested reparse point: $($entry.FullName)"
        }
        if ($entry.PSIsContainer) {
            $pendingDirectories.Push($entry.FullName)
        }
        else {
            $totalBytes += $entry.Length
        }
    }
}

Write-Host ('Rust target cache: {0:N2} GiB / {1:N2} GiB ({2})' -f ($totalBytes / 1GB), $MaxTargetGiB, $targetPath) -ForegroundColor DarkCyan
if ($totalBytes -le ($MaxTargetGiB * 1GB)) {
    return
}
if ($PSCmdlet.ShouldProcess($targetPath, 'Clean rebuildable Rust artifacts above the cache budget')) {
    # Pass the verified absolute destination, never a global Cargo home or inferred env path.
    & cargo clean --manifest-path $manifestPath --target-dir $targetPath
    if ($LASTEXITCODE -ne 0) {
        throw "Cargo cache cleanup failed with exit code $LASTEXITCODE."
    }
}
