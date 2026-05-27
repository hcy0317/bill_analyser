<# 
    Bill Analyser - 一键启动脚本
    功能：按端口清理旧服务，启动 Rust 后端和 Vite 前端，并等待 HTTP 就绪
    版本：v7.0
    日期：2026-05-16
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

function Get-PortFromBind {
    param(
        [string]$Bind,
        [int]$DefaultPort
    )

    if ([string]::IsNullOrWhiteSpace($Bind)) {
        return $DefaultPort
    }

    $lastColon = $Bind.LastIndexOf(":")
    if ($lastColon -lt 0 -or $lastColon -eq ($Bind.Length - 1)) {
        Write-Warn "无法从 BILL_ANALYSER_HTTP_BIND='$Bind' 解析端口，使用默认端口 $DefaultPort"
        return $DefaultPort
    }

    $portText = $Bind.Substring($lastColon + 1)
    $port = 0
    if ([int]::TryParse($portText, [ref]$port) -and $port -gt 0 -and $port -le 65535) {
        return $port
    }

    Write-Warn "无法从 BILL_ANALYSER_HTTP_BIND='$Bind' 解析端口，使用默认端口 $DefaultPort"
    return $DefaultPort
}

function Get-ProbeHostFromBind {
    param(
        [string]$Bind,
        [string]$DefaultHost
    )

    if ([string]::IsNullOrWhiteSpace($Bind)) {
        return $DefaultHost
    }

    $hostText = $DefaultHost
    if ($Bind.StartsWith("[")) {
        $endBracket = $Bind.IndexOf("]")
        if ($endBracket -gt 1) {
            $hostText = $Bind.Substring(1, $endBracket - 1)
        }
    } else {
        $lastColon = $Bind.LastIndexOf(":")
        if ($lastColon -gt 0) {
            $hostText = $Bind.Substring(0, $lastColon)
        }
    }

    $hostText = $hostText.Trim()
    if ([string]::IsNullOrWhiteSpace($hostText) -or $hostText -in @("0.0.0.0", "::", "*")) {
        return "127.0.0.1"
    }

    if ($hostText.Contains(":") -and -not $hostText.StartsWith("[")) {
        return "[$hostText]"
    }

    return $hostText
}

function Get-PreferredShell {
    $pwshCmd = Get-Command pwsh -ErrorAction SilentlyContinue
    if ($pwshCmd) {
        return $pwshCmd.Source
    }

    $powershellCmd = Get-Command powershell -ErrorAction SilentlyContinue
    if ($powershellCmd) {
        return $powershellCmd.Source
    }

    return $null
}

function Get-NpmCommand {
    $npmCmd = Get-Command npm.cmd -ErrorAction SilentlyContinue
    if ($npmCmd) {
        return $npmCmd.Source
    }

    $npmCmd = Get-Command npm -ErrorAction SilentlyContinue
    if ($npmCmd) {
        return $npmCmd.Source
    }

    return $null
}

function Start-RequiredRuntimeServices {
    param([string]$Root)

    $dockerCmd = Get-Command docker -ErrorAction SilentlyContinue
    if (-not $dockerCmd) {
        Write-Err "未找到 Docker。后端现在要求 Postgres 和 Weaviate 运行。"
        Write-Warn "请先安装 Docker Desktop，或手动提供可达的 BILL_ANALYSER_POSTGRES_URL / BILL_ANALYSER_WEAVIATE_ENDPOINT。"
        exit 1
    }

    $composeFile = Join-Path $Root "docker-compose.postgres.yml"
    if (-not (Test-Path $composeFile)) {
        Write-Err "找不到 compose 文件: $composeFile"
        exit 1
    }

    Write-Gray "  正在确保 Postgres 和 Weaviate compose 服务运行..."
    & $dockerCmd.Source compose -f $composeFile up -d postgres weaviate
    if ($LASTEXITCODE -ne 0) {
        Write-Err "  ✗ Postgres/Weaviate compose 服务启动失败"
        exit 1
    }
    Write-Success "  ✓ Postgres 和 Weaviate 已启动或已在运行"
}

# 获取项目根目录
$ProjectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $ProjectRoot
$ShellExe = Get-PreferredShell

if (-not $ShellExe) {
    Write-Err "未找到 PowerShell 可执行文件（pwsh 或 powershell）"
    exit 1
}

