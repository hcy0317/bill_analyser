# 后端服务器启动脚本
# Bill Analyser 系统

Write-Host "================================================" -ForegroundColor Cyan
Write-Host "  Bill Analyser - Backend Server" -ForegroundColor Cyan
Write-Host "================================================" -ForegroundColor Cyan
Write-Host ""

$ProjectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$SrcRoot = Join-Path $ProjectRoot "src"
$BackendEntry = Join-Path $ProjectRoot "src\bill_analyser\api\app.py"
$PythonExe = Join-Path $ProjectRoot ".venv\Scripts\python.exe"
$CargoToml = Join-Path $ProjectRoot "Cargo.toml"
$AuthBridgeName = "bill_auth_bridge.exe"
$AuthBridgeDebugPath = Join-Path $ProjectRoot "target\debug\$AuthBridgeName"
$AuthBridgeReleasePath = Join-Path $ProjectRoot "target\release\$AuthBridgeName"

Set-Location $ProjectRoot

if (-not (Test-Path $PythonExe)) {
    Write-Host "Error: Virtual environment not found" -ForegroundColor Red
    Write-Host "Please run: py -3.14 -m venv .venv" -ForegroundColor Yellow
    Write-Host "Then install dependencies: .\.venv\Scripts\python.exe -m pip install -e ." -ForegroundColor Yellow
    exit 1
}

if (-not (Test-Path $BackendEntry)) {
    Write-Host "Error: Backend entry not found: $BackendEntry" -ForegroundColor Red
    exit 1
}

$pythonVersion = & $PythonExe -c "import sys; print(f'{sys.version_info.major}.{sys.version_info.minor}')"
if ($LASTEXITCODE -ne 0) {
    Write-Host "Error: Failed to inspect Python version" -ForegroundColor Red
    exit 1
}

if ([version]$pythonVersion -lt [version]"3.14") {
    Write-Host "Error: Python 3.14+ is required, current version is $pythonVersion" -ForegroundColor Red
    Write-Host "Please recreate the virtual environment with Python 3.14." -ForegroundColor Yellow
    exit 1
}

if (Test-Path $CargoToml) {
    if ($env:BILL_ANALYSER_RUST_AUTH_BRIDGE) {
        if (-not (Test-Path $env:BILL_ANALYSER_RUST_AUTH_BRIDGE)) {
            Write-Host "Error: Configured Rust auth bridge not found: $env:BILL_ANALYSER_RUST_AUTH_BRIDGE" -ForegroundColor Red
            exit 1
        }
    } else {
        $CargoCommand = Get-Command cargo -ErrorAction SilentlyContinue
        if (-not $CargoCommand) {
            Write-Host "Error: Rust toolchain not found" -ForegroundColor Red
            Write-Host "Please install Rust and run: cargo build -p bill-analyser-core --bin bill_auth_bridge" -ForegroundColor Yellow
            exit 1
        }

        Write-Host "Building Rust auth bridge..." -ForegroundColor Gray
        & $CargoCommand.Source build -p bill-analyser-core --bin bill_auth_bridge
        if ($LASTEXITCODE -ne 0) {
            Write-Host "Error: Failed to build Rust auth bridge" -ForegroundColor Red
            exit 1
        }

        if (Test-Path $AuthBridgeDebugPath) {
            $env:BILL_ANALYSER_RUST_AUTH_BRIDGE = $AuthBridgeDebugPath
        } elseif (Test-Path $AuthBridgeReleasePath) {
            $env:BILL_ANALYSER_RUST_AUTH_BRIDGE = $AuthBridgeReleasePath
        } else {
            Write-Host "Error: Rust auth bridge executable not found" -ForegroundColor Red
            exit 1
        }
    }
}

$env:PYTHONPATH = $SrcRoot

Write-Host "Python: $PythonExe" -ForegroundColor Gray
Write-Host "PYTHONPATH: $SrcRoot" -ForegroundColor Gray
Write-Host "Rust auth bridge: $env:BILL_ANALYSER_RUST_AUTH_BRIDGE" -ForegroundColor Gray
Write-Host "Command: .\.venv\Scripts\python.exe -m bill_analyser.api.app" -ForegroundColor Gray
Write-Host ""
Write-Host "Starting backend server..." -ForegroundColor Green
Write-Host "Listening on: http://127.0.0.1:5000" -ForegroundColor Cyan
Write-Host "Health check: http://127.0.0.1:5000/api/health" -ForegroundColor Cyan
Write-Host ""
Write-Host "Press Ctrl+C to stop server" -ForegroundColor Yellow
Write-Host ""

& $PythonExe -m bill_analyser.api.app
