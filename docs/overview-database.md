# 数据库运行态

数据库运行态由 `src/backend/db` 提供，正常 HTTP 业务路径只接受 PostgreSQL authority。Weaviate 是导入 learning recall 和派生向量索引的必需服务。

## Runtime Contract

- `BILL_ANALYSER_DATABASE_BACKEND=postgres`
- `BILL_ANALYSER_POSTGRES_URL` 必须可连接
- `BILL_ANALYSER_WEAVIATE_ENABLED=true`
- `BILL_ANALYSER_WEAVIATE_ENDPOINT` 必须 ready

`/api/health` 在 PostgreSQL 可达且 Weaviate ready 时返回 `ok`。PostgreSQL URL 或 Weaviate 不满足时，health 明确报告 `unhealthy`。

## Repository Surface

当前 repository 覆盖 auth、user data、accounts、categories、tags、templates、category rules、account rules、bills、budgets、statistics、matching、recurring、backup metadata、import staging、LLM/OCR settings 与 vector outbox。

账号、分类、规则、导入 staging、正式账单、预算快照和备份元数据都按 `user_id` 过滤。金额在 PostgreSQL 中使用 minor units 字段，响应前按 API 合同转换。

## Data Lifecycle

Schema scripts 位于 `src/backend/db/postgres/migrations`，由 SQLx migrator 应用到 PostgreSQL。业务数据只通过当前 repository 写入；route handler 不直接构造跨表事务。
