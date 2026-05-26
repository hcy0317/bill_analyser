# Rust backend startup script for Bill Analyser.

$ErrorActionPreference = "Stop"

[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

Write-Host "================================================" -ForegroundColor Cyan
Write-Host "  Bill Analyser - Rust Backend Server" -ForegroundColor Cyan
Write-Host "================================================" -ForegroundColor Cyan
Write-Host ""

$ProjectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$CargoToml = Join-Path $ProjectRoot "Cargo.toml"
$HttpServerName = "bill_http_server.exe"
$HttpServerDebugPath = Join-Path $ProjectRoot "target\debug\$HttpServerName"
$HttpServerReleasePath = Join-Path $ProjectRoot "target\release\$HttpServerName"
$DefaultDbPath = Join-Path $ProjectRoot "data\bills.db"
$DotenvPath = Join-Path $ProjectRoot ".env"
$ServerConfigCandidates = @(
    (Join-Path $ProjectRoot "data\config\server_config.json"),
    (Join-Path $ProjectRoot "config\server_config.json")
)

Set-Location $ProjectRoot

if (-not (Test-Path $CargoToml)) {
    Write-Host "Error: Cargo.toml not found: $CargoToml" -ForegroundColor Red
    exit 1
}

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

function Set-RustAuthEnvFromServerConfig {
    param(
        [object]$Config,
        [string]$PropertyName,
        [string]$EnvName
    )

    if ([Environment]::GetEnvironmentVariable($EnvName)) {
        return
    }

    $property = $Config.PSObject.Properties | Where-Object { $_.Name -eq $PropertyName } | Select-Object -First 1
    if ($null -eq $property -or $null -eq $property.Value) {
        return
    }

    $value = $property.Value
    if ($value -is [bool]) {
        $value = if ($value) { "true" } else { "false" }
    }
    Set-Item -Path "Env:$EnvName" -Value ([string]$value)
}

$DotenvSettings = Read-DotenvSettings -Path $DotenvPath

if (-not $env:BILL_ANALYSER_SQLITE_DB_PATH) {
    $env:BILL_ANALYSER_SQLITE_DB_PATH = $DefaultDbPath
}

if (-not $env:BILL_ANALYSER_SQLITE_LEGACY_PATH) {
    $env:BILL_ANALYSER_SQLITE_LEGACY_PATH = $env:BILL_ANALYSER_SQLITE_DB_PATH
}

if (-not $env:BILL_ANALYSER_DATABASE_BACKEND) {
    $env:BILL_ANALYSER_DATABASE_BACKEND = "sqlite"
}

if (-not $env:BILL_ANALYSER_MIGRATION_MODE) {
    $env:BILL_ANALYSER_MIGRATION_MODE = "disabled"
}

if (-not $env:BILL_ANALYSER_HTTP_IMPORT_ROUTE_MODE) {
    $env:BILL_ANALYSER_HTTP_IMPORT_ROUTE_MODE = "import_db_runtime"
}

if (-not $env:BILL_ANALYSER_HTTP_BIND) {
    $env:BILL_ANALYSER_HTTP_BIND = "127.0.0.1:5000"
}

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

foreach ($serverConfigPath in $ServerConfigCandidates) {
    if (-not (Test-Path $serverConfigPath)) {
        continue
    }

    try {
        $ServerConfig = Get-Content -Path $serverConfigPath -Raw | ConvertFrom-Json
        if (-not $env:BILL_ANALYSER_AUTH_JWT_SECRET -and $ServerConfig.jwt_secret) {
            $env:BILL_ANALYSER_AUTH_JWT_SECRET = [string]$ServerConfig.jwt_secret
        }
        if (-not $env:BILL_ANALYSER_AUTH_JWT_ALGORITHM -and $ServerConfig.jwt_algorithm) {
            $env:BILL_ANALYSER_AUTH_JWT_ALGORITHM = [string]$ServerConfig.jwt_algorithm
        }
        Set-RustAuthEnvFromServerConfig -Config $ServerConfig -PropertyName "jwt_expiration_days" -EnvName "BILL_ANALYSER_AUTH_JWT_EXPIRATION_DAYS"
        Set-RustAuthEnvFromServerConfig -Config $ServerConfig -PropertyName "refresh_token_expiration_days" -EnvName "BILL_ANALYSER_AUTH_REFRESH_TOKEN_EXPIRATION_DAYS"
        Set-RustAuthEnvFromServerConfig -Config $ServerConfig -PropertyName "max_login_attempts" -EnvName "BILL_ANALYSER_AUTH_MAX_LOGIN_ATTEMPTS"
        Set-RustAuthEnvFromServerConfig -Config $ServerConfig -PropertyName "lockout_duration_minutes" -EnvName "BILL_ANALYSER_AUTH_LOCKOUT_DURATION_MINUTES"
        Set-RustAuthEnvFromServerConfig -Config $ServerConfig -PropertyName "enable_user_registration" -EnvName "BILL_ANALYSER_AUTH_ENABLE_USER_REGISTRATION"
        Set-RustAuthEnvFromServerConfig -Config $ServerConfig -PropertyName "require_email_verification" -EnvName "BILL_ANALYSER_AUTH_REQUIRE_EMAIL_VERIFICATION"
        Set-RustAuthEnvFromServerConfig -Config $ServerConfig -PropertyName "password_min_length" -EnvName "BILL_ANALYSER_AUTH_PASSWORD_MIN_LENGTH"
        Set-RustAuthEnvFromServerConfig -Config $ServerConfig -PropertyName "password_require_uppercase" -EnvName "BILL_ANALYSER_AUTH_PASSWORD_REQUIRE_UPPERCASE"
        Set-RustAuthEnvFromServerConfig -Config $ServerConfig -PropertyName "password_require_lowercase" -EnvName "BILL_ANALYSER_AUTH_PASSWORD_REQUIRE_LOWERCASE"
        Set-RustAuthEnvFromServerConfig -Config $ServerConfig -PropertyName "password_require_digit" -EnvName "BILL_ANALYSER_AUTH_PASSWORD_REQUIRE_DIGIT"
        Set-RustAuthEnvFromServerConfig -Config $ServerConfig -PropertyName "password_require_special" -EnvName "BILL_ANALYSER_AUTH_PASSWORD_REQUIRE_SPECIAL"
    } catch {
        Write-Host "Warning: Failed to read $serverConfigPath for Rust auth settings: $_" -ForegroundColor Yellow
    }
}

$ConfiguredServer = [Environment]::GetEnvironmentVariable("BILL_ANALYSER_RUST_HTTP_SERVER")
if ($ConfiguredServer) {
    if (-not (Test-Path $ConfiguredServer)) {
        Write-Host "Error: Configured Rust HTTP server not found: $ConfiguredServer" -ForegroundColor Red
        exit 1
    }
    $env:BILL_ANALYSER_RUST_HTTP_SERVER = $ConfiguredServer
} else {
    $CargoCommand = Get-Command cargo -ErrorAction SilentlyContinue
    if (-not $CargoCommand) {
        Write-Host "Error: Rust toolchain not found" -ForegroundColor Red
        Write-Host "Please install Rust and run: cargo build -p bill-analyser-http --bin bill_http_server" -ForegroundColor Yellow
        exit 1
    }

    Write-Host "Building Rust HTTP server..." -ForegroundColor Gray
    & $CargoCommand.Source build -p bill-analyser-http --bin bill_http_server
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Error: Failed to build Rust HTTP server" -ForegroundColor Red
        exit 1
    }

    if (Test-Path $HttpServerDebugPath) {
        $env:BILL_ANALYSER_RUST_HTTP_SERVER = $HttpServerDebugPath
    } elseif (Test-Path $HttpServerReleasePath) {
        $env:BILL_ANALYSER_RUST_HTTP_SERVER = $HttpServerReleasePath
    } else {
        Write-Host "Error: Rust HTTP server executable not found" -ForegroundColor Red
        exit 1
    }
}

Write-Host "Rust HTTP server: $env:BILL_ANALYSER_RUST_HTTP_SERVER" -ForegroundColor Gray
Write-Host "Rust import mode: $env:BILL_ANALYSER_HTTP_IMPORT_ROUTE_MODE" -ForegroundColor Gray
Write-Host "SQLite DB: $env:BILL_ANALYSER_SQLITE_DB_PATH" -ForegroundColor Gray
Write-Host "Database backend: $env:BILL_ANALYSER_DATABASE_BACKEND" -ForegroundColor Gray
Write-Host "Migration mode: $env:BILL_ANALYSER_MIGRATION_MODE" -ForegroundColor Gray
Write-Host "Postgres configured: $([bool]$env:BILL_ANALYSER_POSTGRES_URL)" -ForegroundColor Gray
Write-Host "Listening on: http://$env:BILL_ANALYSER_HTTP_BIND" -ForegroundColor Cyan
Write-Host "Health check: http://127.0.0.1:5000/api/health" -ForegroundColor Cyan
Write-Host ""
Write-Host "Press Ctrl+C to stop server" -ForegroundColor Yellow
Write-Host ""

& $env:BILL_ANALYSER_RUST_HTTP_SERVER
exit $LASTEXITCODE
