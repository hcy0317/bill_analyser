# Bill Analyser Rust/Frontend development environment setup.

param(
    [switch]$SkipRust,
    [switch]$SkipNode,
    [switch]$Force
)

$ErrorActionPreference = "Stop"

Write-Host ""
Write-Host "================================================" -ForegroundColor Cyan
Write-Host "  Bill Analyser - Installation Script" -ForegroundColor Cyan
Write-Host "================================================" -ForegroundColor Cyan
Write-Host ""

$ProjectRoot = Split-Path -Parent $PSScriptRoot
if (-not $ProjectRoot) {
    $ProjectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
}
if (-not $ProjectRoot) {
    $ProjectRoot = Get-Location
}

Set-Location $ProjectRoot
Write-Host "Project root: $ProjectRoot" -ForegroundColor Gray

if (-not $SkipRust) {
    Write-Host ""
    Write-Host "[1/3] Checking Rust toolchain..." -ForegroundColor Yellow

    $CargoCommand = Get-Command cargo -ErrorAction SilentlyContinue
    if (-not $CargoCommand) {
        Write-Host "Error: Rust toolchain not found. Install Rust stable first." -ForegroundColor Red
        exit 1
    }

    & $CargoCommand.Source --version
    Write-Host "Building Rust HTTP server..." -ForegroundColor Yellow
    & $CargoCommand.Source build -p bill-analyser-http --bin bill_http_server
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Error: Failed to build Rust HTTP server" -ForegroundColor Red
        exit 1
    }

    Write-Host "Rust setup complete!" -ForegroundColor Green
} else {
    Write-Host "[1/3] Skipping Rust setup" -ForegroundColor Gray
}

if (-not $SkipNode) {
    Write-Host ""
    Write-Host "[2/3] Setting up Node.js environment..." -ForegroundColor Yellow

    $NpmCommand = Get-Command npm.cmd -ErrorAction SilentlyContinue
    if (-not $NpmCommand) {
        Write-Host "Error: npm.cmd not found. Please install Node.js 22+." -ForegroundColor Red
        exit 1
    }

    $frontendPath = Join-Path $ProjectRoot "src\web"
    Set-Location $frontendPath

    $nodeModulesPath = Join-Path $frontendPath "node_modules"
    if ((Test-Path $nodeModulesPath) -and -not $Force) {
        Write-Host "Node modules already exist. Use -Force to reinstall." -ForegroundColor Gray
    } else {
        if (Test-Path $nodeModulesPath) {
            Write-Host "Removing existing node_modules..." -ForegroundColor Yellow
            Remove-Item -Recurse -Force $nodeModulesPath
        }
        Write-Host "Installing frontend dependencies..." -ForegroundColor Yellow
        & $NpmCommand.Source install
        if ($LASTEXITCODE -ne 0) {
            Write-Host "Error: Failed to install frontend dependencies" -ForegroundColor Red
            exit 1
        }
    }

    Set-Location $ProjectRoot
    Write-Host "Node.js setup complete!" -ForegroundColor Green
} else {
    Write-Host "[2/3] Skipping Node.js setup" -ForegroundColor Gray
}

Write-Host ""
Write-Host "[3/3] Creating required directories..." -ForegroundColor Yellow

$directories = @(
    "data",
    "logs",
    "uploads",
    "output",
    "backup",
    "bills",
    "config"
)

foreach ($dir in $directories) {
    $dirPath = Join-Path $ProjectRoot $dir
    if (-not (Test-Path $dirPath)) {
        New-Item -ItemType Directory -Path $dirPath -Force | Out-Null
        Write-Host "  Created: $dir/" -ForegroundColor Gray
    }
}

$serverConfigPath = Join-Path $ProjectRoot "config\server_config.json"
if (-not (Test-Path $serverConfigPath)) {
    $defaultConfig = @{
        host = "127.0.0.1"
        port = 5000
        debug = $true
    } | ConvertTo-Json -Depth 10
    Set-Content -Path $serverConfigPath -Value $defaultConfig -Encoding UTF8
    Write-Host "  Created default server_config.json" -ForegroundColor Gray
}

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
