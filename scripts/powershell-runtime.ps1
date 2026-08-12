function Get-BillAnalyserPowerShell7Path {
    $currentExecutable = $null
    try {
        $currentExecutable = [System.Diagnostics.Process]::GetCurrentProcess().MainModule.FileName
    } catch {
        $currentExecutable = $null
    }

    if (
        -not [string]::IsNullOrWhiteSpace($currentExecutable) -and
        [System.IO.Path]::GetFileName($currentExecutable).Equals(
            "pwsh.exe",
            [System.StringComparison]::OrdinalIgnoreCase
        ) -and
        (Test-Path -LiteralPath $currentExecutable -PathType Leaf)
    ) {
        return $currentExecutable
    }

    $installRoots = @($env:ProgramFiles, $env:ProgramW6432) |
        Where-Object { -not [string]::IsNullOrWhiteSpace($_) } |
        Select-Object -Unique
    foreach ($installRoot in $installRoots) {
        $installedPwsh = Join-Path $installRoot "PowerShell\7\pwsh.exe"
        if (Test-Path -LiteralPath $installedPwsh -PathType Leaf) {
            return $installedPwsh
        }
    }

    $pwshCommand = Get-Command pwsh -CommandType Application -ErrorAction SilentlyContinue |
        Select-Object -First 1
    if ($pwshCommand) {
        return $pwshCommand.Source
    }

    return $null
}
