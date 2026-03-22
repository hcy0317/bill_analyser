<# 
    Bill Analyser - 一键启动脚本
    功能：自动清除旧进程 + 启动后端和前端服务器
    版本：v6.74
    日期：2025-12-11
#>

param(
    [switch]$BackendOnly,      # 仅启动后端
    [switch]$FrontendOnly,     # 仅启动前端
    [switch]$NoAutoStop,       # 不自动停止旧进程
    [switch]$NoBrowser         # 不自动打开浏览器
)

# 设置控制台编码为UTF-8
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

# 颜色定义
function Write-Info { param($msg) Write-Host $msg -ForegroundColor Cyan }
function Write-Success { param($msg) Write-Host $msg -ForegroundColor Green }
function Write-Warn { param($msg) Write-Host $msg -ForegroundColor Yellow }
function Write-Err { param($msg) Write-Host $msg -ForegroundColor Red }
function Write-Gray { param($msg) Write-Host $msg -ForegroundColor Gray }

# 获取项目根目录
$ProjectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $ProjectRoot

# 打印横幅
Write-Host ""
Write-Host "╔═══════════════════════════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "║          Bill Analyser - 账单分析系统一键启动器              ║" -ForegroundColor Cyan
Write-Host "║                       v6.74 (2025-12-11)                     ║" -ForegroundColor Cyan
Write-Host "╚═══════════════════════════════════════════════════════════════╝" -ForegroundColor Cyan
Write-Host ""

# ============================================================
# 函数：安全停止指定端口的进程
# ============================================================
function Stop-ServiceByPort {
    param(
        [int]$Port,
        [string]$ServiceName
    )
    
    Write-Gray "  正在检查端口 $Port ($ServiceName)..."
    
    $connections = Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue
    
    if ($connections) {
        foreach ($conn in $connections) {
            $processId = $conn.OwningProcess
            if ($processId -and $processId -gt 0) {
                $process = Get-Process -Id $processId -ErrorAction SilentlyContinue
                if ($process) {
                    Write-Warn "  发现进程: $($process.Name) (PID: $processId)"
                    try {
                        # 先尝试优雅停止
                        $process | Stop-Process -Force -ErrorAction Stop
                        Start-Sleep -Milliseconds 500
                        Write-Success "  ✓ $ServiceName 已停止 (PID: $processId)"
                    } catch {
                        Write-Err "  ✗ 停止失败: $_"
                    }
                }
            }
        }
        # 等待端口释放
        Start-Sleep -Seconds 1
        return $true
    } else {
        Write-Gray "  - 端口 $Port 未被占用"
        return $false
    }
}

# ============================================================
# 函数：检查端口是否正在监听
# ============================================================
function Test-PortListening {
    param([int]$Port)
    
    $connection = Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue
    return $null -ne $connection
}

# ============================================================
# 函数：等待端口可用
# ============================================================
function Wait-ForPort {
    param(
        [int]$Port,
        [int]$TimeoutSeconds = 30,
        [string]$ServiceName
    )
    
    $startTime = Get-Date
    $timeout = New-TimeSpan -Seconds $TimeoutSeconds
    
    Write-Gray "  等待 $ServiceName 启动 (端口 $Port)..."
    
    while ((Get-Date) - $startTime -lt $timeout) {
        if (Test-PortListening -Port $Port) {
            Write-Success "  ✓ $ServiceName 已就绪 (端口 $Port)"
            return $true
        }
        Start-Sleep -Milliseconds 500
    }
    
    Write-Err "  ✗ $ServiceName 启动超时"
    return $false
}

# ============================================================
# 步骤1: 清理旧进程
# ============================================================
if (-not $NoAutoStop) {
    Write-Info "[步骤 1/3] 清理旧进程..."
    Write-Host ""
    
    if (-not $FrontendOnly) {
        $null = Stop-ServiceByPort -Port 5000 -ServiceName "后端服务器"
    }
    
    if (-not $BackendOnly) {
        $null = Stop-ServiceByPort -Port 8081 -ServiceName "前端服务器"
    }
    
    Write-Host ""
} else {
    Write-Warn "[步骤 1/3] 跳过清理旧进程（使用了 -NoAutoStop 参数）"
    Write-Host ""
}

# ============================================================
# 步骤2: 启动后端服务器
# ============================================================
$backendStarted = $false

