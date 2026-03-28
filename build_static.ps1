# Build Frontend Static Assets Script
# Bill Analyser System

Write-Host "================================================" -ForegroundColor Cyan
Write-Host "  Build Frontend Static Assets" -ForegroundColor Cyan
Write-Host "================================================" -ForegroundColor Cyan
Write-Host ""

$ProjectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$WebDir = Join-Path $ProjectRoot "src\web"
$DistDir = Join-Path $WebDir "dist"
$StaticDir = Join-Path $ProjectRoot "src\bill_analyser\static"

Set-Location $ProjectRoot

if (-not (Test-Path $WebDir)) {
    Write-Host "Error: Frontend directory not found: $WebDir" -ForegroundColor Red
    exit 1
}

if (-not (Test-Path (Join-Path $WebDir "node_modules"))) {
    Write-Host "Installing frontend dependencies..." -ForegroundColor Yellow
    Set-Location $WebDir
    npm install
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Error: Failed to install frontend dependencies" -ForegroundColor Red
        exit 1
    }
    Set-Location $ProjectRoot
}

Write-Host "Building frontend..." -ForegroundColor Yellow
Set-Location $WebDir
npm run build
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
Write-Host "Copying build output to package static directory..." -ForegroundColor Yellow

if (-not (Test-Path $StaticDir)) {
    New-Item -ItemType Directory -Path $StaticDir -Force | Out-Null
    Write-Host "Created static directory: $StaticDir" -ForegroundColor Gray
}

Get-ChildItem -Path $DistDir -Recurse | ForEach-Object {
    $relativePath = $_.FullName.Substring($DistDir.Length + 1)
    $destPath = Join-Path $StaticDir $relativePath

    if ($_.PSIsContainer) {
        if (-not (Test-Path $destPath)) {
            New-Item -ItemType Directory -Path $destPath -Force | Out-Null
        }
    } else {
        $destDir = Split-Path -Parent $destPath
        if (-not (Test-Path $destDir)) {
            New-Item -ItemType Directory -Path $destDir -Force | Out-Null
        }
        Copy-Item -Path $_.FullName -Destination $destPath -Force
    }
}

Write-Host ""
Write-Host "Build complete!" -ForegroundColor Green
Write-Host "Static assets copied to: $StaticDir" -ForegroundColor Cyan
Write-Host ""
