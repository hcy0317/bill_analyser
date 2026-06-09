# Rust 后端导航图

Canonical source: docs/backend-map.md

本文是 Bill Analyser Rust 后端的维护入口。它描述当前运行态事实：`bill_http_server` 是唯一 HTTP 服务入口，业务 API 通过 Rust Axum `REST /api/...` 进入后端，数据库访问由 Rust repository/runtime 层承接，PostgreSQL 是唯一业务数据库，Weaviate 是导入 learning recall 与派生向量索引的必需服务。`docs/backend-map.html` 只是静态阅读入口，不是第二份事实来源。

<a id="backend-layering"></a>
## 后端分层

```mermaid
flowchart LR
  Web["Vue / TypeScript 前端"] --> Http["src/backend/http\nAxum routes + auth + envelopes"]
  Http --> Core["src/backend/core\n业务合同 + 金额/时间/导入/匹配/预算/统计"]
  Http --> Db["src/backend/db\nrepository runtime provider + transaction + user scope"]
  Core --> Parsers["src/backend/parsers\n账单解析 + RawBill/StandardBill"]
  Db --> Postgres["PostgreSQL migrations + SQLx repository runtime"]
  Http --> Weaviate["Weaviate required derived index"]
  Db --> Outbox["vector_outbox_events"]
  Outbox --> Weaviate
```

| 层 | 入口 | 责任 | 不能做 |
| --- | --- | --- | --- |
| HTTP | `src/backend/http/lib.rs`, `router.rs`, `bin/bill_http_server.rs` | 注册 `/api/...` 路由，解析认证上下文和请求 DTO，投影 response envelope | 不把复杂 SQL 或跨表事务散落在 handler 中 |
| Core | `src/backend/core/lib.rs` | 稳定业务合同、金额/时间/分类/统计 primitive、导入/学习/匹配/LLM/OCR/预算规则 | 不直接依赖前端临时字段或数据库行形状 |
| DB | `src/backend/db/lib.rs` | PostgreSQL SQLx pool、schema/migrations、事务 helper、user-scope repository、staging 生命周期、vector outbox | 不绕过事务边界、静默回退仓储 backend，或把 user_id 过滤留给调用方猜 |
| Parsers | `src/backend/parsers/lib.rs` | provider 检测、账单解析、`RawBill` 到 `StandardBill` 标准化、fixture/golden 合同 | 不执行导入 staging、去重、分类或账户写入 |

后端 Rust 文件按层维护中文导读注释，说明该文件所在层、核心职责和主要对接边界；具体文件清单以当前 git 跟踪内容和本节常用根文件为准，不在文档中硬编码易漂移数量。

### 常用根文件

- HTTP 运行态：`src/backend/http/lib.rs`、`src/backend/http/router.rs`、`src/backend/http/runtime.rs`、`src/backend/http/server.rs`、`src/backend/http/state.rs`、`src/backend/http/config.rs`、`src/backend/http/weaviate.rs`、`src/backend/http/bin/bill_http_server.rs`、`src/backend/http/bin/bill_weaviate_derived_index.rs`
- HTTP route facade：`src/backend/http/auth_routes/mod.rs`、`src/backend/http/import_routes/mod.rs`、`src/backend/http/backup_routes/mod.rs`、`src/backend/http/bill_routes/mod.rs`、`src/backend/http/budget_routes.rs`、`src/backend/http/matching_routes.rs`、`src/backend/http/statistics_routes/mod.rs`、`src/backend/http/taxonomy_routes/mod.rs`
- Core 合同：`src/backend/core/import_pipeline.rs`、`src/backend/core/import_learning.rs`、`src/backend/core/weaviate_derived.rs`、`src/backend/core/matching.rs`、`src/backend/core/statistics.rs`、`src/backend/core/budgets.rs`、`src/backend/core/ai_ocr_llm/mod.rs`、`src/backend/core/primitives/mod.rs`、`src/backend/core/migration_governance.rs`
- DB repository：`src/backend/db/runtime.rs`、`src/backend/db/connection.rs`、`src/backend/db/schema.rs`、`src/backend/db/transaction.rs`、`src/backend/db/user_scope.rs`、`src/backend/db/import_staging.rs`、`src/backend/db/vector_outbox.rs`、`src/backend/db/bills.rs`、`src/backend/db/budgets.rs`、`src/backend/db/auth.rs`、`src/backend/db/taxonomy/mod.rs`、`src/backend/db/matching.rs`
- Parser：`src/backend/parsers/lib.rs`、`src/backend/parsers/dedicated/mod.rs`、`src/backend/parsers/dedicated/WeChat.rs`、`src/backend/parsers/dedicated/Alipay.rs`、`src/backend/parsers/dedicated/ICBC.rs`、`src/backend/parsers/dedicated/CMBC.rs`、`src/backend/parsers/dedicated/ABC.rs`、`src/backend/parsers/dedicated/CCB.rs`、`tests/backend/parsers`

