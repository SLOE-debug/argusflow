[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$ProjectRoot,
    [switch]$SkipInstall,
    [ValidateRange(1, 3600)]
    [int]$ReadyTimeoutSeconds = 900
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Get-Sha256Hash {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    # Use the runtime cryptography API so dependency fingerprinting does not depend on module autoloading.
    $fileStream = [IO.File]::OpenRead($Path)
    try {
        $sha256 = [Security.Cryptography.SHA256]::Create()
        try {
            $hashBytes = $sha256.ComputeHash($fileStream)
            return [BitConverter]::ToString($hashBytes).Replace("-", "")
        }
        finally {
            $sha256.Dispose()
        }
    }
    finally {
        $fileStream.Dispose()
    }
}

if ($env:OS -ne "Windows_NT") {
    throw "ArgusFlow Vision worker only supports Windows."
}

$workerRoot = Join-Path $ProjectRoot "workers\argusflow-vision-worker"
$requirementsPath = Join-Path $workerRoot "requirements.lock"
$projectFilePath = Join-Path $workerRoot "pyproject.toml"
$condaEnvironmentRoot = Join-Path $workerRoot ".conda"
$condaPython = Join-Path $condaEnvironmentRoot "python.exe"
$condaMetadataPath = Join-Path $condaEnvironmentRoot "conda-meta\history"
$installStampPath = Join-Path $condaEnvironmentRoot ".argusflow-install.sha256"
# The fingerprint contract forces one dependency refresh when the environment layout changes.
$environmentContract = "conda-prefix-python-3.11-v1"

if (-not (Test-Path -LiteralPath $requirementsPath) -or
    -not (Test-Path -LiteralPath $projectFilePath)) {
    throw "ArgusFlow Vision worker project files are missing from $workerRoot."
}

if (-not (Get-Command conda -ErrorAction SilentlyContinue)) {
    throw "Conda was not found. Install Miniconda or Anaconda and make the conda command available before starting ArgusFlow."
}

$hasCondaPython = Test-Path -LiteralPath $condaPython
$hasCondaMetadata = Test-Path -LiteralPath $condaMetadataPath
if (-not $hasCondaPython -or -not $hasCondaMetadata) {
    if ($SkipInstall) {
        throw "ArgusFlow Vision worker Conda environment is missing; run dev.ps1 without -SkipInstall once."
    }

    Write-Host "Creating the dedicated Conda Python 3.11 Vision worker environment..." -ForegroundColor Cyan
    & conda create `
        --yes `
        --no-default-packages `
        --prefix $condaEnvironmentRoot `
        "python=3.11" `
        pip | Out-Host
    if ($LASTEXITCODE -ne 0 -or
        -not (Test-Path -LiteralPath $condaPython) -or
        -not (Test-Path -LiteralPath $condaMetadataPath)) {
        throw "Failed to create the dedicated Conda Python 3.11 Vision worker environment."
    }
}

# Verify the interpreter contract instead of trusting a stale or manually replaced prefix.
$condaPythonVersion = & $condaPython -I -c "import sys; print(f'{sys.version_info.major}.{sys.version_info.minor}')"
if ($LASTEXITCODE -ne 0 -or $condaPythonVersion.Trim() -ne "3.11") {
    throw "ArgusFlow Vision worker Conda environment must use Python 3.11."
}
Write-Host "Using Conda Vision worker environment: $condaEnvironmentRoot" -ForegroundColor DarkCyan

# Fingerprint dependency declarations; editable install keeps worker source live from the workspace.
$dependencyFingerprint = "{0}:{1}:{2}" -f `
    $environmentContract, `
    (Get-Sha256Hash -Path $requirementsPath), `
    (Get-Sha256Hash -Path $projectFilePath)
$installedFingerprint = if (Test-Path -LiteralPath $installStampPath) {
    [IO.File]::ReadAllText($installStampPath).Trim()
}
else {
    ""
}

if ($installedFingerprint -ne $dependencyFingerprint) {
    if ($SkipInstall) {
        throw "ArgusFlow Vision worker dependencies changed; run dev.ps1 without -SkipInstall once."
    }

    Write-Host "Installing the locked PaddleOCR worker dependencies..." -ForegroundColor Cyan
    & $condaPython -I -m pip install `
        --disable-pip-version-check `
        --requirement $requirementsPath | Out-Host
    if ($LASTEXITCODE -ne 0) {
        throw "Installing ArgusFlow Vision worker dependencies failed with exit code $LASTEXITCODE."
    }
    & $condaPython -I -m pip install `
        --disable-pip-version-check `
        --no-deps `
        --editable $workerRoot | Out-Host
    if ($LASTEXITCODE -ne 0) {
        throw "Installing the ArgusFlow Vision worker package failed with exit code $LASTEXITCODE."
    }
    [IO.File]::WriteAllText($installStampPath, $dependencyFingerprint)
}

$runId = [Guid]::NewGuid().ToString("N")
$pipeName = "\\.\pipe\argusflow-vision-$runId"
# Two independent UUIDs form the one-run credential used to reject unrelated local clients.
$sessionToken = "{0}{1}" -f `
    [Guid]::NewGuid().ToString("N"), `
    [Guid]::NewGuid().ToString("N")
$runtimeDirectory = Join-Path $ProjectRoot ".argusflow\dev\vision-worker"
$statusPath = Join-Path $runtimeDirectory "$runId.status.json"
$stdoutPath = Join-Path $runtimeDirectory "$runId.stdout.log"
$stderrPath = Join-Path $runtimeDirectory "$runId.stderr.log"
[IO.Directory]::CreateDirectory($runtimeDirectory) | Out-Null

# Start-Process joins ArgumentList into one Windows command line, so quote values that may contain spaces.
$workerArguments = @(
    "-I"
    "-m"
    "argusflow_vision_worker"
    "--pipe-name"
    ('"{0}"' -f $pipeName)
    "--session-token"
    ('"{0}"' -f $sessionToken)
    "--status-file"
    ('"{0}"' -f $statusPath)
)
$workerProcess = Start-Process `
    -FilePath $condaPython `
    -ArgumentList $workerArguments `
    -WorkingDirectory $workerRoot `
    -WindowStyle Hidden `
    -RedirectStandardOutput $stdoutPath `
    -RedirectStandardError $stderrPath `
    -PassThru

try {
    # Readiness is published after model warmup and pipe creation to avoid a first-handshake race.
    $readyDeadline = [DateTime]::UtcNow.AddSeconds($ReadyTimeoutSeconds)
    while ([DateTime]::UtcNow -lt $readyDeadline) {
        $workerProcess.Refresh()
        if ($workerProcess.HasExited) {
            throw "ArgusFlow Vision worker exited during startup. See $stderrPath."
        }

        if (Test-Path -LiteralPath $statusPath) {
            try {
                $status = [IO.File]::ReadAllText($statusPath) | ConvertFrom-Json
            }
            catch {
                # The worker publishes by atomic replacement; tolerate transient scanner interference.
                $status = $null
            }
            if ($null -ne $status -and $status.lifecycle -eq "ready") {
                break
            }
            if ($null -ne $status -and $status.lifecycle -eq "failed") {
                throw "ArgusFlow Vision worker failed to load models: $($status.message)"
            }
        }

        Start-Sleep -Milliseconds 500
    }

    if ([DateTime]::UtcNow -ge $readyDeadline) {
        throw "ArgusFlow Vision worker did not become ready within $ReadyTimeoutSeconds seconds. See $stderrPath."
    }

    $env:ARGUSFLOW_VISION_PIPE_NAME = $pipeName
    $env:ARGUSFLOW_VISION_SESSION_TOKEN = $sessionToken
    Write-Host "ArgusFlow Vision worker is ready." -ForegroundColor Green

    [PSCustomObject]@{
        Process = $workerProcess
        PipeName = $pipeName
        StatusPath = $statusPath
        StandardOutputPath = $stdoutPath
        StandardErrorPath = $stderrPath
    }
}
catch {
    $workerProcess.Refresh()
    if (-not $workerProcess.HasExited) {
        Stop-Process -Id $workerProcess.Id -Force
    }
    throw
}
