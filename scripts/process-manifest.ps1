function Get-ManifestListenerPids {
    param([int]$Port)

    $netstat = Join-Path $env:SystemRoot "System32\netstat.exe"
    $listenerPids = @(
        & $netstat -ano -p tcp 2>$null | ForEach-Object {
            if ($_ -match '^\s*TCP\s+\S+:(?<port>\d+)\s+\S+\s+LISTENING\s+(?<pid>\d+)\s*$' -and
                [int]$Matches.port -eq $Port -and [int]$Matches.pid -gt 0) {
                [int]$Matches.pid
            }
        }
    )
    return @($listenerPids | Sort-Object -Unique)
}

function Get-ManifestProcessSnapshot {
    param([int]$ProcessId)

    $process = Get-Process -Id $ProcessId -ErrorAction SilentlyContinue
    if (-not $process) {
        return $null
    }

    $path = $null
    try { $path = $process.Path } catch { $path = $null }

    $startTime = $null
    try { $startTime = $process.StartTime.ToUniversalTime() } catch { $startTime = $null }

    return [pscustomobject]@{
        Process = $process
        Name = $process.ProcessName
        Path = $path
        StartTime = $startTime
    }
}

function Test-ProcessIdentity {
    param(
        [int]$ProcessId,
        [object]$ExpectedIdentity
    )

    $snapshot = Get-ManifestProcessSnapshot -ProcessId $ProcessId
    if ($null -eq $snapshot) {
        return $false
    }

    if ($ExpectedIdentity.process_name -and $snapshot.Name -ne [string]$ExpectedIdentity.process_name) {
        return $false
    }

    if ($ExpectedIdentity.process_path -and $snapshot.Path -and
        $snapshot.Path -ne [string]$ExpectedIdentity.process_path) {
        return $false
    }

    if ($ExpectedIdentity.process_start_time -and $snapshot.StartTime) {
        $expectedStart = if ($ExpectedIdentity.process_start_time -is [datetime]) {
            $ExpectedIdentity.process_start_time.ToUniversalTime()
        } else {
            [datetime]::Parse(
                [string]$ExpectedIdentity.process_start_time,
                [System.Globalization.CultureInfo]::InvariantCulture,
                [System.Globalization.DateTimeStyles]::RoundtripKind
            ).ToUniversalTime()
        }
        if ([math]::Abs(($snapshot.StartTime - $expectedStart).TotalSeconds) -gt 1) {
            return $false
        }
    }

    if ($ExpectedIdentity.port -and [int]$ExpectedIdentity.port -gt 0) {
        $listenerPids = @(Get-ManifestListenerPids -Port ([int]$ExpectedIdentity.port))
        if ($listenerPids -notcontains $ProcessId) {
            return $false
        }
    }

    return $true
}

function Read-ProcessManifest {
    param([string]$Path)

    if (-not (Test-Path -LiteralPath $Path)) {
        return $null
    }

    try {
        return Get-Content -Raw -LiteralPath $Path | ConvertFrom-Json
    } catch {
        throw "无法解析进程清单 ${Path}: $_"
    }
}

function Get-ManifestOwnedProcesses {
    param([object]$Manifest)

    $owned = @()
    if ($null -eq $Manifest -or $null -eq $Manifest.entries) {
        return $owned
    }

    foreach ($entry in @($Manifest.entries)) {
        foreach ($listener in @($entry.listener_processes)) {
            if ($listener.pid -and (Test-ProcessIdentity -ProcessId ([int]$listener.pid) -ExpectedIdentity $listener)) {
                $owned += [pscustomobject]@{ Name = $entry.name; Identity = $listener; Kind = "listener" }
            }
        }

        if ($entry.pid -and (Test-ProcessIdentity -ProcessId ([int]$entry.pid) -ExpectedIdentity $entry)) {
            $owned += [pscustomobject]@{ Name = $entry.name; Identity = $entry; Kind = "wrapper" }
        }
    }

    return @($owned)
}

function Stop-ManifestOwnedProcesses {
    param(
        [string]$Path,
        [switch]$Quiet
    )

    $manifest = Read-ProcessManifest -Path $Path
    if ($null -eq $manifest) {
        if (-not $Quiet) { Write-Host "没有找到托管进程清单。" -ForegroundColor Gray }
        return
    }

    $declared = @()
    foreach ($entry in @($manifest.entries)) {
        $declared += @($entry.listener_processes)
        if ($entry.pid) { $declared += $entry }
    }

    $identityMismatches = @($declared | Where-Object {
        $expectedPid = [int]$_.pid
        (Get-Process -Id $expectedPid -ErrorAction SilentlyContinue) -and
            -not (Test-ProcessIdentity -ProcessId $expectedPid -ExpectedIdentity $_)
    })
    if ($identityMismatches.Count -gt 0) {
        $mismatchPids = @($identityMismatches | ForEach-Object { $_.pid }) -join ", "
        throw "PID 身份校验不匹配 ($mismatchPids)；已保留清单，未停止任何进程。"
    }

    $owned = @(Get-ManifestOwnedProcesses -Manifest $manifest)
    foreach ($item in @($owned | Sort-Object { if ($_.Kind -eq "listener") { 0 } else { 1 } })) {
        $pidValue = [int]$item.Identity.pid
        try {
            Stop-Process -Id $pidValue -Force -ErrorAction Stop
            if (-not $Quiet) {
                Write-Host "已停止 $($item.Name) $($item.Kind) 进程 (PID: $pidValue)" -ForegroundColor Green
            }
        } catch {
            if (-not $Quiet) {
                Write-Host "停止 PID $pidValue 失败: $_" -ForegroundColor Yellow
            }
        }
    }

    if (Test-Path -LiteralPath $Path) {
        Remove-Item -LiteralPath $Path -Force
    }
}

function Test-ManifestHttpEndpoint {
    param([string]$Url)

    if ([string]::IsNullOrWhiteSpace($Url)) {
        return $false
    }

    try {
        $response = Invoke-WebRequest -Uri $Url -UseBasicParsing -TimeoutSec 3 -ErrorAction Stop
        return $response.StatusCode -ge 200 -and $response.StatusCode -lt 400
    } catch {
        return $false
    }
}
