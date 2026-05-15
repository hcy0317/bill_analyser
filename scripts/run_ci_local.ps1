param(
    [switch]$BackendOnly,
    [switch]$FrontendOnly,
    [switch]$SkipCacheTrim
)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot

function Invoke-RepoCommand {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][string]$Command,
        [string]$WorkingDirectory = $repoRoot
    )

    Write-Host "==> $Name"
    Push-Location $WorkingDirectory
    try {
        powershell -NoProfile -ExecutionPolicy Bypass -Command $Command
    } finally {
        Pop-Location
    }
}

$runBackend = -not $FrontendOnly
$runFrontend = -not $BackendOnly

if ($runBackend) {
    Invoke-RepoCommand "Rust fmt" "cargo fmt --all -- --check"
    Invoke-RepoCommand "Rust clippy" "cargo clippy --workspace --all-targets -- -D warnings"
    Invoke-RepoCommand "Rust tests" "cargo test --workspace"
    Invoke-RepoCommand "Rust coverage" "cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 90"
}

if ($runFrontend) {
    Invoke-RepoCommand "Frontend lint" "npm run lint:ci" (Join-Path $repoRoot "src\web")
    Invoke-RepoCommand "Frontend coverage" "npm run test:coverage" (Join-Path $repoRoot "src\web")
    Invoke-RepoCommand "Frontend build" "npm run build" (Join-Path $repoRoot "src\web")
}

if (-not $SkipCacheTrim) {
    & (Join-Path $PSScriptRoot "trim_ci_caches.ps1")
}
