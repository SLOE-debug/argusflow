$ErrorActionPreference = "Stop"

# 该脚本由开发者显式运行；生成物进入 Vite public 目录且不作为源码提交。
$workspaceRoot = Split-Path -Parent $PSScriptRoot
$wasmTarget = Join-Path $workspaceRoot "target\wasm32-unknown-unknown\release\argusflow_query_wasm.wasm"
$outputDirectory = Join-Path $workspaceRoot "public\aql-wasm"

if (-not (Get-Command wasm-bindgen -ErrorAction SilentlyContinue)) {
    throw "未找到 wasm-bindgen CLI。请先安装与 Cargo.lock 匹配的 wasm-bindgen-cli。"
}

cargo build --target wasm32-unknown-unknown --release -p argusflow-query-wasm
if ($LASTEXITCODE -ne 0) {
    throw "argusflow-query-wasm 构建失败，退出码：$LASTEXITCODE"
}

New-Item -ItemType Directory -Force -Path $outputDirectory | Out-Null
wasm-bindgen $wasmTarget --target web --out-dir $outputDirectory --out-name argusflow_query_wasm
if ($LASTEXITCODE -ne 0) {
    throw "wasm-bindgen 生成绑定失败，退出码：$LASTEXITCODE"
}
