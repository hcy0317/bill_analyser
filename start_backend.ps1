# Backend Server Startup Script
# Bill Analyser System

Write-Host "================================================" -ForegroundColor Cyan
Write-Host "  Bill Analyser - Backend Server" -ForegroundColor Cyan
Write-Host "================================================" -ForegroundColor Cyan
Write-Host ""

$ProjectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$SrcRoot = Join-Path $ProjectRoot "src"
$BackendEntry = Join-Path $ProjectRoot "src\bill_analyser\api\app.py"
$PythonExe = Join-Path $ProjectRoot ".venv\Scripts\python.exe"

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

$env:PYTHONPATH = $SrcRoot

Write-Host "Python: $PythonExe" -ForegroundColor Gray
Write-Host "PYTHONPATH: $SrcRoot" -ForegroundColor Gray
Write-Host "Command: .\.venv\Scripts\python.exe -m bill_analyser.api.app" -ForegroundColor Gray
Write-Host ""
Write-Host "Starting backend server..." -ForegroundColor Green
Write-Host "Listening on: http://127.0.0.1:5000" -ForegroundColor Cyan
Write-Host "Health check: http://127.0.0.1:5000/api/health" -ForegroundColor Cyan
Write-Host ""
Write-Host "Press Ctrl+C to stop server" -ForegroundColor Yellow
Write-Host ""

& $PythonExe -m bill_analyser.api.app
