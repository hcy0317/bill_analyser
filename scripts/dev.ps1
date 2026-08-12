param(
    [ValidateSet("start", "status", "logs", "stop", "check")]
    [string]$Command = "start",
    [switch]$BackendOnly,
    [switch]$FrontendOnly,
    [switch]$NoBrowser,
    [int]$Tail = 80
)

$ErrorActionPreference = "Stop"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

$ProjectRoot = Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot "process-manifest.ps1")
. (Join-Path $PSScriptRoot "http-bind.ps1")
. (Join-Path $PSScriptRoot "powershell-runtime.ps1")

$RunRoot = Join-Path $ProjectRoot ".git\ai\dev-services"
$ManifestPath = Join-Path $RunRoot "services.manifest.json"
$LauncherName = -join @([char]0x4E00, [char]0x952E, [char]0x542F, [char]0x52A8, ".ps1")
$LauncherPath = Join-Path $ProjectRoot $LauncherName

function Get-DevShell {
    $shell = Get-BillAnalyserPowerShell7Path
    if (-not $shell) { throw "未找到 PowerShell 7 (pwsh)。请先安装 PowerShell 7。" }
    return $shell
}

function Get-ConfiguredBackendEndpoint {
    $bind = $env:BILL_ANALYSER_HTTP_BIND
    if ([string]::IsNullOrWhiteSpace($bind)) { $bind = "127.0.0.1:5000" }
    return Resolve-BillAnalyserHttpEndpoint -Bind $bind
}

function Show-DevStatus {
    param([string[]]$RequiredServices = @())

    $manifest = Read-ProcessManifest -Path $ManifestPath
    if ($null -eq $manifest) {
        Write-Host "Bill Analyser 未由 dev.ps1 托管。" -ForegroundColor Gray
        return $false
    }

    $owned = @(Get-ManifestOwnedProcesses -Manifest $manifest)
    $managedServices = @($manifest.entries | ForEach-Object { [string]$_.name })
    $backendManaged = $managedServices -contains "backend"
    $frontendManaged = $managedServices -contains "frontend"
    $backendHealthy = $backendManaged -and (Test-ManifestHttpEndpoint -Url "$($manifest.backend_url)/api/health")
    $frontendHealthy = $frontendManaged -and (Test-ManifestHttpEndpoint -Url $manifest.frontend_url)

    if ($RequiredServices.Count -eq 0) {
        $RequiredServices = @($managedServices | Sort-Object -Unique)
    }

    Write-Host "Manifest: $ManifestPath" -ForegroundColor Gray
    Write-Host "托管进程: $($owned.Count)" -ForegroundColor Gray
    Write-Host "后端: $(if (-not $backendManaged) { 'not managed' } elseif ($backendHealthy) { 'ready' } else { 'down' }) $($manifest.backend_url)" -ForegroundColor $(if ($backendHealthy) { "Green" } else { "Yellow" })
    Write-Host "前端: $(if (-not $frontendManaged) { 'not managed' } elseif ($frontendHealthy) { 'ready' } else { 'down' }) $($manifest.frontend_url)" -ForegroundColor $(if ($frontendHealthy) { "Green" } else { "Yellow" })

    $requiredState = @{
        backend = $backendHealthy
        frontend = $frontendHealthy
    }
    return $RequiredServices.Count -gt 0 -and @($RequiredServices | Where-Object { -not $requiredState[$_] }).Count -eq 0
}

function Show-DevLogs {
    $manifest = Read-ProcessManifest -Path $ManifestPath
    if ($null -eq $manifest) {
        Write-Host "没有可用的托管日志。" -ForegroundColor Gray
        return
    }

    foreach ($entry in @($manifest.entries)) {
        foreach ($path in @($entry.stdout_log, $entry.stderr_log)) {
            if (-not $path -or -not (Test-Path -LiteralPath $path)) { continue }
            Write-Host "--- $($entry.name): $path ---" -ForegroundColor Cyan
            Get-Content -LiteralPath $path -Tail $Tail
        }
    }
}

switch ($Command) {
    "start" {
        $requiredServices = @()
        if (-not $FrontendOnly) { $requiredServices += "backend" }
        if (-not $BackendOnly) { $requiredServices += "frontend" }

        if (Show-DevStatus -RequiredServices $requiredServices) {
            Write-Host "服务已经在运行。" -ForegroundColor Green
            exit 0
        }

        if (Test-Path -LiteralPath $ManifestPath) {
            Stop-ManifestOwnedProcesses -Path $ManifestPath -Quiet
        }

        $endpoint = Get-ConfiguredBackendEndpoint
        $requiredPorts = @()
        if (-not $FrontendOnly) { $requiredPorts += $endpoint.Port }
        if (-not $BackendOnly) { $requiredPorts += 8081 }
        foreach ($port in $requiredPorts) {
            $listeners = @(Get-ManifestListenerPids -Port $port)
            if ($listeners.Count -gt 0) {
                throw "端口 $port 已被非当前清单托管的进程占用 (PID: $($listeners -join ', '))。请先确认进程归属。"
            }
        }

        New-Item -ItemType Directory -Force -Path $RunRoot | Out-Null
        $arguments = @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $LauncherPath, "-Headless", "-NoAutoStop", "-ProcessManifestPath", $ManifestPath)
        if ($BackendOnly) { $arguments += "-BackendOnly" }
        if ($FrontendOnly) { $arguments += "-FrontendOnly" }
        if ($NoBrowser) { $arguments += "-NoBrowser" }

        & (Get-DevShell) @arguments
        if ($LASTEXITCODE -ne 0) {
            Write-Host "启动失败，最近日志如下：" -ForegroundColor Red
            Show-DevLogs
            Stop-ManifestOwnedProcesses -Path $ManifestPath -Quiet
            exit $LASTEXITCODE
        }
        Show-DevStatus | Out-Null
    }
    "status" { Show-DevStatus | Out-Null }
    "logs" { Show-DevLogs }
    "stop" { Stop-ManifestOwnedProcesses -Path $ManifestPath }
    "check" {
        $gate = Join-Path $PSScriptRoot "verify_one_click_start.ps1"
        & (Get-DevShell) -NoProfile -ExecutionPolicy Bypass -File $gate
        exit $LASTEXITCODE
    }
}
