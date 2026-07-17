# Bill Analyser 全表面缺陷修复验收报告（2026-07-18）

本报告对照 [2026-07-14 全表面缺陷审计](codebase-bug-audit-2026-07-14.md)，记录当前分支 `codex/remediate-codebase-bug-audit-20260714` 截至实现 HEAD `24273bc3128019efbf69608d75964fdb917bedff` 的修复闭环。原审计报告保留为历史识别基线，不回写当时的 finding 事实。

## 结论

- 22/22 个 confirmed findings 均已有修复提交、当前源码锚点和自动化回归；P0/P1/P2/P3 仍按原始分布 0/13/8/1 统计。
- 两个 needs-corroboration 项均已由后续实现和测试处置：金额极值在 HTTP/DB 边界失败关闭；desktop router 已有 catch-all redirect。
- 本地业务与治理门禁通过；实现 HEAD 已推送，Gitea Actions Run 15586 的 backend、frontend、repo-governance 与 E2E 四个 job 的真实 `conclusion` 全部为 `success`。
- 独立 code review 对最终实现和结构增量均为 `APPROVE`，独立 architecture review 均为 `CLEAR`；已关闭原报告中的审查门禁。

## Confirmed findings 闭环账本

| Finding | 状态 | 主要修复提交 | 当前证据 |
| --- | --- | --- | --- |
| BA-AUTH-001 | 已修复 | `d60c16d7a` | `tests/backend/http/auth_session_authority.rs` 覆盖 revoke 后 access-token replay 与 session authority。 |
| BA-BILL-001 | 已修复 | `5e7d1d49c` | `tests/backend/db/bills_postgres.rs` 覆盖 update/update、update/delete、batch/single 与反向锁序并发。 |
| BA-DEPLOY-001 | 已修复 | `6fceee64d` | `src/backend/db/build.rs` 生成 embedded migration；`tests/backend/db/postgres_embedded_migrations.rs` 校验 manifest 与 SQL 内容。 |
| BA-OPS-001 | 已修复 | `6fceee64d` | `tests/backend/http/health_readiness.rs` 覆盖 PostgreSQL 不可达时 503，以及真实 PostgreSQL/Weaviate readiness。 |
| BA-HTTP-001 | 已修复 | `4a09d61b7` | `tests/backend/http/bill_route_input_contract.rs` 将 malformed JSON 固定为不泄露 serde 细节的 400。 |
| BA-TOOL-001 | 已修复 | `5e7d1d49c`, `899d3b240` | CAS/transaction helper 已接线，遗留 transfer helper 已清理；`cargo clippy --workspace --all-targets -- -D warnings` 通过。 |
| BA-FE-AUTH-001 | 已修复 | `fb86723c9`, `0aca932d6`, `fac7e573c` | `src/web/src/lib/userstate/credentials.ts` 统一 access/refresh 锁生命周期；`tests/web/lib/userstate.authLock.test.ts` 覆盖迁移与失败关闭。 |
| BA-API-001 | 已修复 | `781554eb6`, `623261d2d`, `975b6ae1f` | Rust 注册 import-config CRUD/match/suggest；PostgreSQL、HTTP handler、service envelope 与 ImportDialog 回归均存在。 |
| BA-API-002 | 已修复 | `b3f161a94`, `3600757f3`, `e59aca7be` | learning-rule update 使用 live `/api/learning/rules/{id}`，读写与页面渲染合同一致。 |
| BA-GOV-ROUTE-001 | 已修复 | `5e03a5e38`, `532b0505d` | ownership graph 与 assembled Axum route 双向校验；`tests/backend/http/runtime_route_ownership_contract.rs` 覆盖 legacy 假归属失败。 |
| BA-FE-AUTH-002 | 已修复 | `fb86723c9` | refresh coordinator 先持久化最新 credential，再恢复 blocked request；服务层测试覆盖旧 token 清除。 |
| BA-FE-AUTH-003 | 已修复 | `fb86723c9` | blocked queue 保存 resolve/reject，所有 refresh 失败路径确定性 reject；服务层测试覆盖 settlement。 |
| BA-FE-STATE-001 | 已修复 | `b2b74c208`, `c2c27f522`, `f18517bb2` | transaction/statistics 使用 generation/coordinator；deferred 回归证明旧响应不能覆盖新状态。 |
| BA-GOV-STRUCT-001 | 已修复 | `846acb104`, `29c3e3d99`, `ea9146919` | Rust/frontend 结构拆分完成；Gitea CI 与 `scripts/run_ci_local.ps1` 均执行 structure gates。 |
| BA-GOV-COV-001 | 已修复 | `acae70efe`, `a958beb12` | Jest coverage denominator 扩展到 `src/**/*.{ts,vue}`；全量前端 coverage threshold 受 CI 保护。 |
| BA-GOV-COV-002 | 已修复 | `ea9146919` | Gitea CI 和本地 CI 均以 merge-base/head diff + LCOV 执行严格 `>90%` changed-line gate。 |
| BA-GOV-E2E-001 | 已修复 | `bb006c3a7`, `94cbc0da7`, `87e2a4b38` | Gitea CI 运行受控 Playwright supervisor；本地完整 desktop/mobile 与 strict cleanup 通过。 |
| BA-CONFIG-001 | 已修复 | `d58fe2bd7` | `HttpShellConfig::from_env` 仅允许显式 dev/development/local profile 使用本地默认 URL，其余环境缺失配置失败关闭。 |
| BA-OPS-002 | 已修复 | `03fef2fd4`, `72e0b59fc`, `f7f757775` | 启动、健康探测与停止脚本共用 `scripts/http-bind.ps1`，按解析端口精确定位 PID；PowerShell 5.1 合同有回归。 |
| BA-DOC-001 | 已修复 | `883c180e7` | `AGENTS.md` 与 `PROJECT_OVERVIEW.md` 均指向 `src/backend/http/bin/bill_http_server.rs`。 |
| BA-DOC-002 | 已修复 | `883c180e7` | `AGENTS.md`、`README.md` 与 `PROJECT_OVERVIEW.md` 统一 core/DB/HTTP/frontend 使用整数分或显式 minor units。 |
| BA-GOV-SKILL-001 | 已修复 | `a8a60c9f5` | Gitea cache skill 只引用当前 Node/Rust/PowerShell gates，明确禁止恢复 pytest、`.venv` 与 Python-era 路径。 |

