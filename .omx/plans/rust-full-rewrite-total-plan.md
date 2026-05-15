# Rust Full Rewrite Total Plan

## Objective

Complete the Bill Analyser backend rewrite by making Rust the sole runtime for all business APIs, deleting the old sidecar/runtime sources, and aligning startup, CI, docs, frontend route contracts, and governance checks with the Rust-only state.

## Current Runtime

- HTTP entry: `crates/bill-analyser-http/src/bin/bill_http_server.rs`
- API chain: `REST /api/...`
- Database: SQLite via Rust repositories and transaction helpers
- Frontend: `src/web`
- Unknown API route behavior: Rust structured 404

## Exit Gates

- `git ls-files '*.py'` returns no tracked backend/tooling source.
- Removed interpreter/runtime metadata files are not tracked.
- Startup scripts launch only the Rust backend and frontend.
- CI has Rust, frontend, and repo-governance jobs only.
- Route ownership contains no legacy fallback routing state.
- Frontend generated route fixture is up to date.
- Local verification passes:
  - `cargo fmt --all -- --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`
  - `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 90`
  - `cd src/web && npm run lint`
  - `cd src/web && npm run test:coverage`
  - `cd src/web && npm run build`

## Slice Status

| Slice | Status | Evidence |
|---|---|---|
| P0-P12 | completed | Business domains migrated into Rust runtime and verified in earlier PRs. |
| P13 | completed | Residual tooling, hook prompts, inventory, and agent docs aligned to Rust-primary workflow. |
| P14 | completed | Frontend route ownership fixture and contract guard merged in PR #102. |
| P15 final cutover | completed | Python runtime/tooling source deleted; legacy fallback routing removed; startup/CI/docs aligned to Rust-only. Local gates passed: `.\scripts\run_ci_local.ps1 -SkipCacheTrim`, `node src/web/scripts/generate-rust-route-fixture.mjs --check`, tracked Python/metadata scan clean, targeted legacy-runtime residue scan clean, `git diff --check`. |
