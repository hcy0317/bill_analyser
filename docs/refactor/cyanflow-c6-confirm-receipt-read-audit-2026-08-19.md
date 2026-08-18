# Cyanflow C6d confirm receipt read shadow audit

日期：2026-08-19

状态：本地实现、TDD、安全复核、双轴审查与完整审计门禁完成；尚未对生产数据库执行目标 corpus 审计，terminal replay reader 未切换

范围：提供全目标、单快照、只读、失败关闭的 metadata receipt 与 typed receipt parity 审计；不回填数据、不运行 migration、不改变公开 API、session/confirm 合同、账单/history effect 或 replay reader

## 裁决

C6d 不切换读取权威，也不增加任何生产 writer。`audit_import_confirm_receipt_target_snapshot` 在一个 `REPEATABLE READ, READ ONLY` SQLx transaction 中建立一致快照，以 `confirmed` 且持有 `metadata.confirm_receipt` 的 session 作为目标集合，按 `session_id` 升序以最大 10,000 行的有界批次扫描完整集合。审计无 checkpoint/resume 语义：一次报告只代表同一个数据库快照，失败后应从新快照完整重跑。

metadata 继续通过 C6c 已使用的 canonical projection parser 解析；typed row 与 metadata 逐字段比较 `receipt_schema_version`、`command_fingerprint`、`request_session_version`、`response_schema_version`、`http_status` 和 `success_envelope`，不比较 operational `created_at`。报告只包含目标数量、typed 总数、目标已物化、缺失、意外 typed、字段 mismatch、批次水位、migration version/count、snapshot token 和耗时；不返回 receipt 内容、fingerprint、success envelope、用户或交易字段。无效 metadata、migration ledger 漂移、非法批次或任何 parity drift 均失败关闭。

## Transition ledger

- production writer：仍只有现有 confirm transaction writer；新确认同事务写 metadata 与 typed receipt。
- operational writer：C6c backfill 保持独立；C6d 没有写路径，也不运行 migration。
- read：terminal replay 仍只读 `import_sessions.metadata.confirm_receipt`；typed row 不参与 fingerprint、响应或冲突决策。
- target universe：`status = confirmed` 且 metadata 包含 `confirm_receipt` 的全部 session。
- snapshot：单个 `REPEATABLE READ, READ ONLY` transaction；所有批次和 typed 总量统计属于同一快照。
- batching：按 `session_id` 严格递增；默认 1,000、最大 10,000；不使用 checkpoint、resume 或 `SKIP LOCKED`。
- parity：区分 missing、unexpected typed 与六字段 mismatch；`created_at` 不属于协议 parity。
- observability：CLI 只输出一个安全 JSON report，drift 时返回非零退出码；数据库 URL 和密码片段经过脱敏。
- production execution：本切片没有连接生产数据库，也不声称目标历史 corpus 已回填或 parity 已归零。
- next：C6e 只有在目标库完成 C6c 回填且 C6d 全量报告 `eligible_for_typed_read_cutover = true` 后，才可单独评估 typed replay read cutover。

## TDD、审查与安全边界

RED 首先让真实 PostgreSQL target 引用不存在的 `audit_import_confirm_receipt_target_snapshot`，随后让 CLI 测试引用不存在的 binary；两个失败都先于实现。GREEN 的 5 个真实 PostgreSQL 场景覆盖跨两个小批次的完整匹配、missing/mismatch/unexpected 分类、help 无数据库访问、无效 metadata/ledger/batch 失败关闭，以及 CLI 成功、drift 非零和缺失环境合同。4 个 CLI unit 覆盖参数边界、help 与数据库 URL/密码脱敏。

Cyaness `code-review` 使用固定比较点 `2bdfbb0d894640e3a3e3aff886b4eb901002a3f2`。Standards 轴发现 migration ledger validator 仍由 signal audit 模块持有，且首版 C6d 用版本连续性推导 migration count；修订后通用 ledger/count helper 下沉到共享 `audit_support`，C5 signal audit 与 C6d 共同复用，C6d 直接比较 embedded manifest 的精确 version/count。Spec 轴确认单快照、全目标、有界、只读、无敏感输出、无 typed read cutover 和无生产写入均符合冻结合同。内部 security-review 未被当前 SkillTree 选中，按 Cyanflow 失败关闭记录后使用仓库共享 security-review 清单复核：SQL 全部参数化，批次上限先校验，事务显式只读，CLI 只从环境读取数据库 URL，报告与错误不泄露 fingerprint、receipt、success envelope、用户或交易数据。

## 验证证据

- `import_confirm_receipt_read_audit_postgres`：5 passed，0 failed，0 ignored；所需 PostgreSQL 不存在时失败关闭；
- `bill_import_confirm_receipt_read_audit` CLI unit：4 passed，0 failed；
- C6c backfill：8 passed，0 failed，0 ignored；C5 target audit：3 passed，0 failed，0 ignored；C5 performance audit：4 passed，0 failed，0 ignored；
- fresh PostgreSQL `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 35`：53,079/80,158 行，`66.22%`，migration ledger 27/27；
- 本切片业务可执行变更行覆盖率：316/332，`95.18%`；
- `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、Rust backend structure（615 files / 3 baseline）、Rust-only source tree（2,050 tracked paths）与 `git diff --check` 全部通过；
- 实现 commit：`953144ed0434acd5fbd8370d4246e3339b8425a5`。

首次完整 coverage 连接长期开发库时只发现其既有 migration 25 checksum `VersionMismatch(25)`，没有 C6d 测试失败；该库未被修改或清理。最终完整 coverage 使用 fresh 27/27 隔离数据库并通过，临时数据库随后按精确名称删除；`workspace.lcov` 已清理，既有 `coverage.json` 未修改。

机器可读证据位于 `docs/refactor/evidence/cyanflow-c6-confirm-receipt-read-audit-2026-08-19.json`。
