# Cyanflow C4 preview row CAS

日期：2026-08-16

状态：本地完整审计已通过；PR、exact-head CI 与合并状态由 Gitea 交付记录持有

## 结果

本切片把上一切片已经公开的 `row_version` 接入单行预览更新协议。`PUT /api/bills/import/v2/preview/:session/update` 可选接受正整数 `expected_row_version`；携带 token 时，repository 在 active-session 事务内锁定目标行，并以 `WHERE version = expected` 执行 compare-and-swap。成功响应返回版本递增后的完整 preview row；版本过期返回 `409 PREVIEW_ROW_VERSION_CONFLICT`，同时附带 expected/actual version 与当前用户、当前 session 下的最新安全行。

不携带 token 时继续沿用 legacy last-write-wins，保证旧客户端可渐进迁移。当前 CAS 只属于单行 update；reclassify、selection、decision group、confirm 及 repository 内部派生写入没有被隐式纳入。

## 模块边界

- `ImportPreviewPatch.expected_row_version` 是 repository 的可选并发前置条件；`PreviewRowUpdateTarget` 把 preview、session、user 与 expected version 固定为一个写目标合同。
- SQL 先按既有 session/user scope 锁行，再在 update predicate 中校验行版本；过期请求不会写 payload、typed identity、selection 或 version。
- `DbError::PreviewVersionConflict` 保留 preview id、expected 与 actual，HTTP adapter 负责映射公开错误码和安全响应，不在数据库层拼 JSON。
- confirm patch、learning decision 与 LLM review 等内部 writer 显式传入 `None`，其现有事务、版本与幂等合同保持不变，后续必须按独立切片迁移。
- 前端 service 只在调用方提供 token 时发送 `expected_row_version`；learning decision 前的文本同步使用 draft `_rowVersion`。
- 409 后，前端只把服务端最新行重基到该文本同步负责的 `counterparty`、`paymentMethod`、`comment`、`selected` 与 `_rowVersion`，再展示冲突；不会把 stale draft 继续当成已保存状态。

## 兼容与不变量

- PostgreSQL schema 不变，继续复用 `import_preview_rows.version BIGINT NOT NULL DEFAULT 1`。
- `row_version` 在 TypeScript 模型中仍为 optional，以兼容迁移期旧服务响应；客户端没有 token 时不伪造版本。
- 409 最新行必须经过共享 preview row presenter，并受 user/session scope 约束；错误响应不暴露其他用户或其他 session 行。
- 金额仍使用 `*_amount_cents` 整数分，本切片没有金额转换、parser、dedup、Stage 2 或信号状态机变更。
- session version、selection hash、decision group version、history bill version 与 confirm receipt 职责不变，不能被 row version 替代。

## TDD 与故障恢复

RED：repository 真 PostgreSQL 用例先要求版本相等时递增、旧版本时返回冲突且数据库保持不变；HTTP 用例要求解析 token、成功返回完整新行、冲突返回类型化 409；前端用例要求 wire token、冲突识别和 draft rebase。

GREEN：完成最小 repository、HTTP 与前端接线后，局部 Rust 和前端用例通过。全量门禁进一步发现并修复了三个真实问题：共享 helper 参数过多导致 Clippy 失败；新增弱 `Record<string, unknown>` 触发 API ratchet；409 最初只更新 `_rowVersion` 而保留 stale 文本。最终实现改为 typed conflict DTO，并在冲突时重基该同步边界拥有的文本字段。

已通过的重点验证：

```powershell
$env:BILL_ANALYSER_TEST_POSTGRES_URL='postgresql://bill_analyser:bill_analyser_dev@127.0.0.1:54321/bill_analyser'
cargo test -p bill-analyser-db --test import_staging real_postgres_preview_row_version_cas_rejects_stale_updates_without_writes -- --exact --nocapture
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 35
Set-Location src\web
npm run lint:ci
npm run test:coverage -- --silent
```

## 完整审计

- 带真实 PostgreSQL 的 `cargo test --workspace` 全部通过，数据库场景 0 ignored。
- `cargo fmt --all -- --check` 与 `cargo clippy --workspace --all-targets -- -D warnings` 通过。
- Rust full-runtime 行覆盖率 `65.10%`；changed-line coverage 为 `81/83`，即 `97.59%`。
- 前端 `npm run lint:ci` 以 0 error 通过；141 条 warning 为现有基线。
- 前端 `npm run test:coverage -- --silent` 通过：`308/308` suites、`41,471/41,471` tests，总行覆盖率 `94.22%`。
- 前端 changed-line coverage 为 `34/36`，即 `94.44%`，包含 service token/409、纯 rebase helper 与 SFC 冲突路径。
- Rust-only source-tree gate 通过，扫描 1,975 个 tracked path；frontend structure gate 扫描 616 个文件并确认 39 个大文件 allowlist 无增长。
- `git diff --check` 通过；本切片没有 schema、金额、parser、dedup、Stage 2 或信号状态机变更。

机器可读证据：`docs/refactor/evidence/cyanflow-c4-preview-row-cas-2026-08-16.json`。
