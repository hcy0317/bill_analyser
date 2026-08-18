# Cyanflow C5c import signal schema expand

日期：2026-08-18

状态：本地实现、恢复演练与完整审计门禁完成；等待 exact-head CI

范围：为导入预览行扩展六个 nullable signal 投影列与行级投影版本；不启用新 writer、不回填、不切换读取、不增加 signal 专用索引，也不改变 API 或 confirm 合同

## 裁决

C5c 通过 forward-only migration `0026_import_signal_projection_columns.sql` 增加：

- `signal_parser`、`signal_platform_duplicate`、`signal_transfer`、`signal_history`、`signal_learning`、`signal_llm` 六个 nullable `BOOLEAN`；
- `signal_projection_version SMALLINT NOT NULL DEFAULT 0`；
- 仅允许 projection version `0` 或 `1` 的已验证 CHECK，其中 `0` 表示 legacy/unmaterialized，`1` 表示 `PreviewStateKernel v1`。

迁移不执行 `UPDATE`，不调用 legacy SQL signal 函数，也不创建 signal 专用索引。现有分页筛选、计数和 facet 仍从 `preview_payload` 调用 `import_preview_signal_flags`；旧投影在查询 CTE 中显式使用 `legacy_signal_*` 别名，避免 `SELECT p.*` 纳入新物理列后出现同名歧义。

## Transition ledger

- writer：现有 Rust repository writer 不变，不写六个新列或 projection version；当前不存在第二套 signal writer。
- read：生产查询继续读取 legacy JSON/SQL 投影；新列没有进入 row mapping、filter、count、facet 或 API DTO。
- backfill：本切片没有历史数据回填；全部既有行在 expand 后保持六列 `NULL`、version `0`。
- rollback：迁移未提交时由 SQLx 事务整体回滚；已部署后停止新 binary 并 forward-fix，不删除新列或 CHECK。
- next slice：C5d 只能让同一个 Rust canonical writer 在 evidence mutation 的同一事务写入新投影，并进行只读 shadow parity；legacy SQL 不获得写权。

## TDD 与回归证据

- 红灯：manifest 合同显示 migration 数 `25` 而预期为 `26`；真实 PostgreSQL 查询目标列为 `0` 而预期为 `7`，环境变量缺失时测试直接失败而非 skip。
- 初次绿灯：manifest、embedded migration、静态迁移合同和真实 PostgreSQL expand 合同通过。
- 回归红灯：首次 `bill-analyser-db` 全目标测试有 7 个真实 PostgreSQL 场景因 `signal_parser` 等列名歧义返回 SQLSTATE `42702`；原因是 `SELECT p.*` 已含新物理列，而 legacy lateral projection 仍使用同名 alias。
- 修订：legacy projection 与 predicate 统一使用 `legacy_signal_*`；`import_staging` 36/36 通过，`bill-analyser-db` 全目标通过。
- 完整 Rust 工作区覆盖率在独立空数据库执行且零失败，行覆盖率 `65.9707%`（50,267/76,196）；本切片业务改动行覆盖率 `96.43%`（27/28），严格高于 90%。
- `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、Rust-only source tree gate 与 `git diff --check` 全部通过。
- 实现 commit：`71e46f1a78cca79d39265044a6b82a1d51511611`。

完整 coverage 首次指向长期开发库时，38 个 HTTP 用例在迁移启动阶段统一报 `VersionMismatch(25)`。只读核对确认该库记录的 migration 25 checksum 与当前仓库文件不一致；没有改写该数据库的 migration ledger，也没有在其上应用 migration 26。最终门禁改用独立空数据库，确认完整迁移链为 26 后通过并删除临时库。

## Restore rehearsal

- base head：`5456e387c1acdcb1fe772c517b3c7ae36bcb89ae`；
- custom dump：`60,223,766` bytes，SHA-256 `9b3fb40d46bf1551d631b12fb560f7e0a012e3bd620019909db592910b289a2b`；
- migration SHA-256：`d439f507ea29d4e5645c2c77ab69bf6cca1e5afac08e82706d105da0b4295166`；
- 独立恢复耗时：`28,539.660 ms`；migration 耗时：`489.302 ms`；
- 源 corpus：25 migrations、237,910 preview rows、表总尺寸 492,306,432 bytes、索引 60,989,440 bytes；
- 恢复库 migration 前：25 migrations、237,910 rows、表总尺寸 470,515,712 bytes、索引 47,276,032 bytes；
- migration 后：237,910 行六个 signal 全为 `NULL`，237,910 行 version 为 `0`，未知 version 为 0；表和索引尺寸保持不变；
- version CHECK 已 validated，signal 专用索引为 0，legacy `import_preview_signal_flags` 仍存在且返回可读对象；
- 独立恢复数据库、dump、容器内 SQL 副本与 coverage 临时数据库均已删除。

机器可读证据位于 `docs/refactor/evidence/cyanflow-c5-import-signal-schema-expand-2026-08-18.json`。
