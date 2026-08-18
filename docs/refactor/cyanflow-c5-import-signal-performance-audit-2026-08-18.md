# Cyanflow C5f-c import signal performance audit

日期：2026-08-18

状态：本地实现与完整审计门禁完成；等待 exact-head CI

范围：提供目标数据库单事务的 legacy/typed 查询 p50/p95 与临时表写入退化审计命令；不运行 migration，不执行 backfill，不写业务表，不切换生产 read，不改变 API/session/mutation/confirm 合同，不增加索引

## 裁决

性能证据必须和 C5f-b 的全量 parity 证据分离。`bill_import_signal_performance_audit` 只输出 `performance_gate_passed`，不能生成或替代 `eligible_for_read_cutover`；公开 read cutover 必须同时持有同一目标状态下合格的 C5f-b read audit 与 C5f-c performance audit 报告。

命令在一个 `REPEATABLE READ` 事务中验证完整 migration ledger，并要求每条 preview row 都是 `signal_projection_version=1` 且六列非空。事务不是 read-only，因为写入测量需要临时表；所有写入都限于事务级临时 source/base/typed 表，最终显式 rollback，不执行任何业务 `UPDATE` 或 `DELETE`。

query corpus 复用 C5f-b 的代表性选择：preview row 最多的 session，加上每个可见 signal family 覆盖最多的 session，去重后最多 7 个。每个 session 执行无筛选与六个 canonical family，共 7 个案例，直接调用生产 legacy 与候选 typed 的 page、total、六类计数、选择计数、category/account/tag facets 和 selection hash 路径。

默认执行 1 次预热和 5 次有效测量，预热不进入统计；每轮交替 legacy-first/typed-first，PostgreSQL JIT 关闭，statement timeout 为 60 秒，lock timeout 为 5 秒。耗时使用 Rust `std::time::Instant`，p50/p95 使用连续百分位插值。查询与写入允许的 p95 退化上限均为 15%，CLI 只能收紧；查询差异在 1 ms 绝对噪声余量内不判退化。

写入 corpus 从代表性最大 session 按 row id 确定性截取，最多 50,000 行。报告同时输出来源 session 总行数、上限、实测行数和是否截断，避免大目标库产生无界临时驻留或把截断样本冒充全量写入。

## Transition ledger

- writer：不变；在线与 backfill writer 继续共享 `PreviewStateKernel`。
- production read：不变；公开 page/filter/count/facet 继续固定为 legacy。
- audit query read：在单个可重复读事务中交替运行真实 legacy/typed 路径并同时校验结果 parity。
- audit write：只写事务级临时表，来源最多 50,000 行，结束时显式 rollback。
- migration：命令只验证完整 ledger，不运行 migration。
- failure：未物化、ledger/schema 错误、结果 drift、query/write p95 超限或 rollback 未确认均失败关闭。
- target execution：本切片未连接真实目标数据库，不能把本地隔离库 green 当作目标性能放行。
- next gate：在真实目标库完成 C5e backfill 后，保存 C5f-b `eligible_for_read_cutover=true` 与 C5f-c `performance_gate_passed=true` 两份报告，随后才能提出公开 read cutover 切片。

## TDD 与审计证据

- RED 1：真实 PostgreSQL 测试首次编译因缺少 performance audit repository API 失败。
- RED 2：CLI 测试首次编译因 `bill_import_signal_performance_audit` binary 不存在失败。
- review iteration：只读 Cyaness review 发现临时写入 corpus 无上限且机器报告缺计时合同；实现随后增加 50,000 行硬上限、截断字段及完整 timing contract，复审通过。
- coverage iteration：第一次全工作区 coverage 的两个微型 fixture 在插桩环境下跨越 15% 性能阈值；没有放宽阈值，而是让 repository fixture 只验证测量、parity、边界与 rollback，并让 5,000 行 CLI fixture 验证进程退出码与实际报告一致。第二次完整 coverage 全绿。
- focused real PostgreSQL：`import_signal_performance_audit_postgres` 为 `4 passed / 0 failed / 0 skipped`；覆盖真实 query 路径、rollback checksum、50,005 行来源截断为 50,000、CLI 单报告/退出门禁/secret-safe、未物化与非法配置失败关闭。
- CLI unit：performance audit 与既有 read audit 均为 `4 passed / 0 failed`。
- DB all-targets 全部通过；DB lib `153 passed / 0 failed`，既有 target read audit 为 `3 passed / 0 failed / 0 skipped`，read shadow 为 `2 passed / 0 failed / 0 skipped`，`import_staging` 为 `36 passed / 0 failed / 0 skipped`。
- 完整 Rust workspace coverage 在独立空数据库执行且零失败，迁移账本 `26/26`；行覆盖率 `66.1582%`（52,261/78,994）。
- 本切片业务改动行覆盖率 `96.8%`（726/750）；6 个 coverage-eligible 文件全部匹配 LCOV。
- `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、Rust-only source-tree gate、Rust backend structure gate 与 `git diff --check` 全部通过。
- 实现 commit：`9913da9408eeace5f9df4d9c13772301e72c8373`。

完整 coverage 使用隔离数据库 `bill_analyser_c5fc_perf_cov_20260818_1`，测试后已按精确名称删除并复核不存在。机器可读证据位于 `docs/refactor/evidence/cyanflow-c5-import-signal-performance-audit-2026-08-18.json`。
