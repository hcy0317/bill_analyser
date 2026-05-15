# Bill Analyser Rust-only 完全重构总分 Plan

## 0. Ralplan Consensus

本计划来自 `oh-my-codex:ralplan` 共识流，需求源锁定为 `.omx/specs/deep-interview-rust-full-rewrite.md`。执行策略不是大爆炸删除，也不是新增长期兼容层，而是按功能域做直接 Rust 替换：每个域先达到 Rust 真实实现、验证所有权、迁移测试与覆盖率，再删除对应 Python，最后做 Rust-only 终态切换。

当前事实基线：

- `scripts/rust_migration_inventory.py --json` 当前报告：`python_backend_files=321`、`rust_backend_files=91`、`migration_domains=22`、`port_files=253`、`facade_files=68`、`verified_dead_files=0`。
- 仓库 `.py` 分布：`src=320`、`tests=246`、`scripts=16`、`main.py=1`。
- 当前 route ownership 仍包含 `27` 个 `PythonProxied` endpoint。
- `docs/PROJECT_OVERVIEW.md` 说明 Rust 是主 HTTP 入口，但 `start_backend.ps1` 仍启动 Python/Flask sidecar，并通过 `BILL_ANALYSER_PYTHON_UPSTREAM` 反代未迁移域。
- `crates/bill-analyser-http/src/router.rs` 仍有 fallback proxy，`crates/bill-analyser-http/src/proxy.rs` 仍转发到 `python_upstream`。

这些事实决定了：不能直接删除 Python；必须先用机器可检查的所有权、契约、覆盖率和无残留引用证据逐域推进。

## 1. Requirements Summary

- 终态 Rust Axum HTTP server 是唯一后端进程。
- 仓库终态无 tracked `.py` 源码、Python 测试、Python 脚本、Python CI、Python package metadata。
- 不允许 Python sidecar、proxy fallback、embedded subprocess、generated-code dependency 或新兼容层。
- 保留现有 Vue 前端流程与 REST `/api/...` 契约，不恢复 `/api/v1/*`。
- 保留 SQLite 现有数据兼容、WAL/FK/rollback/user-scope、金额元/分与时间语义。
- 每个功能域独立 branch/commit/push/PR，域内 gate 通过后才能删除对应 Python。
- route ownership 终态无 `PythonProxied`，无 fallback proxy path。
- 覆盖率总量与改动业务代码均保持 `>90%`。
- `docs/PROJECT_OVERVIEW.md`、README、CI、AGENTS/adapters 最终描述 Rust-only 当前状态。

## 2. RALPLAN-DR Summary

### Principles

1. Python deletion is evidence-driven, never route-registration-driven.
2. Each business domain owns its tests, coverage, parity fixtures, and deletion proof in the same phase.
3. Rust crate boundaries prevent reverse dependencies and future compatibility shells.
4. Provider/reporting behavior is preserved unless the user explicitly approves a blocked exception.
5. Final "no Python" is a terminal cutover proof, not an intermediate phase claim.

### Decision Drivers

1. Current state is not deletion-ready: Python-proxied behavior and zero verified-dead Python files remain.
2. User constraint is strict terminal Rust-only with no new compatibility layer.
3. High-risk behavior clusters around import/parser/dedup/auth/provider/reporting/backup and needs fixtures plus DB smoke evidence.

### Options

| Option | Verdict | Steelman | Rejection / choice rationale |
| --- | --- | --- | --- |
| Big-bang rewrite and delete | Rejected | One terminal branch avoids prolonged mixed ownership and duplicated DTOs. | Current Python ownership is too broad; failures would be unreviewable and hard to localize. |
| Domain cutover state machine | Chosen | Keeps PRs reviewable, lets each domain prove parity before deletion, and avoids new compatibility layers. | Only option that satisfies strict Rust-only terminal state and safe staged deletion. |
| Long-lived proxy/compat layer | Rejected | Lowers short-term migration pressure and can reduce immediate regression risk. | Violates deep-interview non-goals and hides ownership drift behind fallback. |

## 3. Core Rules

### Domain Cutover State Machine

Every endpoint/domain must move strictly:

`PythonProxied -> RustImplemented -> RustOwnedVerified -> PythonDeleted`

Definitions:

- `PythonProxied`: Python remains runtime authority; Rust may proxy.
- `RustImplemented`: real Rust handler/service exists and passes focused contract tests, but Python is not deletable.
- `RustOwnedVerified`: real Rust handler is wired; no domain route falls back to proxy; no stub/skeleton response is used; parity/golden tests pass; DB invariants pass where relevant; migrated regression tests exist; coverage gates pass; frontend contract evidence exists when frontend behavior is touched.
- `PythonDeleted`: tracked Python files/references for that domain are removed and regression gates still pass.

