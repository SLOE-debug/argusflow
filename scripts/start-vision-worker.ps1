[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$ProjectRoot,
    [switch]$SkipInstall
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

function Get-PaddleRuntime {
    # The deployment environment must contain exactly one Paddle runtime. A compatible NVIDIA
    # device selects the newest wheel supported by its driver; all other machines use CPU.
    $requestedDevice = [Environment]::GetEnvironmentVariable("ARGUSFLOW_PADDLE_DEVICE", "Process")
    if ($requestedDevice -eq "cpu") {
        return [PSCustomObject]@{ Id = "cpu"; Lock = "requirements-paddle-cpu.lock"; Index = "https://www.paddlepaddle.org.cn/packages/stable/cpu/" }
    }

    $nvidiaSmi = Get-Command nvidia-smi -ErrorAction SilentlyContinue
    if ($null -ne $nvidiaSmi) {
        $capabilityOutput = & $nvidiaSmi.Source --query-gpu=compute_cap --format=csv,noheader 2>$null
        if ($LASTEXITCODE -eq 0 -and @($capabilityOutput).Count -gt 0) {
            $capabilityText = ([string]@($capabilityOutput)[0]).Trim()
            $capability = 0.0
            if ([double]::TryParse(
                $capabilityText,
                [Globalization.NumberStyles]::Float,
                [Globalization.CultureInfo]::InvariantCulture,
                [ref]$capability
            ) -and $capability -ge 7.5) {
                $driverSummary = (& $nvidiaSmi.Source | Out-String)
                $cudaVersionMatch = [regex]::Match($driverSummary, 'CUDA Version:\s*(?<version>\d+\.\d+)')
                if ($cudaVersionMatch.Success -and
                    [version]$cudaVersionMatch.Groups['version'].Value -ge [version]'12.9') {
                    return [PSCustomObject]@{ Id = "gpu-cu129"; Lock = "requirements-paddle-gpu-cu129.lock"; Index = "https://www.paddlepaddle.org.cn/packages/stable/cu129/" }
                }
                return [PSCustomObject]@{ Id = "gpu-cu126"; Lock = "requirements-paddle-gpu-cu126.lock"; Index = "https://www.paddlepaddle.org.cn/packages/stable/cu126/" }
            }
        }
    }
    return [PSCustomObject]@{ Id = "cpu"; Lock = "requirements-paddle-cpu.lock"; Index = "https://www.paddlepaddle.org.cn/packages/stable/cpu/" }
}

if ($env:OS -ne "Windows_NT") {
    throw "ArgusFlow Vision worker only supports Windows."
}

$workerRoot = Join-Path $ProjectRoot "workers\argusflow-vision-worker"
$requirementsPath = Join-Path $workerRoot "requirements.lock"
$projectFilePath = Join-Path $workerRoot "pyproject.toml"
$paddleRuntime = Get-PaddleRuntime
$paddleRequirementsPath = Join-Path $workerRoot $paddleRuntime.Lock
$condaEnvironmentRoot = Join-Path $workerRoot ".conda"
$condaPython = Join-Path $condaEnvironmentRoot "python.exe"
$condaMetadataPath = Join-Path $condaEnvironmentRoot "conda-meta\history"
$installStampPath = Join-Path $condaEnvironmentRoot ".argusflow-install.sha256"
# The fingerprint contract forces one dependency refresh when the environment layout changes.
$environmentContract = "conda-prefix-python-3.11-paddle-runtime-v2"

if (-not (Test-Path -LiteralPath $requirementsPath) -or
    -not (Test-Path -LiteralPath $paddleRequirementsPath) -or
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
Write-Host "Selected Paddle runtime: $($paddleRuntime.Id)" -ForegroundColor DarkCyan

# Fingerprint dependency declarations; editable install keeps worker source live from the workspace.
$dependencyFingerprint = "{0}:{1}:{2}:{3}:{4}" -f `
    $environmentContract, `
    $paddleRuntime.Id, `
    (Get-Sha256Hash -Path $requirementsPath), `
    (Get-Sha256Hash -Path $paddleRequirementsPath), `
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
    # Remove the opposite distribution before installing the selected runtime. Paddle publishes
    # CPU and GPU wheels under different package names but both expose the same Python module.
    & $condaPython -I -m pip uninstall --yes paddlepaddle paddlepaddle-gpu | Out-Host
    & $condaPython -I -m pip install `
        --disable-pip-version-check `
        --index-url $paddleRuntime.Index `
        --requirement $paddleRequirementsPath | Out-Host
    if ($LASTEXITCODE -ne 0) {
        throw "Installing the selected Paddle runtime failed with exit code $LASTEXITCODE."
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
[IO.File]::WriteAllText($statusPath, '{"lifecycle":"starting"}')

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
    "--device"
    $(if ($paddleRuntime.Id -eq "cpu") { "cpu" } else { "auto" })
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
    $env:ARGUSFLOW_VISION_PIPE_NAME = $pipeName
    $env:ARGUSFLOW_VISION_SESSION_TOKEN = $sessionToken
    Write-Host "ArgusFlow Vision worker started; model progress will appear in the app." -ForegroundColor Green

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
