# Cyanflow C6i-a Transactional IdentityMap Loader

日期：2026-08-19

状态：本地实现、TDD、双轴审查与完整审计门禁完成；公开 API、schema、回执、运行配置和生产目标均未改变

范围：统一 confirm、通用 preview 与 learning 路径的 active account/category 事务内读取；不新增 port、trait、writer、事务框架、缓存、数据库迁移、读取源切换、回填、目标审计或真实 confirm 双执行

## 裁决

Cyaness 在精确 `main` 图谱上确认两份 loader 的 SQL、category type 转换和函数指纹完全相同：confirm/general preview owner 有五个直接生产调用方，learning owner 有一个直接生产调用方。相较之下，generic 与 confirm preview patch writer 仍分别承担 row CAS 与 session-level confirm 协议，不能在本切片强行合并。

C6i-a 新增中立 `identity_persistence` 子模块，唯一 `load_import_identity_maps_on_tx` 只借用调用方当前 SQLx transaction 与 `user_id`，重新读取 active accounts/categories 并返回 `ImportIdentityMaps`。单条/批量 preview 插入、通用 patch、confirm mutation、confirm plan 与 learning decision 六个生产读取点全部进入该边界；原 confirm 与 learning loader 删除。

双轴审查曾发现把 loader 放入 `identity_validation.rs` 会混合 SQL persistence 与纯校验，随后在提交前将其移入独立 `identity_persistence.rs`。`identity_validation` 最终保持无 SQLx，loader 不 begin、commit 或缓存事务。

## Transition ledger

- old owner：`confirm/persistence.rs::load_import_identity_maps_for_confirm` 与 `preview_learning_lifecycle/persistence.rs::load_import_identity_maps_in_transaction` 分别维护相同 SQL 和类型转换。
- new owner：`identity_persistence.rs::load_import_identity_maps_on_tx` 独占 user-scoped active account/category 读取；纯校验继续由 `identity_validation` 独占。
- caller：preview 单条/批量 insert、generic preview patch、confirm mutation、confirm plan、learning decision 共六处。
- transaction/CAS：helper 只借用现有 transaction；row/session CAS、父锁顺序、selection hash 与 learning lifecycle 均保持原位。
- effect/order：confirm history、bill、receipt、cleanup 顺序和 terminal receipt replay 不变。
- API/schema/config：REST、DTO、金额 minor units、schema、session/receipt、`typed_v1` 开关均未改变。
- rollback：代码回退即可恢复两个内联 loader，没有数据库或公开契约回滚。
- delete gate：架构合同要求唯一 loader、六个生产调用点、两个旧名称和旧 SQL owner 均消失，并禁止纯 identity validation 引入 SQLx。
- production execution：没有连接或修改生产数据库，没有执行 backfill、target audit、读取源切换或真实业务 confirm。

## TDD 与双轴审查

RED 架构合同首先因中立 loader 不存在而失败。GREEN 后固定唯一 persistence owner、六个生产调用点、active-only user scope SQL、borrowed transaction，以及旧 loader/旧 SQL owner 的删除。Standards 审查要求 persistence 与纯 validation 分离；Spec 审查确认实时重读、事务/CAS/effect 顺序和公开合同均未变化。最终两轴无开放 finding。

## 验证证据

- focused architecture contract：1 passed；DB import staging lib：169 passed；
- 真实 PostgreSQL `import_staging` integration：36 passed，0 failed，0 skipped；
- fresh PostgreSQL `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 35`：53,747/80,793 行，`66.52%`，退出码 0；
- 本切片业务可执行改动行覆盖率：33/33，`100%`，所有 coverage-eligible 业务文件均匹配；
- `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、Rust backend structure（620 files / 3 baseline）、Rust-only source tree（2,068 tracked paths）、backend doc map 与 staged diff check 全部通过；
- 实现 commit：`7b67bb30a694f2106f8e5e278a3793b6033da94b`。

首次完整 coverage 连接长期开发库时只发现其既有 migration 25 checksum `VersionMismatch(25)`，没有修改或清理该库。最终完整 coverage 使用精确命名的 fresh 数据库 `bill_analyser_c6i_cov_20260819_02` 并通过；该临时数据库随后确认删除。覆盖率与 changed-diff 工件位于 git ignore 范围，没有进入提交。

机器可读证据位于 `docs/refactor/evidence/cyanflow-c6-identity-map-loader-2026-08-19.json`。
