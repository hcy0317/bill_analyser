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

# 停止后端服务器（端口5000）
if (-not $OnlyFrontend) {
    Write-Host "正在查找后端服务器（端口5000）..." -ForegroundColor Yellow
    
    $backendProcesses = Get-NetTCPConnection -LocalPort 5000 -State Listen -ErrorAction SilentlyContinue | 
                       Select-Object -ExpandProperty OwningProcess | 
                       Where-Object { $_ -gt 0 } |
                       Get-Unique
    
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