<a id="api-request-lifecycle"></a>
## API 请求生命周期

```mermaid
sequenceDiagram
  participant F as Frontend services.ts
  participant R as Rust router
  participant A as Auth context
  participant C as Core contract
  participant D as DB repository
  participant S as PostgreSQL
  F->>R: REST /api/... request
  R->>A: Bearer token / trusted-user header
  R->>C: normalize DTO, units, route contract
  R->>D: user-scoped repository call
  D->>S: transaction / query
  S-->>D: rows
  D-->>R: domain rows or DTO input
  R-->>F: success/data or success/result envelope
```

维护规则：

- 新 API 先在 `src/backend/http/router.rs` 或对应 route facade 中登记，再把复杂业务交给 core/db。
- response envelope 兼容性在 HTTP 层集中处理，前端 store 不应为同一字段重复补丁。
- 认证相关 route 必须明确当前用户来源；跨用户数据只能通过 user-scope repository 访问。
- 涉及金额、预算、统计、导入金额字段时，必须标注元/分边界并人工复核一次；backend core 值、PostgreSQL minor-unit 字段和 API/import/export DTO 必须通过明确适配器转换，不在 handler/store 中内联换算。

<a id="import-pipeline"></a>
## 导入管线

```mermaid
flowchart TD
  Upload["multipart / JSON 上传"] --> Parse["parsers: exactly-one provider 检测 + RawBill"]
  Parse --> Standard["post_process_raw_bills -> StandardBill"]
  Standard --> Template["db/import_staging: source + standard row + template/session staging"]
  Template --> Category["same-batch/history dedup + transfer + category/type rules + fallback"]
  Category --> Learning["recurring + learning recall/projection"]
  Learning --> Account["account_rules stable-context matching"]
  Account --> Preview["preview staging + page query"]
  Preview --> Mutations["update / reclassify / accept/reject/clear decisions"]
  Mutations --> Confirm["confirm_preview_to_bills transaction"]
  Confirm --> Cleanup["confirm/cancel/failure 清理 staging"]
```

关键边界：

