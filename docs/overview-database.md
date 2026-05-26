# 数据库与数据流

数据库运行态由 `src/backend/db` 提供，默认使用 SQLite WAL 模式。迁移期同时提供 `DatabaseRuntimeProvider`，用于描述 SQLite legacy 与 PostgreSQL repository runtime 的选择；Postgres pool 采用 lazy 构造，具体业务仓储未迁移前不会静默回退或接管 route。

repository 调用路径、事务边界和 row helper 约定见 [Rust 后端导航图](backend-map.md#repository-data-flow)。本页保留数据库运行态职责摘要。

## 核心职责

- SQLite path guard、连接初始化、foreign keys、WAL、Postgres lazy pool provider 和事务 helper。
- schema 幂等初始化与 legacy 约束补齐。
- bills、accounts、account rules、categories、tags、templates、budgets、statistics、matching、backup、auth、import staging、LLM/OCR settings 等 repository；其中 auth repository 已按 user/session/profile/cloud settings/2FA/log/row helper 拆分，auth registration 已按默认 seed/注册/分类/账户拆分，import staging repository 已按 session/template/preview/decision/LLM memory/confirm/row helper 拆分，matching repository 已按 schema/actions/candidate query/reconciliation/serialization/helper 拆分，budget repository 已按 CRUD/import/execution/history/forecast/hierarchy/row helper 拆分。
- user-scope 查询与写入。
- 导入 session/source/standard-row/template/preview staging 只保存当前导入过程所需临时数据；source 与 standard row 台账记录 parser decision、标准化行和后续决策分组锚点，`import_decision_groups` / members 保存同批重复和历史重复的成员证据，`import_history_materializations` 保存历史账单预览改写 payload；stage2 写入前会清理上一轮失败遗留的 preview/materialization/decision 状态；preview page 查询负责按条件分页和返回轻量 facets/counts；confirm、cancel、失败后新建 session 会清理对应用户的 import staging，未完成导入不作为可续传数据保留。

## 数据流

HTTP route 解析当前用户与请求 DTO 后调用 domain runtime；route helper 先通过 `HttpAppState` 的 repository boundary 打开当前仓储运行时，再由 repository 层开启事务并执行读写；响应由 HTTP 层投影为前端兼容 DTO。当前默认边界是 `sqlite_legacy`，配置为 Postgres 但业务仓储尚未切换时会得到显式未接管错误。

## 约束

- 业务写入优先单事务完成。
- 账户余额、账单、导入 confirm、预算 import、settings bundle import 等跨表行为必须保持 rollback-on-error。
- 金额字段必须明确元/分边界。
- 认证和业务审计为 best-effort，不应破坏主事务的关键业务结果。
- 新增 repository 或移动 SQL helper 时，同步检查 `docs/backend-map.md` 的 source/test path index 和验证矩阵。
