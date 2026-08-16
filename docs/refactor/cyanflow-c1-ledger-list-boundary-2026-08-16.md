# C1 Ledger 列表边界交付合同

> 状态：`IMPLEMENTED / LOCAL_AUDIT_PASS`
> 正式阶段：C1 第二项 canary；Cyaness -> Cyanflow；未使用 OMX

本 canary 只迁移正式账单列表 `GET /api/bills` 的读取边界，不改数据库 schema、公开路径、查询别名、响应 envelope 或任何 writer。它与 missing-category canary 共同构成 C1；两项均关闭后，正式下一阶段为 C2 `PreviewStateKernel`。

## 所有权变化

| 字段 | 本 canary 合同 |
| --- | --- |
| Current owner | `list_bills_handler` 同时打开 PostgreSQL runtime、解析筛选、调用 query、读取 `BillRecord` 并组织逐行标签 presenter |
| Target owner | `HTTP mapper -> PostgresLedgerQueries::list(principal, LedgerListQuery) -> LedgerEntryPage -> HTTP mapper` |
| Old read | handler -> runtime/pool -> category lookup -> bill query -> per-row tag query -> frontend presenter |
| New read | handler 只完成认证和 query/response 映射；repository 在 user scope 内完成筛选、分页、数据库行映射与当前页标签批量聚合 |
| Writer | 无；本 canary 是只读，不新增 repository writer、trait 或持久状态 |
| Compatibility | `/api/bills`、query aliases、分页 500 上限、排序、整数分金额、交易类型编码、`success/result` 与分页别名保持不变 |
| Rollback | 仅恢复旧 list caller；无 schema 或数据回滚 |
| Delete gate | list handler 不再出现 runtime、pool、SQL query、`BillRecord` 或旧 DB-backed presenter；真实 PostgreSQL 与 REST fixture 均通过且 0 skip |

## 模块边界

- Core 的 `LedgerListQuery`、`LedgerEntry` 和 `LedgerEntryPage` 只表达账本读取语义；金额统一使用 `Money`（整数分），不持有 SQL、连接池或 HTTP JSON。
- DB 的 `PostgresLedgerQueries` 是当前实现 owner。它显式接收 `UserId`，分类 ID 查找、所有筛选和标签查询都受同一 user scope 约束。
- 当前页标签改为一次 `ANY($2)` 批量查询，删除列表路径的 N+1 查询；详情和按月接口仍沿用兼容路径，不在本切片冒进迁移。
- HTTP 只把既有 query aliases 归一到 typed query，并把 typed page 投影回既有 envelope；错误转换保留在边界层。
- `TransactionType::from_backend_name` 收回既有中英文/数字存储值解析，避免 adapter 与 repository 复制同一业务枚举规则。

## TDD 与行为证据

- Red：引入 typed query/repository/typed tag 的测试时，旧代码无法解析 `LedgerListQuery`、`PostgresLedgerQueries` 和 `LedgerTag`。
- Repository：真实 PostgreSQL list 用例验证 user isolation、分类/标签/账户筛选、金额分、分页 clamp 与空结果；必需数据库缺失时 fail-closed。
- Route：真实 Axum `GET /api/bills` 用例验证公开 envelope、分页别名、金额分、标签及跨用户隔离，1/1 执行、0 skip。
- Unit/delete test：query alias 映射、typed presenter 以及 handler source boundary 均通过；source boundary 明确禁止 runtime/pool/SQL row 回流。
- 完整 Rust workspace coverage 命令通过 `--fail-under-lines 35`；本切片业务改动行覆盖 304/314，96.82%。
- `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、Rust structure/route ownership 与 Gitea workflow 静态门禁通过。

## 覆盖率门禁修正

审计发现 LCOV 会为同一 Rust 源文件输出多个 `SF:` record。旧 normalizer 以后一个 record 覆盖前一个，导致已执行异步行被误报为未覆盖。修正后按源文件和行号合并命中次数，并用重复 `SF:` fixture 锁定行为；没有降低阈值、排除业务文件或使用其他测试总数冒充目标覆盖率。

## C1 Exit

missing-category 与 Ledger list 两项 canary 均已有独立 rollback、删除门禁和真实运行证据。C1 在本切片 PR 合并且 CI 全绿后关闭；下一切片按 Cyanflow 重新进入 C2 `PreviewStateKernel`，先冻结六类 signal、issue、unknown-state 与 likely-transfer 双 membership fixture，再建立 canonical projector，不提前切换 SQL reader。
