# Cyanflow C5b import schema invariants

日期：2026-08-17

状态：本地实现、恢复演练与完整审计门禁完成；等待 exact-head CI

范围：导入 staging 的数据库合法值、session/user ownership 与 decision member nullable uniqueness；不改变 API、signal 语义、writer、read path 或 confirm 行为

## 裁决

C5b 通过 forward-only migration `0025_import_schema_invariants.sql` 收紧既有 PostgreSQL 结构：

- `import_sessions` 增加 `(id, user_id)` 唯一约束，作为子表复合外键目标；
- source、standard row、preview row、decision group、history materialization 和 confirm operation 增加 `(session_id, user_id)` 复合外键；
- session/import mode、preview operation、decision group、confirm operation 与 learning lifecycle 增加有限值 CHECK；
- decision member 保留合法 multi-ref，不增加“恰好一个引用非空”约束；
- preview、standard、history 三类非空引用分别通过 partial unique index 约束同一 group/member role 内唯一。

所有 CHECK 与复合外键先 `NOT VALID` 添加，让新写入立即 fail-closed，再显式 `VALIDATE CONSTRAINT`。migration 不做删除、映射、回填或 signal 状态复制。

learning lifecycle 的有限集合同时保留 core 阈值状态 `yellow/green/auto_applied/downgraded/suppressed`、Stage 2 兼容状态 `pending/accepted` 与规则中心启停状态 `disabled`。这是当前同表多消费者合同，不把仅在 preview evidence 中使用的任意 review 文本放入数据库状态集合。

## Transition ledger

- writer：现有 Rust repository writer 不变；数据库只校验写入是否满足 ownership 与合法值。
- read：现有查询与 API DTO 不变；没有新 read path。
- historical data：未知值或 ownership mismatch 会使 validation 失败并整体回滚，不自动改写历史数据。
- rollback：migration 未提交时由 SQLx 事务整体回滚；已上线后只允许 forward-fix，不删除已经保护新写入的数据约束。
- next slice：C5c 只能增加 nullable signal columns 与 projection version；不得在本切片引入第二套 signal writer。

## TDD 证据

- 红灯：manifest 合同显示 `left: 24, right: 25`；真实 PostgreSQL 查询目标约束显示 `left: 0, right: 14`，环境变量缺失时测试直接失败而非 skip。
- 绿灯：manifest、migration 文本和 embedded migration 合同全部通过。
- 真实 PostgreSQL：1 个集成测试通过，覆盖 8 个 CHECK 的 `23514`、6 个跨用户 session 关系的 `23503`、3 类 member partial uniqueness 的 `23505`，并证明合法 multi-ref member 可写入。
- 回归红灯：首次完整 DB 测试有 2 个 Stage 2 场景因合法 `accepted` 被过窄 CHECK 拒绝；修订后为全部 8 个 learning lifecycle 合法值增加正向数据库覆盖。
- `bill-analyser-db` 全目标、`cargo fmt --all -- --check` 与 `cargo clippy --workspace --all-targets -- -D warnings` 全部通过。
- 真实 PostgreSQL 完整 Rust workspace coverage 零失败，行覆盖率 `65.96%`；staged Rust 改动行覆盖率 `94.12%`（32/34），严格高于 90%。
- 实现 commit：`b2e678fe1f60dcbddbeeb920f65d6c6018524d53`。

## Restore rehearsal

- base head：`93cdff93183f73fe64a83c63b2294fd4940e4eab`；
- final custom dump：`60,209,715` bytes，SHA-256 `c890db568ceee62f25baf21aa98b7fe3f2e3a05502f2636d59c397f95d242e09`；
- final migration SHA-256：`bd4fd45f08d6a1ddc2322e3988c7052c904550b4d0a82fb789407ea988d26396`；
- 独立恢复耗时：`21,306.272 ms`；migration 耗时：`613.908 ms`；
- corpus：1,235 users、1,242 sessions、237,870 preview rows、5,043 decision groups、9,098 decision members；
- 验证结果：15 个目标约束全部 validated、3 个 partial index 存在、ownership mismatch 为 0、4,816 条 multi-ref member 保留；
- 独立恢复数据库、dump 与容器内 migration 副本已删除。

机器可读证据位于 `docs/refactor/evidence/cyanflow-c5-import-schema-invariants-2026-08-17.json`。