Hard rule: planned routes, routable stubs, skeleton handlers, `not_yet_owned_response`, and manifest-only entries are `ContractOnly/Planned`, not ownership. They cannot satisfy `RustImplemented` or `RustOwnedVerified`.

### P0 Manifest Schema

P0 must introduce a machine-checkable manifest with at least:

```json
{
  "domain": "bills-import",
  "endpoint": "POST /api/bills/import/v2/parse",
  "state": "PythonProxied|RustImplemented|RustOwnedVerified|PythonDeleted|ContractOnly|Planned",
  "handler": "crates/bill-analyser-http/src/import_routes.rs::...",
  "python_owner_files": ["src/bill_analyser/..."],
  "rust_owner_files": ["crates/..."],
  "tests_migrated": ["crates/.../tests/..."],
  "fixtures": ["tests/fixtures/..."],
  "db_invariant_ids": ["wal_mode", "foreign_keys", "rollback_on_error", "positive_user_scope", "amount_units"],
  "coverage_evidence": "workspace.lcov",
  "deletion_blockers": ["frontend_import_flow", "provider_parity"],
  "blocked_status": "none|blocked:user_choice_required",
  "unsupported_behavior": "exact provider/report/export behavior that cannot yet be ported",
  "decision_required": "port|remove|defer",
  "decision_owner": "user",
  "transition_evidence": ["route_matrix", "golden_fixture", "db_smoke", "frontend_contract"]
}
```

### Crate Dependency Rules

Target crate graph:

- `bill-analyser-http -> core/db/parsers/providers/reporting`
- `bill-analyser-db -> core`
- `bill-analyser-parsers`: pure parsing; no `http`, `db`, `providers`, `reporting`, or `tools`
- `bill-analyser-core`: no dependency on `http`, `db`, `providers`, `reporting`, or `tools`
- `bill-analyser-tools`: may depend on runtime crates
- Runtime crates must not depend on `bill-analyser-tools`

P0 must add a dependency graph gate using `cargo metadata` or `cargo tree` plus a denylist enforcing these layering rules.

### Provider/Reporting Exception Rule

Unsupported provider/report/export behavior cannot be silently removed. For each provider/report surface, the domain PR must either:

- port behavior with parity evidence, or
- mark the domain `blocked:user_choice_required` with exact unsupported behavior and no deletion.

## 4. Verification Gates

### Business-Domain PR Gate

Every business-domain PR must pass:

- State transition proof: no skipped state; no stub counted as ownership.
- Commit shaping proof: when a domain slice has separable rule, implementation, test, or docs steps, prefer multiple small commits in one PR instead of one large commit.
- Commit title proof: use a Chinese Conventional Commit subject that stays on one line, such as `feat(auth): 接管外部登录绑定`; when useful, put subtopics in newline body bullets instead of dash-appending them to the subject.
- PR title proof: use a Conventional Commit heading such as `feat(auth): 接管账号恢复路由` and title the PR by functional-domain outcome, not by blindly copying the first commit title.
- PR merge proof: prefer squash / compressed merge unless the user or repository policy explicitly requires a different merge mode.
- Branch cleanup proof: after the PR merges, delete the source branch through forge auto-delete when available, otherwise delete the remote branch after merge is confirmed.
- Co-author trailer proof: hooks, gates, and commit templates must not require or automatically append `Co-authored-by: OmX <omx@oh-my-codex.dev>` unless the user explicitly asks for it.
- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- focused `cargo test` for touched crates/domain
- `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 90`
- changed/domain Rust executable line coverage `>90%` from `workspace.lcov`
- route ownership matrix updated
- golden fixtures for response envelopes, status codes, edge cases, and frontend expectations
- DB invariants where relevant: WAL, FK, rollback, user scope, amount units, time normalization
- Python tests for that domain are ported in the same domain phase before deletion
- if `src/web/**` is touched: `npm run lint` and `npm run test:coverage`
- provider/reporting parity-or-blocked evidence where relevant
- `docs/PROJECT_OVERVIEW.md` updated only when stable behavior/contracts change

### Final Cutover Gate

Only P15 may require:

