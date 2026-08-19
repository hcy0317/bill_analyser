# Cyanflow C6g-a pure ConfirmPlan boundary

日期：2026-08-19

状态：本地实现、TDD、双轴审查与完整审计门禁完成；公开 API、schema、持久回执、运行配置和生产目标均未改变

范围：从现有 confirm transaction orchestration 提取纯计划/write-set 边界；不新增 writer、业务 effect、数据库迁移、读取源切换、回填、目标审计或真实 confirm 双执行

## 裁决

C6g-a 将确认决策拆为两个无 SQL、无 HTTP、无持久化的纯函数。`prepare_confirm_plan` 在 preview patch 与选择集落入当前事务、selected rows 重新加载后，统一执行未知信号与 history acknowledgement 校验；事务随后重新加载当前用户 active 分类/账户 map，`build_confirm_plan` 继续执行身份与 persisted review 校验，并一次性产出 `ConfirmPlan { history_writes, bill_drafts, result }`。

`confirm_import_command_in_transaction` 不再同时拥有业务决策和写集构造，只负责锁、CAS、权威数据加载及计划执行。它仍是唯一 SQLx transaction executor 和唯一真实 writer，严格保持 history CAS、bill insert、confirm-time effect boundary、receipt persistence、staging cleanup、commit 的既有顺序。最终 `confirmed_count` 仍以实际创建账单数量加已执行 history write 数量计算；纯计划不会被第二次执行到真实业务状态。

## Transition ledger

- old boundary：混合 orchestration 在事务函数内交错完成校验、history plan、bill draft 构造和副作用执行。
- new boundary：纯 `prepare_confirm_plan`/`build_confirm_plan` 拥有校验与 write-set；orchestration 只加载权威状态并执行计划。
- validation contract：验证错误使用 `ConfirmPlanValidationKind` 枚举；日志只输出稳定 kind 和可选 count，不输出 row、receipt、fingerprint、命令或用户数据。
- writer：仍只有 `confirm_import_command_in_transaction` 所在单一 SQLx transaction；没有 shadow write、第二 writer 或真实 confirm 双执行。
- effect order：history CAS -> bill insert -> confirm-time effects -> receipt -> staging cleanup -> commit，未改变。
- compatibility：公开 HTTP payload、`ConfirmOutcome`、durable receipt v1、metadata/`typed_v1` reader、fingerprint 与 session CAS 语义均未改变。
- database：没有 schema、migration、索引、backfill、目标 audit 或生产配置变更。
- rollback：回退本切片代码即可恢复混合 orchestration；无需数据库或公开契约回滚。
- delete gate：本切片没有删除 legacy receipt adapter，也不改变 C6c/C6d 与公开 typed rollout 的既有门禁。

## TDD 与双轴审查

RED 先让单元测试引用不存在的纯计划函数，随后让 orchestration source contract 要求生产路径消费计划；GREEN 后补齐 unknown signal、history acknowledgement、identity、persisted review、history/bill 分离、orphaned history write invariant、验证日志和架构治理测试。第一次完整 diff 覆盖率只有 162/182（`89.01%`），按仓库规则判定失败；补充分支测试后最终精确 HEAD 达到 186/191（`97.38%`），其中新 `confirm/plan.rs` 为 `100%`。

Cyaness `code-review` 使用固定比较点 `24dc8eb3c875ee4bfa8d0b37ded84cc3c49ebffa`。Spec 轴无 finding；Standards 轴发现验证 kind 使用字符串，最终改为封闭枚举并让全部变体进入测试。复核后两轴均无 finding。该切片不增加端点、请求字段、动态 SQL、凭据或敏感日志，也不改变 user/session scope。

## 验证证据

- focused 纯计划与 confirm 合同测试：全部通过；
- fresh PostgreSQL `import_staging` integration：36 passed，0 failed，0 skipped；
- fresh PostgreSQL `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 35`：53,541/80,608 行，`66.42%`，退出码 0；
- 本切片业务可执行改动行覆盖率：186/191，`97.38%`；新纯计划模块 100%；
- `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、Rust backend structure（617 files / 3 baseline）、Rust-only source tree（2,059 tracked paths）、backend doc map 与 staged diff check 全部通过；
- 实现 commit：`e6686d7537ef4d04f44437ee3ee80852e4e05a97`。

最终完整 coverage 使用精确命名的 fresh 数据库 `bill_analyser_c6g_cov_20260819_03` 并通过；该临时数据库随后确认删除。覆盖率与 diff 临时文件位于 git ignore 范围，自动清理命令被本机安全层拒绝，因此未进入提交但保留为本地可复核工件。没有连接或修改生产数据库。

机器可读证据位于 `docs/refactor/evidence/cyanflow-c6-confirm-plan-boundary-2026-08-19.json`。
