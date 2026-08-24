[CmdletBinding()]
param(
    [string]$RuntimeEnvPath,
    [string]$PostgresEnvPath,
    [switch]$Force
)

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $PSScriptRoot
if ([string]::IsNullOrWhiteSpace($RuntimeEnvPath)) {
    $RuntimeEnvPath = Join-Path $ProjectRoot ".runtime\docker\backend.env"
}
if ([string]::IsNullOrWhiteSpace($PostgresEnvPath)) {
    $PostgresEnvPath = Join-Path $ProjectRoot ".runtime\docker\postgres.env"
}
$RuntimeEnvPath = [System.IO.Path]::GetFullPath($RuntimeEnvPath)
$PostgresEnvPath = [System.IO.Path]::GetFullPath($PostgresEnvPath)

function Read-DotEnvFile {
    param([string]$Path)
    $values = @{}
    if (-not (Test-Path -LiteralPath $Path)) { return $values }
    foreach ($line in Get-Content -LiteralPath $Path) {
        if ($line -notmatch '^\s*([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.*)$') { continue }
        $value = $Matches[2].Trim()
        if ($value.Length -ge 2 -and (($value[0] -eq '"' -and $value[-1] -eq '"') -or ($value[0] -eq "'" -and $value[-1] -eq "'"))) {
            $value = $value.Substring(1, $value.Length - 2)
        }
        $values[$Matches[1]] = $value
    }
    return $values
}

function Read-ContainerEnvironment {
    param([string]$ContainerName)
    $values = @{}
    $docker = Get-Command docker -ErrorAction SilentlyContinue
    if (-not $docker) { return $values }
    try {
        $rows = & $docker.Source inspect $ContainerName 2>$null | ConvertFrom-Json
        foreach ($entry in @($rows[0].Config.Env)) {
            $parts = [string]$entry -split '=', 2
            if ($parts.Count -eq 2) { $values[$parts[0]] = $parts[1] }
        }
    } catch {
        return @{}
    }
    return $values
}

function First-ConfiguredValue {
    param([string[]]$Names, [hashtable[]]$Sources, [string]$DefaultValue = "")
    foreach ($name in $Names) {
        $processValue = [Environment]::GetEnvironmentVariable($name)
        if (-not [string]::IsNullOrWhiteSpace($processValue)) { return $processValue }
        foreach ($source in $Sources) {
            if ($source.ContainsKey($name) -and -not [string]::IsNullOrWhiteSpace([string]$source[$name])) {
                return [string]$source[$name]
            }
        }
    }
    return $DefaultValue
}

function New-RandomSecret {
    $bytes = New-Object byte[] 48
    $rng = [System.Security.Cryptography.RandomNumberGenerator]::Create()
    try { $rng.GetBytes($bytes) } finally { $rng.Dispose() }
    return [Convert]::ToBase64String($bytes)
}

function Assert-DockerEnvValue {
    param([string]$Name, [string]$Value)
    if ($Value.IndexOfAny([char[]]@("`0", "`r", "`n")) -ge 0) {
        throw "$Name contains a character that cannot be stored in a Docker env file."
    }
}

function Protect-PrivateFile {
    param([string]$Path)
    if ($IsWindows -or $env:OS -eq "Windows_NT") {
        $identity = [System.Security.Principal.WindowsIdentity]::GetCurrent()
        $requiredSids = @(
            $identity.User,
            (New-Object System.Security.Principal.SecurityIdentifier('S-1-5-18')),
            (New-Object System.Security.Principal.SecurityIdentifier('S-1-5-32-544'))
        )
        $currentAcl = Get-Acl -LiteralPath $Path
        $currentRules = @($currentAcl.Access | Where-Object {
            $_.AccessControlType -eq [System.Security.AccessControl.AccessControlType]::Allow -and
            ($_.FileSystemRights -band [System.Security.AccessControl.FileSystemRights]::FullControl) -eq [System.Security.AccessControl.FileSystemRights]::FullControl
        })
        $currentSidValues = @($currentRules | ForEach-Object {
            $_.IdentityReference.Translate([System.Security.Principal.SecurityIdentifier]).Value
        } | Sort-Object -Unique)
        $requiredSidValues = @($requiredSids | ForEach-Object { $_.Value } | Sort-Object -Unique)
        if ($currentAcl.AreAccessRulesProtected -and
            @($currentSidValues | Where-Object { $_ -notin $requiredSidValues }).Count -eq 0 -and
            @($requiredSidValues | Where-Object { $_ -notin $currentSidValues }).Count -eq 0) {
            return
        }
        $acl = New-Object System.Security.AccessControl.FileSecurity
        $acl.SetAccessRuleProtection($true, $false)
        foreach ($sid in $requiredSids) {
            $rule = New-Object System.Security.AccessControl.FileSystemAccessRule(
                $sid,
                [System.Security.AccessControl.FileSystemRights]::FullControl,
                [System.Security.AccessControl.AccessControlType]::Allow
            )
            [void]$acl.AddAccessRule($rule)
        }
        Set-Acl -LiteralPath $Path -AclObject $acl
    }
}

