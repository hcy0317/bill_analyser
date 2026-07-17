# 停止 Bill Analyser 服务器
# PowerShell脚本 - 安全停止前后端服务器

param(
    [switch]$OnlyBackend,
    [switch]$OnlyFrontend
)

Write-Host "========================================"  -ForegroundColor Cyan
Write-Host "停止 Bill Analyser 服务器" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

$ProjectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
. (Join-Path $ProjectRoot "scripts\http-bind.ps1")
$ConfiguredBackendBind = $env:BILL_ANALYSER_HTTP_BIND
if ([string]::IsNullOrWhiteSpace($ConfiguredBackendBind)) {
    $DotenvPath = Join-Path $ProjectRoot ".env"
    if (Test-Path -LiteralPath $DotenvPath) {
        $BindLine = Get-Content -LiteralPath $DotenvPath |
            Where-Object { $_ -match '^\s*BILL_ANALYSER_HTTP_BIND\s*=' } |
            Select-Object -Last 1
        if ($BindLine) {
            $ConfiguredBackendBind = ($BindLine -split '=', 2)[1].Trim().Trim('"').Trim("'")
        }
    }
}
$BackendEndpoint = Resolve-BillAnalyserHttpEndpoint -Bind $ConfiguredBackendBind

# 按配置的后端监听端口精确停止服务器
if (-not $OnlyFrontend) {
    Write-Host "正在查找后端服务器（端口$($BackendEndpoint.Port)）..." -ForegroundColor Yellow
    
    $backendProcesses = Get-NetTCPConnection -LocalPort $BackendEndpoint.Port -State Listen -ErrorAction SilentlyContinue |
                       Select-Object -ExpandProperty OwningProcess | 
                       Where-Object { $_ -gt 0 } |
                       Sort-Object -Unique
    
    if ($backendProcesses) {
        foreach ($processId in $backendProcesses) {
            $process = Get-Process -Id $processId -ErrorAction SilentlyContinue
            if ($process) {
                Write-Host "  找到进程: $($process.Name) (PID: $processId)" -ForegroundColor Gray
                try {
                    Stop-Process -Id $processId -Force -ErrorAction Stop
                    Write-Host "  ✓ 后端服务器已停止 (PID: $processId)" -ForegroundColor Green
                } catch {
                    Write-Host "  ✗ 停止失败: $_" -ForegroundColor Red
                }
            }
        }
    } else {
        Write-Host "  - 未找到运行中的后端服务器" -ForegroundColor Gray
    }
    Write-Host ""
}

# 停止前端服务器（端口8081）
if (-not $OnlyBackend) {
    Write-Host "正在查找前端服务器（端口8081）..." -ForegroundColor Yellow
    
    $frontendProcesses = Get-NetTCPConnection -LocalPort 8081 -State Listen -ErrorAction SilentlyContinue | 
                        Select-Object -ExpandProperty OwningProcess | 
                        Where-Object { $_ -gt 0 } |
                        Get-Unique
    
    if ($frontendProcesses) {
        foreach ($processId in $frontendProcesses) {
            $process = Get-Process -Id $processId -ErrorAction SilentlyContinue
            if ($process) {
                Write-Host "  找到进程: $($process.Name) (PID: $processId)" -ForegroundColor Gray
                try {
                    Stop-Process -Id $processId -Force -ErrorAction Stop
                    Write-Host "  ✓ 前端服务器已停止 (PID: $processId)" -ForegroundColor Green
                } catch {
                    Write-Host "  ✗ 停止失败: $_" -ForegroundColor Red
                }
            }
        }
    } else {
        Write-Host "  - 未找到运行中的前端服务器" -ForegroundColor Gray
    }
    Write-Host ""
}

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "操作完成" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan

# 如果从BAT调用，不需要暂停
if (-not $OnlyBackend -and -not $OnlyFrontend) {
    Write-Host ""
    Write-Host "按任意键退出..." -ForegroundColor Gray
    $null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown")
}
