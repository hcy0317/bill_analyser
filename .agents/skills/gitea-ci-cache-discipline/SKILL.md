---
name: gitea-ci-cache-discipline
description: Maintain Bill Analyser Gitea Actions cache hygiene for the Rust, npm, governance, coverage, and E2E pipelines.
---

# Gitea CI Cache Discipline

Use this repository-specific skill when changing `.gitea/workflows/ci.yml`, `actions/cache` usage, Rust/npm setup, coverage cleanup, or act_runner cache behavior. Bill Analyser is Rust-only on the backend; do not restore retired Python-era gates or sidecars.

## Read First

Before editing, read:

- `AGENTS.md`
- `.gitea/workflows/ci.yml`
- `scripts/check-gitea-workflow.mjs`
- `scripts/check-rust-only-source-tree.mjs`
- `scripts/run_ci_local.ps1`
- `scripts/trim_ci_caches.ps1`
- the current `git diff`
- `docs/AI_WORKFLOW.md` only when changing documented entrypoints

## Current Runtime and Gate Contract

- Rust `bill_http_server` is the only HTTP runtime; never add a Python/sidecar fallback.
- The canonical tracked-source gate is `node scripts/check-rust-only-source-tree.mjs`.
- The canonical workflow parser and contract gate is `node scripts/check-gitea-workflow.mjs`; it resolves the lockfile-pinned `js-yaml` through `src/web/package.json`.
- The canonical local CI wrapper is `scripts/run_ci_local.ps1`.
- Active hook/config validation uses PowerShell `ConvertFrom-Json` and scans only the frozen active config set for removed `scripts/hooks/**` or `scripts/agent_stack_health.py` command references.
- Real Gitea evidence uses the existing authenticated `tea` CLI; a successful dispatch alone is not a successful run.

Do not use `.venv`, pytest, `tests/test_gitea_workflows.py`, `tests/test_local_ci_scripts.py`, global Python YAML parsing, or any other removed Python-era command.

## Cache Contract

Cleanup must happen after the artifacts that consume the files:

- Backend cleanup runs after Rust tests, full coverage, and changed-line coverage.
- Frontend cleanup runs after frontend coverage and build.
- Local cleanup stays in `scripts/trim_ci_caches.ps1` and is called by `scripts/run_ci_local.ps1` unless explicitly skipped.
- Do not delete coverage inputs before their gate finishes.
- Keep step names recognizable: `Trim backend caches before cache save` and `Trim frontend caches before cache save`.

Never cache expanded/generated outputs:

- `target`
- `node_modules`
- `dist`
- `coverage`
- `workspace.lcov`
- `coverage.json`
- `~/.cargo/registry/src`
- `~/.cargo/git/checkouts`

Prefer reusable download/tool metadata:

- Cargo: `~/.cargo/registry/index`, `~/.cargo/registry/cache`, `~/.cargo/git/db`
- Rust coverage: `~/.cargo/bin/cargo-llvm-cov` and Cargo install metadata
- npm: `~/.npm`
- rustup: toolchains, update hashes, settings, rustup executable, and proxy binaries

Current budgets:

- Cargo cache: 450 MB after trim
- npm cache: 300 MB after trim
- rustup cache: the explicit cap in `.gitea/workflows/ci.yml`; use 950 MB or lower only when a sub-1 GB cap is explicitly requested

When cache paths or key semantics change, bump the relevant `slim-vN` key segment.

## Existing actcache Cleanup

Workflow changes prevent future growth; they do not delete existing cache blobs. Clean an existing cache store only when the user explicitly authorizes the exact local cache-owned path. Remote runner, NAS, object-store, or Gitea-hosted cleanup is an operator action. Otherwise rely on configured retention/TTL and do not claim old storage was reclaimed.

## Implementation Checklist

1. Inspect the current workflow, scripts, and diff; do not apply an older Python-era assumption.
2. Preserve Gitea compatibility: absolute action URLs, no unsupported `timeout-minutes`, `continue-on-error`, top-level `concurrency`, or job `environment`.
3. Update `scripts/check-gitea-workflow.mjs` whenever required steps, order, cache paths, keys, or cleanup semantics change.
4. Keep immutable diff resolution, Rust/frontend changed-line coverage, route ownership, Rust-only source, and deterministic E2E steps merge-blocking.
5. Parse and self-test the workflow through the locked Node checker.
6. Validate active JSON files and the narrow removed-hook residual scan when `.agents/**`, `.github/**`, `.claude/**`, `.codex/**`, or hook adapters change.

## Verification

From the repository root:

```powershell
node scripts/check-rust-only-source-tree.mjs --self-test
node scripts/check-rust-only-source-tree.mjs
node scripts/check-gitea-workflow.mjs --self-test
node scripts/check-gitea-workflow.mjs
node scripts/check-governance-normalizers.mjs
node scripts/resolve-ci-diff-refs.mjs --self-test
```

Validate active configuration existence and JSON syntax:

```powershell
$explicit = @('.codex/hooks.json','.codex/config.toml','opencode.json','.opencode/package.json')
$missing = @($explicit | Where-Object { -not (Test-Path -LiteralPath $_ -PathType Leaf) })
if ($missing.Count) { $missing; exit 1 }
$json = @('.codex/hooks.json','opencode.json','.opencode/package.json') + @(
    Get-ChildItem -Path '.claude/settings*.json','.github/hooks/*.json' -File | ForEach-Object FullName
)
foreach ($path in $json) {
    Get-Content -LiteralPath $path -Raw | ConvertFrom-Json | Out-Null
}
```

Scan only active hook/config surfaces; `.opencode/package.json` is parsed above but deliberately excluded from the residual scan:

```powershell
$active = @('.codex/hooks.json','.codex/config.toml','opencode.json') + @(
    Get-ChildItem -Path '.claude/settings*.json','.github/hooks/*.json' -File | ForEach-Object FullName
)
$hits = & rg -n 'scripts[/\\]hooks|scripts[/\\]agent_stack_health\.py' -- $active
if ($LASTEXITCODE -eq 0) { $hits; exit 1 }
if ($LASTEXITCODE -ne 1) { exit $LASTEXITCODE }
```

For real Gitea evidence, push the target branch and use explicit repository arguments:

```powershell
tea actions workflows list --repo hcy0317/bill_analyser
tea actions workflows dispatch ci.yml --ref <remote-branch> --repo hcy0317/bill_analyser
tea actions runs list --repo hcy0317/bill_analyser
tea actions runs view <run-id> --jobs --repo hcy0317/bill_analyser
tea actions runs logs <run-id> --job <job-id> --repo hcy0317/bill_analyser
tea api /repos/hcy0317/bill_analyser/actions/runs/<run-id>/jobs
```

Accept the run only when its head SHA equals the pushed branch HEAD and every required job/step exists with `conclusion=success`.

## Anti-patterns

- caching `target` or `node_modules`
- deleting coverage artifacts before coverage gates complete
- restoring pytest, pip cache, Python YAML, or `.venv` commands to this Rust-only repository
- scanning the whole `.github`, `.claude`, or `.opencode` documentation tree for active hook references
- treating Gitea dispatch exit 0 as run success
- claiming workflow edits reclaimed existing remote actcache storage
- caching rustup toolchains without the rustup executable and proxy binaries
- saving an over-budget rustup cache after its paths were removed