- parser 只负责识别来源和标准化账单，不写 staging。
- dedicated parser 自动识别必须产生 exactly-one 决策证据；未命中或多 parser 冲突停留在 unmatched 文件，不生成标准账单。
- mixed multipart 必须保留每个文件自己的 parser id 和 parser tags。
- stage2 phase precedence 固定为 parser/standard row → 同批/跨批 dedup 与转账 materialization → category/type deterministic rules 与 fallback → recurring/learning recall/projection → account_rules 基于稳定 post-learning context 匹配 → preview/update/reclassify → confirm。
- `import_sources` 与 `import_standard_rows` 保存文件级 parser decision、标准化行、分单位金额和 parser payload，后续 duplicate/transfer/learning 切片以它们作为决策台账锚点。
- 同批重复在 stage2 按秒级时间窗口、同向金额、方向和文本证据折叠，合并交易对方/支付方式/描述，并通过 `preview_matching_feedback_json.dedup.source_chain` 与 `import_decision_groups` 保留来源证据。
- 同批转账按秒级时间窗口、同额反向金额和不同来源配对，以支出侧为基底合并交易对方/支付方式/描述；支出/收入两侧原始字段保存在 `preview_matching_feedback_json.transfer.source_chain`，并通过 `same_batch_transfer` decision group 保留成员证据。
- 历史重复查询正式 `bills` 时保持 user-scope，并以 `import_standard_rows` 的日期窗口限制候选；命中还需要金额方向和文本证据，之后生成 `database_duplicate` 预览行、`reconciliation.planned_operation=update_history` feedback 和 `import_history_materializations`。confirm 必须校验 selected preview ids、operation id、history bill id/version、acknowledgement token 和可见“将改写/合并历史账单”标记，验证通过后在事务中更新历史账单并写入 `import_confirm_operations`。
- 历史转账同样保持 user-scope 和 standard-row 日期窗口，以同日时间容差、同额反向金额和不同来源命中正式账单；stage2 生成 `transfer_cross_batch` 预览行、`reconciliation.planned_operation=merge_transfer_history` feedback、`import_history_materializations` 和 `historical_transfer` decision group。confirm ack 通过后会更新历史支出侧或创建转账基底并删除被合并历史收入侧，移动历史标签、清理 matching suppression 残留并同步账户余额。
- Check Data 首屏只读取 preview page；筛选、排序、计数和批量选择都由 Rust preview page/query/update 处理。
- transfer、learning、recurring、dedup、parser、annotation、reconciliation 信号从 `preview_matching_feedback_json` 投影，缺分类/缺账户状态按当前预览字段动态计算；stage2 账户识别以 `account_rules` 为权威，转账使用隐藏支出/收入侧字段分别匹配来源/目标账户，投资使用 parser/支付方式匹配来源账户并按交易对方优先、描述兜底匹配投资账户，收入/支出只匹配当前类型账户规则；旧账户别名只允许单账号恢复工具一次性转换为 `account_rules`，正常导入不再读取别名回退。learning 信号通过 `recommendation_key` 连接 `import_learning_lifecycle`、`import_learning_feedback_events` 和 `import_learning_suppressions`：yellow 只推荐，green/auto-applied 才能自动投影分类/账户，转账证据存在时类型受保护。
- confirm 在事务内写正式 bills、tags、accounts、learning side effects；cancel 和失败后新建 session 清理 staging，不保留导入续传状态。
- Weaviate 派生索引不参与 parser/dedup/transfer/rule/lifecycle 决策。`vector_outbox_events` 只记录待同步派生对象，`bill_weaviate_derived_index` 负责 health、schema bootstrap、outbox batch 和按用户 rebuild；运行态 health 要求 Weaviate ready，显式禁用仅用于 isolated tests/fixtures。

主要源码与测试：

- `src/backend/http/import_routes/mod.rs`
- `src/backend/http/import_routes/stage_handlers.rs`
- `src/backend/http/import_routes/preview_mutation_helpers.rs`
- `src/backend/db/import_staging.rs`
- `src/backend/db/import_staging/`
- `src/backend/db/taxonomy/account_rules.rs`
- `src/backend/core/import_pipeline.rs`
- `src/backend/core/import_learning.rs`
- `src/backend/core/account_rules/`
- `tests/backend/http/import_runtime_contract.rs`
- `tests/backend/core/import_pipeline_contracts.rs`
- `tests/backend/core/import_learning_contracts.rs`
- `tests/backend/db/import_staging.rs`

<a id="repository-data-flow"></a>
## Repository 数据流

```mermaid
flowchart LR
  Handler["HTTP handler"] --> Runtime["Core/domain runtime"]
  Runtime --> Repo["DB repository facade"]
  Repo --> Tx["transaction helper"]
  Tx --> Rows["row helpers / SQL"]
  Rows --> Tx
  Tx --> Repo
  Repo --> Handler
  Handler --> Envelope["frontend-compatible envelope"]
```

DB 层导读注释应优先解释：