$BackendBind = if ($env:BILL_ANALYSER_HTTP_BIND) { $env:BILL_ANALYSER_HTTP_BIND } else { "127.0.0.1:5000" }
$BackendPort = Get-PortFromBind -Bind $BackendBind -DefaultPort 5000
$BackendProbeHost = Get-ProbeHostFromBind -Bind $BackendBind -DefaultHost "127.0.0.1"
$FrontendPort = 8081
$BackendBaseUrl = "http://${BackendProbeHost}:$BackendPort"
$BackendHealthUrl = "$BackendBaseUrl/api/health"
$FrontendUrl = "http://127.0.0.1:$FrontendPort"

# 打印横幅
Write-Host ""
Write-Host "╔═══════════════════════════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "║          Bill Analyser - 账单分析系统一键启动器              ║" -ForegroundColor Cyan
Write-Host "║                       v7.0 (2026-05-16)                      ║" -ForegroundColor Cyan
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
# 函数：检查 HTTP 服务是否可访问
# ============================================================
function Test-HttpEndpoint {
    param([string]$Url)

    try {
        $response = Invoke-WebRequest -Uri $Url -UseBasicParsing -TimeoutSec 2 -ErrorAction Stop
        return ($response.StatusCode -ge 200 -and $response.StatusCode -lt 400)
    } catch {
        return $false
    }
}

# ============================================================
# 函数：等待 HTTP 服务就绪
# ============================================================
function Wait-ForHttpEndpoint {
    param(
        [string]$Url,
        [int]$TimeoutSeconds = 30,
        [string]$ServiceName
    )
    
    $startTime = Get-Date
    $timeout = New-TimeSpan -Seconds $TimeoutSeconds
    
    Write-Gray "  等待 $ServiceName 就绪 ($Url)..."
    
    while ((Get-Date) - $startTime -lt $timeout) {
        if (Test-HttpEndpoint -Url $Url) {
            Write-Success "  ✓ $ServiceName 已就绪"
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
        $null = Stop-ServiceByPort -Port $BackendPort -ServiceName "后端服务器"
    }
    
    if (-not $BackendOnly) {
        $null = Stop-ServiceByPort -Port $FrontendPort -ServiceName "前端服务器"
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

    Start-RequiredRuntimeServices -Root $ProjectRoot

    # 启动后端（在新窗口中）
    $backendScript = Join-Path $ProjectRoot "start_backend.ps1"
    if (Test-Path $backendScript) {
        Start-Process $ShellExe -ArgumentList "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $backendScript -WorkingDirectory $ProjectRoot -WindowStyle Normal
        Write-Gray "  后端服务器窗口已启动"
        
        # 等待后端就绪
        if (Wait-ForHttpEndpoint -Url $BackendHealthUrl -TimeoutSeconds 45 -ServiceName "后端服务器") {
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
    
    $npmCmd = Get-NpmCommand
    if (-not $npmCmd) {
        Write-Err "  ✗ 未找到 npm，请先安装 Node.js 22+"
        exit 1
    }

    # 检查node_modules
    $nodeModulesPath = Join-Path $ProjectRoot "src\web\node_modules"
    if (-not (Test-Path $nodeModulesPath)) {
        Write-Warn "  正在安装前端依赖..."
        $webPath = Join-Path $ProjectRoot "src\web"
        Push-Location $webPath
        & $npmCmd install
        if ($LASTEXITCODE -ne 0) {
            Pop-Location
            Write-Err "  ✗ 前端依赖安装失败"
            exit 1
        }
        Pop-Location
    }
    
    # 启动前端（在新窗口中）
    $frontendScript = Join-Path $ProjectRoot "start_frontend.ps1"
    if (Test-Path $frontendScript) {
        Start-Process $ShellExe -ArgumentList "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $frontendScript -WorkingDirectory $ProjectRoot -WindowStyle Normal
        Write-Gray "  前端服务器窗口已启动"
        
        # 等待前端就绪
        if (Wait-ForHttpEndpoint -Url $FrontendUrl -TimeoutSeconds 60 -ServiceName "前端服务器") {
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
    Write-Success "  ✓ 后端服务器: $BackendBaseUrl"
    Write-Gray "    健康检查:   $BackendHealthUrl"
}

if ($frontendStarted -or -not $BackendOnly) {
    Write-Success "  ✓ 前端应用:   $FrontendUrl"
}

Write-Host ""
Write-Gray "  提示: 关闭对应的 PowerShell 窗口可停止服务器"
Write-Gray "  或者运行: .\停止服务器.ps1"
Write-Host ""

# 自动打开浏览器
if (-not $NoBrowser -and $frontendStarted) {
    Write-Info "正在打开浏览器..."
    Start-Process $FrontendUrl
}

# v7.0: 启动完成后自动退出，无需等待按键
Write-Host "启动完成，此窗口将自动关闭..." -ForegroundColor Gray
Start-Sleep -Seconds 2
