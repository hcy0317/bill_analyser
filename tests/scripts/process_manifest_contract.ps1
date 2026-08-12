$ErrorActionPreference = "Stop"

$ProjectRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
. (Join-Path $ProjectRoot "scripts\process-manifest.ps1")

$process = Get-Process -Id $PID
$identity = [pscustomobject]@{
    pid = $PID
    process_name = $process.ProcessName
    process_path = $process.Path
    process_start_time = $process.StartTime.ToUniversalTime().ToString("o")
    port = 0
}

$roundTripped = $identity | ConvertTo-Json | ConvertFrom-Json
if (-not (Test-ProcessIdentity -ProcessId $PID -ExpectedIdentity $roundTripped)) {
    throw "JSON-round-tripped process identity must match the current process"
}

$roundTripped.process_start_time = (Get-Date).AddDays(-1)
if (Test-ProcessIdentity -ProcessId $PID -ExpectedIdentity $roundTripped) {
    throw "A mismatched process start time must fail closed"
}

Write-Host "Process manifest identity contract checks passed."

$listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 0)
try {
    $listener.Start()
    $listenerPort = ([System.Net.IPEndPoint]$listener.LocalEndpoint).Port
    $listenerPids = @(Get-ManifestListenerPids -Port $listenerPort)
    if ($listenerPids -notcontains $PID) {
        throw "Listener PID lookup must find the current process on port $listenerPort"
    }
} finally {
    $listener.Stop()
}

Write-Host "Process manifest listener contract checks passed."
