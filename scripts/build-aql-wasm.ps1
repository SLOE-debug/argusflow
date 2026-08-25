$ErrorActionPreference = "Stop"

# This script generates ignored source modules that Vite can transform and bundle.
$workspaceRoot = Split-Path -Parent $PSScriptRoot
$wasmTarget = Join-Path $workspaceRoot "target\wasm32-unknown-unknown\release\argusflow_query_wasm.wasm"
$outputDirectory = Join-Path $workspaceRoot "src\features\aql-editor\generated"

if (-not (Get-Command wasm-bindgen -ErrorAction SilentlyContinue)) {
    throw "wasm-bindgen CLI was not found. Install the version required by Cargo.lock."
}

cargo build --target wasm32-unknown-unknown --release -p argusflow-query-wasm
if ($LASTEXITCODE -ne 0) {
    throw "argusflow-query-wasm build failed with exit code $LASTEXITCODE."
}

New-Item -ItemType Directory -Force -Path $outputDirectory | Out-Null
wasm-bindgen $wasmTarget --target web --out-dir $outputDirectory --out-name argusflow_query_wasm
if ($LASTEXITCODE -ne 0) {
    throw "wasm-bindgen binding generation failed with exit code $LASTEXITCODE."
}
