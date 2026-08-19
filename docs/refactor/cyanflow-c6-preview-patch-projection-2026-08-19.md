# Cyanflow C6h-a PreviewPatchProjection

日期：2026-08-19

状态：本地实现、TDD、双轴审查与完整审计门禁完成；公开 API、schema、回执、运行配置和生产目标均未改变

范围：统一 generic preview mutation、confirm patch 与 learning decision 的纯 patch 投影语义；不新增 port、trait、writer、事务框架、数据库迁移、读取源切换、回填、目标审计或真实 confirm 双执行

## 裁决

Cyaness 三路只读设计探索一致否决新增 `ConfirmTransactionExecutor`、`ConfirmCommitPort` 或通用 `UnitOfWork`：confirm 只有一个真实执行器，抽象没有第二个生产实现。证据最强的真实漂移位于 preview patch 映射：generic 路径负责 manual ownership，confirm 与 learning 路径各自循环映射字段，三条生产调用路径可能对同一 patch 产生不同 effective row 与 signal projection。

C6h-a 新增纯 `project_preview_patch` 边界，返回 `PreviewPatchProjection { preview, payload, signal_projection, amount_cents, direction }`。该边界统一拥有 21 个 patch 字段的字段/值组合校验、clear 语义、preview/payload 映射、manual ownership、active 分类/账户身份校验、金额绝对值与方向及 signal projection。非法字段和值组合立即返回 `InvalidOperation`，调用方无法取得或持久化部分投影。

三个适配器只共享这套纯语义，不共享事务协议。generic 路径继续拥有 expected row version CAS；confirm 路径继续受 session lock、session CAS 和既有 effect 顺序约束；learning 路径继续拥有 lifecycle transition 与 row CAS。现有 query builder 仍是唯一 SQL row writer，遥测标签分别保持 `preview_patch`、`confirm_preview_patch` 和 `learning_decision`。

## Transition ledger

- old owner：generic helper、confirm patch 与 learning decision 分别拥有字段循环及部分 manual/signal 语义。
- new owner：`PreviewPatchProjection` 独占纯字段映射、ownership、身份校验和 signal projection；调用适配器只负责加载、并发、事务、SQL 与遥测。
- writer：现有共享 SQL update builder；生产 writer 数量没有增加。
- CAS/lock：generic row CAS、confirm session lock/CAS、learning lifecycle/row CAS 均保持各自边界。
- API/schema/session：公开 DTO、REST、金额 minor units、schema、session contract、receipt 与 fingerprint 均未改变。
- compatibility：没有新旧 writer 双写或读取 flag；现有 metadata/`typed_v1` receipt 兼容门禁不变。
- rollback：回退本切片实现即可恢复各适配器的内联映射，无数据库或公开契约回滚。
- delete gate：架构治理测试要求三个适配器调用共享 kernel，并禁止 confirm/learning 重新引入直接 patch 循环；旧 receipt adapter 的删除门禁不在本切片范围。
- production execution：没有连接或修改生产数据库，没有执行 backfill、target audit、读取源切换或真实业务 confirm。

## TDD 与双轴审查

RED 先让测试引用不存在的 `project_preview_patch`，随后固定非法 `Date + Bool` 必须失败关闭，再用 source architecture contract 要求 generic、confirm 和 learning 三个生产适配器全部进入共享 kernel。GREEN 后覆盖 21 个字段的 canonical value、组合式 manual identity + signal clear、非法字段/值、三个 adapter 调用关系与纯模块不得依赖 SQLx/QueryBuilder/execute/await。

Cyaness `code-review` 使用固定比较点 `45126f4389c8f0a3bc0cc9001b8b7568a1c23142`。Standards 与 Spec 两轴均无 finding：纯模块不获取锁、不执行 I/O，不创建新 port；三个 adapter 的 CAS、事务、SQL writer、遥测标签与 confirm effect order 保持原位。金额改动只沿用既有 `amount.abs()`/direction 投影，没有改变元/分边界。

## 验证证据

- focused projection unit：7 passed；DB import staging lib：102 passed；
- 真实 PostgreSQL confirm patch/replay、preview rollback、row-version CAS、learning/LLM lifecycle：各 1 passed；完整 `import_staging` integration：36 passed，0 failed，0 skipped；
- fresh PostgreSQL `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 35`：51,761/76,856 行，`67.35%`，退出码 0；
- 本切片业务可执行改动行覆盖率：281/289，`97.23%`；三个调用适配器均为 100%，新纯投影模块为 `96.97%`；
- `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、Rust backend structure（619 files / 3 baseline）、Rust-only source tree（2,063 tracked paths）、backend doc map 与 staged diff check 全部通过；
- 实现 commit：`7b36224e81bea9ab3cf4475a2a4bad090f3893c1`。

最终完整 coverage 使用精确命名的 fresh 数据库 `bill_analyser_c6h_cov_20260819_01` 并通过；该临时数据库随后确认删除。覆盖率与 changed-diff 工件位于 git ignore 范围，没有进入提交。没有连接或修改生产数据库。

机器可读证据位于 `docs/refactor/evidence/cyanflow-c6-preview-patch-projection-2026-08-19.json`。
