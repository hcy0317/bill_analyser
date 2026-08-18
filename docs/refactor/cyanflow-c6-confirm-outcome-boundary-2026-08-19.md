# Cyanflow C6f-a typed ConfirmOutcome boundary

日期：2026-08-19

状态：本地实现、TDD、安全复核、双轴审查与完整审计门禁完成；公开 API、持久回执格式和生产读取配置均未改变

范围：净化现有 confirm repository outward boundary；不新增 schema、writer、业务 effect、运行配置、回填、目标审计或真实 confirm 双执行

## 裁决

C6f-a 将首次确认和 terminal replay 的 repository 返回值统一为 transport-neutral `ConfirmOutcome { result, replayed }`。DB 不再把 HTTP status 或 success envelope 暴露给调用方；现有 HTTP handler 根据 typed result 生成原有 `import_stage_confirm_success` 响应，因此公开 status、字段名和计数语义保持不变。

durable receipt v1 仍按既有 immutable 合同保存 HTTP 200 与 success envelope。该历史持久格式没有在本切片迁移，而是由 `confirm/receipt_compat.rs` 单点编解码：新确认只在唯一 writer 内编码一次；replay 从所选 metadata/typed 投影解码为 `ConfirmPreviewResult`，随后越过 repository 边界的只有 typed outcome。损坏或缺少 data 的回执失败关闭，不产生 fallback、第二 writer 或第二次业务 effect。

## Transition ledger

- old outward boundary：DB 返回 `ConfirmReceiptResponse { http_status, success_envelope, replayed }`，HTTP 仅透传。
- new outward boundary：DB 返回 `ConfirmOutcome { result, replayed }`，HTTP 独占公开成功 envelope 组装。
- durable format：receipt schema v1、HTTP 200、success envelope、fingerprint 与 request session version 均不变。
- read source：metadata 默认与 dormant `typed_v1` 严格读取语义不变；typed 缺失或非法仍不回退 metadata。
- writer：仍只有 `confirm_import_command_in_transaction -> persist_confirm_receipt`，同事务 metadata + typed 双投影后再 cleanup/commit。
- compatibility：`receipt_compat.rs` 是旧 envelope 的唯一运行时适配器，不是第二个公共响应 builder。
- rollback：回退本切片代码即可恢复旧 outward type；数据库 schema、receipt 数据与公开 payload 无需回滚。
- delete gate：只有目标库完成 C6c/C6d、公开 typed rollout 与 legacy session/receipt 删除门禁后，才可另行移除 v1 adapter。
- production execution：本切片没有连接生产数据库，没有执行 backfill、target audit 或读取源切换。

## TDD、审查与安全边界

RED 首先让测试引用尚不存在的 `ConfirmOutcome`；第二个 RED 让真实 PostgreSQL 合同访问 `outcome.result`，旧 `ConfirmReceiptResponse` 因没有该字段产生编译失败。GREEN 后，typed reader、metadata replay、复杂 confirm、故障注入和 HTTP compatibility 均消费 typed result；持久层断言继续固定旧 v1 envelope。覆盖率复核发现兼容解码器的损坏输入分支未执行，又补充缺少 data 与 malformed data 两条失败关闭测试。

Cyaness `code-review` 使用固定比较点 `f3bd6d88093512d14c1e6217729bbf0666998347`。Standards 与 Spec 两轴均无 finding：唯一事务 writer、读取源、user scope、receipt 格式与公开 API 都未分叉；compatibility adapter 明确受删除门禁约束。仓库 security-review 清单复核无新增凭据、动态 SQL、请求输入、端点或敏感日志。

## 验证证据

- `import_confirm_receipt_typed_read_postgres`：2 passed，0 failed，0 skipped；
- 真实 PostgreSQL confirm command：1 passed；confirm failure-injection matrix：1 passed；
- HTTP legacy confirm compatibility 与 stale session version：各 1 passed；fresh 测试库完成后删除；
- durable receipt 损坏输入：2 passed；DB 全目标编译通过；
- fresh PostgreSQL `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 35`：53,203/80,280 行，`66.27%`；
- 本切片业务可执行改动行覆盖率：60/61，`98.36%`；
- `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、Rust backend structure（615 files / 3 baseline）、Rust-only source tree（2,055 tracked paths）、backend doc map 与 `git diff --check` 全部通过；
- 实现 commit：`8dd6fca8b4d7197c34e45fc803c463270520bf38`。

最终完整 coverage 使用精确命名的 fresh 数据库 `bill_analyser_c6f_cov_20260819_03` 并通过；该临时数据库随后删除，`workspace.lcov` 在覆盖率核算后清理。长期开发库仅在早期 focused HTTP 验证中暴露其既有 migration 25 checksum `VersionMismatch(25)`，未被本切片修改；相同合同在 fresh exact 数据库上通过。

机器可读证据位于 `docs/refactor/evidence/cyanflow-c6-confirm-outcome-boundary-2026-08-19.json`。