## 原 needs-corroboration 项

| 项目 | 处置结果 | 当前证据 |
| --- | --- | --- |
| BA-MONEY-SUS-001 | 已证实风险并修复 | `4a09d61b7` 与 `3752d78af`；HTTP、adapter、preview/confirm 与真实 PostgreSQL 测试拒绝 `i64::MIN`，DB 使用 `checked_abs`。 |
| BA-FE-ROUTER-SUS-001 | 已补齐合同 | `b2b74c208`；`src/web/src/router/desktop.ts` 使用 `/:pathMatch(.*)*` 重定向 `/`，desktop/mobile route smoke 均通过。 |

## 审查回流补强

- `ce6cedb31` 将批量文件解析改为有界顺序执行，并在临时文件与数据库持久化前执行 64 MiB 累计标准账单物化预算；稳定 `error_code`/`errorCode` 取代前端对展示文案的分支依赖。
- `fdb9b1d75` 按结构门禁拆分 response payload 的预算实现与回归测试，不改变解析顺序、响应合同或持久化边界。
- `24273bc31` 让稳定错误码通过生产响应路径生成真实 LCOV 记录；保持严格 `require-matched-files`，没有降低阈值、排除 runtime 文件或增加 coverage 特判。
- Run 15584 因新文件超过 600 行结构 ratchet 失败；Run 15585 因纯类型字段缺少 LCOV 文件记录失败。两次均按实际 job `conclusion` 判定失败并完成修复，没有把 `completed` 或总体覆盖率达标误报为成功。

## 当前验证证据

- Rust：`cargo fmt --all -- --check`、workspace Clippy、真实 PostgreSQL `cargo test --workspace` 全部通过。
- Rust coverage：本地完整工作区 line coverage 64.78%，高于 35% baseline；Run 15586 对 65/65 个 coverage-eligible Rust 文件完成匹配，改动业务可执行行 8631/9163，94.19%。
- Frontend：lint 0 errors（140 个既有 warnings），304/304 suites 与 41,420/41,420 tests 通过，production build 通过；statements 94.09%、branches 91.24%、functions 91.90%、lines 94.20%；改动业务行 707/711，99.44%。
- Browser E2E：本地完整 desktop/mobile Playwright 43/43 通过；Run 15586 的 dependency-readiness、start-rust、desktop-smoke、mobile-smoke 与 cleanup 五个 phase 全部成功。
- Exact-head CI：Gitea Actions Run 15586 精确对应 `24273bc3128019efbf69608d75964fdb917bedff`，backend-ci、frontend-ci、repo-governance、e2e-ci 四个 job 的真实 `conclusion` 全部为 `success`。
- Cleanup：专用 PostgreSQL 业务数据和 run-scoped Weaviate collection 均验证清空；5000/8081 无残留监听；临时 E2E PowerShell runner 已删除且从未进入 Git，当前不存在修改或未跟踪的 `.ps1`。

## 审查与交付门禁

- 最终行为增量：独立 reviewer `APPROVE`，独立 architect `CLEAR`。
- 最终结构增量：独立 reviewer `APPROVE`，独立 architect `CLEAR`。
- LCOV 匹配修复增量：独立 reviewer `APPROVE`，独立 architect `CLEAR`。
- 非阻塞风险：累计标准账单预算在单文件解析完成后核算，并非严格瞬时 RSS 上限；如未来需要进程级硬上限，应将预算下沉到 parser 增量输出 sink。`DedicatedParserDecision.error_code` 字段仍公开以保持 Rust API 兼容，优先使用访问器仍属于当前调用约定。

截至实现 HEAD，22 项 confirmed finding、2 项原 needs-corroboration、独立审查、本地全量验证与 exact-head 远端 CI 均已闭环；没有剩余阻断性交付门禁。
