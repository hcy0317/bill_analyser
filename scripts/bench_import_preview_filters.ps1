param(
    [Parameter(Mandatory = $true)][string]$BaseUrl,
    [Parameter(Mandatory = $true)][string]$SessionId,
    [Parameter(Mandatory = $true)][string]$BearerToken,
    [Parameter(Mandatory = $true)][string]$DatabaseUrl,
    [Parameter(Mandatory = $true)][string]$ColdSetupCommand,
    [string]$DockerPsqlContainer,
    [int]$ColdRuns = 10,
    [int]$WarmRuns = 10,
    [bool]$RequireServerTiming = $true,
    [int]$MinimumParserCardinality = 1,
    [int]$MinimumSignalCardinality = 6,
    [int]$MinimumPayloadCardinality = 1000,
    [double]$MaximumP95Ms = 1500,
    [double]$MaximumMedianMs = 2000,
    [string]$OutputPath = ".omx/context/import-preview-filter-baseline.json"
)

$ErrorActionPreference = "Stop"
if ($ColdRuns -lt 1 -or $WarmRuns -lt 1) { throw "ColdRuns and WarmRuns must both be at least 1" }
if ([string]::IsNullOrWhiteSpace($ColdSetupCommand)) { throw "ColdSetupCommand is required for auditable cold measurements" }
$headers = @{ Authorization = "Bearer $BearerToken" }
function Invoke-BenchmarkPsql {
    param([string]$Sql)
    if (-not [string]::IsNullOrWhiteSpace($DockerPsqlContainer)) {
        $output = $Sql | docker exec -i $DockerPsqlContainer psql $DatabaseUrl -X -q -A -t -F "`t" -v ON_ERROR_STOP=1 -v "session_id=$SessionId"
    } else {
        $psql = Get-Command psql -ErrorAction Stop
        $output = & $psql.Source $DatabaseUrl -X -q -A -t -F "`t" -v ON_ERROR_STOP=1 -v "session_id=$SessionId" -c $Sql
    }
    if ($LASTEXITCODE -ne 0) { throw "PostgreSQL command failed" }
    return ($output -join "`n").Trim()
}
$fixtureSql = @"
WITH target_session AS (
  SELECT id FROM import_sessions WHERE session_key = :'session_id'
), source_stats AS (
  SELECT COUNT(DISTINCT s.id) AS source_files
  FROM import_sources s JOIN target_session t ON t.id = s.session_id
), preview_stats AS (
  SELECT COUNT(*) AS preview_rows,
    COUNT(DISTINCT p.preview_payload->>'preview_parser_id')
      FILTER (WHERE COALESCE(p.preview_payload->>'preview_parser_id','') <> '') AS parser_cardinality,
    COUNT(DISTINCT p.preview_payload) AS payload_cardinality
  FROM import_preview_rows p JOIN target_session t ON t.id = p.session_id
), signal_stats AS (
  SELECT COUNT(DISTINCT signal.key) AS signal_cardinality
  FROM import_preview_rows p JOIN target_session t ON t.id = p.session_id
  CROSS JOIN LATERAL jsonb_object_keys(COALESCE(p.preview_payload->'preview_matching_feedback','{}'::jsonb)) signal(key)
)
SELECT source_files, preview_rows, parser_cardinality, signal_cardinality, payload_cardinality
FROM source_stats CROSS JOIN preview_stats CROSS JOIN signal_stats;
"@
$fixtureRaw = Invoke-BenchmarkPsql $fixtureSql
if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($fixtureRaw)) { throw "Unable to verify benchmark fixture from PostgreSQL" }
$fixtureParts = $fixtureRaw -split "`t"
$fixture = [ordered]@{
    sourceFiles = [int]$fixtureParts[0]
    previewRows = [int]$fixtureParts[1]
    parserCardinality = [int]$fixtureParts[2]
    signalCardinality = [int]$fixtureParts[3]
    payloadCardinality = [int]$fixtureParts[4]
}
if ($fixture.sourceFiles -ne 64) { throw "Fixture rejected: source file count must equal 64, observed $($fixture.sourceFiles)" }
if ($fixture.previewRows -lt 1000) { throw "Fixture rejected: preview row count $($fixture.previewRows) is below 1000" }
if ($fixture.parserCardinality -lt $MinimumParserCardinality) { throw "Fixture rejected: parser cardinality $($fixture.parserCardinality) is below $MinimumParserCardinality" }
if ($fixture.signalCardinality -lt $MinimumSignalCardinality) { throw "Fixture rejected: signal cardinality $($fixture.signalCardinality) is below $MinimumSignalCardinality" }
if ($fixture.payloadCardinality -lt $MinimumPayloadCardinality) { throw "Fixture rejected: payload cardinality $($fixture.payloadCardinality) is below $MinimumPayloadCardinality" }

