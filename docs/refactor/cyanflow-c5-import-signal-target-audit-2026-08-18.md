# Cyanflow C5f-b import signal target audit

日期：2026-08-18

状态：已通过 PR #323 合并到 `main`；exact-head Gitea CI 全绿

范围：提供目标数据库单快照、只读、全量行 parity 与有界 query corpus 审计命令；不运行 migration，不执行 backfill，不切换生产 read，不改变 API/session/mutation/confirm 合同，不增加索引

## 裁决

跨进程 checkpoint 不能证明累计结果来自同一个数据库快照，因此 C5f-b 不复用 backfill 的可续跑水位。`bill_import_signal_read_audit` 使用一个 `REPEATABLE READ, READ ONLY` 事务完成全部读取，并在事务提交后一次性输出 JSON。长时间运行时可以调整 row batch 大小，但不能把多次命令输出拼接成 read cutover 证据。

命令首先要求目标库 `_sqlx_migrations` 的完整成功版本序列与当前 binary 的 embedded manifest 相同；它不会代替部署流程运行 migration。随后按 row id 分批扫描全部 `import_preview_rows`，对每行核对 `signal_projection_version=1`、六列非空，以及六列和 `import_preview_signal_flags(preview_payload)` 的逐项一致性。

只有全量行 parity 为零时，命令才运行 query parity。corpus 由 preview row 数最多的 session，以及每个可见 family 中 row 数最多的 session组成并按 session 去重，最多 7 个；每个 corpus session 执行无筛选与六个 canonical family 筛选，共 7 个案例，复用 C5f-a 的 legacy/typed page、total、六类计数、选择计数、category/account/tag facets 与 selection hash 比较。

报告包含 migration ledger、PostgreSQL snapshot token、row batches、preview/session 数、未物化行、逐行 mismatch、query corpus/case 与 mismatch dimensions。任一差异都会使 `eligible_for_read_cutover=false`，报告仍输出到 stdout 供保存，随后进程以非零状态退出。错误输出会遮蔽完整数据库 URL 与密码片段。

## Transition ledger

- writer：不变；在线与 backfill writer 继续共享 `PreviewStateKernel`。
- production read：不变；公开 page/filter/count/facet 继续固定为 legacy。
- audit read：单个只读可重复读快照，先全量 row parity，再有界 query corpus parity。
- migration：命令只验证完整 ledger，不运行 migration。
- checkpoint：没有跨进程 resume；一个最终 JSON 对应一个已提交 snapshot。
- failure：未物化、row drift、query drift、schema/ledger 错误或非法预算全部失败关闭。
- target execution：本切片未连接真实目标数据库，不能把本地隔离库 green 当作目标 parity=0。
- next gate：在真实目标库先运行 C5e backfill，再保存本命令 `eligible_for_read_cutover=true` 的完整报告，并完成 query/write p95 门禁后才能设计公开 read cutover。

## TDD 与审计证据

- RED 1：真实 PostgreSQL target 首次编译因缺少 `audit_import_preview_signal_target_snapshot` 失败。
- RED 2：未物化行首次执行因 nullable family aggregate 解码失败，随后改为显式 `COALESCE(..., false)` 并保持 query audit 跳过。
- RED 3：CLI target 首次编译因二进制文件不存在失败。
- focused real PostgreSQL：`import_signal_target_audit_postgres` 为 `3 passed / 0 failed / 0 skipped`；覆盖多 session/多 batch/14 query cases、未物化、typed drift、非法预算、CLI 单报告、只读与 secret-safe。
- CLI unit：`bill_import_signal_read_audit` 为 `4 passed / 0 failed`。
- DB all-targets 全部通过；DB lib `149 passed / 0 failed`，既有 `import_staging` 为 `36 passed / 0 failed / 0 skipped`，C5f-a read shadow 为 `2 passed / 0 failed / 0 skipped`。
- 完整 Rust workspace coverage 在独立空数据库执行且零失败，迁移账本 `26/26`；行覆盖率 `67.0025%`（49,612/74,045）。
- 本切片业务改动行覆盖率 `95.82%`（459/479）；3 个 coverage-eligible 文件全部匹配 LCOV。
- `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、Rust-only source-tree gate、Rust backend structure gate 与 `git diff --check` 全部通过。
- 实现 commit：`e8e742b0d`。
- 交付：PR #323，exact head `050f48a078d79f8c693ac4ae5e12a0046ba1f883`，Gitea Actions run `16184` 的 backend/frontend/governance/E2E 四个 job 全部成功；squash 后 `main` 为 `c5dea65841ab436b60434a30caa8900142fb9152`，PR head tree 与 main tree 均为 `45491e427f3aeaca937f383b20054e75860c8fca`，来源分支已删除。

完整 coverage 使用隔离数据库 `bill_analyser_c5fb_audit_cov_20260818_1`，测试后已按精确名称删除并复核不存在。机器可读证据位于 `docs/refactor/evidence/cyanflow-c5-import-signal-target-audit-2026-08-18.json`。
