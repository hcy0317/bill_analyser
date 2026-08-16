# Cyanflow C4 recurring decision row CAS

日期：2026-08-17

状态：本地完整审计已通过；PR、exact-head CI 与合并状态由 Gitea 交付记录持有

## 结果

本切片把已有 preview `row_version` 接入 recurring decision 的 PUT 与 DELETE。后端 repository 在同一个 active-session 锁事务内读取 preview 行并校验可选 token，过期 token 在任何 recurring patch 前返回 `409 PREVIEW_ROW_VERSION_CONFLICT` 与最新完整行；当前 token 成功写入后返回版本递增的完整行；无 token 的旧调用继续兼容。

桌面 recurring mutation 不再在页面内手写 `fetch`，改由 `importPreviewRecurring.ts` 的 typed service adapter 统一 endpoint、PUT/DELETE payload 与 response envelope。类型化 409 到达后，页面用权威快照重基行版本、文本和 recurring 状态，终止当前操作且不自动重试。

## 模块边界

- `update_preview_recurring_match_decision` 位于 DB import staging repository，负责 active-session lock、expected state/row version 校验、写入和最新行回读；HTTP handler 只解析 payload 并投影 typed 409。
- 行版本冲突与旧 expected-state 冲突保持不同响应：只有 row version 不一致才携带最新完整行；分类、账户等旧状态冲突继续返回原有通用 409。
- `importPreviewRecurring.ts` 承担 recurring endpoint 与 wire payload；`importPreview.ts` 继续作为兼容 facade 暴露能力。
- 本切片不修改 recurring 候选或匹配语义，不修改数据库 schema、金额单位、selection token、decision group、reclassify 或 confirm。

## 原子性与兼容边界

- repository 先锁 active session，再从同一事务读取目标 preview，随后比较 `expectedState.rowVersion`；校验与写入之间没有第二 writer 窗口。
- stale token 在首个 recurring 字段写入前结束事务，响应包含 expected/actual version 和当前用户、当前 session 下的最新完整行。
- PUT 接受 recurring candidate，DELETE 清空 recurring 投影；两者成功后都返回递增 `row_version`。
- 无 token 调用复用同一事务和 user/session scope，但不承诺并发冲突检测；这是迁移期显式兼容分支。

## TDD 与审计恢复

RED：真实 PostgreSQL 测试先读取旧版本，再通过另一个 writer 更新同一 preview；旧 recurring PUT 仍返回 200 并覆盖更新后的行。前端 service 行为测试在 adapter 尚不存在时编译失败。

GREEN：stale PUT 返回类型化 409、最新完整行且数据库零写入；当前版本 PUT/DELETE 分别递增版本并返回完整行；无 token PUT 保持兼容。Rust 聚焦命令命中 2 条测试，2 passed、0 failed、0 ignored；前端聚焦回归为 3 个 suite、63/63 tests。

## 完整审计

- 带真实 PostgreSQL 的 `cargo test --workspace` 通过；本切片 PostgreSQL 聚焦 discovery 命中 2 条且全部执行。
- 带真实 PostgreSQL 的 `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 35` 通过；Rust full-runtime 行覆盖率 `65.51%`。
- `cargo fmt --all -- --check` 与 `cargo clippy --workspace --all-targets -- -D warnings` 通过。
- Rust changed-line gate 匹配 2 个 coverage-eligible 业务文件、32 个可执行变更行，覆盖 30 行，即 `93.75%`；缺失 LCOV 文件为 0。
- 前端 `npm run lint:ci` 以 0 error 通过；141 条 warning 为现有基线；`npm run build` 通过。
- 前端完整 coverage 为 `308/308` suites、`41,484/41,484` tests，总行覆盖率 `94.23%`、分支 `91.23%`、函数 `91.99%`。
- 前端 changed-line coverage 为 `31/32`，即 `96.88%`；新增 recurring service 为 `100%`，Vue 改动文件已匹配。
- Rust-only source-tree、Rust backend structure 与 frontend structure gate 均通过；frontend 只有既有文件行数减少提示，没有新增 failure。
- `git diff --check` 通过；本切片没有 schema 或金额转换。

机器可读证据：`docs/refactor/evidence/cyanflow-c4-recurring-decision-row-cas-2026-08-17.json`。
