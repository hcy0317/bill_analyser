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
$TaxonomyBridgeName = "bill_taxonomy_bridge.exe"
$TaxonomyBridgeDebugPath = Join-Path $ProjectRoot "target\debug\$TaxonomyBridgeName"
$TaxonomyBridgeReleasePath = Join-Path $ProjectRoot "target\release\$TaxonomyBridgeName"

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
    $CargoCommand = Get-Command cargo -ErrorAction SilentlyContinue
    $BridgeSpecs = @(
        @{
            DisplayName = "auth"
            EnvName = "BILL_ANALYSER_RUST_AUTH_BRIDGE"
            Package = "bill-analyser-core"
            Bin = "bill_auth_bridge"
            DebugPath = $AuthBridgeDebugPath
            ReleasePath = $AuthBridgeReleasePath
        },
        @{
            DisplayName = "taxonomy"
            EnvName = "BILL_ANALYSER_RUST_TAXONOMY_BRIDGE"
            Package = "bill-analyser-db"
            Bin = "bill_taxonomy_bridge"
            DebugPath = $TaxonomyBridgeDebugPath
            ReleasePath = $TaxonomyBridgeReleasePath
        }
    )

    foreach ($BridgeSpec in $BridgeSpecs) {
        $ConfiguredBridge = [Environment]::GetEnvironmentVariable($BridgeSpec.EnvName)
        if ($ConfiguredBridge) {
            if (-not (Test-Path $ConfiguredBridge)) {
                Write-Host "Error: Configured Rust $($BridgeSpec.DisplayName) bridge not found: $ConfiguredBridge" -ForegroundColor Red
                exit 1
            }
            continue
        }

        if (-not $CargoCommand) {
            Write-Host "Error: Rust toolchain not found" -ForegroundColor Red
            Write-Host "Please install Rust and run: cargo build -p $($BridgeSpec.Package) --bin $($BridgeSpec.Bin)" -ForegroundColor Yellow
            exit 1
        }

        Write-Host "Building Rust $($BridgeSpec.DisplayName) bridge..." -ForegroundColor Gray
        & $CargoCommand.Source build -p $BridgeSpec.Package --bin $BridgeSpec.Bin
        if ($LASTEXITCODE -ne 0) {
            Write-Host "Error: Failed to build Rust $($BridgeSpec.DisplayName) bridge" -ForegroundColor Red
            exit 1
        }

        if (Test-Path $BridgeSpec.DebugPath) {
            Set-Item -Path ("Env:" + $BridgeSpec.EnvName) -Value $BridgeSpec.DebugPath
        } elseif (Test-Path $BridgeSpec.ReleasePath) {
            Set-Item -Path ("Env:" + $BridgeSpec.EnvName) -Value $BridgeSpec.ReleasePath
        } else {
            Write-Host "Error: Rust $($BridgeSpec.DisplayName) bridge executable not found" -ForegroundColor Red
            exit 1
        }
    }
}

$env:PYTHONPATH = $SrcRoot

Write-Host "Python: $PythonExe" -ForegroundColor Gray
Write-Host "PYTHONPATH: $SrcRoot" -ForegroundColor Gray
Write-Host "Rust auth bridge: $env:BILL_ANALYSER_RUST_AUTH_BRIDGE" -ForegroundColor Gray
Write-Host "Rust taxonomy bridge: $env:BILL_ANALYSER_RUST_TAXONOMY_BRIDGE" -ForegroundColor Gray
Write-Host "Command: .\.venv\Scripts\python.exe -m bill_analyser.api.app" -ForegroundColor Gray
Write-Host ""
Write-Host "Starting backend server..." -ForegroundColor Green
Write-Host "Listening on: http://127.0.0.1:5000" -ForegroundColor Cyan
Write-Host "Health check: http://127.0.0.1:5000/api/health" -ForegroundColor Cyan
Write-Host ""
Write-Host "Press Ctrl+C to stop server" -ForegroundColor Yellow
Write-Host ""

& $PythonExe -m bill_analyser.api.app
