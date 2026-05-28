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

function Test-TruthyEnvValue {
    param([string]$Value)

    if (-not $Value) {
        return $false
    }
    $normalized = $Value.Trim().ToLowerInvariant()
    return @("1", "true", "yes", "on") -contains $normalized
}

function Resolve-ConfiguredPort {
    param(
        [string]$Name,
        [string]$Value,
        [int]$DefaultPort
    )

    if (-not $Value) {
        return $DefaultPort
    }

    $port = 0
    if ([int]::TryParse($Value, [ref]$port) -and $port -gt 0 -and $port -le 65535) {
        return $port
    }

    Write-Host "Warning: Invalid $Name='$Value'; using default port $DefaultPort." -ForegroundColor Yellow
    return $DefaultPort
}

function ConvertTo-UrlPart {
    param([string]$Value)
    return [System.Uri]::EscapeDataString($Value)
}

function Resolve-RequiredEndpoint {
    param(
        [string]$Name,
        [string]$Url,
        [int]$DefaultPort
    )

    try {
        $uri = [System.Uri]$Url
    } catch {
        Write-Host "Error: Invalid $Name URL: $Url" -ForegroundColor Red
        exit 1
    }

    if (-not $uri.Host) {
        Write-Host "Error: Invalid $Name URL without host: $Url" -ForegroundColor Red
        exit 1
    }

    $port = if ($uri.Port -gt 0) { $uri.Port } else { $DefaultPort }
    return @{
        Name = $Name
        Host = $uri.Host
        Port = $port
        Url = $Url
    }
}

function Test-TcpEndpoint {
    param(
        [string]$HostName,
        [int]$Port,
        [int]$TimeoutMs = 1200
    )

    $client = [System.Net.Sockets.TcpClient]::new()
    try {
        $connection = $client.BeginConnect($HostName, $Port, $null, $null)
        if (-not $connection.AsyncWaitHandle.WaitOne($TimeoutMs, $false)) {
            return $false
        }
        $client.EndConnect($connection)
        return $true
    } catch {
        return $false
    } finally {
        $client.Dispose()
    }
}

