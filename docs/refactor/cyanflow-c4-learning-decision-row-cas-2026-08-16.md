# Cyanflow C4 learning decision row CAS

日期：2026-08-16

状态：本地完整审计已通过；PR、exact-head CI 与合并状态由 Gitea 交付记录持有

## 结果

本切片把已有 preview `row_version` 接入 learning matching candidate 的 accept、reject 与 clear。桌面和移动端在 `expectedState.rowVersion` 发送当前正整数 token；repository 在 learning 事务锁定 preview row 后、任何 lifecycle 或 preview 写入前校验版本。版本过期时事务无写入地回滚，HTTP 返回 `409 PREVIEW_ROW_VERSION_CONFLICT`，并附带 expected/actual version 与当前用户、当前 session 的最新完整 preview row。

同一 terminal action 的重复请求继续优先走既有幂等返回，不会因客户端仍持有旧版本而退化成冲突。未携带 token 的旧客户端继续使用原 expected-state 合同。本切片不把 CAS 隐式扩展到 transfer、LLM、reclassify、selection、decision group 或 confirm。

## 模块边界

- `ImportPreviewExpectedState.expected_row_version` 是 learning decision 的可选 repository 前置条件；HTTP 只解析正整数 `rowVersion` / `row_version`，数据库层不解析 JSON。
- `apply_preview_learning_decision` 先按 user/session scope 锁行，先处理相同 terminal action 的幂等重放，再校验 row version；冲突发生在 lifecycle event、feedback 和 preview version 更新之前。
- `DbError::PreviewVersionConflict` 继续承载 preview id、expected 与 actual；matching HTTP adapter 只对当前 learning candidate 映射类型化 409，并通过 user-scoped repository 回读最新行。
- 最新行复用共享 matching presenter，同时附带 canonical `preview_state`；错误响应不会泄露其他用户或其他 session 的 preview。
- 桌面端冲突后重基 decision snapshot 与文本同步字段；移动端重基 row record、selection 与 signal view model。两端都直接消费响应快照，不额外发 candidate/page reload。
- 共享 expected-state DTO 仍被 LLM 入口复用，但 LLM repository 本切片不消费 row version；后续必须用独立切片迁移并锁定它自己的幂等优先级。

## 兼容与不变量

- PostgreSQL schema 不变，继续复用 `import_preview_rows.version BIGINT NOT NULL DEFAULT 1`。
- 不带 `rowVersion` 时，learning accept/reject/clear 保留原 expected-state、状态机与响应行为。
- stale token 不增加 preview version、不改变 learning pending 状态、不追加 lifecycle event。
- terminal same-action retry 保持 idempotent；terminal opposite-action 仍按既有 state conflict 处理。
- 金额继续使用整数分；本切片没有金额转换、parser、dedup、Stage 2、transfer/LLM lifecycle 或 confirm 变更。

## TDD 与故障恢复

RED：Rust DTO 首先因缺少 `expected_row_version` 编译失败；HTTP payload 测试随后要求 camel/snake alias 和非正整数失败关闭；前端测试要求 desktop/mobile wire token，并要求 409 直接重基权威 snapshot。初始实现完成后，仓库 changed-line gate 暴露 handler 的真实 PostgreSQL 冲突分支未被执行，Rust 变更行覆盖率只有 `86.36%`。

GREEN：补充真实 handler 集成测试，在测试 PostgreSQL 中创建唯一用户、session 和 pending learning preview，经实际 matching action handler 发送 stale token，验证 409 最新快照、版本不变和 pending 不变。重新生成完整 LCOV 后 Rust 变更行覆盖率提升到 `95.45%`。

已通过的重点验证：

```powershell
$env:BILL_ANALYSER_TEST_POSTGRES_URL='postgresql://bill_analyser:bill_analyser_dev@127.0.0.1:54321/bill_analyser'
cargo test -p bill-analyser-http learning_action_handler_rebases_stale_row_version_from_postgres -- --nocapture
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 35
Set-Location src\web
npm run lint:ci
npm run test:coverage -- --silent
```

## 完整审计

- 带真实 PostgreSQL 的 full-runtime coverage 执行全部 Rust 工作区测试，数据库场景 0 ignored。
- `cargo fmt --all -- --check` 与 `cargo clippy --workspace --all-targets -- -D warnings` 通过。
- Rust full-runtime 行覆盖率 `65.17%`；changed-line coverage 为 `63/66`，即 `95.45%`。
- 前端 `npm run lint:ci` 以 0 error 通过；141 条 warning 为现有基线。
- 前端 `npm run test:coverage -- --silent` 通过：`308/308` suites、`41,474/41,474` tests，总行覆盖率 `94.23%`。
- 前端 changed-line coverage 为 `25/25`，即 `100%`，包含 desktop/mobile token 与冲突 snapshot rebase。
- Rust-only source-tree gate 通过，纳入本切片文档后扫描 1,979 个 tracked path；frontend structure gate 扫描 616 个文件并确认 39 个大文件 allowlist 无增长。
- `git diff --check` 通过；本切片没有 schema、金额、parser、dedup、Stage 2 或 confirm 变更。

机器可读证据：`docs/refactor/evidence/cyanflow-c4-learning-decision-row-cas-2026-08-16.json`。
