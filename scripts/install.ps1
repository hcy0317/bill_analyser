# Bill Analyser - 安装脚本
# =====================================
# 本脚本用于初始化完整开发环境
# =====================================

param(
    [switch]$SkipPython,
    [switch]$SkipNode,
    [switch]$Force
)

$ErrorActionPreference = "Stop"

Write-Host ""
Write-Host "================================================" -ForegroundColor Cyan
Write-Host "  Bill Analyser - Installation Script" -ForegroundColor Cyan
Write-Host "================================================" -ForegroundColor Cyan
Write-Host ""

# 获取项目根目录
$ProjectRoot = Split-Path -Parent $PSScriptRoot
if (-not $ProjectRoot) {
    $ProjectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
}
if (-not $ProjectRoot) {
    $ProjectRoot = Get-Location
}

Set-Location $ProjectRoot
Write-Host "Project root: $ProjectRoot" -ForegroundColor Gray

# ============================================
# Python 环境设置
# ============================================

if (-not $SkipPython) {
    Write-Host ""
    Write-Host "[1/4] Setting up Python environment..." -ForegroundColor Yellow
    
    # 检查 Python 版本
    try {
        $pythonVersion = python --version 2>&1
        Write-Host "Python version: $pythonVersion" -ForegroundColor Gray
        
        $versionMatch = $pythonVersion -match "Python (\d+)\.(\d+)"
        if ($versionMatch) {
            $major = [int]$Matches[1]
            $minor = [int]$Matches[2]
            if ($major -lt 3 -or ($major -eq 3 -and $minor -lt 10)) {
                Write-Host "Error: Python 3.10+ is required" -ForegroundColor Red
                exit 1
            }
        }
    }
    catch {
        Write-Host "Error: Python not found. Please install Python 3.10+" -ForegroundColor Red
        exit 1
    }
    
    # 创建虚拟环境
    $venvPath = Join-Path $ProjectRoot ".venv"
    if ((Test-Path $venvPath) -and -not $Force) {
        Write-Host "Virtual environment already exists. Use -Force to recreate." -ForegroundColor Gray
    }
    else {
        if (Test-Path $venvPath) {
            Write-Host "Removing existing virtual environment..." -ForegroundColor Yellow
            Remove-Item -Recurse -Force $venvPath
        }
        Write-Host "Creating virtual environment..." -ForegroundColor Yellow
        python -m venv .venv
        if ($LASTEXITCODE -ne 0) {
            Write-Host "Error: Failed to create virtual environment" -ForegroundColor Red
            exit 1
        }
    }
    
    # 安装 Python 依赖
    Write-Host "Installing Python dependencies..." -ForegroundColor Yellow
    & ".\.venv\Scripts\pip.exe" install --upgrade pip -q
    & ".\.venv\Scripts\pip.exe" install -r requirements.txt -q
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Error: Failed to install Python dependencies" -ForegroundColor Red
        exit 1
    }
    
    Write-Host "Python environment setup complete!" -ForegroundColor Green
}
else {
    Write-Host "[1/4] Skipping Python environment setup" -ForegroundColor Gray
}

# ============================================
# Node.js 环境设置
# ============================================

if (-not $SkipNode) {
    Write-Host ""
    Write-Host "[2/4] Setting up Node.js environment..." -ForegroundColor Yellow
    
    # 检查 Node.js 版本
    try {
        $nodeVersion = node --version 2>&1
        Write-Host "Node.js version: $nodeVersion" -ForegroundColor Gray
        
        $versionMatch = $nodeVersion -match "v(\d+)"
        if ($versionMatch) {
            $major = [int]$Matches[1]
            if ($major -lt 18) {
                Write-Host "Error: Node.js 18+ is required" -ForegroundColor Red
                exit 1
            }
        }
    }
    catch {
        Write-Host "Error: Node.js not found. Please install Node.js 18+" -ForegroundColor Red
        exit 1
    }
    
    # 安装前端依赖
    $frontendPath = Join-Path $ProjectRoot "src\web"
    Set-Location $frontendPath
    
    $nodeModulesPath = Join-Path $frontendPath "node_modules"
    if ((Test-Path $nodeModulesPath) -and -not $Force) {
        Write-Host "Node modules already exist. Use -Force to reinstall." -ForegroundColor Gray
    }
    else {
        if (Test-Path $nodeModulesPath) {
            Write-Host "Removing existing node_modules..." -ForegroundColor Yellow
            Remove-Item -Recurse -Force $nodeModulesPath
        }
        Write-Host "Installing frontend dependencies..." -ForegroundColor Yellow
        npm install --silent
        if ($LASTEXITCODE -ne 0) {
            Write-Host "Error: Failed to install frontend dependencies" -ForegroundColor Red
            exit 1
        }
    }
    
    Set-Location $ProjectRoot
    Write-Host "Node.js environment setup complete!" -ForegroundColor Green
}
else {
    Write-Host "[2/4] Skipping Node.js environment setup" -ForegroundColor Gray
}

# ============================================
# 创建所需目录
# ============================================

Write-Host ""
Write-Host "[3/4] Creating required directories..." -ForegroundColor Yellow

$directories = @(
    "data",
    "logs",
    "uploads",
    "output",
    "backup",
    "bills"
)

foreach ($dir in $directories) {
    $dirPath = Join-Path $ProjectRoot $dir
    if (-not (Test-Path $dirPath)) {
        New-Item -ItemType Directory -Path $dirPath -Force | Out-Null
        Write-Host "  Created: $dir/" -ForegroundColor Gray
    }
}

Write-Host "Directories setup complete!" -ForegroundColor Green

# ============================================
# 初始化配置
# ============================================

Write-Host ""
Write-Host "[4/4] Checking configuration..." -ForegroundColor Yellow

$configPath = Join-Path $ProjectRoot "config"
if (-not (Test-Path $configPath)) {
    New-Item -ItemType Directory -Path $configPath -Force | Out-Null
}

# 如不存在则创建默认服务端配置
$serverConfigPath = Join-Path $configPath "server_config.json"
if (-not (Test-Path $serverConfigPath)) {
    $defaultConfig = @{
        host = "127.0.0.1"
        port = 5000
        debug = $true
    } | ConvertTo-Json -Depth 10
    Set-Content -Path $serverConfigPath -Value $defaultConfig -Encoding UTF8
    Write-Host "  Created default server_config.json" -ForegroundColor Gray
}

Write-Host "Configuration check complete!" -ForegroundColor Green

# ============================================
# 摘要
# ============================================

Write-Host ""
Write-Host "================================================" -ForegroundColor Green
Write-Host "  Installation Complete!" -ForegroundColor Green
Write-Host "================================================" -ForegroundColor Green
Write-Host ""
Write-Host "Next steps:" -ForegroundColor Cyan
Write-Host "  1. Start backend:  .\启动后端.bat  (or .\start_backend.ps1)" -ForegroundColor White
Write-Host "  2. Start frontend: .\启动前端.bat  (or .\start_frontend.ps1)" -ForegroundColor White
Write-Host "  3. Open browser:   http://127.0.0.1:8081" -ForegroundColor White
Write-Host ""
Write-Host "For more information, see README.md" -ForegroundColor Gray
Write-Host ""
