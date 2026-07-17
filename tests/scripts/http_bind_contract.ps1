$ErrorActionPreference = "Stop"

$ProjectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
. (Join-Path $ProjectRoot "scripts\http-bind.ps1")

function Assert-Equal {
    param($Actual, $Expected, [string]$Label)
    if ($Actual -ne $Expected) {
        throw "$Label expected '$Expected' but got '$Actual'"
    }
}

$default = Resolve-BillAnalyserHttpEndpoint -Bind ""
Assert-Equal $default.Bind "127.0.0.1:5000" "default bind"
Assert-Equal $default.Port 5000 "default port"
Assert-Equal $default.HealthUrl "http://127.0.0.1:5000/api/health" "default health"

$custom = Resolve-BillAnalyserHttpEndpoint -Bind "127.0.0.1:5123"
Assert-Equal $custom.Port 5123 "custom port"
Assert-Equal $custom.HealthUrl "http://127.0.0.1:5123/api/health" "custom health"

$localhost = Resolve-BillAnalyserHttpEndpoint -Bind "localhost:5124"
Assert-Equal $localhost.ProbeHost "localhost" "localhost probe"
Assert-Equal $localhost.HealthUrl "http://localhost:5124/api/health" "localhost health"

$wildcard = Resolve-BillAnalyserHttpEndpoint -Bind "0.0.0.0:5125"
Assert-Equal $wildcard.ProbeHost "127.0.0.1" "wildcard probe"

$ipv6 = Resolve-BillAnalyserHttpEndpoint -Bind "[::1]:5126"
Assert-Equal $ipv6.Bind "[::1]:5126" "IPv6 bind"
Assert-Equal $ipv6.HealthUrl "http://[::1]:5126/api/health" "IPv6 health"

foreach ($invalidBind in @("not-a-bind", "example.invalid:5123", "localhost:0")) {
    $invalidRejected = $false
    try {
        $null = Resolve-BillAnalyserHttpEndpoint -Bind $invalidBind
    } catch {
        $invalidRejected = $true
    }
    Assert-Equal $invalidRejected $true "invalid bind rejection: $invalidBind"
}

$startSource = Get-Content -Raw -LiteralPath (Join-Path $ProjectRoot "start_backend.ps1")
$stopScriptName = -join @([char]0x505C, [char]0x6B62, [char]0x670D, [char]0x52A1, [char]0x5668, ".ps1")
$launcherScriptName = -join @([char]0x4E00, [char]0x952E, [char]0x542F, [char]0x52A8, ".ps1")
$stopSource = Get-Content -Raw -LiteralPath (Join-Path $ProjectRoot $stopScriptName)
$launcherSource = Get-Content -Raw -LiteralPath (Join-Path $ProjectRoot $launcherScriptName)

foreach ($entry in @(
    @{ Name = "start_backend.ps1"; Source = $startSource },
    @{ Name = $stopScriptName; Source = $stopSource },
    @{ Name = $launcherScriptName; Source = $launcherSource }
)) {
    if ($entry.Source -notmatch "Resolve-BillAnalyserHttpEndpoint") {
        throw "$($entry.Name) must consume the shared HTTP endpoint parser"
    }
}
if ($startSource -match 'Health check: http://127\.0\.0\.1:5000') {
    throw "start_backend.ps1 must not hard-code the backend health port"
}
if ($stopSource -match 'LocalPort\s+5000') {
    throw "$stopScriptName must not hard-code the backend stop port"
}
if ($launcherSource -match 'function\s+Get-(Port|ProbeHost)FromBind') {
    throw "$launcherScriptName must not retain a second HTTP bind parser"
}

Write-Host "HTTP bind contract checks passed."