$probeUri = "$BaseUrl/api/bills/import/v2/preview/${SessionId}?page=1&pageSize=1"
$probe = Invoke-WebRequest -Uri $probeUri -Headers $headers -Method Get -UseBasicParsing
if ($probe.StatusCode -ne 200) { throw "Fixture API probe failed with HTTP $($probe.StatusCode)" }
$probeBody = $probe.Content | ConvertFrom-Json
$apiTotalValue = if ($null -ne $probeBody.data -and $null -ne $probeBody.data.total) { $probeBody.data.total } else { $probeBody.total }
$apiTotal = [int]$apiTotalValue
if ($apiTotal -lt 1000) { throw "Fixture rejected: API reports only $apiTotal preview rows" }
$metadata = if ($null -ne $probeBody.data -and $null -ne $probeBody.data.metadata) { $probeBody.data.metadata } else { $probeBody.metadata }
$facetCardinality = [ordered]@{
    categories = @($metadata.facets.categories).Count
    accounts = @($metadata.facets.accounts).Count
    tags = @($metadata.facets.tags).Count
    annotationKinds = @($metadata.counts.annotations.psobject.Properties).Count
    signalKinds = @($metadata.counts.signals.psobject.Properties).Count
}
$explainSql = @"
EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON)
SELECT p.id FROM import_preview_rows p
JOIN import_sessions i ON i.id = p.session_id
WHERE i.session_key = :'session_id'
  AND p.category_id IS NULL
  AND p.account_id IS NULL
LIMIT 50;
"@
$explainRaw = Invoke-BenchmarkPsql $explainSql
if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($explainRaw)) { throw "SQL timing unavailable: EXPLAIN ANALYZE failed" }
$explain = $explainRaw | ConvertFrom-Json
$sqlExecutionTimeMs = [double]$explain[0].'Execution Time'
if ($sqlExecutionTimeMs -le 0) { throw "SQL timing unavailable: EXPLAIN returned no positive execution time" }

function Convert-ServerTiming([string]$HeaderValue) {
    $metrics = @()
    foreach ($entry in ($HeaderValue -split ',')) {
        if ($entry.Trim() -match '^(?<name>[^;]+)(?:;dur=(?<duration>[0-9.]+))?(?:;desc="?(?<description>[^";]+)"?)?') {
            $metrics += [ordered]@{
                name = $Matches.name.Trim()
                durationMs = if ($Matches.duration) { [double]$Matches.duration } else { $null }
                description = $Matches.description
            }
        }
    }
    return $metrics
}

function Invoke-ColdSetup {
    $global:LASTEXITCODE = 0
    & ([scriptblock]::Create($ColdSetupCommand))
    if (-not $? -or $LASTEXITCODE -ne 0) {
        throw "Cold cache reset failed; refusing to report a cold sample"
    }
}

function Invoke-MeasuredPreviewRequest([string]$Uri) {
    $watch = [System.Diagnostics.Stopwatch]::StartNew()
    $response = Invoke-WebRequest -Uri $Uri -Headers $headers -Method Get -UseBasicParsing
    $watch.Stop()
    if ($response.StatusCode -ne 200) { throw "Unexpected HTTP $($response.StatusCode): $Uri" }
    $serverTimingHeader = [string]$response.Headers['Server-Timing']
    if ($RequireServerTiming -and [string]::IsNullOrWhiteSpace($serverTimingHeader)) {
        throw "Server-Timing is required but missing for $Uri"
    }
    return [ordered]@{
        requestMs = [math]::Round($watch.Elapsed.TotalMilliseconds, 3)
        serverTimingRaw = $serverTimingHeader
        serverTiming = if ($serverTimingHeader) { @(Convert-ServerTiming $serverTimingHeader) } else { @() }
    }
}
$scenarios = [ordered]@{
    category = "category=__none__"
    account = "account=__none__"
    learning = "signal=learning"
    transfer = "signal=transfer"
    annotation = "annotation=needs-review"
    combined = "category=__none__&account=__none__&annotation=needs-review"
}
$results = @()

