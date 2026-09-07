# Standalone regression checks; Cargo and running processes are isolated from the host.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$projectRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$testRoot = Join-Path $projectRoot ('.argusflow\tests\rust-cache-' + [guid]::NewGuid().ToString('N'))
$fixtureRoot = Join-Path $testRoot 'workspace'
$fixtureScripts = Join-Path $fixtureRoot 'scripts'
$fixtureTarget = Join-Path $fixtureRoot 'target'
$cleanupScript = Join-Path $fixtureScripts 'clean-rust-cache.ps1'
$junctionPath = Join-Path $fixtureTarget 'external'
$cacheTestState = @{ CargoCalls = @(); HasRunningProcess = $false }

function cargo {
    $cacheTestState.CargoCalls += ,@($args)
    $global:LASTEXITCODE = 0
}

function Get-Process {
    param($Name, $ErrorAction)
    if ($cacheTestState.HasRunningProcess) {
        [pscustomobject]@{ Name = 'cargo' }
    }
}

function Assert-Condition {
    param([bool]$Condition, [string]$Message)
    if (-not $Condition) { throw $Message }
}

try {
    New-Item -ItemType Directory -Path $fixtureScripts, $fixtureTarget -Force | Out-Null
    Set-Content -LiteralPath (Join-Path $fixtureRoot 'Cargo.toml') -Value '[workspace]'
    Copy-Item -LiteralPath (Join-Path $projectRoot 'scripts\clean-rust-cache.ps1') -Destination $cleanupScript

    & $cleanupScript -MaxTargetGiB 0.001
    Assert-Condition ($cacheTestState.CargoCalls.Count -eq 0) 'Empty cache must not invoke Cargo.'

    # A sparse fixture crosses the budget without creating a large test artifact.
    $artifact = [IO.File]::Create((Join-Path $fixtureTarget 'artifact.bin'))
    try { $artifact.SetLength(2MB) } finally { $artifact.Dispose() }
    & $cleanupScript -MaxTargetGiB 0.001 -WhatIf
    Assert-Condition ($cacheTestState.CargoCalls.Count -eq 0) 'Preview must not invoke Cargo.'

    $cacheTestState.HasRunningProcess = $true
    & $cleanupScript -MaxTargetGiB 0.001
    Assert-Condition ($cacheTestState.CargoCalls.Count -eq 0) 'Active builds must defer cleanup.'
    $cacheTestState.HasRunningProcess = $false

    & $cleanupScript -MaxTargetGiB 0.001
    Assert-Condition ($cacheTestState.CargoCalls.Count -eq 1) 'An over-budget idle cache must invoke Cargo once.'
    $expectedArguments = @('clean', '--manifest-path', (Join-Path $fixtureRoot 'Cargo.toml'), '--target-dir', $fixtureTarget)
    Assert-Condition (($cacheTestState.CargoCalls[0] -join '|') -eq ($expectedArguments -join '|')) 'Cargo must receive only the verified local target.'

    # A link to a sibling containing user data must be rejected before Cargo sees it.
    $externalRoot = Join-Path $testRoot 'user-data'
    New-Item -ItemType Directory -Path $externalRoot | Out-Null
    $sentinel = Join-Path $externalRoot 'keep.txt'
    Set-Content -LiteralPath $sentinel -Value 'keep'
    New-Item -ItemType Junction -Path $junctionPath -Value $externalRoot | Out-Null
    $rejected = $false
    try { & $cleanupScript -MaxTargetGiB 0.001 }
    catch { $rejected = $_.Exception.Message -like '*reparse point*' }
    Assert-Condition $rejected 'Nested junctions must be rejected.'
    Assert-Condition ($cacheTestState.CargoCalls.Count -eq 1) 'Unsafe paths must never reach Cargo.'
    Assert-Condition (Test-Path -LiteralPath $sentinel) 'User data outside the target must remain intact.'

    Write-Host 'Rust cache regression checks passed.' -ForegroundColor Green
}
finally {
    # Remove the junction itself before recursively removing this exact generated fixture.
    if (Test-Path -LiteralPath $junctionPath) { [IO.Directory]::Delete($junctionPath) }
    $resolvedTestRoot = [IO.Path]::GetFullPath($testRoot)
    $allowedTestRoot = [IO.Path]::GetFullPath((Join-Path $projectRoot '.argusflow\tests')) + [IO.Path]::DirectorySeparatorChar
    if (-not $resolvedTestRoot.StartsWith($allowedTestRoot, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Unsafe test cleanup destination: $resolvedTestRoot"
    }
    if (Test-Path -LiteralPath $resolvedTestRoot) { Remove-Item -LiteralPath $resolvedTestRoot -Recurse -Force }
}
