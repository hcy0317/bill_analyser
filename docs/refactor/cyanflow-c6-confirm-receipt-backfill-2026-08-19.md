# Cyanflow C6c historical confirm receipt backfill

日期：2026-08-19

状态：本地实现、TDD、安全复核、双轴审查与完整审计门禁完成；尚未对生产数据库执行回填

范围：提供有界、可续跑、失败关闭的历史 metadata receipt -> typed receipt 运维回填；不切换 terminal replay reader、不改变公开 API、session/confirm 合同、账单/history effect 或 migration schema

## 裁决

C6c 不新增 confirm 实现，也不让 replay、HTTP 或后台任务隐式补写历史数据。`backfill_import_confirm_receipt_batch` 只读取已经 `confirmed` 且包含 `metadata.confirm_receipt` 的 session；当前唯一生产 confirm writer 继续由 `confirm_import_command_in_transaction -> persist_confirm_receipt` 持有。生产双投影和历史回填共同复用 `insert_confirm_receipt_projection` 的一份 typed column mapping，避免两条写入路径分别维护字段顺序与类型，但回填不创建账单、history effect、terminal 状态或 staging cleanup。

每批事务先取得固定 advisory transaction lock，再要求 `_sqlx_migrations` 成功版本与当前 embedded migration manifest 完全一致。checkpoint 使用严格递增的 `session_id`；若 `id <= after_session_id` 仍存在没有 typed receipt 的历史目标，批次失败而不是越过缺口。目标行按主键升序锁定，单批最多 1,000 个 session，不使用 `SKIP LOCKED`。

metadata receipt 通过现有 canonical parser 读取；已有 typed row 在任何写入前逐字段比较。缺失 typed row 使用同一事务补写，其 `created_at` 固定为 terminal session 的 `updated_at`。写入后重新读取整个批次，再次验证行数和字段 parity；任何无效 metadata、既有漂移、触发器篡改、插入失败或 post-insert mismatch 都回滚整批。只有 commit 成功后 CLI 才输出 JSON checkpoint。

## Transition ledger

- production writer：仍只有现有 confirm transaction writer；新确认同事务写 metadata 与 typed receipt。
- operational writer：C6c 命令只物化历史 confirmed metadata receipt，不执行 confirm effect；与生产 writer 共享 typed insert mapping。
- read：terminal replay 仍只读 metadata；typed row 不参与 fingerprint、响应或冲突决策。
- checkpoint：`after_session_id -> next_after_session_id` 只在事务 commit 后输出；默认批次 200、批间 50 ms，最大批次 1,000。
- parity：已有行 preflight 与整个已写批次 post-insert 均做完整字段比较；mismatch 失败关闭且不推进 checkpoint。
- migration：命令只接受精确 ledger，不运行 migration；部署者必须先完成正常 schema rollout。
- rollback：停止命令并从最后一次持久化的成功 checkpoint 续跑；已插入的 immutable typed row 保留，重复扫描只做 parity。
- production execution：本切片没有连接或修改生产数据库，也不声称目标历史 corpus 已完成回填。
- next：C6d 在目标库建立独立、全量、只读 snapshot shadow audit；只有 backfill 完成且累计 mismatch 为 0，C6e 才可评估 typed read cutover。

## TDD、审查与安全边界

RED 首先让真实 PostgreSQL target 引用不存在的 `backfill_import_confirm_receipt_batch`，随后让 CLI 测试引用不存在的 binary；两个失败都先于实现。GREEN 的 8 个真实 PostgreSQL 场景覆盖首次回填、续跑/幂等/空批/错误水位、无效 metadata 整批回滚、既有 typed mismatch、`BEFORE INSERT` 触发器篡改后的 post-insert parity 回滚、精确 migration ledger、CLI help/缺失环境和真实 checkpoint。

Cyaness `code-review` 使用固定比较点 `377c463439d44f65385b0edac765bb4226d21449`。Standards 轴发现生产 writer 与 backfill 首版各自维护一份 typed insert 映射；修订后两者复用私有 projection helper，且生产 writer 仍只有一个。Spec 轴确认没有 typed read、公开 API、schema 或 confirm effect 变化。内部 security-review 未被当前 SkillTree 选中，按 Cyanflow 失败关闭记录后使用仓库共享 security-review 清单复核：数据库 URL 只来自环境且错误会删除完整 URL/密码片段，SQL 全部参数化，user/session 归属由 join 与复合约束保持，错误输出不包含 receipt、fingerprint、源账单或凭据。

## 验证证据

- `import_confirm_receipt_backfill_postgres`：8 passed，0 failed，0 ignored；所需 PostgreSQL 不存在时失败关闭；
- `bill_import_confirm_receipt_backfill` CLI unit：4 passed，0 failed；
- focused receipt/replay：1 passed，0 failed，0 ignored；focused confirm failure matrix：1 passed，0 failed，0 ignored；
- receipt schema：2 passed，0 failed，0 ignored；`bill-analyser-db --all-targets` 完整通过；
- fresh PostgreSQL `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 35`：52,784/79,713 行，`66.22%`，migration ledger 27/27；
- 本切片业务可执行变更行覆盖率：424/444，`95.50%`；
- `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、Rust backend structure（610 files / 3 baseline）、Rust-only source tree（2,041 tracked paths）与 `git diff --check` 全部通过；
- 实现 commit：`dc517c006171a8cb98919297af07f5d769643b18`。

完整 coverage 使用 fresh 27/27 隔离数据库并在命令的 `finally` 清理；长期开发库与生产库均未执行历史回填。`workspace.lcov` 在覆盖率核算后清理，既有 `coverage.json` 未修改。

机器可读证据位于 `docs/refactor/evidence/cyanflow-c6-confirm-receipt-backfill-2026-08-19.json`。
