# 数据库与数据流

数据库运行态由 `src/backend/db` 提供，默认使用 SQLite WAL 模式。

## 核心职责

- SQLite path guard、连接初始化、foreign keys、WAL 和事务 helper。
- schema 幂等初始化与 legacy 约束补齐。
- bills、accounts、categories、tags、templates、budgets、statistics、matching、backup、auth、import staging、LLM/OCR settings 等 repository；其中 auth repository 已按 user/session/profile/cloud settings/2FA/log/row helper 拆分，auth registration 已按默认 seed/注册/分类/账户拆分，import staging repository 已按 session/template/preview/decision/LLM memory/confirm/row helper 拆分，matching repository 已按 schema/actions/candidate query/reconciliation/serialization/helper 拆分，budget repository 已按 CRUD/import/execution/history/forecast/hierarchy/row helper 拆分。
- user-scope 查询与写入。
- 导入 session/template/preview staging 只保存当前导入过程所需临时数据，preview page 查询负责按条件分页和返回轻量 facets/counts；confirm、cancel、失败后新建 session 会清理对应用户的 import staging，未完成导入不作为可续传数据保留。

## 数据流

HTTP route 解析当前用户与请求 DTO 后调用 domain runtime；domain runtime 通过 repository 层开启事务并执行读写；响应由 HTTP 层投影为前端兼容 DTO。

## 约束

- 业务写入优先单事务完成。
- 账户余额、账单、导入 confirm、预算 import、settings bundle import 等跨表行为必须保持 rollback-on-error。
- 金额字段必须明确元/分边界。
- 认证和业务审计为 best-effort，不应破坏主事务的关键业务结果。
