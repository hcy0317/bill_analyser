# Cyanflow C5d import signal writer shadow

日期：2026-08-18

状态：已由 Gitea PR #320 squash 合并到 `main`（`0baf76ec9c3af1a68ebe088b922873aafed5dc56`）；exact-head CI 全绿

范围：启用 `PreviewStateKernel` 驱动的唯一 Rust signal projection writer，并让 legacy SQL 只承担有界只读 shadow parity；不回填历史行、不切换生产 read、不增加索引、不改变 API 或 confirm 协议

## 裁决

`import_preview_signal_projection_from_payload` 是 DB adapter 内唯一的列投影入口。它从 canonical preview payload 调用 core `PreviewStateKernel`，把同一个 `SignalSet` 映射为六个 boolean 列并写入 `signal_projection_version=1`。投影与原始 evidence 处于同一 repository 事务，SQL 函数不生成任何写入值。

writer 覆盖以下 mutation boundary：

- 初始单行与分块批量 preview insert；
- 通用 preview patch 与 confirm 内 patch；
- transfer decision-group、learning decision 与 LLM decision；
- historical rejection 与 same-batch transfer rejection 的 preview 重建。

纯 selection writer 只更新 `selected` 及兼容 JSON 字段，不推进 row version，也不把 `signal_projection_version=0`、signal 为 `NULL` 的旧行升级为 v1。旧行只有在发生 evidence mutation 或进入 C5e backfill 时才会物化。

legacy `import_preview_signal_flags` 的 shadow SQL 只有 `SELECT`，逐 family 使用 `IS DISTINCT FROM` 比较。mismatch 仅写结构化 `operation/expected_rows/observed_rows/mismatch_rows`，不包含用户、session、preview id 或交易内容，不回写、不改变业务结果。oracle SQL 缺失或执行失败仍按 schema/deployment 不完整 fail-closed；它不是可接受的 parity mismatch。

为限制线上写入额外成本，批量 preview insert 在整笔事务中只执行一次 shadow query，并最多比较首 64 个确定性样本；单行和 decision mutation 会比较其实际目标行。真实 PostgreSQL 测试独立读取并逐行核对完整测试批次，C5e/C5f 仍要求全量 backfill parity=0，运行时采样不替代 read cutover 门禁。

## Transition ledger

- writer：`PreviewStateKernel` 是唯一语义 owner；Rust repository 在 evidence mutation 的同一事务写 payload、typed fields、六列 signal 和 projection version。
- read：生产 page/filter/count/facet 继续读取显式别名 `legacy_signal_*`；新列尚未进入 read path 或 API DTO。
- backfill：本切片没有扫描或修改 version `0` 历史行；新行/evidence mutation 行为 version `1`。
- shadow：legacy SQL 只读；运行时比较有界采样，真实 PG 验收比较测试批次全部目标行。
- rollback：停止新 binary 并 forward-fix；已写 version `1` 行与 nullable 列保留，旧 read 无需降级或删列。
- next slice：C5e 增加 Rust watermark backfill command，按小批次只 claim version `0` 行，复用同一 kernel，绝不覆盖在线 version `1` writer。

## TDD 与回归证据

- 红灯 1：`bulk_insert_query_builders_preserve_insert_shapes` 发现 preview insert SQL 未包含 `signal_projection_version`。
- 红灯 2：真实 PostgreSQL `real_postgres_import_six_signal_families_e2e` 发现新写行 projection version 仍为 `0`。
- focused 绿灯：只读 shadow SQL 契约 `1 passed / 0 failed`；六信号真实 PostgreSQL E2E `1 passed / 0 failed / 0 skipped`。
- DB 绿灯：`bill-analyser-core` + `bill-analyser-db` 全目标通过；`tests/backend/db/import_staging.rs` 为 `36 passed / 0 failed / 0 skipped`，覆盖 insert、patch、confirm、transfer/learning/LLM decision、拒绝重建、并发与 selection-only 不升级。
- 完整 Rust workspace coverage 在独立空数据库执行且零失败，数据库迁移账本为 `26/26`；行覆盖率 `66.0560%`（50,519/76,479）。
- 本切片业务改动行覆盖率 `91.64%`（252/275），严格高于 90%；9 个 coverage-eligible 业务文件全部匹配 LCOV。
- `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、Rust-only source-tree gate、Rust backend structure gate 与 `git diff --check` 全部通过。
- 实现 commit：`f674f90a4c60b5522d5d1c75af3da6194cf94dd8`。

完整 coverage 使用隔离数据库 `bill_analyser_c5d_cov_20260818_2`，测试后已精确删除并复核不存在；长期开发数据库的 migration ledger 未被修改。

机器可读证据位于 `docs/refactor/evidence/cyanflow-c5-import-signal-writer-shadow-2026-08-18.json`。
