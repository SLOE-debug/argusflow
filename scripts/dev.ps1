[CmdletBinding()]
param(
    [string]$Proxy = "",
    [switch]$SkipInstall
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if ($env:OS -ne "Windows_NT") {
    throw "ArgusFlow only supports Windows."
}

$projectRoot = Split-Path -Parent $PSScriptRoot
$nodeModulesPath = Join-Path $projectRoot "node_modules"
$lockfilePath = Join-Path $projectRoot "pnpm-lock.yaml"

if (-not (Get-Command pnpm -ErrorAction SilentlyContinue)) {
    throw "pnpm was not found. Install pnpm before starting ArgusFlow."
}

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw "Cargo was not found. Install the Rust MSVC toolchain before starting ArgusFlow."
}

if ($Proxy) {
    $env:HTTP_PROXY = $Proxy
    $env:HTTPS_PROXY = $Proxy
    Write-Host "Using proxy: $Proxy" -ForegroundColor DarkCyan
}

Push-Location $projectRoot
try {
    if (-not $SkipInstall -and -not (Test-Path -LiteralPath $nodeModulesPath)) {
        if (-not (Test-Path -LiteralPath $lockfilePath)) {
            throw "pnpm-lock.yaml is missing; refusing an unlocked dependency install."
        }

        Write-Host "Installing frontend dependencies from pnpm-lock.yaml..." -ForegroundColor Cyan
        & pnpm install --frozen-lockfile
        if ($LASTEXITCODE -ne 0) {
            throw "pnpm install failed with exit code $LASTEXITCODE."
        }
    }

    Write-Host "Starting ArgusFlow (Tauri + Vite)..." -ForegroundColor Cyan
    & pnpm exec tauri dev
    if ($LASTEXITCODE -ne 0) {
        throw "ArgusFlow exited with code $LASTEXITCODE."
    }
}
finally {
    Pop-Location
}

