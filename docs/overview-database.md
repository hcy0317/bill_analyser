# 数据库与数据流

数据库运行态由 `src/backend/db` 提供，正常 HTTP 业务运行态目标是 PostgreSQL authority + 必需 Weaviate。SQLite runtime 只保留为 legacy 迁移输入、隔离测试 fixture 或显式 migration tooling；不再作为 `/api/...` 业务 fallback。迁移期同时提供 `DatabaseRuntimeProvider`，用于描述 SQLite legacy 与 PostgreSQL repository runtime 的选择；Postgres pool 采用 lazy 构造，具体业务仓储未迁移前不会静默回退或接管 route。默认 `BILL_ANALYSER_DATABASE_BACKEND=postgres` 且 `BILL_ANALYSER_REQUIRE_POSTGRES_AFTER_CUTOVER=true`；health 会把未满足的 Postgres 权威条件、尚未接管的 Postgres 仓储或未 ready 的 Weaviate 标为 unhealthy。

repository 调用路径、事务边界和 row helper 约定见 [Rust 后端导航图](backend-map.md#repository-data-flow)。本页保留数据库运行态职责摘要。

## 核心职责

- SQLite path guard、连接初始化、foreign keys、WAL、Postgres lazy pool provider 和事务 helper。
- schema 幂等初始化与 legacy 约束补齐。
- bills、accounts、account rules、categories、tags、templates、budgets、statistics、matching、backup、auth、import staging、LLM/OCR settings、vector outbox 等 repository；其中 auth repository 已按 user/session/profile/cloud settings/2FA/log/row helper 拆分，auth registration 已按默认 seed/注册/分类/账户拆分，import staging repository 已按 session/template/preview/decision/LLM memory/confirm/row helper 拆分，matching repository 已按 schema/actions/candidate query/reconciliation/serialization/helper 拆分，budget repository 已按 CRUD/import/execution/history/forecast/hierarchy/row helper 拆分。
- user-scope 查询与写入。
- 导入 session/source/standard-row/template/preview staging 只保存当前导入过程所需临时数据；source 与 standard row 台账记录 parser decision、标准化行和后续决策分组锚点，`import_decision_groups` / members 保存同批重复、同批转账、历史重复和历史转账的成员证据，`import_history_materializations` 保存历史账单预览改写/合并 payload；stage2 写入前会清理上一轮失败遗留的 preview/materialization/decision 状态；preview page 查询负责按条件分页和返回轻量 facets/counts；confirm、cancel、失败后新建 session 会清理对应用户的 import staging，未完成导入不作为可续传数据保留。
- `vector_outbox_events` 是 PostgreSQL 权威 outbox，用于把学习样本/特征变化投递给必需 Weaviate 派生索引；claim 使用短事务和 `FOR UPDATE SKIP LOCKED`，失败按 attempts/backoff 回到 pending 或 failed，不影响业务表。

## 数据流

HTTP route 解析当前用户与请求 DTO 后调用 domain runtime；route helper 先通过 `HttpAppState` 的 repository boundary 打开当前仓储运行时，Postgres 路径复用 `HttpAppState` 中按需初始化并缓存的 shared lazy pool，再由 repository 层开启事务并执行读写；响应由 HTTP 层投影为前端兼容 DTO。当前默认边界是 `postgres_authority`，表示正常 HTTP 仓储已以 PostgreSQL 为权威，`/api/health` 在 Postgres URL configured 且 Weaviate ready probe 为 `healthy` 时返回 `ok`；backend/URL 不满足时边界为 `postgres_required_after_cutover`；直接 env 配置 SQLite 会得到 `sqlite_legacy_disabled` 并被拒绝；`sqlite_legacy` 只用于隔离测试 fixture 的显式 test-only 访问，不允许让正常 HTTP health 变成 `ok`。Postgres 已接管的读取面包含登录、注册、邮箱验证、密码找回/重置、refresh token、token session 管理、2FA status/verify/enable/disable/recovery、Profile GET/更新/头像/cloud settings/external auth、user-data 统计/导出/清空、主数据列表、交易列表/按月列表/详情/导出、对账单汇总、周期模板候选/绑定/解绑、统计金额概览、分类统计/饼图/商户排行、matching pairs、预算列表和备份任务/记录元数据；账户 CRUD/排序直接写 PostgreSQL authoritative `accounts` 表，并把前端分单位金额转成 `balance_cents`；分类 CRUD、批量创建、导入、导出、排序、统计、quick-add 和账单 refresh 直接读写 PostgreSQL authoritative `categories` / `category_rules` / `bills` 表；分类规则与账户规则的列表、CRUD、重排、测试、账户别名迁移和 legacy 分类规则配置缓存直接读写 PostgreSQL authoritative `category_rules` / `account_rules` / `settings`；标签 CRUD/批量创建/排序与模板 CRUD/排序也已直接写 PostgreSQL authoritative `tags` / `transaction_templates` 表，周期模板匹配使用 `transaction_templates(template_type=2)` 与 `bills.standard_payload.created_from_recurring`；预算 CRUD、导出、导入、执行、预测、历史读取和历史快照也已走 PostgreSQL authoritative `budgets` / `budget_history` 表，并保持主/子预算、月/季/年父周期同步、账户/标签筛选和历史快照 upsert 语义；backup ops 的 zip/Fernet 文件 I/O 仍在 Rust HTTP 层，记录、任务和审计元数据写入 PostgreSQL authoritative `backup_records` / `backup_jobs` / `backup_audit_logs`；账单读取按 user_id、is_deleted、日期、类型、账户、分类、标签、关键字和金额过滤，并把 Postgres 分单位金额显式转换回前端分字段；手工账单新增、更新、删除、批量新增、批量更新、批量删除和 legacy modify/delete 兼容入口直接写 PostgreSQL authoritative `bills` / `bill_tags`，按收入、支出、转账/投资语义增量同步相关 `accounts.balance_cents`。任何正常 HTTP 业务路径若尝试打开 SQLite runtime，repository boundary 都会以 503 fail-closed 拒绝，不能静默回退。user-data 清空在 PostgreSQL 事务内删除当前用户业务表数据，保留用户、settings 与审计台账，并通过 `business_audit_events` 记录操作结果；auth token、2FA recovery code 和外部登录绑定分别由 `token_sessions`、`user_two_factor_recovery_codes` 和 `user_external_auths` 管理。模板金额列由追加迁移收敛为精确整数 minor units，避免在 Postgres authoritative 表中保存浮点金额。

## 约束

- 业务写入优先单事务完成。
- 账户余额、账单、导入 confirm、预算 import、settings bundle import 等跨表行为必须保持 rollback-on-error。
- 金额字段必须明确元/分边界。
- 认证和业务审计为 best-effort，不应破坏主事务的关键业务结果。
- 新增 repository 或移动 SQL helper 时，同步检查 `docs/backend-map.md` 的 source/test path index 和验证矩阵。