foreach ($scenario in $scenarios.GetEnumerator()) {
    $uri = "$BaseUrl/api/bills/import/v2/preview/${SessionId}?page=1&pageSize=50&$($scenario.Value)"
    $coldMeasurements = @()
    for ($index = 0; $index -lt $ColdRuns; $index++) {
        Invoke-ColdSetup
        $coldMeasurements += @(Invoke-MeasuredPreviewRequest $uri)
    }
    $warmMeasurements = @()
    for ($index = 0; $index -lt $WarmRuns; $index++) {
        $warmMeasurements += @(Invoke-MeasuredPreviewRequest $uri)
    }
    $coldSamples = @($coldMeasurements | ForEach-Object { $_.requestMs })
    $warmSamples = @($warmMeasurements | ForEach-Object { $_.requestMs })
    $coldSorted = @($coldSamples | Sort-Object)
    $warmSorted = @($warmSamples | Sort-Object)
    $coldMedian = $coldSorted[[math]::Floor(($coldSorted.Count - 1) * 0.5)]
    $coldP95 = $coldSorted[[math]::Max(0, [math]::Ceiling($coldSorted.Count * 0.95) - 1)]
    $warmMedian = $warmSorted[[math]::Floor(($warmSorted.Count - 1) * 0.5)]
    $warmP95 = $warmSorted[[math]::Max(0, [math]::Ceiling($warmSorted.Count * 0.95) - 1)]
    $results += [ordered]@{
        scenario = $scenario.Key
        coldRuns = $coldSamples.Count
        coldMedianMs = $coldMedian
        coldP95Ms = $coldP95
        warmRuns = $warmSamples.Count
        warmMedianMs = $warmMedian
        warmP95Ms = $warmP95
        coldSamplesMs = $coldSamples
        warmSamplesMs = $warmSamples
        coldServerTiming = @($coldMeasurements | ForEach-Object { $_.serverTiming })
        warmServerTiming = @($warmMeasurements | ForEach-Object { $_.serverTiming })
    }
}

$violations = @($results | Where-Object {
    $_.coldP95Ms -gt $MaximumP95Ms -or $_.warmP95Ms -gt $MaximumP95Ms -or
    $_.coldMedianMs -gt $MaximumMedianMs -or $_.warmMedianMs -gt $MaximumMedianMs
} | ForEach-Object { $_.scenario })
$report = [ordered]@{
    generatedAt = (Get-Date).ToUniversalTime().ToString("o")
    fixtureContract = @{
        exactSourceFiles = 64
        minimumRows = 1000
        minimumParserCardinality = $MinimumParserCardinality
        minimumSignalCardinality = $MinimumSignalCardinality
        minimumPayloadCardinality = $MinimumPayloadCardinality
    }
    fixtureObserved = $fixture
    apiObserved = @{
        totalRows = $apiTotal
        facetCardinality = $facetCardinality
        probeExcludedFromMeasurements = $true
        probeMayWarmServerAndDatabaseCaches = $true
    }
    databaseExplainEvidence = @{ executionTimeMs = $sqlExecutionTimeMs; explainAnalyze = $explain; role = "supplemental" }
    measurementSemantics = @{
        apiProbeOccursBeforeAllColdSetups = $true
        everyColdSampleRequiresSuccessfulColdSetup = $true
        coldSetupResetsProbeWarmingBeforeEachColdSample = $true
        coldSetupCommand = $ColdSetupCommand
        requireServerTiming = $RequireServerTiming
    }
    environment = @{ baseUrl = $BaseUrl; powershell = $PSVersionTable.PSVersion.ToString(); database = if ($DockerPsqlContainer) { "PostgreSQL verified via docker exec psql ($DockerPsqlContainer)" } else { "PostgreSQL verified via local psql" } }
    performanceGate = @{ maximumP95Ms = $MaximumP95Ms; maximumMedianMs = $MaximumMedianMs; baselineMedianMs = 5000; requiredImprovementPercent = 60; passed = ($violations.Count -eq 0); violatingScenarios = $violations }
    results = $results
}
$directory = Split-Path -Parent $OutputPath
if ($directory) { New-Item -ItemType Directory -Force -Path $directory | Out-Null }
$report | ConvertTo-Json -Depth 8 | Set-Content -Encoding utf8 $OutputPath
$report | ConvertTo-Json -Depth 8
if ($violations.Count -gt 0) { throw "Performance gate failed for: $($violations -join ', ')" }
