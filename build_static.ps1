# Build frontend static assets.

$ErrorActionPreference = "Stop"

Write-Host "================================================" -ForegroundColor Cyan
Write-Host "  Build Frontend Static Assets" -ForegroundColor Cyan
Write-Host "================================================" -ForegroundColor Cyan
Write-Host ""

$ProjectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$WebDir = Join-Path $ProjectRoot "src\web"
$DistDir = Join-Path $WebDir "dist"

Set-Location $ProjectRoot

if (-not (Test-Path $WebDir)) {
    Write-Host "Error: Frontend directory not found: $WebDir" -ForegroundColor Red
    exit 1
}

$NpmCmd = Get-Command npm.cmd -ErrorAction SilentlyContinue
if (-not $NpmCmd) {
    Write-Host "Error: npm.cmd not found, please install Node.js 22+" -ForegroundColor Red
    exit 1
}

if (-not (Test-Path (Join-Path $WebDir "node_modules"))) {
    Write-Host "Installing frontend dependencies..." -ForegroundColor Yellow
    Set-Location $WebDir
    & $NpmCmd.Source install
    Set-Location $ProjectRoot
}

Write-Host "Building frontend..." -ForegroundColor Yellow
Set-Location $WebDir
& $NpmCmd.Source run build
if ($LASTEXITCODE -ne 0) {
    Write-Host "Error: Frontend build failed" -ForegroundColor Red
    exit 1
}
Set-Location $ProjectRoot

if (-not (Test-Path $DistDir)) {
    Write-Host "Error: Build output not found: $DistDir" -ForegroundColor Red
    exit 1
}

Write-Host ""
Write-Host "Build complete!" -ForegroundColor Green
Write-Host "Static assets: $DistDir" -ForegroundColor Cyan
Write-Host ""
