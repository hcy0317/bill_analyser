# Backend Server Startup Script
# Bill Analyser System

Write-Host "================================================" -ForegroundColor Cyan
Write-Host "  Bill Analyser - Backend Server" -ForegroundColor Cyan  
Write-Host "================================================" -ForegroundColor Cyan
Write-Host ""

# Get project root directory
$ProjectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $ProjectRoot

# Check virtual environment
if (-not (Test-Path ".venv\Scripts\python.exe")) {
    Write-Host "Error: Virtual environment not found" -ForegroundColor Red
    Write-Host "Please run: python -m venv .venv" -ForegroundColor Yellow
    Write-Host "Then install dependencies: .\.venv\Scripts\pip install -r requirements.txt" -ForegroundColor Yellow
    Read-Host "Press Enter to exit"
    exit 1
}

# Check Flask dependency
Write-Host "Checking Flask..." -ForegroundColor Yellow
.\.venv\Scripts\pip show Flask >$null 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Host "Installing Flask..." -ForegroundColor Yellow
    .\.venv\Scripts\pip install Flask Flask-CORS -q
}

# Set PYTHONPATH to project root
$env:PYTHONPATH = $ProjectRoot
Write-Host "PYTHONPATH set to: $ProjectRoot" -ForegroundColor Gray

# Start server
Write-Host "Starting backend server..." -ForegroundColor Green
Write-Host "Listening on: http://127.0.0.1:5000" -ForegroundColor Cyan
Write-Host "Health check: http://127.0.0.1:5000/api/health" -ForegroundColor Cyan
Write-Host ""
Write-Host "Press Ctrl+C to stop server" -ForegroundColor Yellow
Write-Host ""

.\.venv\Scripts\python.exe src\api\app.py