if (-not $FrontendOnly) {
    Write-Info "[步骤 2/3] 启动后端服务器..."
    Write-Host ""
    
    # 检查虚拟环境
    $pythonPath = Join-Path $ProjectRoot ".venv\Scripts\python.exe"
    if (-not (Test-Path $pythonPath)) {
        Write-Err "  ✗ 虚拟环境未找到！"
        Write-Warn "  请运行: python -m venv .venv"
        Write-Warn "  然后安装依赖: .\.venv\Scripts\pip install -r requirements.txt"
        exit 1
    }
    
    # 启动后端（在新窗口中）
    $backendScript = Join-Path $ProjectRoot "start_backend.ps1"
    if (Test-Path $backendScript) {
        Start-Process powershell -ArgumentList "-ExecutionPolicy", "Bypass", "-File", $backendScript -WindowStyle Normal
        Write-Gray "  后端服务器窗口已启动"
        
        # 等待后端就绪
        if (Wait-ForPort -Port 5000 -TimeoutSeconds 30 -ServiceName "后端服务器") {
            $backendStarted = $true
        } else {
            Write-Err "  ✗ 后端服务器启动失败"
            exit 1
        }
    } else {
        Write-Err "  ✗ 找不到后端启动脚本: $backendScript"
        exit 1
    }
    
    Write-Host ""
} else {
    Write-Warn "[步骤 2/3] 跳过启动后端（使用了 -FrontendOnly 参数）"
    Write-Host ""
}

# ============================================================
# 步骤3: 启动前端服务器
# ============================================================
$frontendStarted = $false

if (-not $BackendOnly) {
    Write-Info "[步骤 3/3] 启动前端服务器..."
    Write-Host ""
    
    # 检查node_modules
    $nodeModulesPath = Join-Path $ProjectRoot "src\web\node_modules"
    if (-not (Test-Path $nodeModulesPath)) {
        Write-Warn "  正在安装前端依赖..."
        $webPath = Join-Path $ProjectRoot "src\web"
        Push-Location $webPath
        npm install
        Pop-Location
    }
    
    # 启动前端（在新窗口中）
    $frontendScript = Join-Path $ProjectRoot "start_frontend.ps1"
    if (Test-Path $frontendScript) {
        Start-Process powershell -ArgumentList "-ExecutionPolicy", "Bypass", "-File", $frontendScript -WindowStyle Normal
        Write-Gray "  前端服务器窗口已启动"
        
        # 等待前端就绪
        if (Wait-ForPort -Port 8081 -TimeoutSeconds 60 -ServiceName "前端服务器") {
            $frontendStarted = $true
        } else {
            Write-Err "  ✗ 前端服务器启动失败"
            exit 1
        }
    } else {
        Write-Err "  ✗ 找不到前端启动脚本: $frontendScript"
        exit 1
    }
    
    Write-Host ""
} else {
    Write-Warn "[步骤 3/3] 跳过启动前端（使用了 -BackendOnly 参数）"
    Write-Host ""
}

# ============================================================
# 完成：显示状态摘要
# ============================================================
Write-Host "╔═══════════════════════════════════════════════════════════════╗" -ForegroundColor Green
Write-Host "║                      启动完成                                ║" -ForegroundColor Green
Write-Host "╚═══════════════════════════════════════════════════════════════╝" -ForegroundColor Green
Write-Host ""

if ($backendStarted -or -not $FrontendOnly) {
    Write-Success "  ✓ 后端服务器: http://127.0.0.1:5000"
    Write-Gray "    健康检查:   http://127.0.0.1:5000/api/health"
}

if ($frontendStarted -or -not $BackendOnly) {
    Write-Success "  ✓ 前端应用:   http://127.0.0.1:8081"
}

Write-Host ""
Write-Gray "  提示: 关闭对应的 PowerShell 窗口可停止服务器"
Write-Gray "  或者运行: .\停止服务器.ps1"
Write-Host ""

# 自动打开浏览器
if (-not $NoBrowser -and $frontendStarted) {
    Write-Info "正在打开浏览器..."
    Start-Process "http://127.0.0.1:8081"
}

# v6.79: 启动完成后自动退出，无需等待按键
Write-Host "启动完成，此窗口将自动关闭..." -ForegroundColor Gray
Start-Sleep -Seconds 2
