---
name: 'bill-analyser-repo-topology'
description: 'Bill Analyser 仓库拓扑与边界：canonical sources、source-of-truth/generated/runtime-state/temp/diagnostics/backups 分桶、测试 taxonomy、Rust backend、route/adapter 边界与 protected zones。'
applyTo: 'AGENTS.md,CLAUDE.md,README.md,Cargo.toml,Cargo.lock,*.ps1,*.bat,docs/**,scripts/**,src/backend/**,src/web/src/**,src/web/node_modules/**,tests/**,.github/**,.agents/**,.claude/**,.tmp/**,backup/**,output/**,logs/**,uploads/**,data/**'
---

# Bill Analyser Repository Topology

> This file captures Bill Analyser-specific repository topology and edit-boundary facts that generic common/typescript rules do not cover.

## Canonical Sources

- Restore repository intent from `AGENTS.md` and `docs/PROJECT_OVERVIEW.md`.
- Validate Rust backend runtime topology against `src/backend/**` and frontend runtime topology against `src/web/src/**`.
- Treat `.github/**`, `.agents/**`, and `.claude/**` as committed AI customization surfaces, not `.tmp/**` or other unpacked copies.
- Use `tests/test_repository_layout.py` and `scripts/hooks/pre_tool_repo_guard.py` as layout/governance evidence when deciding where code or artifacts should live.
- Rust runtime code belongs under `src/backend/**`. Do not reintroduce Python runtime code or top-level shadow directories such as `src/api/`, `src/core/`, `src/parsers/`, `src/utils/`, `src/data/`, or `src/uploads/`.
- Runtime log artifacts belong in root `logs/`, not `src/logs/`.

## Path Buckets

- **source-of-truth**
  - `src/backend/**`
  - `src/web/src/**`
  - `tests/**`
  - `scripts/**`
  - `docs/**`
  - `.github/**`
  - `.agents/**`
  - `.claude/**`
  - `AGENTS.md`, `CLAUDE.md`, `README.md`
  - Default promotion and long-lived edit targets.

- **generated**
  - `output/**`
  - Coverage, charts, reports, and similar generated artifacts. Prefer regenerating over hand-editing.

- **runtime-state**
  - `data/bills.db*`
  - `data/config/**`
  - `logs/**`
  - `uploads/**`
  - `data/logs/**`
  - `data/uploads/**`
  - Local runtime data and state. Do not use these paths as the source for repository rules or feature implementation.

- **temp**
  - `.tmp/**`
  - Scratch, unpacked, or experiment areas. Do not anchor stable instructions, tests, or architecture decisions here.

- **diagnostics**
  - `tests/check_*`
  - `tests/debug_*`
  - `tests/quick_*`
  - `tests/manual_test_*`
  - `tests/e2e_*`
  - `scripts/agent_stack_health.py`
  - `docs/AGENT_STACK_TESTING.md`
  - Useful for evidence and debugging, but not the sole source of truth for business behavior or repository policy.

- **backups**
  - `backup/**`
  - Historical safety copies only. Do not edit these as if they were active runtime code.

- Only `source-of-truth` paths are normal promotion targets. `generated`, `runtime-state`, `temp`, `diagnostics`, and `backups` may inform decisions, but they should not become the default edit target.

## Testing Taxonomy

- `tests/backend/**` is the Rust backend integration and contract test tree; package manifests expose these tests to `cargo test --workspace`.
- `tests/web/**` is the frontend test root; keep frontend tests there instead of under `src/web/src/**`.
- Root-level `tests/check_*`, `tests/debug_*`, `tests/quick_*`, `tests/manual_test_*`, and `tests/e2e_*` are diagnostic or historical helpers; treat them as supporting evidence, not as the final authority by themselves.
- `tests/test_repository_layout.py` is a repository topology/layout regression, not a normal business-flow spec.

## Rust Backend

- New persistence/runtime work belongs in `src/backend/db` or the owning Rust crate.
- Keep HTTP request handling in `src/backend/http`, shared business contracts in `src/backend/core`, and parser-only behavior in `src/backend/parsers`.
- Keep oversized Rust route files split by functional domain directories rather than re-growing single-file route modules.

## Route / Adapter Boundary

- New Rust-owned HTTP orchestration and request/auth handling belong in `src/backend/http/**`.
- Put Rust-owned API field-shape, time, money, account, category, and transaction conversion logic in the owning Rust HTTP/domain crate, usually `src/backend/http/**` or the corresponding `src/backend/*` crate.
- Do not recreate legacy `v1_*` adapter wrappers.
- Do not duplicate contract conversion logic across routes, services, and frontend stores. If a mapping is reused or contract-sensitive, move it to an adapter.

## Protected / No-Edit Zones

- Do not directly edit generated or protected directories such as `output/`, `backup/`, `.tmp/ecc-unpacked/`, or `src/web/node_modules/`.
- Do not treat `logs/`, `uploads/`, `data/logs/`, or `data/uploads/` as source files.
- Treat `data/bills.db*` and `data/config/` as runtime-critical state; do not delete or rewrite them for convenience.
- If a requested change appears to require editing a protected zone, change the real source file, fixture, or entrypoint instead.
- If a protected path is the only place showing the problem, use it as evidence to find the real source-of-truth file rather than patching the artifact directly.
