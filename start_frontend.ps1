# Frontend Server Launcher

Write-Host "================================================" -ForegroundColor Cyan
Write-Host "  Bill Analyser - Frontend Server" -ForegroundColor Cyan
Write-Host "================================================" -ForegroundColor Cyan
Write-Host ""

$ProjectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $ProjectRoot

$FrontendPath = Join-Path $ProjectRoot "src\web"
if (-not (Test-Path $FrontendPath)) {
    Write-Host "Error: Frontend directory not found" -ForegroundColor Red
    Read-Host "Press Enter to exit"
    exit 1
}

Set-Location $FrontendPath

if (-not (Test-Path "node_modules")) {
    Write-Host "Installing dependencies..." -ForegroundColor Yellow
    npm install
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Error: npm install failed" -ForegroundColor Red
        Read-Host "Press Enter to exit"
        exit 1
    }
}

Write-Host "Starting frontend server..." -ForegroundColor Green
Write-Host "URL: http://127.0.0.1:8081" -ForegroundColor Cyan
Write-Host ""
Write-Host "Press Ctrl+C to stop" -ForegroundColor Yellow
Write-Host ""

npm run dev