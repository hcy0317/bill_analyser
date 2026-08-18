# Cyanflow C6a typed confirm receipt schema expand

日期：2026-08-18

状态：本地实现、双轴审查、恢复演练与完整审计门禁完成

范围：只增加独立 typed confirm receipt 存储及数据库合同；不改变现有 confirm writer/read、公开 API、session 合同、账单/history effect、backfill 或 read cutover

## 裁决

C6a 通过 forward-only migration `0027_import_confirm_receipts.sql` 增加 `import_confirm_receipts`。它不是第二套业务 writer，也不是新 repository 抽象；现有 `confirm_import_command_in_transaction -> persist_confirm_receipt` 仍是唯一真实写链，当前 writer 和 replay reader 继续只使用 `import_sessions.metadata.confirm_receipt`。

新表冻结以下合同：

- `session_id` 主键保证每 session 最多一个 terminal receipt；
- `(session_id, user_id)` 复合外键保证 receipt 与 session 归属一致，账户级用户数据清理通过 session cascade 删除；
- receipt/response schema version 只接受 `1`，command fingerprint 只接受 64 位小写十六进制，request session version 必须大于零；
- 当前兼容 envelope 只接受 HTTP `200` 和 JSON object；
- receipt row 禁止 `UPDATE`，不含 `updated_at`；普通 staging cleanup 不直接包含该表；
- migration 不写入 receipt、不 backfill metadata、不修改 `import_sessions`。

## Transition ledger

- writer：现有 metadata confirm transaction writer 不变；typed table 没有生产 writer。
- read：terminal replay 继续从 metadata 读取；typed table 没有生产 reader。
- backfill：无；真实恢复 corpus 的 9 个 metadata receipts 保持原位，新表保持 0 行。
- rollback：migration 未提交时由 SQLx 事务整体回滚；已部署后停止新 binary 并 forward-fix，不删除新表或篡改 terminal receipt。
- next：C6b 只能修改现有唯一 confirm transaction writer，使 typed row 与 metadata 旧投影同事务提交；read cutover 必须等待 backfill、shadow parity 与 replay compatibility 独立通过。

## TDD 与双轴审查

RED 首先证明 migration manifest 仍停留在 26，真实 PostgreSQL 中目标列为空且 relation 不存在。GREEN 后 focused schema tests 验证列、类型、NOT NULL/default、8 个 validated constraints、不可更新 trigger、唯一性、复合归属、非法版本/fingerprint/status/envelope 失败关闭、普通 cleanup 保留与账户级 cascade 删除。

Cyaness `code-review` 使用固定比较点 `b1904186d02fc218cdddd82de002a8b2122c1a2e`：Standards 轴无 finding；Spec 轴首次发现账户级数据清理尚无显式真实 PostgreSQL 断言。该 finding 交回 Cyanflow reducer 后，以 `clear_postgres_user_data` 的 session cascade 用例闭合并重新通过 focused test 与 Clippy。审查没有引入新 trait/port，也没有扩大 C6a 到 runtime writer/read。

## 验证证据

- focused receipt schema：2 passed，0 failed，0 ignored；依赖缺失 fail-closed；
- `bill-analyser-db --all-targets`：完整通过，包含 `import_staging` 36/36、signal performance 4/4、target audit 3/3 与 embedded migration 1/1；
- `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、Rust backend structure、Rust-only source tree 与 `git diff --check` 全部通过；
- fresh PostgreSQL 完整 Rust workspace coverage：50,332/75,144 行，`66.9807%`，migration ledger 27/27；
- 变更业务代码覆盖率：20/21，可执行变更行 `95.24%`；
- 实现 commit：`94fd0f5df96dafe3c8229e307cd06de6269be35d`。

## Restore rehearsal

从长期开发库 migration 25 生成 custom dump：60,223,766 bytes，SHA-256 `27779d877672390c89d8a0f035a05eca22773ceb081da55fc7ae9c592c7f3ca5`；dump 8,000.093 ms，恢复 38,750.122 ms。恢复 corpus 含 1,275 sessions、237,910 preview rows、5,051 decision groups、982 confirm operations 与 9 个 metadata receipts。

恢复库按 forward 顺序应用 migration 26 与 27，耗时分别为 532.379 ms 与 257.483 ms。migration 后 237,910 行仍为 signal version 0 且六列全 NULL；preview relation/index 尺寸保持 470,540,288/47,276,032 bytes；9 个 metadata receipts 不变；typed receipt table 为 0 行、9 列、8 个 validated constraints、1 个 immutable trigger，总 relation 16,384 bytes。

当前 binary 对该长期开发库的历史 migration 25 checksum 漂移继续按既有合同报告 `VersionMismatch(25)`。本切片没有修改源库或恢复库 ledger；fresh database 已用当前 binary 验证完整 27/27 migration chain。恢复数据库、coverage 数据库、dump 与临时 SQL 副本均已精确删除。

机器可读证据位于 `docs/refactor/evidence/cyanflow-c6-confirm-receipt-schema-expand-2026-08-18.json`。
