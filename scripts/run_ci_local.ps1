param(
    [switch]$BackendOnly,
    [switch]$FrontendOnly,
    [switch]$SkipCacheTrim,
    [switch]$SelfTest,
    [string]$DiffBase = "main",
    [string]$DiffHead = "HEAD"
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
        $global:LASTEXITCODE = 0
        $commandBlock = [scriptblock]::Create($Command)
        & $commandBlock
        $commandSucceeded = $?
        $childExitCode = $LASTEXITCODE
        if (-not $commandSucceeded -or $childExitCode -ne 0) {
            throw "$Name failed with exit code $childExitCode"
        }
    } finally {
        Pop-Location
    }
}

if ($SelfTest) {
    $failureObserved = $false
    try {
        Invoke-RepoCommand "Expected native failure" "node -e `"process.exit(23)`""
    } catch {
        if ($_.Exception.Message -notmatch "exit code 23") {
            throw
        }
        $failureObserved = $true
    }
    if (-not $failureObserved) {
        throw "Local CI wrapper accepted a failing native command"
    }
    Write-Host "PASS local CI fail-closed self-test"
    exit 0
}

$runBackend = -not $FrontendOnly
$runFrontend = -not $BackendOnly

Invoke-RepoCommand "Immutable CI diff resolver self-test" "node scripts/resolve-ci-diff-refs.mjs --self-test"
Invoke-RepoCommand "E2E scope classifier" "node scripts/check-e2e-scope.mjs"
Invoke-RepoCommand "E2E outcome verifier" "node scripts/check-e2e-outcome.mjs"
Invoke-RepoCommand "Governance normalizer self-test" "node scripts/check-governance-normalizers.mjs"
Invoke-RepoCommand "Rust-only source tree" "node scripts/check-rust-only-source-tree.mjs"
Invoke-RepoCommand "Gitea workflow checker self-test" "node scripts/check-gitea-workflow.mjs --self-test"
Invoke-RepoCommand "Gitea workflow contract" "node scripts/check-gitea-workflow.mjs"
Invoke-RepoCommand "Resolve local immutable diff refs" "node scripts/resolve-ci-diff-refs.mjs resolve --local-base `"$DiffBase`" --local-head `"$DiffHead`" --out .omx/ultragoal/evidence/ci-diff-refs.json"

if ($runBackend) {
    Invoke-RepoCommand "Rust backend structure" "node scripts/check-rust-backend-structure.mjs"
    Invoke-RepoCommand "Rust fmt" "cargo fmt --all -- --check"
    Invoke-RepoCommand "Rust clippy" "cargo clippy --workspace --all-targets -- -D warnings"
    Invoke-RepoCommand "Assembled runtime route ownership" "cargo test -p bill-analyser-http --test runtime_route_ownership_contract -- --nocapture"
    Invoke-RepoCommand "Rust workspace tests with coverage" "cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 35"
    Invoke-RepoCommand "Rust changed-line coverage" '$mergeBase = node scripts/resolve-ci-diff-refs.mjs read --input .omx/ultragoal/evidence/ci-diff-refs.json --field merge_base_sha; $head = node scripts/resolve-ci-diff-refs.mjs read --input .omx/ultragoal/evidence/ci-diff-refs.json --field head_sha; git -c gc.auto=0 diff --quiet "${mergeBase}...${head}" -- src/backend; $rustBusinessDiffExit = $LASTEXITCODE; if ($rustBusinessDiffExit -eq 0) { Write-Host "Rust business source unchanged; changed-line coverage not applicable." } elseif ($rustBusinessDiffExit -eq 1) { git -c gc.auto=0 diff --output=.omx/ultragoal/evidence/rust-changed.diff --unified=0 "${mergeBase}...${head}" -- src/backend tests/backend; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }; node scripts/governance-normalizers.mjs changed-coverage --lcov workspace.lcov --diff .omx/ultragoal/evidence/rust-changed.diff --threshold 90 --require-matched-files --require-executable-lines --allow-no-business-files } else { exit $rustBusinessDiffExit }'
}

if ($runFrontend) {
    Invoke-RepoCommand "Frontend structure" "npm --prefix src/web run structure:check"
    Invoke-RepoCommand "Frontend lint" "npm run lint:ci" (Join-Path $repoRoot "src\web")
    Invoke-RepoCommand "Frontend coverage" "npm run test:coverage" (Join-Path $repoRoot "src\web")
    Invoke-RepoCommand "Frontend changed-line coverage" '$mergeBase = node scripts/resolve-ci-diff-refs.mjs read --input .omx/ultragoal/evidence/ci-diff-refs.json --field merge_base_sha; $head = node scripts/resolve-ci-diff-refs.mjs read --input .omx/ultragoal/evidence/ci-diff-refs.json --field head_sha; git -c gc.auto=0 diff --quiet "${mergeBase}...${head}" -- src/web/src; $frontendBusinessDiffExit = $LASTEXITCODE; if ($frontendBusinessDiffExit -eq 0) { Write-Host "Frontend business source unchanged; changed-line coverage not applicable." } elseif ($frontendBusinessDiffExit -eq 1) { git -c gc.auto=0 diff --output=.omx/ultragoal/evidence/frontend-changed.diff --unified=0 "${mergeBase}...${head}" -- src/web tests/web; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }; node scripts/governance-normalizers.mjs changed-coverage --lcov src/web/coverage/lcov.info --diff .omx/ultragoal/evidence/frontend-changed.diff --threshold 90 --require-matched-files --require-executable-lines --allow-no-business-files } else { exit $frontendBusinessDiffExit }'
    Invoke-RepoCommand "Frontend build" "npm run build" (Join-Path $repoRoot "src\web")
}

if (-not $SkipCacheTrim) {
    & (Join-Path $PSScriptRoot "trim_ci_caches.ps1")
}
