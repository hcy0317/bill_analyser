param(
    [string]$BackendHost = "127.0.0.1",
    [int]$BackendPort = 5000,
    [int]$FrontendPort = 8081,
    [int]$LauncherTimeoutSeconds = 420,
    [switch]$KeepRunning
)

$ErrorActionPreference = "Stop"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

$ProjectRoot = Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot "process-manifest.ps1")
. (Join-Path $PSScriptRoot "powershell-runtime.ps1")
$LauncherName = -join @([char]0x4E00, [char]0x952E, [char]0x542F, [char]0x52A8, ".ps1")
$LauncherPath = Join-Path $ProjectRoot $LauncherName
$RunRoot = Join-Path $ProjectRoot ".git\ai\startup-gate"
$RunId = Get-Date -Format "yyyyMMdd-HHmmss"
$ManifestPath = Join-Path $RunRoot "one-click-$RunId.manifest.json"
$LauncherOut = Join-Path $RunRoot "one-click-$RunId.out.log"
$LauncherErr = Join-Path $RunRoot "one-click-$RunId.err.log"
$PreviousBackendBind = $env:BILL_ANALYSER_HTTP_BIND

function Get-GateShell {
    $shell = Get-BillAnalyserPowerShell7Path
    if (-not $shell) { throw "PowerShell 7 (pwsh) is required." }
    return $shell
}

function Assert-PortFree {
    param([int]$Port, [string]$Name)
    $pids = @(Get-ManifestListenerPids -Port $Port)
    if ($pids.Count -gt 0) {
        throw "$Name port $Port is occupied by PID(s): $($pids -join ', '). The gate will not stop processes it did not start."
    }
}

function Assert-BackendHealth {
    $url = "http://${BackendHost}:$BackendPort/api/health"
    if (-not (Test-ManifestHttpEndpoint -Url $url)) { throw "Backend health check failed: $url" }
    Write-Host "backend ready: $url" -ForegroundColor Green
}

function Assert-FrontendReady {
    $url = "http://127.0.0.1:$FrontendPort"
    if (-not (Test-ManifestHttpEndpoint -Url $url)) { throw "Frontend check failed: $url" }
    Write-Host "frontend ready: $url" -ForegroundColor Green
}

function Write-GateLogTail {
    foreach ($path in @($LauncherOut, $LauncherErr)) {
        if (Test-Path -LiteralPath $path) {
            Write-Host "--- $path ---" -ForegroundColor Yellow
            Get-Content -LiteralPath $path -Tail 100
        }
    }
}

$launcherProcess = $null
try {
    Assert-PortFree -Port $BackendPort -Name "Backend"
    Assert-PortFree -Port $FrontendPort -Name "Frontend"
    New-Item -ItemType Directory -Force -Path $RunRoot | Out-Null
    $env:BILL_ANALYSER_HTTP_BIND = "${BackendHost}:$BackendPort"

    $arguments = @(
        "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $LauncherPath,
        "-NoBrowser", "-NoAutoStop", "-Headless", "-ProcessManifestPath", $ManifestPath
    )
    $launcherProcess = Start-Process -FilePath (Get-GateShell) -ArgumentList $arguments `
        -WorkingDirectory $ProjectRoot -WindowStyle Hidden -RedirectStandardOutput $LauncherOut `
        -RedirectStandardError $LauncherErr -PassThru

    if (-not $launcherProcess.WaitForExit($LauncherTimeoutSeconds * 1000)) {
        Stop-Process -Id $launcherProcess.Id -Force -ErrorAction SilentlyContinue
        Write-GateLogTail
        throw "One-click launcher timed out after $LauncherTimeoutSeconds seconds."
    }

    $launcherProcess.Refresh()
    $launcherExitCode = $launcherProcess.ExitCode
    if ($null -ne $launcherExitCode -and $launcherExitCode -ne 0) {
        Write-GateLogTail
        throw "One-click launcher failed with exit code $launcherExitCode."
    }

    Assert-BackendHealth
    Assert-FrontendReady
    Write-Host "PASS one-click startup gate" -ForegroundColor Green
} finally {
    if ($null -ne $PreviousBackendBind) {
        $env:BILL_ANALYSER_HTTP_BIND = $PreviousBackendBind
    } else {
        Remove-Item Env:BILL_ANALYSER_HTTP_BIND -ErrorAction SilentlyContinue
    }

    if (-not $KeepRunning) {
        Stop-ManifestOwnedProcesses -Path $ManifestPath -Quiet
    } else {
        Write-Host "Services kept running. Manifest: $ManifestPath" -ForegroundColor Yellow
    }
}