function Assert-RequiredRuntimeService {
    param([hashtable]$Endpoint)

    if (Test-TcpEndpoint -HostName $Endpoint.Host -Port $Endpoint.Port) {
        return
    }

    Write-Host "Error: Required $($Endpoint.Name) service is not reachable at $($Endpoint.Host):$($Endpoint.Port)." -ForegroundColor Red
    Write-Host "Start required services first: docker compose -f docker-compose.postgres.yml up -d postgres weaviate" -ForegroundColor Yellow
    exit 1
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

foreach ($databaseEnvName in @(
    "BILL_ANALYSER_DATABASE_BACKEND",
    "BILL_ANALYSER_POSTGRES_URL",
    "BILL_ANALYSER_POSTGRES_DB",
    "BILL_ANALYSER_POSTGRES_USER",
    "BILL_ANALYSER_POSTGRES_PASSWORD",
    "BILL_ANALYSER_POSTGRES_PORT",
    "BILL_ANALYSER_REQUIRE_POSTGRES_AFTER_CUTOVER",
    "BILL_ANALYSER_MIGRATION_MODE",
    "BILL_ANALYSER_WEAVIATE_ENABLED",
    "BILL_ANALYSER_WEAVIATE_ENDPOINT",
    "BILL_ANALYSER_WEAVIATE_PORT",
    "BILL_ANALYSER_WEAVIATE_API_KEY",
    "BILL_ANALYSER_WEAVIATE_COLLECTION_PREFIX",
    "BILL_ANALYSER_WEAVIATE_TIMEOUT_MS",
    "BILL_ANALYSER_WEAVIATE_RETRY_ATTEMPTS",
    "BILL_ANALYSER_WEAVIATE_BATCH_SIZE",
    "BILL_ANALYSER_WEAVIATE_VECTOR_DIMENSIONS"
)) {
    if (-not [Environment]::GetEnvironmentVariable($databaseEnvName) -and $DotenvSettings.ContainsKey($databaseEnvName)) {
        Set-Item -Path "Env:$databaseEnvName" -Value ([string]$DotenvSettings[$databaseEnvName])
    }
}

if (-not $env:BILL_ANALYSER_REQUIRE_POSTGRES_AFTER_CUTOVER) {
    $env:BILL_ANALYSER_REQUIRE_POSTGRES_AFTER_CUTOVER = "true"
}

$RequirePostgresAfterCutover = Test-TruthyEnvValue -Value $env:BILL_ANALYSER_REQUIRE_POSTGRES_AFTER_CUTOVER

if (-not $env:BILL_ANALYSER_SQLITE_DB_PATH) {
    $env:BILL_ANALYSER_SQLITE_DB_PATH = $DefaultDbPath
}

if (-not $env:BILL_ANALYSER_SQLITE_LEGACY_PATH) {
    $env:BILL_ANALYSER_SQLITE_LEGACY_PATH = $env:BILL_ANALYSER_SQLITE_DB_PATH
}

if (-not $env:BILL_ANALYSER_DATABASE_BACKEND) {
    $env:BILL_ANALYSER_DATABASE_BACKEND = "postgres"
}

$SelectedDatabaseBackend = $env:BILL_ANALYSER_DATABASE_BACKEND.Trim().ToLowerInvariant()
$UsePostgresRuntime = $SelectedDatabaseBackend -in @("postgres", "postgresql")
if ($SelectedDatabaseBackend -notin @("sqlite", "sqlite_legacy", "legacy_sqlite", "postgres", "postgresql")) {
    Write-Host "Error: Unsupported BILL_ANALYSER_DATABASE_BACKEND='$env:BILL_ANALYSER_DATABASE_BACKEND'." -ForegroundColor Red
    exit 1
}

if ($RequirePostgresAfterCutover -and -not $UsePostgresRuntime) {
    Write-Host "Error: BILL_ANALYSER_REQUIRE_POSTGRES_AFTER_CUTOVER=true requires BILL_ANALYSER_DATABASE_BACKEND=postgres." -ForegroundColor Red
    exit 1
}

if (-not $UsePostgresRuntime) {
    Write-Host "Error: Normal HTTP business runtime requires BILL_ANALYSER_DATABASE_BACKEND=postgres. SQLite is legacy migration/test input only." -ForegroundColor Red
    exit 1
}

if ($UsePostgresRuntime -and -not $env:BILL_ANALYSER_POSTGRES_URL) {
    $postgresPort = Resolve-ConfiguredPort -Name "BILL_ANALYSER_POSTGRES_PORT" -Value $env:BILL_ANALYSER_POSTGRES_PORT -DefaultPort 5432
    $postgresDb = if ($env:BILL_ANALYSER_POSTGRES_DB) { $env:BILL_ANALYSER_POSTGRES_DB } else { "bill_analyser" }
    $postgresUser = if ($env:BILL_ANALYSER_POSTGRES_USER) { $env:BILL_ANALYSER_POSTGRES_USER } else { "bill_analyser" }
    $postgresPassword = if ($env:BILL_ANALYSER_POSTGRES_PASSWORD) { $env:BILL_ANALYSER_POSTGRES_PASSWORD } else { "bill_analyser_dev" }
    $env:BILL_ANALYSER_POSTGRES_URL = "postgres://$(ConvertTo-UrlPart $postgresUser):$(ConvertTo-UrlPart $postgresPassword)@127.0.0.1:$postgresPort/$(ConvertTo-UrlPart $postgresDb)"
    Write-Host "Info: BILL_ANALYSER_POSTGRES_URL not set; using local docker-compose port $postgresPort." -ForegroundColor Yellow
}

if (-not $env:BILL_ANALYSER_WEAVIATE_ENABLED) {
    $env:BILL_ANALYSER_WEAVIATE_ENABLED = "true"
}

if (-not (Test-TruthyEnvValue -Value $env:BILL_ANALYSER_WEAVIATE_ENABLED)) {
    Write-Host "Error: Rust backend now requires BILL_ANALYSER_WEAVIATE_ENABLED=true." -ForegroundColor Red
    exit 1
}

if (-not $env:BILL_ANALYSER_WEAVIATE_ENDPOINT) {
    $weaviatePort = Resolve-ConfiguredPort -Name "BILL_ANALYSER_WEAVIATE_PORT" -Value $env:BILL_ANALYSER_WEAVIATE_PORT -DefaultPort 8088
    $env:BILL_ANALYSER_WEAVIATE_ENDPOINT = "http://127.0.0.1:$weaviatePort"
    Write-Host "Info: BILL_ANALYSER_WEAVIATE_ENDPOINT not set; using local docker-compose port $weaviatePort." -ForegroundColor Yellow
}

$WeaviateEndpoint = Resolve-RequiredEndpoint -Name "Weaviate" -Url $env:BILL_ANALYSER_WEAVIATE_ENDPOINT -DefaultPort 8080
$PostgresEndpoint = Resolve-RequiredEndpoint -Name "Postgres" -Url $env:BILL_ANALYSER_POSTGRES_URL -DefaultPort 5432
Assert-RequiredRuntimeService -Endpoint $PostgresEndpoint
Assert-RequiredRuntimeService -Endpoint $WeaviateEndpoint

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
Write-Host "Weaviate endpoint: $env:BILL_ANALYSER_WEAVIATE_ENDPOINT" -ForegroundColor Gray
Write-Host "Listening on: http://$env:BILL_ANALYSER_HTTP_BIND" -ForegroundColor Cyan
Write-Host "Health check: http://127.0.0.1:5000/api/health" -ForegroundColor Cyan
Write-Host ""
Write-Host "Press Ctrl+C to stop server" -ForegroundColor Yellow
Write-Host ""

& $env:BILL_ANALYSER_RUST_HTTP_SERVER
exit $LASTEXITCODE
