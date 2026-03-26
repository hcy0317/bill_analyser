# Frontend Server Launcher

Write-Host "================================================" -ForegroundColor Cyan
Write-Host "  Bill Analyser - Frontend Server" -ForegroundColor Cyan
Write-Host "================================================" -ForegroundColor Cyan
Write-Host ""

$ProjectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $ProjectRoot

$FrontendPath = Join-Path $ProjectRoot "src\web"
$NpmCmd = Get-Command npm.cmd -ErrorAction SilentlyContinue
if (-not (Test-Path $FrontendPath)) {
    Write-Host "Error: Frontend directory not found" -ForegroundColor Red
    exit 1
}

if (-not $NpmCmd) {
    Write-Host "Error: npm.cmd not found, please install Node.js 18+" -ForegroundColor Red
    exit 1
}

Set-Location $FrontendPath

if (-not (Test-Path "node_modules")) {
    Write-Host "Installing dependencies..." -ForegroundColor Yellow
    & $NpmCmd.Source install
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Error: npm install failed" -ForegroundColor Red
        exit 1
    }
}

Write-Host "Directory: $FrontendPath" -ForegroundColor Gray
Write-Host "Command: npm run dev" -ForegroundColor Gray
Write-Host "Starting frontend server..." -ForegroundColor Green
Write-Host "URL: http://127.0.0.1:8081" -ForegroundColor Cyan
Write-Host ""
Write-Host "Press Ctrl+C to stop" -ForegroundColor Yellow
Write-Host ""

& $NpmCmd.Source run dev