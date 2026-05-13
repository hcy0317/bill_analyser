---
name: gitea-ci-cache-discipline
description: Maintain Bill Analyser Gitea Actions cache hygiene for Python, Rust, npm, act_runner cache growth, and coverage-gated CI cleanup.
---

# Gitea CI Cache Discipline

Use this skill when changing `.gitea/workflows/ci.yml`, `actions/cache` usage, Rust/Python/npm setup steps, coverage cleanup, or act_runner cache behavior for this repository.

This is a Bill Analyser-specific workflow. It is not a generic GitHub Actions caching tutorial.

## When to Use

Use this skill when you need to:

- adjust Gitea Actions cache paths, keys, restore/save ordering, or cache-size limits
- reduce act_runner or actcache disk growth from CI runs
- cache Rust stable setup, `llvm-tools-preview`, `cargo-llvm-cov`, pip, npm, or Cargo dependency metadata
- move cleanup relative to Rust coverage, pytest coverage, frontend coverage, or job completion
- explain whether existing cache blobs can be cleaned by workflow changes or require runner/cache storage cleanup

Do not use this skill for:

- business-code coverage policy changes outside CI workflow wiring
- local developer `.venv`, `target`, or `node_modules` cleanup unrelated to CI
- changing Gitea runner deployment, service units, or external cache storage without explicit operator scope

## Read First

Before editing anything, read these files:

- `AGENTS.md`
- `.gitea/workflows/ci.yml`
- `tests/test_gitea_workflows.py`
- `docs/AI_WORKFLOW.md` if changing entrypoint documentation
- the current `git diff`, because CI cache changes often happen while unrelated Rust or frontend work is active

## Core Cache Contract

Cleanup must happen after the artifacts that need the files have already run.

- Backend cleanup belongs after `Run Rust coverage` and `Run pytest coverage`, before cache save or job completion.
- Frontend cleanup belongs after `npm run test:coverage` and `npm run build`, before cache save or job completion.
- Do not delete coverage inputs before the corresponding coverage gate has finished.
- Keep the workflow's cleanup step names recognizable: `Trim backend caches before cache save` and `Trim frontend caches before cache save`.

Never cache generated build outputs or bulky expanded source trees:

- `target`
- `node_modules`
- `dist`
- `coverage`
- `workspace.lcov`
- `coverage.json`
- `.coverage` / `.coverage.*`
- `~/.cargo/registry/src`
- `~/.cargo/git/checkouts`

Prefer small reusable cache inputs:

- Cargo dependency metadata and archives: `~/.cargo/registry/index`, `~/.cargo/registry/cache`, `~/.cargo/git/db`
- Rust coverage binary: `~/.cargo/bin/cargo-llvm-cov`, plus Cargo install metadata files
- pip download cache: `~/.cache/pip`
- npm download cache: `~/.npm`
- Rustup toolchain cache: `~/.rustup/toolchains`, `~/.rustup/update-hashes`, `~/.rustup/settings.toml`
- Rustup bootstrap proxies: `~/.cargo/bin/rustup`, `~/.cargo/bin/cargo`, `~/.cargo/bin/rustc`, `~/.cargo/bin/rustdoc`, and related rustup proxy binaries

## Rust Toolchain and Coverage Tool

Rust toolchain caching must stay separate from Cargo dependency caching.

- Restore rustup paths before `Setup Rust stable`.
- Cache the rustup executable and proxy binaries with the rustup toolchain paths; caching only `~/.rustup/**` still lets `dtolnay/rust-toolchain` download the rustup installer every run.
- Give `dtolnay/rust-toolchain@stable` an `id`, then use its `cachekey` output when saving the rustup cache.
- Save the rustup cache only after the backend trim step has measured the cache size.
- Guard save with an output such as `rust_toolchain_cache_save=true`; if the measured rustup cache is over budget, delete the rustup paths and skip save.
- `llvm-tools-preview` lives inside the rustup toolchain cache.
- `cargo-llvm-cov` lives in Cargo's bin cache and the install step must stay idempotent with `command -v cargo-llvm-cov`.

Current repo budgets:

- Cargo cache target: 450 MB maximum after trim.
- pip cache target: 300 MB maximum after trim.
- npm cache target: 300 MB maximum after trim.
- rustup cache target: use the explicit cap in `.gitea/workflows/ci.yml` and mirror it in `tests/test_gitea_workflows.py`. If the user asks for a sub-1 GB rustup cache, set the cap to 950 MB or lower.

When cache paths or key semantics change, bump the `slim-vN` key segment so old oversized exact-key cache entries are not restored as the new baseline.

## Existing actcache Cleanup

Workflow edits only prevent future cache growth. They do not delete old cache blobs that were already saved by act_runner or the Gitea cache backend.

For an already large actcache directory:

- If the cache directory is on the current machine and the user explicitly authorizes cleanup, inspect the exact path and clean only cache-owned entries.
- If the cache lives on a remote runner, NAS, object store, or Gitea deployment host, cleanup is an operator/deployment-side action.
- If no manual cleanup is performed, rely on the cache backend TTL or retention policy.
- Do not claim that changing `.gitea/workflows/ci.yml` will shrink existing 56 GB cache storage by itself.

## Implementation Checklist

1. Inspect the current workflow and tests before editing; do not apply stale assumptions from an older branch.
2. Keep Gitea compatibility rules intact:
   - use absolute action URLs such as `https://github.com/actions/cache@v4`
   - do not add unsupported `timeout-minutes`, `continue-on-error`, top-level `concurrency`, or job `environment`
   - keep jobs independent unless Gitea support is verified
3. Add or update `tests/test_gitea_workflows.py` for every cache path, key, cleanup-order, or size-guard contract.
4. Parse the workflow YAML after editing.
5. Run the Gitea workflow tests and repo agent-stack health check.

## Verification

Minimum verification for CI cache workflow changes:

```powershell
.\.venv\Scripts\python.exe -m pytest tests/test_gitea_workflows.py -v
.\.venv\Scripts\python.exe -c "import pathlib, yaml; yaml.safe_load(pathlib.Path('.gitea/workflows/ci.yml').read_text(encoding='utf-8')); print('yaml parse ok')"
.\.venv\Scripts\python.exe scripts\agent_stack_health.py --mode repo
```

If AI workflow docs, skills, or hook-adjacent assets changed, also run the relevant agent-stack pytest set:

```powershell
.\.venv\Scripts\python.exe -m pytest tests/test_ai_workflow_docs.py tests/test_gitea_workflows.py tests/test_agent_stack_health.py -v
```

## Anti-patterns

Avoid these mistakes:

- caching `target` to speed up Rust builds while silently growing actcache by many GB
- caching `node_modules` instead of the npm download cache
- deleting backend caches before Rust or pytest coverage completes
- relying on setup-node's implicit npm cache when explicit Gitea cache behavior is needed
- using a static rustup cache key that cannot track stable toolchain changes
- caching `~/.rustup` without `~/.cargo/bin/rustup` and its proxies, which leaves `Setup Rust stable` stuck reinstalling rustup
- saving rustup cache after deleting the paths without a conditional save guard
- telling the user old actcache storage is cleaned when only future workflow behavior changed
