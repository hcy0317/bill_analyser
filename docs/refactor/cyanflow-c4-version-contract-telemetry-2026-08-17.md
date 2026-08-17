# Cyanflow C4 version contract telemetry

日期：2026-08-17

状态：已由 PR #315 交付并合并；本地门禁与 exact-head CI 全部完成

范围：导入竞争 mutation 与 confirm 的 version 合同可观测性；不改变 REST、CAS、兼容或持久化行为

## 裁决

C4 保留缺少 version 的 legacy 兼容分支，但此前该分支静默执行，无法证明旧调用是否已经归零。当前切片增加一个 HTTP-owned 结构化 telemetry helper，统一记录：

- `operation`；
- `token_kind`（`row_version` 或 `session_version`）；
- `required_tokens`、`present_tokens`、`missing_tokens`；
- `contract`（`versioned` 或 `legacy_missing_version`）。

日志禁止携带 user、session、preview id、账单正文、请求 payload 或凭据。telemetry 只观察既有解析结果，不参与是否接受请求、CAS、事务、响应或写入分支。

## 覆盖入口

| Operation | Token | 批次语义 |
| --- | --- | --- |
| `preview_update` | row version | 单行 |
| `preview_reclassify` | row version | 一次记录批次 required/present，不逐行刷日志 |
| `preview_selection_patch` | row version | 仅条件选择携带 preview patches 时记录；纯集合选择继续由 `selection_hash` 保护 |
| `recurring_decision` | row version | accept/clear 共用 |
| `transfer_decision` | row version | legacy preview route anchor |
| `llm_preview_decision` | row version | 导入专用 LLM route |
| `matching_candidate_decision` | row version | learning/LLM 通用 matching route |
| `confirm` | session version | terminal receipt replay 与现有 session CAS 顺序不变 |

## Transition ledger

- 当前 owner：HTTP ingress 解析 optional version；repository 继续唯一持有 CAS 与业务写入。
- 当前 writer：无新增 writer；telemetry 只写结构化日志。
- old read：缺 version 请求继续进入原 legacy 兼容分支。
- new read：运维可按 `domain=import_contract`、`contract=legacy_missing_version` 聚合调用量。
- rollback：删除 helper 调用即可恢复静默兼容，不涉及 schema 或数据回滚。
- delete/enforce gate：legacy 调用连续观察为 0、兼容期限和外部客户端影响已记录，并另获 breaking-contract 批准后，才能把缺 version 升级为拒绝。

## TDD 与验收

- 红灯：focused contract 因 telemetry 模块不存在而失败，`0 passed / 1 failed / 10 filtered out`。
- 绿灯：focused contract `1 passed / 0 failed / 10 filtered out`。
- telemetry unit：`2 passed / 0 failed`。
- import runtime contract：`11 passed / 0 failed`。
- 结构门禁同时要求各竞争入口使用统一 helper，并禁止 helper 出现 `session_id` 或 `user_id`。
- `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings` 与 Rust-only source tree gate 均通过。
- 使用真实 PostgreSQL 执行 `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 35`，全工作区行覆盖率 `65.95%`，数据库场景零 skip、零失败。
- 基于 staged backend diff 的改动行覆盖率为 `92.73%`，满足严格大于 `90%` 的门禁。
- exact-head Gitea Actions run `16140` 对提交 `850b7bf63bf904514280db9e4cc64ccc4e57e503` 执行；backend `19093`、frontend `19094`、governance `19095`、E2E `19096` 全部成功。
- PR #315 以 squash 合并为 `a3994963b400f125064dd11a51fbc9ee82c21b0c`，来源分支已删除。

完成上述完整门禁后，C4 满足“当前 web 发送 version、409 可重基、legacy 缺 version 显式可观测”的退出条件；强制 version 必填仍是另行批准的 breaking change，不属于 C4 退出要求。
