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
$CategoryRuleBridgeName = "bill_category_rule_bridge.exe"
$CategoryRuleBridgeDebugPath = Join-Path $ProjectRoot "target\debug\$CategoryRuleBridgeName"
$CategoryRuleBridgeReleasePath = Join-Path $ProjectRoot "target\release\$CategoryRuleBridgeName"
$HttpServerName = "bill_http_server.exe"
$HttpServerDebugPath = Join-Path $ProjectRoot "target\debug\$HttpServerName"
$HttpServerReleasePath = Join-Path $ProjectRoot "target\release\$HttpServerName"
$DefaultDbPath = Join-Path $ProjectRoot "data\bills.db"
$ServerConfigPath = Join-Path $ProjectRoot "data\config\server_config.json"
$DotenvPath = Join-Path $ProjectRoot ".env"

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
        },
        @{
            DisplayName = "category rule"
            EnvName = "BILL_ANALYSER_RUST_CATEGORY_RULE_BRIDGE"
            Package = "bill-analyser-core"
            Bin = "bill_category_rule_bridge"
            DebugPath = $CategoryRuleBridgeDebugPath
            ReleasePath = $CategoryRuleBridgeReleasePath
        },
        @{
            DisplayName = "HTTP server"
            EnvName = "BILL_ANALYSER_RUST_HTTP_SERVER"
            Package = "bill-analyser-http"
            Bin = "bill_http_server"
            DebugPath = $HttpServerDebugPath
            ReleasePath = $HttpServerReleasePath
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

function Read-DotenvSettings {
    param([string]$Path)

    $settings = @{}
    if (-not (Test-Path $Path)) {
        return $settings
    }

    foreach ($line in Get-Content -Path $Path) {
        $trimmed = $line.Trim()
        if (-not $trimmed -or $trimmed.StartsWith("#")) {
            continue
        }

        $separator = $trimmed.IndexOf("=")
        if ($separator -le 0) {
            continue
        }

        $key = $trimmed.Substring(0, $separator).Trim()
        $value = $trimmed.Substring($separator + 1).Trim().Trim('"').Trim("'")
        if ($key) {
            $settings[$key] = $value
        }
    }

    return $settings
}

if (-not $env:BILL_ANALYSER_SQLITE_DB_PATH) {
    $env:BILL_ANALYSER_SQLITE_DB_PATH = $DefaultDbPath
}

$DotenvSettings = Read-DotenvSettings -Path $DotenvPath

if (-not $env:BILL_ANALYSER_AUTH_JWT_SECRET) {
    if ($env:JWT_SECRET_KEY) {
        $env:BILL_ANALYSER_AUTH_JWT_SECRET = $env:JWT_SECRET_KEY
    } elseif ($DotenvSettings.ContainsKey("JWT_SECRET_KEY")) {
        $env:BILL_ANALYSER_AUTH_JWT_SECRET = [string]$DotenvSettings["JWT_SECRET_KEY"]
    }
}

if (-not $env:BILL_ANALYSER_AUTH_JWT_ALGORITHM) {
    if ($env:JWT_ALGORITHM) {
        $env:BILL_ANALYSER_AUTH_JWT_ALGORITHM = $env:JWT_ALGORITHM
    } elseif ($DotenvSettings.ContainsKey("JWT_ALGORITHM")) {
        $env:BILL_ANALYSER_AUTH_JWT_ALGORITHM = [string]$DotenvSettings["JWT_ALGORITHM"]
    }
}

if (Test-Path $ServerConfigPath) {
    try {
        $ServerConfig = Get-Content -Path $ServerConfigPath -Raw | ConvertFrom-Json
        if (-not $env:BILL_ANALYSER_AUTH_JWT_SECRET -and $ServerConfig.jwt_secret) {
            $env:BILL_ANALYSER_AUTH_JWT_SECRET = [string]$ServerConfig.jwt_secret
        }
        if (-not $env:BILL_ANALYSER_AUTH_JWT_ALGORITHM -and $ServerConfig.jwt_algorithm) {
            $env:BILL_ANALYSER_AUTH_JWT_ALGORITHM = [string]$ServerConfig.jwt_algorithm
        }
    } catch {
        Write-Host "Warning: Failed to read server_config.json for Rust auth settings: $_" -ForegroundColor Yellow
    }
}

if (-not $env:BILL_ANALYSER_HTTP_IMPORT_ROUTE_MODE) {
    $env:BILL_ANALYSER_HTTP_IMPORT_ROUTE_MODE = "import_db_runtime"
}
if (-not $env:BILL_ANALYSER_HTTP_BIND) {
    $env:BILL_ANALYSER_HTTP_BIND = "127.0.0.1:5000"
}
$PythonFallbackHost = if ($env:BILL_ANALYSER_PYTHON_FALLBACK_HOST) { $env:BILL_ANALYSER_PYTHON_FALLBACK_HOST } else { "127.0.0.1" }
$PythonFallbackPort = if ($env:BILL_ANALYSER_PYTHON_FALLBACK_PORT) { [int]$env:BILL_ANALYSER_PYTHON_FALLBACK_PORT } else { 5001 }
$env:BILL_ANALYSER_API_HOST = $PythonFallbackHost
$env:BILL_ANALYSER_API_PORT = [string]$PythonFallbackPort
$env:BILL_ANALYSER_PYTHON_UPSTREAM = "http://${PythonFallbackHost}:${PythonFallbackPort}"

function Wait-LocalTcpPort {
    param(
        [string]$HostName,
        [int]$Port,
        [int]$Retries = 60
    )
    for ($i = 0; $i -lt $Retries; $i++) {
        if (Test-LocalTcpPortOpen -HostName $HostName -Port $Port) {
            return $true
        }
        Start-Sleep -Milliseconds 250
    }
    return $false
}

function Test-LocalTcpPortOpen {
    param(
        [string]$HostName,
        [int]$Port
    )

    $client = [System.Net.Sockets.TcpClient]::new()
    try {
        $task = $client.ConnectAsync($HostName, $Port)
        return ($task.Wait(500) -and $client.Connected)
    } catch {
        return $false
    } finally {
        $client.Dispose()
    }
}

function Test-PythonFallbackHealth {
    param([string]$Upstream)

    try {
        $Health = Invoke-RestMethod -Uri "$Upstream/api/health" -Method Get -TimeoutSec 2
        return ($Health.success -eq $true -and $Health.status -eq "healthy")
    } catch {
        return $false
    }
}

Write-Host "Python: $PythonExe" -ForegroundColor Gray
Write-Host "PYTHONPATH: $SrcRoot" -ForegroundColor Gray
Write-Host "Rust auth bridge: $env:BILL_ANALYSER_RUST_AUTH_BRIDGE" -ForegroundColor Gray
Write-Host "Rust taxonomy bridge: $env:BILL_ANALYSER_RUST_TAXONOMY_BRIDGE" -ForegroundColor Gray
Write-Host "Rust category rule bridge: $env:BILL_ANALYSER_RUST_CATEGORY_RULE_BRIDGE" -ForegroundColor Gray
Write-Host "Rust HTTP server: $env:BILL_ANALYSER_RUST_HTTP_SERVER" -ForegroundColor Gray
Write-Host "Rust import mode: $env:BILL_ANALYSER_HTTP_IMPORT_ROUTE_MODE" -ForegroundColor Gray
Write-Host "SQLite DB: $env:BILL_ANALYSER_SQLITE_DB_PATH" -ForegroundColor Gray
Write-Host "Python fallback: $env:BILL_ANALYSER_PYTHON_UPSTREAM" -ForegroundColor Gray
Write-Host "Command: $env:BILL_ANALYSER_RUST_HTTP_SERVER" -ForegroundColor Gray
Write-Host ""
Write-Host "Starting Python fallback sidecar..." -ForegroundColor Green
Write-Host "Python fallback listening on: $env:BILL_ANALYSER_PYTHON_UPSTREAM" -ForegroundColor Cyan
Write-Host "Starting Rust primary backend server..." -ForegroundColor Green
Write-Host "Rust listening on: http://$env:BILL_ANALYSER_HTTP_BIND" -ForegroundColor Cyan
Write-Host "Health check: http://127.0.0.1:5000/api/health" -ForegroundColor Cyan
Write-Host ""
Write-Host "Press Ctrl+C to stop server" -ForegroundColor Yellow
Write-Host ""

if (Test-LocalTcpPortOpen -HostName $PythonFallbackHost -Port $PythonFallbackPort) {
    Write-Host "Error: Python fallback port already in use before sidecar start: $env:BILL_ANALYSER_PYTHON_UPSTREAM" -ForegroundColor Red
    Write-Host "Stop the process on that port or set BILL_ANALYSER_PYTHON_FALLBACK_PORT." -ForegroundColor Yellow
    exit 1
}

$PythonProcess = Start-Process -FilePath $PythonExe `
    -ArgumentList @("-m", "bill_analyser.api.app") `
    -WorkingDirectory $ProjectRoot `
    -PassThru `
    -WindowStyle Hidden

try {
    if (-not (Wait-LocalTcpPort -HostName $PythonFallbackHost -Port $PythonFallbackPort)) {
        Write-Host "Error: Python fallback did not start on $env:BILL_ANALYSER_PYTHON_UPSTREAM" -ForegroundColor Red
        exit 1
    }

    if (-not (Test-PythonFallbackHealth -Upstream $env:BILL_ANALYSER_PYTHON_UPSTREAM)) {
        Write-Host "Error: Python fallback health check failed on $env:BILL_ANALYSER_PYTHON_UPSTREAM/api/health" -ForegroundColor Red
        exit 1
    }

    & $env:BILL_ANALYSER_RUST_HTTP_SERVER
    exit $LASTEXITCODE
} finally {
    if ($PythonProcess -and -not $PythonProcess.HasExited) {
        Stop-Process -Id $PythonProcess.Id -ErrorAction SilentlyContinue
    }
}
