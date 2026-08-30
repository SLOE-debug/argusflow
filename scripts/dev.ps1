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
$visionWorker = $null
$usesManagedVisionWorker = $false
$usesManagedVisionDiagnostics = $false

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

    Write-Host "Building the AQL WASM language service..." -ForegroundColor Cyan
    & pnpm build:aql-wasm
    if ($LASTEXITCODE -ne 0) {
        throw "AQL WASM build failed with exit code $LASTEXITCODE."
    }

    # Reuse an explicitly configured deployment worker; otherwise own one for this dev run.
    $configuredPipeName = [Environment]::GetEnvironmentVariable("ARGUSFLOW_VISION_PIPE_NAME", "Process")
    $configuredSessionToken = [Environment]::GetEnvironmentVariable("ARGUSFLOW_VISION_SESSION_TOKEN", "Process")
    $hasConfiguredPipe = -not [string]::IsNullOrWhiteSpace($configuredPipeName)
    $hasConfiguredToken = -not [string]::IsNullOrWhiteSpace($configuredSessionToken)
    if ($hasConfiguredPipe -xor $hasConfiguredToken) {
        throw "Vision worker configuration is incomplete; pipe name and session token must be set together."
    }

    if ($hasConfiguredPipe) {
        Write-Host "Using the externally managed ArgusFlow Vision worker..." -ForegroundColor Cyan
    }
    else {
        Write-Host "Starting the local ArgusFlow Vision worker..." -ForegroundColor Cyan
        $visionWorker = & (Join-Path $PSScriptRoot "start-vision-worker.ps1") `
            -ProjectRoot $projectRoot `
            -SkipInstall:$SkipInstall
        $usesManagedVisionWorker = $true
    }

    # Persist window pixels only for failed dev runs and only under an explicitly inherited directory.
    $configuredDiagnosticsDirectory = [Environment]::GetEnvironmentVariable(
        "ARGUSFLOW_VISION_DIAGNOSTICS_DIR",
        "Process"
    )
    if ([string]::IsNullOrWhiteSpace($configuredDiagnosticsDirectory)) {
        $configuredDiagnosticsDirectory = Join-Path $projectRoot ".argusflow\dev\vision-diagnostics"
        $env:ARGUSFLOW_VISION_DIAGNOSTICS_DIR = $configuredDiagnosticsDirectory
        $usesManagedVisionDiagnostics = $true
    }
    Write-Host "Vision failure diagnostics: $configuredDiagnosticsDirectory" -ForegroundColor DarkCyan

    Write-Host "Starting ArgusFlow (Tauri + Vite)..." -ForegroundColor Cyan
    & pnpm exec tauri dev
    if ($LASTEXITCODE -ne 0) {
        throw "ArgusFlow exited with code $LASTEXITCODE."
    }
}
finally {
    if ($usesManagedVisionWorker) {
        Remove-Item Env:ARGUSFLOW_VISION_PIPE_NAME -ErrorAction SilentlyContinue
        Remove-Item Env:ARGUSFLOW_VISION_SESSION_TOKEN -ErrorAction SilentlyContinue
    }
    if ($usesManagedVisionDiagnostics) {
        Remove-Item Env:ARGUSFLOW_VISION_DIAGNOSTICS_DIR -ErrorAction SilentlyContinue
    }
    if ($null -ne $visionWorker) {
        $visionWorker.Process.Refresh()
        if (-not $visionWorker.Process.HasExited) {
            Stop-Process -Id $visionWorker.Process.Id -Force
        }
    }
    Pop-Location
}