- PostgreSQL URL 配置、迁移目录、SQLx pool、repository runtime provider、vector outbox 和 schema 幂等初始化。
- repository facade 供哪个 route/runtime 调用。
- user-scope 在哪里强制。
- 哪些写入必须 rollback-on-error，哪些审计是 best-effort。
- import staging 的 session/source/standard row/template/preview/decision/history materialization/LLM memory/confirm 生命周期。
- row helper 负责数据库行和前端兼容 DTO 之间的字段转换。

<a id="development-recipes"></a>
## 开发路径

### 新增或调整 API

1. 在 `src/backend/http/*_routes*` 找到对应 route facade。
2. 确认认证上下文、请求 DTO、response envelope 和 user-scope。
3. 复杂规则放进 `src/backend/core`，SQL 放进 `src/backend/db`。
4. 更新 `docs/overview-api-routes.md` 和本文件的路径索引。
5. 运行 HTTP/domain focused tests；若 route ownership fixture 变更，再运行前端 route contract。

### 新增 parser

1. 在 `src/backend/parsers/dedicated/` 增加 provider 实现并登记。
2. 注释里说明 source identity、RawBill 字段来源、StandardBill 标准化边界和 fixture/golden 期望。
3. 不在 parser 中写导入 staging、分类、账户或去重。
4. 运行 `cargo test -p bill-analyser-parsers`。

### 修改导入预览

1. 先读 `src/backend/core/import_pipeline.rs` 和 `src/backend/http/import_routes/*`。
2. 明确影响的是 parser、dedup/materialization、category/type rules、learning projection、account rules、preview page、selection、transfer、recurring 还是 confirm。
3. 保持 `category_id`、账户、转账类型和 learning signal 与当前用户 taxonomy 兼容；账户规则必须消费 learning 后的稳定预览上下文。
4. 运行对应 core/http/db 导入测试，并人工复核金额单位。

### 修改 repository 查询或 schema-adjacent 代码

1. 在 repository facade 中确认 user-scope 和事务边界。
2. schema 改动必须有迁移/初始化幂等说明；普通查询不要顺手改变 schema。
3. row helper 变化需要说明前端兼容字段含义。
4. 运行 DB focused tests 和结构门禁。

### 修改预算、统计或认证

- 预算：复核元/分、period 层级、父子预算派生和 import/export。
- 统计：复核汇率、账户过滤、日期范围和空账户图例。
- 认证：复核 token/session、2FA/step-up、user-data 操作、备份下载/恢复权限和审计。

<a id="verification-matrix"></a>
## 验证矩阵

| 变更范围 | 最小验证 | 扩展验证 |
| --- | --- | --- |
| 文档/静态 HTML | `node scripts/check-backend-doc-map.mjs`、`git diff --check -- docs scripts` | `node scripts/check-rust-backend-structure.mjs` |
| Parser 注释或小重命名 | `cargo fmt --all -- --check`、`cargo test -p bill-analyser-parsers` | parser fixture/golden 相关测试 |
| Core 合同注释或小重命名 | `cargo fmt --all -- --check`、`cargo test -p bill-analyser-core` | `cargo test -p bill-analyser-core --test import_pipeline_contracts`、`cargo test -p bill-analyser-core --test import_learning_contracts` |
| DB repository 注释或小重命名 | `cargo fmt --all -- --check`、`cargo test -p bill-analyser-db` | `cargo test -p bill-analyser-db --test import_staging` |
| HTTP route 注释或小重命名 | `cargo fmt --all -- --check`、`cargo test -p bill-analyser-http` | `cargo test -p bill-analyser-http --test import_runtime_contract`、`Set-Location src\web; npm run test -- frontendRustRouteContract` |
| 任意业务源码改动 | focused tests | `cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、`cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 35`，且被改业务代码自身按 diff 行或文件级覆盖率 > 90% |

最终验收还要运行：

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 35
node scripts/check-rust-backend-structure.mjs
node scripts/check-backend-doc-map.mjs
git diff --check
```
