$ErrorActionPreference = "Stop"
$workspaceRoot = Split-Path -Parent $PSScriptRoot
Push-Location $workspaceRoot
try {
    if (-not (Get-Command wasm-bindgen -ErrorAction SilentlyContinue)) {
        throw "Install wasm-bindgen-cli version 0.2.127."
    }
    $bindgenVersion = & wasm-bindgen --version
    if ($bindgenVersion -ne "wasm-bindgen 0.2.127") {
        throw "wasm-bindgen CLI must be 0.2.127, matching Cargo.lock."
    }
    cargo build --locked --target wasm32-unknown-unknown --release -p argusflow-aql-wasm
    if ($LASTEXITCODE -ne 0) { throw "AQL WASM build failed." }
    $wasmPath = Join-Path $workspaceRoot "target/wasm32-unknown-unknown/release/argusflow_aql_wasm.wasm"
    $outputPath = Join-Path $workspaceRoot "src/features/aql/generated"
    wasm-bindgen $wasmPath --target web --out-dir $outputPath --out-name aql
    if ($LASTEXITCODE -ne 0) { throw "AQL WASM bindings generation failed." }
}
finally { Pop-Location }
