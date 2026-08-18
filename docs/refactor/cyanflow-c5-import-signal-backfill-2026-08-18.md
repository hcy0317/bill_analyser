# Cyanflow C5e import signal backfill

日期：2026-08-18

状态：已由 PR #321 合并；exact-head CI run `16174` 的 backend、frontend、governance、E2E 全部通过，main 为 `aca34e7e864d89e04190017256960c22ced1b65c`

范围：提供可续跑、限速、并发安全的 Rust 信号投影回填命令；不切换生产 read、不改变 API/session/confirm 合同、不增加索引、不对长期开发数据库执行历史回填

## 裁决

`backfill_import_preview_signal_projection_batch` 是唯一 backfill repository boundary。它只处理 `signal_projection_version=0` 的行，并复用 C5d 的 `import_preview_signal_projection_from_payload` 与 core `PreviewStateKernel`；backfill 不创建第二套信号语义。

每批事务先获取固定 advisory lock，再拒绝 `id <= after_id` 范围内仍存在 version `0` 的不安全水位。合法批次使用主键升序、普通 `FOR UPDATE` 锁定最多 1,000 行；禁止 `SKIP LOCKED`，因为越过暂时锁定的低主键后推进 checkpoint 会永久遗漏该行。投影完成后以一次 `UNNEST` bulk update 写六列，并保留 `WHERE signal_projection_version=0` CAS；更新数与领取数不一致会整批回滚，因此在线 version `1` writer 永远不会被覆盖。

legacy `import_preview_signal_flags` 仍只作为只读 oracle。本切片对每个 backfill 批次的全部目标行做 parity，不采样；mismatch 被记录但不改变 canonical Rust 投影。schema/oracle 不存在、观察行数异常或 SQL 执行失败属于部署错误，直接失败关闭。

命令 `bill_import_signal_backfill` 在执行前验证全部 embedded migrations。默认从水位 `0` 开始，每批 200 行、批间暂停 50 ms；支持 `--after-id`、`--batch-size`、`--max-batches` 和 `--sleep-ms`。每批事务成功提交后才输出一行 JSON，包含 `start_after_id`、`next_after_id`、`scanned_rows`、`updated_rows`、`mismatch_rows`、`has_remaining_rows` 与 `duration_ms`。调用方必须持久化最新 `next_after_id` 后再续跑。

## Transition ledger

- writer：在线 evidence mutation 继续由 C5d writer 写 version `1`；C5e 只补 version `0`，两者共享同一 kernel。
- read：生产 page/filter/count/facet 继续读取 legacy SQL aliases；typed columns 尚未成为查询权威。
- concurrency：backfill 批次用固定事务锁串行；在线 writer 通过行锁与 version `0` CAS 获得优先保护。
- checkpoint：只在事务 commit 后输出；错误水位、毒数据和 CAS 异常不产生已提交的半批结果。
- parity：每个回填批次全量比较六类 signal；目标数据库的完整累计 mismatch 必须为 0 才能进入 C5f-b 公开 read cutover。
- rollback：停止命令并从最后一次已持久化 checkpoint 续跑；已经写入的 version `1` projection 保留，不删除 additive columns。
- next slice：C5f-a 只建立私有 read shadow；C5f-b 必须在真实目标库完成回填、累计 parity=0 后，才允许把 query read 切到 typed columns 并测量写入退化与查询 p95。

## TDD 与审计证据

- RED：新增真实 PostgreSQL target 首次编译因缺少 `backfill_import_preview_signal_projection_batch` 失败，证明测试先于实现。
- focused：`import_signal_backfill_postgres` 为 `6 passed / 0 failed / 0 skipped`；覆盖分批续跑、终态幂等、在线 v1 保护、错误水位、毒数据整批回滚、全批 mismatch、双进程串行、真实 CLI checkpoint/限速/help/缺失环境错误。
- CLI unit：`bill_import_signal_backfill` 为 `4 passed / 0 failed`，覆盖默认值、两种参数格式与非法边界。
- DB 全目标：`bill-analyser-db --all-targets` 全绿；既有 `import_staging` 仍为 `36 passed / 0 failed / 0 skipped`。
- 完整 Rust workspace coverage 在独立空数据库执行且零失败，迁移账本 `26/26`；行覆盖率 `66.8931%`（48,931/73,148）。
- 本切片业务改动行覆盖率 `95.98%`（310/323）；3 个 coverage-eligible 文件全部匹配 LCOV。
- `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、Rust-only source-tree gate、Rust backend structure gate 与 `git diff --check` 全部通过。
- 实现 commit：`635589b11b2eab7c4dafaa4b9c63fb1d6e93012a`。
- Gitea：PR #321 已 squash merge；exact-head CI run `16174`，backend job `19174`、frontend job `19175`、governance job `19176`、E2E job `19177` 全部成功。

完整 coverage 使用隔离数据库 `bill_analyser_c5e_cov_20260818_3`，测试后已精确删除并复核不存在；长期开发数据库的 migration ledger 与历史 preview rows 未被修改。

机器可读证据位于 `docs/refactor/evidence/cyanflow-c5-import-signal-backfill-2026-08-18.json`。