- `git ls-files '*.py'` returns empty
- no `BILL_ANALYSER_PYTHON_UPSTREAM`, `PythonProxied`, Flask runtime, pytest, pyproject, or Python startup/tooling references
- no fallback proxy path and no `proxy.rs`
- Rust-only fresh clone startup works
- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 90`
- `Set-Location src/web; npm run lint; npm run test:coverage; npm run build`

## 5. Phase Table

| Phase | Scope | Work | Exit / PR Gate |
| --- | --- | --- | --- |
| P0 Governance/contracts only | Ownership, inventory, deletion contracts | Add strict state machine, route/domain manifest, minimal Rust inventory/route manifest, crate dependency policy, dependency graph gate, coverage evidence contract, fixture ledger | Branch `codex/rust-rewrite/governance-contracts`; no runtime domain work; no broad hook/CI rewrite |
| P1 HTTP runtime shell | Axum ownership enforcement | Expand router modules without claiming ownership; classify planned/stub routes; proxy negative tests; no fallback for verified domains | HTTP shell can express all states; proxy remains only for `PythonProxied` |
| P2 DB/schema/repositories | SQLite lifecycle | Port migrations/repositories; user scope, rollback, WAL/FK, amount/time contracts; migrate DB Python tests here | DB domains reach `RustOwnedVerified` before deleting DB Python facade |
| P3 Auth/security/user data | Login/session/2FA/profile/tokens/export-clear | Port password/JWT/session/2FA/recovery/profile/OAuth/user data; migrate auth Python tests | Auth routes reach `PythonDeleted` only after OAuth/external-auth surfaces are real Rust implementations with parity tests; any unported OAuth surface stays `ContractOnly/Planned` or `blocked:user_choice_required` and blocks deletion |
| P4 Taxonomy/rules/settings | Accounts/categories/tags/templates/category rules/settings bundle | Port CRUD, seeds, import/export, rule CRUD/reorder/test/matcher, settings bundle; migrate tests | Rule center and import preview use same Rust taxonomy engine |
| P5 Bills core/export/media/reconciliation | Non-import bill operations | Complete export, pictures, reconciliation, recurring helpers, category quick actions; migrate tests | `/api/bills/*` non-import routes no proxy fallback |
| P6 Import parsers | Parser stack | New pure parser crate; port WeChat/Alipay/banks/generic CSV/XLS/XLSX/HTML/encoding; migrate parser fixtures/tests | Parser golden outputs match current Python behavior |
| P7 Import pipeline/dedup/preview/learning | Highest-risk import domain | Port v2 parse/dedup/confirm, preview paging/update/reclassify, smart dedup order, learning promotion/apply/rollback; migrate tests | Five import deletion gates pass in the same phase |
| P8 Matching/recurring/calendar/networth | Pairing and recurrence | Port candidates/manual pairs/accept-reject-clear/feedback, recurring suggestions, calendar, networth; migrate tests | Import preview and formal bills share Rust matching engine |
| P9 Budgets | Mostly Rust-owned cleanup | Verify CRUD/export/execution/forecast/history/import; migrate remaining budget tests; delete Python remnants | Budget Python deleted; parent/child rollup and forecast behavior covered |
| P10 Statistics/exchange/reporting | Analyzer/providers/reports | Port overview/trends/comparison/category/trend, exchange providers/custom writes, CSV/XLSX/HTML/PDF reports; migrate tests | Reports/providers ported or blocked for user choice |
| P11 OCR/LLM/learning center | AI providers and global learning | Port OCR recognition, LLM providers, preview recommend/analyze/rule synthesis, learning suggestions/rules; migrate tests | Secret redaction, provider errors, rate limits covered |
| P12 Backup/ops/sync/runtime config | Operational runtime | Port backup/restore/download/jobs, encryption status/migration behavior, sync providers, config/logging; migrate tests | Runtime ops have no Python dependency |
| P13 Residual tooling/hooks/CI/global cleanup | Non-business repo surface only | Rewrite remaining hooks/scripts/structure gates/CI glue after business tests moved; remove pytest/pylint jobs only when equivalent gates exist | No business test migration belongs here; `git ls-files '*.py'` is not an exit gate |
| P14 Frontend integrated contract verification | Vue contract proof | Audit `services.ts`/stores, generated Rust fixtures, run full flows against Rust-only backend candidate | `npm run lint`, `npm run test:coverage`, build verification pass |
| P15 Final cutover/deletion | Terminal Rust-only repo | Delete remaining Python source/tests/scripts/config, remove proxy, update startup/docs/adapters | `git ls-files '*.py'` empty; fresh clone starts without Python |

## 6. PLAN_DAG

Shared root-owned paths for all waves: `Cargo.toml`, crate `Cargo.toml` files, ownership manifests, CI/workflow files, startup scripts, `docs/PROJECT_OVERVIEW.md`.

| Node | Depends on | Parallel group | Owned paths | Main gates |
| --- | --- | --- | --- | --- |
| P0 | none | wave-0 | governance contracts, migration manifest, minimal tools | state machine, dependency guard, route/inventory manifest |
| P1 | P0 | wave-1 | `crates/bill-analyser-http/**` | no stub ownership, proxy boundary tests |
| P2 | P0 | wave-1 | `crates/bill-analyser-db/**`, DB contracts | DB invariants, migrated DB tests |
| P6 | P0 | wave-1 | `crates/bill-analyser-parsers/**` | pure crate rule, parser fixtures |
| P3 | P1,P2 | wave-2 | auth/security routes/core/db | auth coverage, frontend if touched |
| P4 | P1,P2 | wave-2 | taxonomy/rules/settings | matcher/rule parity |
| P5 | P1,P2 | wave-2 | bills/export/media/reconciliation | upload/download security |
| P9 | P1,P2 | wave-2 | budgets | budget coverage and deletion |
| P10 | P1,P2,P5,P9 | wave-3 | stats/exchange/reporting | provider/report exception gate |
| P7 | P2,P4,P5,P6 | wave-4 | import/dedup/preview/learning | five import deletion gates |
| P11 | P1,P2,P4,P7 | wave-4 | OCR/LLM/learning | provider parity/redaction |
| P8 | P4,P5,P7 | wave-5 | matching/recurring/networth | shared matching engine |
| P12 | P2,P10 | wave-5 | backup/sync/config/ops | restore/encryption/sync gates |
| P13 | P3-P12 | wave-6 | hooks/scripts/CI/tooling only | no business migration work |
| P14 | P3-P12 | wave-6 | `src/web/**`, contract fixtures | frontend coverage/build |
| P15 | P13,P14, all domains `PythonDeleted` | wave-7 | final deletion/docs/startup | no tracked Python, no proxy |

## 7. Expanded Test Plan

| Layer | Required evidence |
| --- | --- |
| Unit | Pure Rust domain tests for adapters, parsers, rules, matching, budgets, statistics, providers, report builders |
| Integration | Axum route tests, SQLite migration/repository tests, auth/session tests, import staging/confirm tests |
| E2E | Rust-only backend plus Vue critical flows: login, import check-data, bills CRUD, budgets, statistics, settings bundle, OCR/LLM where configured |
| Fixture/golden | Parser samples, response envelopes, report outputs, provider error mapping, legacy frontend DTOs |
| Migration | Existing SQLite DB copy opens/migrates; rollback on failure; user scope and amount/time semantics preserved |
| Provider-failure | Disabled provider, rate limit, timeout, invalid response, redaction, missing credentials |
| Observability | Request IDs, structured errors, startup health/runtime metadata, backup/restore audit output |

High-risk domains P3/P7/P10/P11/P12 require the full matrix entries relevant to their surface before deletion.

## 8. ADR

### Decision

Adopt a domain cutover program with strict state transitions and per-domain PRs. P0 is the first execution slice and is limited to governance/contracts plus minimal machine-checkable inventory/manifest gates.

### Drivers

- Existing repo still has Python-proxied behavior and Python tests/scripts.
- Rust ownership must be proven by behavior, DB writes, frontend contracts, and coverage.
- The final state requires no tracked Python, but that proof belongs only to terminal cutover.

### Alternatives Considered

- Big-bang rewrite: rejected for parity, review, data safety, and rollback risk.
- Keep proxy until final deletion: rejected because it lets ownership drift and conflicts with no-compat terminal intent.
- Move all test migration to P13: rejected because it hides business regressions until too late.

### Consequences

- More PRs, but each PR is reviewable and reversible.
- Some domains may block on explicit user choice for unsupported providers/reports.
- Tooling cleanup is delayed until business domains own their tests.

### Follow-ups

- Execute P0 only first after plan approval.
- Do not start P1-P15 until P0 manifests and gates exist.
- Treat provider/reporting unsupported behavior as a user decision blocker, not as implementation discretion.

## 9. Staffing And Launch Guidance

Available agent types:

`planner`, `architect`, `explorer`, `database-reviewer`, `security-reviewer`, `performance-analyst`, `frontend-engineer`, `tdd-guide`, `code-reviewer`, `build-fixer`, `worker`, `doc-writer`, `repo-workflow-extractor`, `harness-optimizer`.

### Ralph Path

Use `$ralph` for tight sequential control, especially P0/P2/P3/P7/P13/P15:

- `explorer` verifies current source ownership.
- `tdd-guide` or `worker` implements the slice.
- `database-reviewer`, `security-reviewer`, or `frontend-engineer` reviews domain-specific risk.
- `code-reviewer` performs terminal review.
- `build-fixer` only after concrete build/test failures.

Suggested launch after approval:

`$ralph --deliberate ".omx/plans/rust-full-rewrite-total-plan.md P0 only: governance/contracts, no business domain implementation"`

### Reasoning By Lane

| Lane | Suggested reasoning | Notes |
| --- | --- | --- |
| Coordinator / planner | xhigh | Owns source-lock, DAG ordering, shared manifests, user-choice blockers, and final gate decisions |
| Implementation worker | high | Uses focused owned paths; does not edit shared manifests unless assigned by coordinator |
| DB reviewer | xhigh | Required for P2/P7/P10/P12 schema, migration, rollback, amount/time, and user-scope gates |
| Security reviewer | xhigh | Required for P3/P11/P12 auth, secrets, provider, backup, encryption, and data-clear flows |
| Performance analyst | high | Use for import/dedup/statistics/reporting regressions and parser throughput gates |
| Frontend engineer | high | Required for P14 and any `src/web/**` contract-impacting domain PR |
| Build fixer | high | Invoked only for concrete cargo/npm/coverage failures; fixes minimal failing surface |
| Doc/update lane | medium | Updates current-state docs after behavior/contract changes; no implementation ownership |

### Team Path

Use `$team` only after P0 manifests exist:

- P0: 2-3 agents max: worker + explorer + reviewer.
- Domain waves: 4-5 agents max with disjoint owned paths.
- Shared/root-owned paths stay with the coordinator: ownership manifests, `Cargo.toml`, CI, startup scripts, `docs/PROJECT_OVERVIEW.md`, final deletion.

Suggested launch after P0 approval:

`omx team 3 "Execute P0 governance/contracts from .omx/plans/rust-full-rewrite-total-plan.md only; do not implement business domains"`

Team verification path:

- Workers prove their domain transition state.
- Root coordinator verifies manifest consistency, dependency graph, coverage artifacts, and shared docs.
- Ralph or code-reviewer performs final cross-domain review before each wave closes.

## 10. Pre-Mortem

| Failure | Cause | Mitigation |
| --- | --- | --- |
| Python deleted while Rust route is only a stub | Stub counted as ownership | State machine forbids stub/planned ownership; deletion requires `RustOwnedVerified` |
| Coverage looks green but changed domain is untested | Only workspace coverage checked | Require workspace `llvm-cov` plus changed/domain `>90%` |
| Provider/report behavior disappears | Unsupported provider silently omitted | Provider/report exception gate blocks domain for explicit user choice |
| Crate cycles grow during migration | Runtime crates depend on tools or core depends upward | P0 dependency guard and root-owned manifest review |
| P13 becomes a dumping ground | Business tests deferred | Every domain phase owns its own test migration; P13 limited to residual tooling/CI/hooks |
| Final no-Python claim is premature | Intermediate phase uses `git ls-files '*.py'` | Only P15 uses the empty tracked-Python terminal gate |
| Cross-domain DTO drift breaks frontend | Parallel workers change envelopes inconsistently | P0 fixture ledger plus P14 frontend integrated contract verification |

## 11. First Execution Slice

First branch: `codex/rust-rewrite/governance-contracts`.

Scope:

- Add the cutover state machine to the Rust migration governance surface.
- Add the P0 manifest schema and route/domain manifest.
- Add minimal Rust inventory/route manifest tooling sufficient to make deletion gates machine-checkable.
- Add crate dependency policy and dependency-graph check.
- Add fixture ledger and coverage evidence contract.

Explicit non-scope:

- No runtime domain replacement.
- No broad hook/CI rewrite.
- No Python deletion except generated/unused P0-only files if verified irrelevant.
- No `git ls-files '*.py'` terminal gate.

P0 done when:

- The manifest can represent all current `PythonProxied`, `RustImplemented`, `RustOwnedVerified`, `PythonDeleted`, `ContractOnly`, and `Planned` states.
- Existing inventory counts are reproduced or intentionally explained.
- Dependency layering can be checked mechanically.
- Later domain PRs have a concrete evidence schema to fill.

## 12. Applied Consensus Changes

- Added strict `PythonProxied -> RustImplemented -> RustOwnedVerified -> PythonDeleted` state machine.
- Made stubs/planned routes non-ownership by definition.
- Moved test migration into P2-P12 domain phases; P13 is residual tooling/hooks/CI only.
- Added workspace and changed/domain Rust coverage gates.
- Added positive `RustOwnedVerified` definition.
- Added P0 manifest schema.
- Added crate dependency graph rules and mechanical dependency check.
- Added provider/reporting parity-or-blocked rule.
- Added blocked exception manifest fields for unsupported provider/reporting behavior.
- Clarified P3 OAuth/external-auth stubs cannot satisfy Python deletion.
- Added reasoning-by-lane guidance required for execution handoff.
- Narrowed first execution slice to P0 governance/contracts only.

## 13. Execution Progress Ledger

Last updated: 2026-05-15T15:19:45+08:00.

| Phase | Status | Evidence | Gate result |
| --- | --- | --- | --- |
| P0 | completed | `crates/bill-analyser-core/src/migration_governance.rs`; `crates/bill-analyser-core/tests/migration_governance_contracts.rs`; `scripts/check_rust_workspace_dependencies.py` | Governance state machine and dependency policy exist. |
| P1 | completed | `crates/bill-analyser-http/src/router.rs`; `crates/bill-analyser-http/src/proxy.rs`; `crates/bill-analyser-http/tests/proxy_contract.rs`; `crates/bill-analyser-http/tests/import_skeleton_contract.rs` | HTTP shell and proxy boundary contracts exist. |
| P2 | completed | `crates/bill-analyser-db/src/schema.rs`; `crates/bill-analyser-db/src/connection.rs`; `crates/bill-analyser-db/src/user_scope.rs`; `crates/bill-analyser-db/tests/sqlite_runtime.rs` | Foundational DB/schema/user-scope contracts exist. |
| P6 | completed | PR #70; merge `d8d00c557d8707e1f845bcc8eddc57779989ba5f`; `crates/bill-analyser-parsers/src/lib.rs`; `crates/bill-analyser-parsers/tests/parser_contracts.rs` | Pure parser contract crate, parser golden contracts, workspace dependency policy, migration inventory, and architecture docs are merged. |
| P3 | completed | PR #71; merge `b97c5e925fb0918877678761f4ecbea2a5b1bfd1`; `crates/bill-analyser-http/tests/auth_runtime_contract.rs`; `crates/bill-analyser-core/tests/auth_security_contracts.rs`; `scripts/omx_plan_progress.py audit-early` | OAuth2 authorize is recorded as Rust-owned disabled-safe/not-implemented with no Python proxy remainder; P0/P1/P2/P6/P3 early-phase audit passes. |
| P4 | completed | PR #65, #66, #67, #68, #72, #73, #74, #75; merge `d5544ec0082e41719c3d30ed84071b0511cfb6b7` | Settings encryption status, rules overview, accounts route shell, categories route shell, category-rules route shell, tags route shell, templates route shell, and settings bundle route shell are merged; taxonomy route shell deletion blockers are cleared. |
| P5 | completed | PR #76, #77, #78, #79; merge `68176fd004545b6fbaa4275bf89506a70b32fe5b` | Bills category-actions, reconciliation, legacy modify/delete, CRUD/list/export/pictures/recurring Flask route shells are deleted; Python bills sidecar remains import-only. |
| P9 | completed | PR #80; merge `bd25a1d0fe5acb943bfd1d6c6cec22365b7c7adf` | Budgets CRUD/export/execution/forecast/history/snapshot/import Flask route package is deleted; `/api/budgets*` governance is `PythonDeleted`; PR and main CI passed. |
| P10 | completed | PR #81; merge `50d41980c9cde7f070fa6763d2417183a0e2cbf9`; PR #82; merge `c139f90631b7113a50bbf3d66e0c01e1b2943b59`; PR #85; merge `848c9343408a438cb274e9dbec0bf67e3f958fa5` | Statistics read/exchange, Analyzer, exchange, and insights Flask route shells are deleted and governed as `PythonDeleted`; local Rust/Python/frontend gates passed, and Gitea CI monitoring is paused until the user asks to resume. |
| P7 | completed | PR #86; merge `563a3a33f079ac1175d9865116bb9c81897cd1c3`; `src/bill_analyser/api/routes/bills/**` deleted; `src/bill_analyser/api/app.py` no longer registers `bills.bp`; `crates/bill-analyser-core/tests/migration_governance_contracts.rs`; `crates/bill-analyser-http/tests/import_runtime_contract.rs`; Gitea Actions run `12848` success | Rust import runtime owns `/api/bills/import*` and `/api/bills/parse_import`; old Flask bills import route package is deleted; provider/global Learning Center/LLM/OCR recognition remain explicit P11 PythonProxied boundaries. Post-merge main push run `12850` was still `in_progress` at stop time, with logs stopped in runner `actions/*` setup rather than a code failure. |
| P11a | completed | `crates/bill-analyser-http/src/import_routes.rs`; `crates/bill-analyser-core/src/migration_governance.rs`; `src/bill_analyser/api/routes/learning.py` deleted; `cargo test --workspace`; full Python coverage gate `1436 passed`, total coverage `90.63%` | Rust import runtime owns global `/api/learning/suggestions*` and `/api/learning/rules*`; old Flask `learning.py` route shell is deleted; remaining P11 boundaries are true LLM provider generation and OCR image recognition. |
| P11b | completed | `crates/bill-analyser-db/src/llm.rs`; `crates/bill-analyser-http/src/import_routes.rs`; `crates/bill-analyser-core/src/migration_governance.rs`; `cargo fmt --check`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test -p bill-analyser-db --test llm_runtime`; `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 90` total lines `90.02%`, changed `llm.rs` lines `95.65%` | Rust import runtime now owns provider-bypassed `/api/llm/config*` and `/api/llm/candidates*`; saved configs remain DB-backed and redacted, temporary runtime config stays process-local, candidate accept/reject is provider-free. Remaining P11 boundaries are true LLM provider generation (`preview-recommend`/`analyze-transactions`/`rule-synthesis`) and OCR image recognition. |
| P11c | completed | `crates/bill-analyser-core/src/ai_ocr_llm.rs`; `crates/bill-analyser-http/src/import_routes.rs`; `crates/bill-analyser-core/src/migration_governance.rs`; `cargo fmt --check`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test --workspace`; `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 90`; changed executable lines total `91.39%`, `import_routes.rs` diff lines `90.17%` | Rust import runtime now owns `POST /api/ml/receipt-recognition`; disabled/cloud_stub/tesseract provider behavior, cancellation, rate limit, process unavailable/nonzero/timeout errors, payment screenshot parsing, runtime metadata, governance and docs are covered. Remaining P11 boundary is true LLM provider generation (`preview-recommend`/`analyze-transactions`/`rule-synthesis`). |
| P11d | completed | `crates/bill-analyser-core/src/ai_ocr_llm.rs`; `crates/bill-analyser-db/src/llm.rs`; `crates/bill-analyser-db/src/import_staging.rs`; `crates/bill-analyser-http/src/import_routes.rs`; `crates/bill-analyser-core/src/migration_governance.rs`; `crates/bill-analyser-http/tests/import_runtime_contract.rs`; `crates/bill-analyser-db/tests/llm_runtime.rs`; `cargo check -p bill-analyser-core -p bill-analyser-db -p bill-analyser-http`; `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test --workspace`; `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 90` total lines `91.17%`, changed production executable lines `90.90%`, changed `import_routes.rs` lines `90.34%`, changed `llm.rs` lines `100%` | Rust import runtime now owns true LLM provider generation for `POST /api/llm/preview-recommend`, `POST /api/llm/analyze-transactions`, and `POST /api/llm/rule-synthesis`; OpenAI-compatible, Claude, and Ollama request/response shapes, explicit Base URL allowlist/SSRF checks with scheme-bound allowlist, bounded provider bodies/raw candidate snippets/prompt memory, current-user account ID resolution, strict ID/update validation/dedupe/caps before mutation, no-evidence rule-synthesis early return, provider errors/rate limits, JSON parsing, preview suggestion application, classification/rule-induction/rule-synthesis candidate creation, runtime metadata, governance and docs are covered. P11 LLM/OCR provider boundaries are complete. |
| P8a | completed | `crates/bill-analyser-http/src/matching_routes.rs`; `crates/bill-analyser-db/src/recurring.rs`; `crates/bill-analyser-db/src/statistics.rs`; `crates/bill-analyser-core/src/migration_governance.rs`; `crates/bill-analyser-http/tests/matching_runtime_contract.rs`; `crates/bill-analyser-db/tests/recurring_runtime.rs`; `src/bill_analyser/api/routes/recurring.py` deleted; `src/bill_analyser/api/routes/calendar.py` deleted; `src/bill_analyser/api/routes/networth.py` deleted; `cargo check -p bill-analyser-core -p bill-analyser-db -p bill-analyser-http`; `cargo test -p bill-analyser-http --test matching_runtime_contract -- --nocapture`; `cargo test -p bill-analyser-db --test recurring_runtime -- --nocapture` | Rust matching recurring calendar networth runtime now owns recurring suggestion list/detect/accept/reject, calendar events with recurring projections, and net-worth snapshot. Formal matching candidates/feedback/manual pairs/reconcile history/investment settings remain Python-proxied for P8b. |
| P8b | completed | `crates/bill-analyser-db/src/matching.rs`; `crates/bill-analyser-http/src/matching_routes.rs`; `crates/bill-analyser-core/src/migration_governance.rs`; `src/bill_analyser/api/routes/matching/**` deleted; `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 90` total lines `90.60%`; full Python coverage gate `1347 passed`, total coverage `90.54%` | Rust matching runtime now owns formal bill candidates, feedback, manual pairs, accept/reject/clear, reconcile history, reconciliation candidate listing, and retired investment settings. Matching Flask route shell is deleted and governance marks `/api/matching/*` as `PythonDeleted`. |
| P12a | completed | `crates/bill-analyser-db/src/backup.rs`; `crates/bill-analyser-http/src/backup_routes.rs`; `crates/bill-analyser-core/src/migration_governance.rs`; `crates/bill-analyser-db/tests/backup_runtime.rs`; `crates/bill-analyser-http/tests/backup_runtime_contract.rs`; `cargo fmt --all -- --check`; `cargo test -p bill-analyser-db --test backup_runtime -- --nocapture`; `cargo test -p bill-analyser-http --test backup_runtime_contract -- --nocapture`; `cargo test -p bill-analyser-core --test migration_governance_contracts -- --nocapture`; `cargo test -p bill-analyser-http --test proxy_contract -- --nocapture` | Rust backup ops runtime now owns `GET|POST /api/backup/jobs`, validates job payloads, persists `backup_jobs`, writes `backup_job_saved` audit rows, and governance removes jobs from PythonProxied while keeping backup file list/create/verify/download/delete/restore/cleanup proxied for the next P12 slice. |
| P12b | completed | `crates/bill-analyser-http/src/backup_routes.rs`; `crates/bill-analyser-db/src/backup.rs`; `crates/bill-analyser-core/src/ops.rs`; `crates/bill-analyser-core/src/migration_governance.rs`; `crates/bill-analyser-http/tests/backup_runtime_contract.rs`; `crates/bill-analyser-db/tests/backup_runtime.rs`; `crates/bill-analyser-core/tests/ops_contracts.rs`; `docs/PROJECT_OVERVIEW.md`; `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 90`; `.venv\Scripts\python.exe -m pytest --cov=src/bill_analyser --cov-report=term-missing --cov-report=json:coverage.json --cov-fail-under=90 tests/ -v -n auto --dist loadfile`; targeted backup/core/db/http contract tests | Rust backup ops runtime now owns backup file list/create/restore verify/download/delete/restore/cleanup plus jobs. It creates safe local `data/` zip backups, exposes Fernet-compatible `.zip.enc` encrypted backup names, requires step-up for Bearer file operations, streams downloads, snapshots SQLite-in-data with `VACUUM INTO`, rejects unsafe zip members, restores via staging plus unique `before_restore_*` snapshots, updates `backup_records`, and records backup audit rows without Python proxy. |
| P12c | completed | `crates/bill-analyser-http/src/backup_sync.rs`; `crates/bill-analyser-http/src/backup_routes.rs`; `crates/bill-analyser-core/src/migration_governance.rs`; `crates/bill-analyser-http/tests/backup_runtime_contract.rs`; `crates/bill-analyser-http/tests/bills_runtime_contract.rs`; `crates/bill-analyser-http/tests/import_runtime_contract.rs`; `docs/PROJECT_OVERVIEW.md`; `docs/overview-ops.md`; `docs/overview-api-routes.md`; `docs/overview-backend.md`; `docs/overview-database.md`; `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test -p bill-analyser-http --test backup_runtime_contract -- --nocapture`; `cargo test -p bill-analyser-core --test migration_governance_contracts -- --nocapture`; `cargo test -p bill-analyser-core --test ops_contracts -- --nocapture`; `cargo test -p bill-analyser-http --test proxy_contract -- --nocapture`; `.venv\Scripts\python.exe -m pytest tests/domains/runtime/unit/test_rust_runtime_workspace.py tests/domains/runtime/unit/test_rust_migration_inventory.py -q`; `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 90` total lines `90.09%` | Rust backup ops runtime now owns `POST /api/backup/sync`. It creates a local backup, validates and redacts sync config, uploads to OSS/S3/COS/Azure Blob/WebDAV without Python SDK execution, records `backup_cloud_synced`, writes sync metadata to `backup_records`, and clears P12 sync provider/runtime config deletion blockers. |
| P13 | completed | PR #97 merge `e18793d1b4db05b43696874631c95454bab49115`; PR #98 merge `35ef0af09c9ba401775adf0ef5a814a8d61cdc07`; PR #99 merge `36b9755c369fcbce145940d11074396b0d27cab4`; PR #100 merge `b2e38e7da5b3a1cee8a0bddde8e005a64160997a`; `scripts/omx_plan_progress.py`; `scripts/hooks/work_context.py`; `scripts/hooks/post_tool_validation_hint.py`; `scripts/hooks/task_state.py`; `scripts/rust_migration_inventory.py`; `AGENTS.md`; `docs/AI_WORKFLOW.md` | Residual plan-progress, hook prompt, migration inventory, and agent-facing workflow surfaces now treat the Rust HTTP server as the primary backend and keep Python as residual sidecar; P13 PR/main Gitea Actions gates passed after rerunning external checkout/network failures. |

Next allowed phase by gate: P14 frontend integrated contract verification; P15 remains blocked until P14 passes and final Python deletion readiness is proven.