function Protect-PrivateDirectory {
    param([string]$Path)
    if ($IsWindows -or $env:OS -eq "Windows_NT") {
        $identity = [System.Security.Principal.WindowsIdentity]::GetCurrent()
        $requiredSids = @(
            $identity.User,
            (New-Object System.Security.Principal.SecurityIdentifier('S-1-5-18')),
            (New-Object System.Security.Principal.SecurityIdentifier('S-1-5-32-544'))
        )
        $currentAcl = Get-Acl -LiteralPath $Path
        $currentRules = @($currentAcl.Access | Where-Object {
            $_.AccessControlType -eq [System.Security.AccessControl.AccessControlType]::Allow -and
            ($_.FileSystemRights -band [System.Security.AccessControl.FileSystemRights]::FullControl) -eq [System.Security.AccessControl.FileSystemRights]::FullControl
        })
        $currentSidValues = @($currentRules | ForEach-Object {
            $_.IdentityReference.Translate([System.Security.Principal.SecurityIdentifier]).Value
        } | Sort-Object -Unique)
        $requiredSidValues = @($requiredSids | ForEach-Object { $_.Value } | Sort-Object -Unique)
        if ($currentAcl.AreAccessRulesProtected -and
            @($currentSidValues | Where-Object { $_ -notin $requiredSidValues }).Count -eq 0 -and
            @($requiredSidValues | Where-Object { $_ -notin $currentSidValues }).Count -eq 0) {
            return
        }
        $acl = New-Object System.Security.AccessControl.DirectorySecurity
        $acl.SetAccessRuleProtection($true, $false)
        foreach ($sid in $requiredSids) {
            $rule = New-Object System.Security.AccessControl.FileSystemAccessRule(
                $sid,
                [System.Security.AccessControl.FileSystemRights]::FullControl,
                [System.Security.AccessControl.InheritanceFlags]'ContainerInherit, ObjectInherit',
                [System.Security.AccessControl.PropagationFlags]::None,
                [System.Security.AccessControl.AccessControlType]::Allow
            )
            [void]$acl.AddAccessRule($rule)
        }
        Set-Acl -LiteralPath $Path -AclObject $acl
    }
}

$runtimeDirectories = @((Split-Path -Parent $RuntimeEnvPath), (Split-Path -Parent $PostgresEnvPath)) | Select-Object -Unique
foreach ($directory in $runtimeDirectories) {
    New-Item -ItemType Directory -Force -Path $directory | Out-Null
    Protect-PrivateDirectory -Path $directory
}

if ((Test-Path -LiteralPath $RuntimeEnvPath) -and (Test-Path -LiteralPath $PostgresEnvPath) -and -not $Force) {
    Protect-PrivateFile -Path $RuntimeEnvPath
    Protect-PrivateFile -Path $PostgresEnvPath
    Write-Host "Docker runtime environments already exist: $RuntimeEnvPath, $PostgresEnvPath"
    exit 0
}

$dotenv = Read-DotEnvFile -Path (Join-Path $ProjectRoot ".env")
$existingBackend = Read-DotEnvFile -Path $RuntimeEnvPath
$container = Read-ContainerEnvironment -ContainerName "bill-analyser-postgres"
$sources = @($container, $existingBackend, $dotenv)

$postgresDb = First-ConfiguredValue -Names @("POSTGRES_DB", "BILL_ANALYSER_POSTGRES_DB") -Sources $sources -DefaultValue "bill_analyser"
$postgresUser = First-ConfiguredValue -Names @("POSTGRES_USER", "BILL_ANALYSER_POSTGRES_USER") -Sources $sources -DefaultValue "bill_analyser"
$postgresPassword = First-ConfiguredValue -Names @("POSTGRES_PASSWORD", "BILL_ANALYSER_POSTGRES_PASSWORD") -Sources $sources
if ([string]::IsNullOrWhiteSpace($postgresPassword)) { $postgresPassword = New-RandomSecret }
$postgresUrl = "postgres://{0}:{1}@postgres:5432/{2}" -f @(
    [Uri]::EscapeDataString($postgresUser),
    [Uri]::EscapeDataString($postgresPassword),
    [Uri]::EscapeDataString($postgresDb)
)

