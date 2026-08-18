# Cyanflow C6b typed confirm receipt dual projection

日期：2026-08-18

状态：已由 PR #326 squash merge；exact-head CI run `16206` 的 backend、frontend、governance、E2E 全部通过，main 为 `377c463439d44f65385b0edac765bb4226d21449`

范围：只让现有唯一 confirm transaction writer 同步持久化 metadata 与 typed receipt；不切换 replay reader、不 backfill 历史 session、不改变公开 API、账单/history effect 或 session version 合同

## 裁决

C6b 保留 `confirm_import_command_in_transaction -> persist_confirm_receipt` 这一条生产写链，不新增 repository port、后台 writer 或第二套 confirm 实现。新确认在同一个 SQLx transaction 内依次完成账单/history effect、terminal metadata receipt、typed receipt、staging 子表清理和 commit。typed insert 或后续 cleanup 任一失败都会回滚整笔事务，不会留下单边 receipt、正式账单、history effect 或 terminal session。

`StoredConfirmReceipt` 的 receipt/response schema version 与 HTTP status 使用和 PostgreSQL `SMALLINT` 一致的内部类型；metadata JSON 与 typed row 从同一个对象投影，session CAS 也直接读取该对象内的 `request_session_version`，避免同一写点出现两个可漂移 token。

## Transition ledger

- writer：唯一 confirm transaction writer 同事务写 metadata 和 typed receipt；typed 表没有独立 writer。
- read：terminal replay 继续只读 `import_sessions.metadata.confirm_receipt`；typed row 不参与响应或 fingerprint 决策。
- compatibility：迁移前的 metadata-only terminal session 继续可重放，重放只读且不会隐式创建 typed row。
- failure boundary：metadata update、typed insert、child cleanup 与所有业务 effect 共用一个 transaction；任一步失败全部回滚。
- backfill：无；历史 terminal session 的 typed row 由后续独立切片处理。
- rollback：合并前可回退 binary；部署后保留 additive 表与 metadata replay，使用 forward-fix，不删除 immutable terminal row。
- next：在不打开 typed read 的前提下，对历史 metadata receipt 做有界、可恢复 backfill，并建立 metadata/typed shadow parity。

## TDD 与双轴审查

配置真实 PostgreSQL 后，RED 用现有 terminal receipt/replay 场景读取 typed row，得到 `RowNotFound`，证明 C6a 只有 schema、没有生产 writer。GREEN 只改现有 persistence seam；同一场景随后证明 metadata 与 typed 的 version、fingerprint、request version、HTTP status 和 success envelope 完全一致，连续 replay 始终只有一个 typed row。测试再删除 typed row 模拟历史 metadata-only session，证明 replay 仍成功且不会自动 backfill。

故障矩阵新增 typed receipt `BEFORE INSERT` 注入点，并把 typed rows 纳入事务快照。typed insert 失败和 typed insert 后的 child cleanup 失败均证明正式账单、history effect、metadata receipt、typed receipt、session 状态与 staging 子表整体恢复且可重试。

Cyaness `code-review` 使用固定比较点 `c8b7405b6264eb7fc1d5c300ca91fc31cde45273`。Standards 轴首次发现 `request_session_version` 同时作为函数参数与 receipt 字段传入，存在未来双投影漂移风险；修订为 metadata、typed 与 CAS 共用 receipt 内唯一 token 后，Standards 与 Spec 两轴均通过。安全复核确认 SQL 全部参数化、typed row 由 user/session 复合外键归属，且没有持久化原始命令、acknowledgement token、凭据或源账单正文。

## 验证证据

- focused receipt/replay：1 passed，0 failed，0 ignored；
- focused confirm failure matrix：1 passed，0 failed，0 ignored；
- `import_staging`：36 passed，0 failed，0 ignored；
- receipt schema：2 passed，0 failed，0 ignored；
- `bill-analyser-db --all-targets`：完整通过；
- fresh PostgreSQL `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 35`：52,338/79,037 行，`66.22%`，migration ledger 27/27；
- 变更业务代码覆盖率：30/30 个可执行变更行，`100%`；
- `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、Rust backend structure、Rust-only source tree 与 `git diff --check` 全部通过；
- 实现 commit：`9b1ed88277b25e0ebafe724dba79c8cd5553f8ac`。
- Gitea：PR #326 已 squash merge；exact-head CI run `16206`，backend job `19240`、frontend job `19241`、governance job `19242`、E2E job `19243` 全部成功；来源分支已删除。

长期开发库仍按既有合同报告历史 migration 25 `VersionMismatch(25)`；C6b 没有修改其 ledger。完整 coverage 改用 fresh 27/27 database 并通过。RED 阶段遗留的 2 个隔离 test database 与最终 coverage database 均按精确名称删除，`workspace.lcov` 已清理，既有 `coverage.json` 未修改。

机器可读证据位于 `docs/refactor/evidence/cyanflow-c6-confirm-receipt-dual-write-2026-08-18.json`。
