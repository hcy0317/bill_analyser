# Cyanflow C5f-a import signal read shadow

日期：2026-08-18

状态：已由 PR #322 合并；exact-head CI run `16176` 的 backend、frontend、governance、E2E 全部通过，main 为 `60f106c1cdc6909d4274f945f3510f1bc4899bb1`

范围：建立 typed signal columns 的私有读取和单快照 parity 审计边界；不执行目标库历史回填，不切换生产 read，不改变 API/session/mutation/confirm 合同，不增加索引

## 裁决

公开 `query_preview_page_by_session` 继续固定使用 legacy payload projection。typed v1 读取只通过 `audit_import_preview_signal_read_parity` 暴露给迁移审计，不接受 `preview_ids` 旁路，也不对 HTTP route 或客户端开放。

审计先在 user/session scope 内检查所有 preview row 都是 `signal_projection_version=1`；存在 version `0` 或未知版本时立即失败关闭。通过版本门禁后，它在同一 `REPEATABLE READ, READ ONLY` PostgreSQL 事务中依次执行 legacy 与 typed page/filter/count/facet 查询，避免并发 mutation 让两次读取观察到不同数据库状态。

typed 查询直接读取 `signal_parser`、`signal_platform_duplicate`、`signal_transfer`、`signal_history`、`signal_learning` 与 `signal_llm`，并要求 projection version `1`；它不调用 `import_preview_signal_flags`。审核状态限定仍从共享 evidence 文本读取，因此本切片只迁移六类 family membership 的读取权威，不复制 decision lifecycle。

`ImportPreviewSignalReadParityReport` 比较完整 page rows、total、六类 signal counts、selection counts、category/account/tag facets 与 selection hash。报告仅用于证明差异，不写数据库、不改变 canonical Rust projection，也不允许 mismatch 自动回退成 typed production read。

## Transition ledger

- writer：仍由 C5d 在线 writer 与 C5e backfill writer共享唯一 `PreviewStateKernel`；本切片不写 signal projection。
- production read：继续固定为 legacy payload projection。
- shadow read：只对迁移审计开放 typed v1；同一只读可重复读快照内与 legacy 比较。
- version gate：session 内任何非 version `1` 行都失败关闭，不把 `NULL` 或 version `0` 当作 false。
- parity：rows、total、signals、selection、facets、selection hash 任一差异都被显式报告。
- rollback：停止调用审计 API 即可；没有 schema、数据或公开 runtime 状态需要回滚。
- next slice：C5f-b 提供可审计的目标库批次编排与累计 parity 证据；在真实目标库全量回填和累计 mismatch=0 前继续禁止公开 read cutover。

## TDD 与审计证据

- RED：真实 PostgreSQL 测试首次编译因缺少 `PreviewSignalReadSource` 与 typed query builder 失败，证明测试先于实现。
- focused real PostgreSQL：`import_signal_read_shadow_postgres` 为 `2 passed / 0 failed / 0 skipped`，覆盖六类 family、status filter、非空 facets、未物化 fail-closed、故意 typed drift 与 `preview_ids` 拒绝。
- DB lib：`149 passed / 0 failed`；DB all-targets 全部通过，既有 `import_staging` 为 `36 passed / 0 failed / 0 skipped`。
- 完整 Rust workspace coverage 在独立空数据库执行且零失败，迁移账本 `26/26`；行覆盖率 `67.0068%`（49,191/73,412）。
- 本切片业务改动行覆盖率 `97.33%`（474/487）；3 个 coverage-eligible 文件全部匹配 LCOV。
- `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、Rust-only source-tree gate、Rust backend structure gate 与 `git diff --check` 全部通过。
- 实现 commit：`789a019a3`。
- Gitea：PR #322 已 squash merge；exact-head CI run `16176`，backend job `19182`、frontend job `19183`、governance job `19184`、E2E job `19185` 全部成功。

完整 coverage 使用隔离数据库 `bill_analyser_c5f_shadow_cov_20260818_2`，测试后已按精确名称删除并复核不存在。机器可读证据位于 `docs/refactor/evidence/cyanflow-c5-import-signal-read-shadow-2026-08-18.json`。