$serverConfig = @{}
$serverConfigPath = Join-Path $ProjectRoot "config\server.json"
if (Test-Path -LiteralPath $serverConfigPath) {
    try {
        $parsed = Get-Content -LiteralPath $serverConfigPath -Raw | ConvertFrom-Json
        if ($parsed.jwt_secret) { $serverConfig["BILL_ANALYSER_AUTH_JWT_SECRET"] = [string]$parsed.jwt_secret }
    } catch {
        throw "Unable to parse config/server.json without exposing its contents."
    }
}
$jwtSecret = First-ConfiguredValue -Names @("BILL_ANALYSER_AUTH_JWT_SECRET", "JWT_SECRET_KEY") -Sources @($existingBackend, $dotenv, $serverConfig)
if ([string]::IsNullOrWhiteSpace($jwtSecret)) { $jwtSecret = New-RandomSecret }

$postgresValues = [ordered]@{
    POSTGRES_DB = $postgresDb
    POSTGRES_USER = $postgresUser
    POSTGRES_PASSWORD = $postgresPassword
}
$backendValues = [ordered]@{
    BILL_ANALYSER_POSTGRES_URL = $postgresUrl
    BILL_ANALYSER_AUTH_JWT_SECRET = $jwtSecret
    BILL_ANALYSER_IMPORT_CONFIRM_RECEIPT_READ_SOURCE = (First-ConfiguredValue -Names @("BILL_ANALYSER_IMPORT_CONFIRM_RECEIPT_READ_SOURCE") -Sources @($dotenv) -DefaultValue "typed")
}

foreach ($name in @(
    "BILL_ANALYSER_OPERATION_PASSWORD",
    "BILL_ANALYSER_BACKUP_ENCRYPTION_KEY",
    "BILL_ANALYSER_TRUSTED_USER_HEADER_SECRET",
    "BILL_ANALYSER_AUTH_JWT_ALGORITHM",
    "BILL_ANALYSER_AUTH_JWT_EXPIRATION_DAYS",
    "BILL_ANALYSER_AUTH_REFRESH_TOKEN_EXPIRATION_DAYS",
    "BILL_ANALYSER_AUTH_MAX_LOGIN_ATTEMPTS",
    "BILL_ANALYSER_AUTH_LOCKOUT_DURATION_MINUTES",
    "BILL_ANALYSER_AUTH_ENABLE_USER_REGISTRATION",
    "BILL_ANALYSER_AUTH_REQUIRE_EMAIL_VERIFICATION",
    "BILL_ANALYSER_HTTP_TIMEOUT_MS",
    "BILL_ANALYSER_HTTP_BODY_LIMIT_BYTES",
    "BILL_ANALYSER_WEAVIATE_API_KEY",
    "BILL_ANALYSER_WEAVIATE_COLLECTION_PREFIX",
    "BILL_ANALYSER_WEAVIATE_TIMEOUT_MS",
    "BILL_ANALYSER_WEAVIATE_RETRY_ATTEMPTS",
    "BILL_ANALYSER_WEAVIATE_BATCH_SIZE",
    "BILL_ANALYSER_WEAVIATE_VECTOR_DIMENSIONS",
    "BILL_ANALYSER_LLM_BASE_URL_ALLOWLIST",
    "BILL_ANALYSER_LLM_TOKEN_URL_ALLOWLIST",
    "BILL_ANALYSER_BACKUP_SYNC_ENDPOINT_ALLOWLIST"
)) {
    $value = First-ConfiguredValue -Names @($name) -Sources @($existingBackend, $dotenv)
    if (-not [string]::IsNullOrWhiteSpace($value)) { $backendValues[$name] = $value }
}

foreach ($path in @((Join-Path $ProjectRoot "data"), (Join-Path $ProjectRoot "backup"))) {
    New-Item -ItemType Directory -Force -Path $path | Out-Null
}

foreach ($target in @(
    [ordered]@{ Path = $PostgresEnvPath; Values = $postgresValues },
    [ordered]@{ Path = $RuntimeEnvPath; Values = $backendValues }
)) {
    $lines = foreach ($entry in $target.Values.GetEnumerator()) {
        $value = [string]$entry.Value
        Assert-DockerEnvValue -Name ([string]$entry.Key) -Value $value
        "{0}={1}" -f $entry.Key, $value
    }
    [System.IO.File]::WriteAllLines($target.Path, $lines, (New-Object System.Text.UTF8Encoding($false)))
    Protect-PrivateFile -Path $target.Path
}
Write-Host "Prepared protected Docker runtime environments: $RuntimeEnvPath, $PostgresEnvPath"
