param([ValidateSet('cpu','cuda','all')][string]$Device = 'cpu')
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$repoRoot = Split-Path -Parent $PSScriptRoot
$depsRoot = Join-Path $repoRoot '.deps'
$manifest = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'native-deps.lock.json') -Raw | ConvertFrom-Json
foreach ($artifact in $manifest.artifacts) {
    if ($artifact.group -ne 'models' -and $Device -ne 'all' -and $artifact.group -ne $Device) { continue }
    $destination = Join-Path $depsRoot $artifact.path
    $destinationDirectory = Split-Path -Parent $destination
    New-Item -ItemType Directory -Force -Path $destinationDirectory | Out-Null
    if (!(Test-Path -LiteralPath $destination) -or (Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash -ne $artifact.sha256) {
        Write-Host ('Downloading ' + $artifact.path)
        $partialPath = $destination + '.partial'
        for ($attempt = 1; $attempt -le 3; $attempt++) {
            try {
                Invoke-WebRequest -UseBasicParsing -Uri $artifact.url -OutFile $partialPath -TimeoutSec 1800
                break
            } catch {
                if ($attempt -eq 3) { throw }
                Start-Sleep -Seconds $attempt
            }
        }
        if ((Get-FileHash -LiteralPath $partialPath -Algorithm SHA256).Hash -ne $artifact.sha256) { throw ('SHA-256 mismatch: ' + $artifact.path) }
        Move-Item -LiteralPath $partialPath -Destination $destination -Force
    }
    if ($artifact.path.EndsWith('.zip')) {
        $archiveName = [IO.Path]::GetFileNameWithoutExtension($destination)
        $extractRoot = Join-Path $depsRoot ('unpacked/' + $archiveName)
        $marker = Join-Path $extractRoot '.verified'
        if (!(Test-Path -LiteralPath $marker) -or [IO.File]::ReadAllText($marker) -ne $artifact.sha256) {
            Expand-Archive -LiteralPath $destination -DestinationPath $extractRoot -Force
            [IO.File]::WriteAllText($marker, $artifact.sha256)
        }
        $runtimeRoot = Join-Path $depsRoot ('runtime/' + $artifact.group)
        New-Item -ItemType Directory -Force -Path $runtimeRoot | Out-Null
        Get-ChildItem -LiteralPath $extractRoot -Filter '*.dll' -Recurse | ForEach-Object {
            Copy-Item -LiteralPath $_.FullName -Destination $runtimeRoot -Force
        }
        # Preserve upstream license texts alongside redistributed native libraries.
        Get-ChildItem -LiteralPath $extractRoot -File -Recurse | Where-Object { $_.Name -match 'LICENSE|NOTICE' } | ForEach-Object {
            Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $runtimeRoot ($archiveName + '-' + $_.Name)) -Force
        }
    }
}
Write-Host ('Verified native dependencies in ' + $depsRoot)
