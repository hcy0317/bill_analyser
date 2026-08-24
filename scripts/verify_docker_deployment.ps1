[CmdletBinding()]
param(
    [string]$ProjectRoot,
    [int]$WebPort = 18082,
    [string]$PublicUrl = "https://hcy-bill.long-antares.ts.net",
    [string]$LocalOpsUrl = "http://127.0.0.1:9600"
)

$ErrorActionPreference = "Stop"
if ([string]::IsNullOrWhiteSpace($ProjectRoot)) {
    $ProjectRoot = Split-Path -Parent $PSScriptRoot
}
$ProjectRoot = [System.IO.Path]::GetFullPath($ProjectRoot)
$composePath = Join-Path $ProjectRoot "compose.yml"

& docker compose --project-name bill_analyser --project-directory $ProjectRoot --file $composePath config --quiet
if ($LASTEXITCODE -ne 0) { throw "docker compose config validation failed" }

$required = @(
    "bill-analyser-postgres",
    "bill-analyser-weaviate",
    "bill-analyser-backend",
    "bill-analyser-frontend"
)
$containers = @{}
foreach ($name in $required) {
    $row = @(& docker inspect $name 2>$null | ConvertFrom-Json)[0]
    if ($null -eq $row) { throw "Required container is missing: $name" }
    $health = if ($row.State.Health) { [string]$row.State.Health.Status } else { "none" }
    if (-not $row.State.Running -or $health -ne "healthy") {
        throw "Container is not healthy: $name (running=$($row.State.Running), health=$health)"
    }
    $containers[$name] = [ordered]@{ running = $true; health = $health }
}

$postgresInspect = @(docker inspect "bill-analyser-postgres" | ConvertFrom-Json)[0]
$backendInspect = @(docker inspect "bill-analyser-backend" | ConvertFrom-Json)[0]
$postgresEnvNames = @($postgresInspect.Config.Env | ForEach-Object { ([string]$_ -split "=", 2)[0] })
$backendEnvNames = @($backendInspect.Config.Env | ForEach-Object { ([string]$_ -split "=", 2)[0] })
if ($postgresEnvNames -contains "BILL_ANALYSER_AUTH_JWT_SECRET") {
    throw "PostgreSQL container received the application JWT secret"
}
if ($postgresEnvNames -contains "BILL_ANALYSER_OPERATION_PASSWORD") {
    throw "PostgreSQL container received the application operation password"
}
if ($backendEnvNames -contains "POSTGRES_PASSWORD") {
    throw "Backend container received the raw PostgreSQL password variable"
}
if ($postgresEnvNames -notcontains "POSTGRES_PASSWORD" -or $backendEnvNames -notcontains "BILL_ANALYSER_AUTH_JWT_SECRET") {
    throw "Required container-specific runtime variables are missing"
}

$localBase = "http://127.0.0.1:$WebPort"
$localHealth = Invoke-RestMethod -Uri "$localBase/api/health/ready" -TimeoutSec 20
$frontendHealth = Invoke-WebRequest -Uri "$localBase/healthz" -TimeoutSec 10
if ([int]$frontendHealth.StatusCode -ne 200) { throw "Frontend health check failed" }

$serve = & docker exec hcy-tailscale-service-host tailscale serve status --json | ConvertFrom-Json
if ($null -eq $serve.Services.'svc:hcy-bill') { throw "svc:hcy-bill is absent from Tailscale Serve" }

[void](Resolve-DnsName -Name "hcy-bill.long-antares.ts.net" -ErrorAction Stop)
$public = Invoke-WebRequest -Uri $PublicUrl -TimeoutSec 20
if ([int]$public.StatusCode -ne 200) { throw "Public Tailscale URL did not return HTTP 200" }

$credentialPath = Join-Path $env:LOCALAPPDATA "LocalOps\control-credential.json"
$credential = Get-Content -LiteralPath $credentialPath -Raw | ConvertFrom-Json
$headers = @{ Authorization = "Bearer $($credential.token)" }
$state = Invoke-RestMethod -Uri "$LocalOpsUrl/api/state" -Headers $headers -TimeoutSec 15
$app = @($state.apps | Where-Object {
    $_.dockerResource -and $_.dockerResource.kind -eq "compose" -and $_.dockerResource.projectName -eq "bill_analyser"
}) | Select-Object -First 1
if ($null -eq $app) { throw "LocalOps does not contain the bill_analyser Compose resource" }

[ordered]@{
    compose = "valid"
    containers = $containers
    local_ready = [bool]$localHealth
    frontend_status = [int]$frontendHealth.StatusCode
    tailscale_service = "svc:hcy-bill"
    public_url = $PublicUrl
    public_status = [int]$public.StatusCode
    environment_isolation = "postgres-only database credentials; backend-only application secrets"
    local_ops_app = [ordered]@{ id = $app.id; name = $app.name; running = $app.running }
} | ConvertTo-Json -Depth 8
