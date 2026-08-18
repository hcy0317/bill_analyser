# Cyanflow C6e-a confirm receipt typed reader seam

日期：2026-08-19

状态：本地实现、TDD、安全复核、双轴审查与完整审计门禁完成；metadata 仍为默认读取权威，尚未对生产数据库执行回填、全量审计或 typed read rollout

范围：在现有唯一 confirm repository port 上增加可回滚的 terminal receipt read source；不新增 schema、writer、业务 effect、公开 HTTP payload 或真实 confirm 双执行

## 裁决

C6e-a 只建立 dormant cutover seam，不宣称完成 typed read cutover。`ConfirmReceiptReadSource` 固定为 `Metadata | TypedV1`；既有 `confirm_import_command` 始终委托 metadata，HTTP 启动配置 `BILL_ANALYSER_IMPORT_CONFIRM_RECEIPT_READ_SOURCE` 缺失时也固定为 metadata。只有显式值 `typed_v1` 才选择 typed reader，任何未知值在服务监听前失败。

terminal session 仍先在唯一 SQLx transaction 中按 user scope 锁定。metadata 模式复用现有 canonical receipt parser；typed 模式在同一 transaction 内以 `(session_id, user_id)` 读取不可变 typed row，并复用 receipt schema validation。typed row 缺失、SQL/映射失败或协议版本不支持时直接失败，不读取 metadata 兜底。active session 的新确认不受 reader 选择影响，仍由 `persist_confirm_receipt` 在同一业务事务中写 metadata + typed 两个投影，再执行 staging cleanup 和 commit。

## Transition ledger

- default reader：metadata；现有 repository compatibility entry 与 HTTP 缺省配置保持不变。
- dormant reader：typed_v1；只影响 terminal replay，不参与 active confirm 的计划或 effect。
- writer：仍只有 `confirm_import_command_in_transaction -> persist_confirm_receipt`。
- fallback：typed_v1 为严格模式，missing/invalid typed projection 不回退 metadata。
- authorization：session 先按 `session_key + user_id` 锁定，typed row 再按内部 `session_id + user_id` 查询。
- configuration：只接受空值/`metadata`/`typed_v1` 白名单；读取源以低敏感标签进入 tracing，不记录 receipt、fingerprint 或 envelope。
- rollback：在 metadata 投影仍保留且双写持续期间，运行配置可回到 metadata；已提交的 confirm effect 不会重放或回滚。
- production execution：本切片没有连接生产数据库，也没有执行 C6c backfill 或 C6d target audit。
- activation gate：目标库先完成 C6c，再取得 C6d `eligible_for_typed_read_cutover = true` 的同目标全量报告；未满足前不得把公开配置设为 typed_v1。

## TDD、审查与安全边界

RED 先让 HTTP 配置合同引用不存在的 `confirm_receipt_read_source`/错误类型，并让真实 PostgreSQL target 引用不存在的 `confirm_import_command_with_receipt_read_source`/`ConfirmReceiptReadSource`。GREEN 后，配置测试锁定 metadata 默认、typed_v1 显式选择和未知值启动失败；真实 PostgreSQL 测试先通过唯一生产 writer 生成双投影，证明 typed_v1 可回放，然后删除 typed test projection，证明 typed_v1 严格失败而默认 metadata 仍可回放。

Cyaness `code-review` 使用固定比较点 `7507a37ecf6d7ef1ad047cd385396bffc1059f37`。Standards 与 Spec 两轴均无阻断 finding：读取源位于 repository port、HTTP 只传配置、active writer 未分叉、typed 查询参数化且 user scoped、默认值未提前激活。仓库 security-review 清单复核无新增凭据、请求输入、动态 SQL 或敏感日志；剩余风险仅为外部 operator 在缺少目标证据时错误启用 typed_v1，因此文档与运行合同都把目标回填/全量审计作为开放 rollout 门禁。

## 验证证据

- `import_confirm_receipt_typed_read_postgres`：1 passed，0 failed，0 skipped；
- C6c backfill：8 passed；C6d read audit：5 passed；既有复杂 confirm transaction 回归：1 passed；HTTP 配置与 handler seam：各 1 passed；
- fresh PostgreSQL `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 35`：53,135/80,254 行，`66.21%`；
- 本切片业务可执行改动行覆盖率：92/95，`96.84%`；
- `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、Rust backend structure（615 files / 3 baseline）、Rust-only source tree（2,052 tracked paths）、backend doc map 与 `git diff --check` 全部通过；
- 实现 commit：`c8191c9f4816556039c4371eec5383fd160b06ee`。

首次完整 coverage 连接长期开发库时只发现其既有 migration 25 checksum `VersionMismatch(25)`，未修改该库。最终完整 coverage 使用精确命名的 fresh 27/27 数据库 `bill_analyser_c6e_cov_20260819_01` 并通过；该临时数据库随后删除，`workspace.lcov` 在覆盖率核算后清理。

机器可读证据位于 `docs/refactor/evidence/cyanflow-c6-confirm-receipt-typed-reader-seam-2026-08-19.json`。
