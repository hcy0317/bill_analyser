# D5 budget-statistics-exchange-assets 结构债 ledger

本 ledger 固化 D5 预算、统计、汇率和资产趋势域的当前结构边界。D5 的目标是按“功能域 -> 功能文件夹”继续复制 D1-D4 模板，先补行为锁定，再拆后端和前端结构，再补中文说明和治理基线。

## 1. 域边界

D5 覆盖：

- 后端预算 CRUD、导出、导入、执行、预测、历史快照和 period 层级同步。
- 后端统计 category statistics、category trends、pie、top merchants、transaction amounts、asset trends、net worth、calendar events、Analyzer、insights anomaly、汇率 provider fallback 和用户自定义汇率。
- 前端桌面预算页、桌面统计页、预算 store、统计 store、预算 REST adapter、统计 helper、汇率/货币常量和 exchange rate store。

D5 不覆盖：

- 认证、注册默认包、profile、user-data export/clear，归 D6。
- `src/web/src/lib/services.ts` 的全局 axios、refresh token 和大 facade 清理，归 D10；D5 只能在已有预算/统计 service adapter 范围内记录或申请 shared lease。
- `src/web/src/stores/index.ts`、`src/core/theme.ts`、`src/models/imported_transaction.ts` 等共享结构债，归 D10 或其他对应域。
- UI 视觉重设计；本计划只做结构和注释治理，保留现有预算/统计页面视觉与交互。

## 2. 当前结构 gate 快照

采集时间：2026-06-21，基于 `bill_analyser/main` 的 `c82d60919d111f58e4509f5428edc180ae1d5c6f`。

Rust 后端结构 gate：

- `node scripts/check-rust-backend-structure.mjs` 失败 6 项。
- D5 直接失败项：
  - `src/backend/db/budgets/postgres_reads.rs`：结构 gate 计 2094 行，超过 baseline 2043。
  - `src/backend/db/statistics.rs`：结构 gate 计 893 行，超过 baseline 872。
  - `src/backend/db/budgets.rs`：结构 gate 计 617 行，超过新文件限制 600。
- D5 相关 warning：
  - `src/backend/core/statistics.rs`：较 baseline 减少 8 行但仍是超大统计核心文件。
- 非 D5 失败项：
  - `src/backend/db/auth_postgres.rs`
  - `src/backend/db/auth_registration_defaults.rs`
  - `src/backend/http/auth_routes/public_auth_handlers.rs`

前端结构 gate：

- `Set-Location src/web; npm run structure:check` 失败 7 项。
- D5 直接失败项：
  - `src/views/desktop/budgets/ListPage.vue`：结构 gate 计 3493 行。
  - `src/views/desktop/budgets/ListPage.vue` template section：195 行，超过 baseline 194。
  - `src/views/desktop/budgets/ListPage.vue` script section：2731 行，超过 baseline 2727。
- D5 相关超大但当前不在 failure 列表的治理对象：
  - `src/web/src/stores/statistics.ts`：约 1739 行。
  - `src/web/src/consts/currency.ts`：约 1286 行。
  - `src/web/src/views/desktop/statistics/TransactionPage.vue`：约 1195 行。
  - `src/web/src/stores/budget.ts`：约 532 行。
- 非 D5 失败项：
  - `src/lib/services.ts`
  - `src/stores/index.ts`
  - `src/core/theme.ts`
  - `src/models/imported_transaction.ts`

## 3. 后端文件职责地图

### 3.1 `src/backend/db/budgets/postgres_reads.rs`

当前集中承载预算 PostgreSQL repository 主链：

- 列表、详情、创建、更新、删除、导出和导入。
- 预算执行查询、执行候选查询、spent amount 聚合和列表 enrichment。
- 预算预测 rows 查询、forecast 聚合、budget map 和 forecast response 组装。
- 预算历史查询、on-demand history build、history row projector 和 snapshot 创建。
- 月/季/年 period 层级同步、父预算 floor 更新、主分类预算同步、子预算合计和 sync group 查询。
- `BudgetRecord` 与 PostgreSQL row/value 的双向映射。
- 金额 cents、日期、布尔、文本、QueryBuilder bind-list 等 helper。

拆分建议：

1. `listing.rs`：列表、详情、raw query 和 listing enrichment。
2. `mutation.rs`：create/update/delete/import row 事务函数。
3. `import_export.rs`：导出、导入入口和 import payload 清洗。
4. `execution.rs`：执行详情、候选、spent amount 和 execution item 组装。
5. `forecast.rs`：forecast rows、聚合、budget map 和 group expression。
6. `history.rs`：history 查询、on-demand build、snapshot、history projector。
7. `period_sync.rs`：月/季/年层级同步、父子预算合计和 group key。
8. `mapping.rs`：row -> `BudgetRecord`、amount/date/value helper。
9. `sql_helpers.rs`：category SQL expression、bind-list、user id 转换等底层 helper。

### 3.2 `src/backend/db/budgets.rs`

当前集中承载内存预算领域 helper：

- create/update payload normalization。
- `amount_cents` 必填校验和 import 有效性过滤。
- execution candidate 过滤、去重、period window 解析和执行 item 构建。
- history items enrichment、exact item 提取、period overlap、merge/sort 和 filter summary。
- `BudgetRecord` 文本、数字、字段和 JSON helper。
- 模块内测试覆盖 cents 与 execution helper。

拆分建议：

1. `payloads.rs`：create/update/import normalization 与 amount cents 校验。
2. `execution.rs`：execution candidates、window 和 execution item。
3. `history.rs`：history enrichment、merge、sort 和 identity key。
4. `record_helpers.rs`：record/value/text/date/number helper。
5. `tests.rs`：保持现有 helper 合同测试。

### 3.3 `src/backend/db/statistics.rs`

当前集中承载统计 PostgreSQL repository：

- category statistics、category trends、category pie、top merchants。
- asset trends、net worth、calendar events 和 account balance deltas。
- transaction amount period 查询。
- Analyzer report、trends、comparison、category。
- insights anomaly summary。
- 用户默认货币、用户自定义汇率 list/upsert/delete 和 settings JSON 存取。
- 统计账单、分类、账户和 recurring rules loader。
- 日期、金额、类型、分类 path、JSON number 等 helper。

拆分建议：

1. `category.rs`：category statistics、trends、pie、top merchants。
2. `asset.rs`：asset trends、net worth、balance deltas 和 asset helper。
3. `analyzer.rs`：Analyzer report/trends/comparison/category。
4. `calendar.rs`：calendar events 与 recurring projections loader。
5. `exchange.rs`：默认货币、自定义汇率 settings JSON 读写。
6. `loaders.rs`：bills/categories/accounts/recurring 读取。
7. `mapping.rs`：PostgreSQL row/value -> core input。
8. `helpers.rs`：日期、金额、类型、过滤和 JSON helper。

### 3.4 `src/backend/core/statistics.rs`

当前是 D5 最大核心聚合文件，承载统计 DTO、构建器、验证和汇率合同：

- timestamp/year-month range 解析与 asset span 校验。
- category statistics/trends、asset trends、legend、net worth。
- insights anomaly、calendar events、category pie、top merchants。
- transaction amounts 和统计图表 response。
- Analyzer report/trends/comparison/category/trend bucket。
- 汇率 provider options、provider candidate order、用户自定义汇率、provider/fallback result、CNY quote 转换和 provider base currency 转换。
- 大量日期、金额、分类、账户、异常检测和图表 helper。

拆分建议：

1. `ranges.rs`：timestamp、year-month、asset span、period range。
2. `category.rs`：category statistics/trends/pie/top merchants。
3. `asset.rs`：asset trends、legend、net worth。
4. `calendar.rs`：calendar events 和 recurring projection。
5. `analyzer.rs`：Analyzer report/trends/comparison/category/chart plan。
6. `exchange.rs`：provider options、candidate order、custom/provider/fallback result 和 rate conversion。
7. `anomalies.rs`：insight anomaly builders。
8. `types.rs`：DTO、input、output 和 error type。
9. `helpers.rs`：amount/date/category/account/value helper。

### 3.5 HTTP route 边界

`src/backend/http/budget_routes.rs` 已是 facade，并聚合 `budget_routes/handlers.rs`、`payloads.rs`、`routes.rs`、`runtime_helpers.rs`。D5 backend-shape 默认不优先拆它，除非 behavior-lock 发现 handler 仍有结构回涨。

`src/backend/http/statistics_routes/mod.rs` 已拆为 read handlers、Analyzer handlers、exchange handlers、exchange providers、query 和 response 子文件。D5 backend-shape 可只补必要中文说明或轻量整理，不把已经拆开的 route 重新打散。

## 4. 前端文件职责地图

### 4.1 `src/web/src/views/desktop/budgets/ListPage.vue`

当前是 D5 前端最主要 failure，集中承载：

- 桌面预算列表整体页面、筛选条、视图切换、预算类型切换和 period filter。
- 预算分组、一级/二级分类展示、预算/执行率颜色、金额格式化和 drilldown 到交易列表。
- 预算历史、历史 polar chart、legend/focus state 和历史 period 请求。
- 预算预测 panel、预测策略、风险摘要和 forecast load。
- 新增、编辑、删除、导入、导出、文件选择和保存回调。
- 搜索、预算金额/支出/执行率筛选、filter preset 保存/加载/删除。
- 大量 watcher、reload、store action 编排和页面状态。

拆分建议：

1. 外置 template/style，保留 `ListPage.vue` 为页面 facade。
2. `useBudgetListPageState.ts`：view mode、budget type、period/filter preset、dialog/file 状态。
3. `budgetFilters.ts`：keyword、spent/budget/execution rate filters、label/display name、preset。
4. `budgetActions.ts`：add/edit/delete/import/export/save/reload/loadForecast 编排。
5. `budgetPresentation.ts`：金额、颜色、execution rate、confidence label/class。
6. `budgetDrilldown.ts`：交易页跳转和 date range 映射。
7. 已存在的 `categorySelection.ts`、`historyGrouping.ts`、`forecastRequest.ts`、`forecastDisplay.ts`、history-polar 子文件继续复用，不重复发明。

### 4.2 `src/web/src/stores/statistics.ts`

当前集中承载：

- 分类统计、概览、账户统计、趋势统计和资产趋势 derived data。
- account/category/tag filter 初始化、更新和 URL 参数构造。
- 统计页到交易列表的 query 映射。
- category/account info 组装、排序和图表数据转换。
- categorical/trend/asset API load action。

拆分建议：

1. `statistics/types.ts`：复杂 store 内部类型。
2. `statistics/filters.ts`：filter init/update、page params、transaction list params。
3. `statistics/categoryAnalysis.ts`：category/account/category overview derived data。
4. `statistics/trends.ts`：trend derived data 和 date aggregation。
5. `statistics/assets.ts`：asset trend derived data、legend 过滤和 zero-data 判定。
6. `statistics/actions.ts`：load categorical/trend/asset actions。

### 4.3 `src/web/src/views/desktop/statistics/TransactionPage.vue`

当前承载统计桌面页的图表类型、筛选、日期控制、导出、点击跳转和 reload 编排。D5 frontend-shape 应把 template/style、筛选控制、chart option/interaction 和 export dialog 接线拆出，保持 route/query/visual 不变。

### 4.4 `src/web/src/consts/currency.ts`

当前约 1286 行，集中承载货币元数据、显示名、符号、精度、小数位、国家/地区或可选币种表。D5 可把静态货币表分组到 `consts/currency/`，保留 `consts/currency.ts` facade；不得改变 currency code、display name、symbol、minor unit 或汇率目标列表。

### 4.5 预算/汇率 store 和 adapter

- `src/web/src/stores/budget.ts`：预算列表缓存、execution/history/forecast action、CRUD、导入导出和 invalidation。
- `src/web/src/lib/services/budget.ts`：REST budget mapper、query builder、amount cents 映射和 import/export adapter。
- `src/web/src/stores/exchangeRates.ts`：latest rates localStorage、provider selection、custom rate update/delete 和金额换算。
- `src/web/src/models/budget.ts`、`src/web/src/models/exchange_rate.ts`：DTO 和前端模型合同。

D5 可以在 behavior-lock 中覆盖这些 adapter/store 合同；frontend-shape 阶段只在 owned/shared lease 明确后拆 facade。

## 5. 行为锁定测试锚点

后端现有锚点：

- `tests/backend/core/budget_contracts.rs`
- `tests/backend/core/statistics_contracts.rs`
- `tests/backend/db/statistics_postgres.rs`
- `tests/backend/http/internal/statistics_exchange_providers.rs`
- `tests/backend/core/runtime_governance_contracts.rs`

前端现有锚点：

- `tests/web/lib/services.budget.test.ts`
- `tests/web/lib/services.statistics.test.ts`
- `tests/web/stores/statistics.test.ts`
- `tests/web/models/budget.test.ts`
- `tests/web/views/desktop/budgets/periodFilters.test.ts`
- `tests/web/views/desktop/budgets/listPageCategoryIcons.test.ts`
- `tests/web/views/desktop/budgets/historyGrouping.test.ts`
- `tests/web/views/desktop/budgets/historyPolarChart.test.ts`
- `tests/web/views/desktop/budgets/forecastRequest.test.ts`
- `tests/web/views/desktop/budgets/forecastDisplay.test.ts`
- `tests/web/views/desktop/budgets/categorySelection.test.ts`
- `tests/web/views/mobile/budgetsListPage.test.ts`
- `tests/web/views/mobile/budgetsEditSheet.test.ts`

D5 behavior-lock 建议补强：

- 预算 CRUD、import/export、execution、forecast、history snapshot 的 cents、period、type、category filter 和 user-scope 合同。
- 预算 period 层级同步：月/季/年父子预算 floor、删除最后子预算、主分类/二级分类同步。
- 统计 category/trend/asset/net worth/analyzer 的 cents 符号、转账边界、空账户 legend 和日期范围。
- 汇率：用户自定义优先、provider fallback 顺序、内置兜底、provider base currency 转换和 no-live-network 测试约束。
- 桌面预算页源码合同：拆分后仍使用 `useBudgetStore`、现有 helper、现有 dialog 和现有 drilldown query。
- 桌面统计页源码合同：拆分后仍使用 `useStatisticsStore`、现有 date/filter query、chart click 和 export dialog。

### 5.1 本次 behavior-lock 执行记录

执行时间：2026-06-21T23:48:20+08:00，基于 `bill_analyser/main` 的 `b5a1e991501312a59a38cdb4048c9c720b7def02`。

本切片新增或扩展的 D5 行为锁定：

- `tests/web/views/desktop/budgets/listPageCategoryIcons.test.ts` 新增桌面预算页源码合同，锁定预算页 facade 对 `useBudgetStore`、`useTransactionCategoriesStore`、预算历史/预测/list table/edit dialog、forecast helper、drilldown helper、reload、forecast、导入导出、删除和 `@budget:saved` 回调的接线。
- `tests/web/views/desktop/statistics/transactionPageSourceContract.test.ts` 新增桌面统计页源码合同，锁定统计页 facade 对 `useStatisticsStore`、`ExportDialog`、filter/page params、category/trend/asset load actions、日期控制、图表 drilldown 和导出动作的接线。
- 复用现有后端预算合同 `tests/backend/core/budget_contracts.rs` 锁定预算 cents、period、execution、history、forecast、import/export、filter 和路由 scope 语义。
- 复用现有后端统计合同 `tests/backend/core/statistics_contracts.rs` 锁定统计 cents 符号、category/trend/asset、net worth、calendar、insights、Analyzer、汇率 provider/custom/fallback 和范围校验语义。

已通过的 focused 验证：

- `cargo test -p bill-analyser-core --test budget_contracts`：11 passed。
- `cargo test -p bill-analyser-core --test statistics_contracts`：6 passed。
- `Set-Location src/web; npm run test -- --runTestsByPath ../../tests/web/lib/services.budget.test.ts ../../tests/web/lib/services.statistics.test.ts ../../tests/web/stores/statistics.test.ts ../../tests/web/views/desktop/budgets/forecastRequest.test.ts ../../tests/web/views/desktop/budgets/forecastDisplay.test.ts ../../tests/web/views/desktop/budgets/categorySelection.test.ts ../../tests/web/views/desktop/budgets/listPageCategoryIcons.test.ts ../../tests/web/views/desktop/statistics/transactionPageSourceContract.test.ts`：8 suites / 40 tests passed。

已通过的扩展验证：

- `Set-Location src/web; npm run lint:ci`：通过，输出仅保留既有 `no-explicit-any` warning。
- `Set-Location src/web; npm run test:coverage`：91 suites / 38984 tests passed，All files line coverage 99.13%。
- `node scripts/check-backend-doc-map.mjs`：通过。
- `git diff --check`：通过。

结构 gate 结果：

- `node scripts/check-rust-backend-structure.mjs` 仍失败 6 项；D5 项仍为 `db/budgets/postgres_reads.rs`、`db/statistics.rs`、`db/budgets.rs`，非 D5 项仍为 auth/public auth debt。该结果符合 behavior-lock 不移动结构的约束。
- `Set-Location src/web; npm run structure:check` 仍失败 7 项；D5 项仍为 `views/desktop/budgets/ListPage.vue` 及其 template/script section，非 D5 项仍为 shared shell debt。该结果符合 behavior-lock 不移动结构的约束。

## 6. Shared path lease

D5 需要提前记录的共享面：

- `src/web/src/lib/services.ts`：属于 D10 全局 services facade。D5 不直接重构该文件；如行为锁定发现预算/统计 REST 入口仍需要引用，只允许最小源码合同测试或记录 shared lease。
- `src/web/src/lib/services/budget.ts`：预算 service adapter 属 D5 业务边界，但不在原始 D5 owned_paths 中；D5 ledger 记录它为 D5 shared lease 候选，behavior-lock 可补测试，frontend-shape 如需拆分应先写入 progress shared_path_lease。
- `src/web/src/core/statistics.ts`、`src/web/src/lib/statistics.ts`、`src/web/src/stores/exchangeRates.ts`、`src/web/src/views/desktop/exchangerates/**`：统计/汇率相关，但原始 D5 owned_paths 未完全列出；D5 可在 ledger 中纳入读侧盘点，写侧改动需补 lease。
- `docs/PROJECT_OVERVIEW.md`：只有 backend/frontend shape 改变稳定模块关系时更新当前事实；ledger/behavior-lock 不写总览。

## 7. 建议切片顺序

### 7.1 behavior-lock

1. 后端补预算 execution/history/forecast/period sync 合同测试，优先覆盖 cents、period、父子预算同步和 user-scope。
2. 后端补统计/汇率合同测试，覆盖 category/trend/asset/analyzer/custom rate/provider fallback，不触发 live external provider 调用。
3. 前端补预算页和统计页源码合同，锁定 store、service adapter、drilldown query、date filter 和 export dialog 接线。
4. 运行 focused Rust/frontend tests、doc map、结构 gate；记录现有 D5 failure。

### 7.2 backend-shape

1. 拆 `db/budgets/postgres_reads.rs`，先按 listing/mutation/import-export/execution/forecast/history/period-sync/mapping/sql-helper 分组。
2. 拆 `db/budgets.rs`，把 payload normalization、execution、history 和 record helper 分离。
3. 拆 `db/statistics.rs`，按 category/asset/analyzer/calendar/exchange/loaders/mapping/helpers 分离。
4. 拆 `core/statistics.rs`，按 ranges/category/asset/calendar/analyzer/exchange/anomalies/types/helpers 分离。
5. 跑 focused tests、fmt、clippy 和 full Rust coverage gate；结构 gate 中 D5 backend failure 应清零或仅剩明确 baseline 解释。

### 7.2.1 本次 backend-shape 执行记录

执行时间：2026-06-22T00:09:06+08:00，基于 `bill_analyser/main` 的 `42847b73ecc1155e5430b85874a5d9e6817b44c2`。

本切片完成的后端结构拆分：

- `src/backend/db/budgets.rs` 保留预算 DTO facade，并通过 `budgets/payloads.rs`、`execution.rs`、`history.rs`、`record_helpers.rs`、`tests.rs` 拆出 payload normalization、预算执行、历史合并、record/value helper 和模块测试。
- `src/backend/db/budgets/postgres_reads.rs` 保留 PostgreSQL 预算仓储 facade，并通过 `postgres_reads/public_api.rs`、`listing.rs`、`execution.rs`、`forecast.rs`、`history.rs`、`mapping.rs`、`mutation_sql.rs`、`period_sync.rs`、`value_helpers.rs`、`tests.rs` 拆出预算 CRUD/导入导出、列表 enrichment、执行、预测、历史、period 层级同步、row mapping 和 SQL/value helper。
- `src/backend/db/statistics.rs` 保留 PostgreSQL 统计仓储 facade，并通过 `statistics/category.rs`、`asset_analyzer.rs`、`exchange.rs`、`ranges.rs`、`loaders.rs`、`helpers.rs` 拆出 category/pie/top merchants、asset/analyzer/net worth/calendar/insights、汇率 settings、日期范围、loader 和映射 helper。
- `src/backend/core/statistics.rs` 保留统计核心 facade 和 DTO，并通过 `statistics/ranges.rs`、`category.rs`、`asset.rs`、`anomalies.rs`、`calendar.rs`、`breakdown.rs`、`analyzer.rs`、`exchange.rs`、`misc_public.rs`、`analyzer_helpers.rs`、`date_helpers.rs`、`insight_helpers.rs`、`calendar_helpers.rs`、`exchange_helpers.rs`、`value_helpers.rs` 拆出统计核心构建器和 helper。

已通过的验证：

- `cargo fmt --all -- --check`：通过。
- `cargo test -p bill-analyser-core --test budget_contracts`：11 passed。
- `cargo test -p bill-analyser-core --test statistics_contracts`：6 passed。
- `cargo test -p bill-analyser-db budget`：7 passed。
- `cargo test -p bill-analyser-db statistics`：`statistics_postgres_queries_preserve_explicit_cents_and_transfer_boundaries` passed。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 35`：通过，完整 Rust 工作区 coverage gate 生成 `workspace.lcov`。
- `node scripts/check-rust-backend-structure.mjs`：D5 backend failure 清零；仅剩 D6 auth 历史债。
- `node scripts/check-backend-doc-map.mjs`：通过。
- `git diff --check`：通过。

结构结果：

- D5 backend facade 和新增子文件均低于 600 行；当前最大新增文件是 `src/backend/db/budgets/postgres_reads/period_sync.rs` 550 行。
- `src/backend/core/statistics.rs`、`src/backend/db/statistics.rs`、`src/backend/db/budgets/postgres_reads.rs` 在结构 gate 中只剩 line-reduction warning。

### 7.3 frontend-shape

1. 拆桌面预算 `ListPage.vue` 为 facade + template/style + state/filter/action/presentation/drilldown 子文件。
2. 拆 `stores/statistics.ts` 为 facade + filters/category/trends/assets/actions/types 子文件。
3. 拆桌面统计 `TransactionPage.vue` 的 template/style、filter/date、chart interaction、reload/export 接线。
4. 拆 `consts/currency.ts` 静态数据表为 `consts/currency/**`，保持原 import facade。
5. 必要时拆 `stores/budget.ts` 和 `stores/exchangeRates.ts`，但必须先有 behavior-lock 覆盖。
6. 跑 lint、focused tests、coverage、build 和 frontend structure gate；保持 UI 视觉不变。

本切片执行结果：

- `src/web/src/views/desktop/budgets/ListPage.vue` 保留预算页 facade，template/style 下沉到 `views/desktop/budgets/list/ListPage.template.html` 与 `ListPage.css`，预算页类型、金额筛选、历史周期 helper 和展示 helper 下沉到 `views/desktop/budgets/list/**`。
- `src/web/src/stores/statistics.ts` 保留 `useStatisticsStore` facade，统计筛选类型下沉到 `stores/statistics/types.ts`，统计页和交易列表 query 构建下沉到 `stores/statistics/pageParams.ts`。
- `src/web/src/views/desktop/statistics/TransactionPage.vue` 保留桌面统计页 facade，template/style 下沉到 `views/desktop/statistics/transaction/TransactionPage.template.html` 与 `TransactionPage.css`，统计页/交易页链接构建下沉到 `transaction/pageLinks.ts`。
- `src/web/src/consts/currency.ts` 保留 `ALL_CURRENCIES`、`DEFAULT_CURRENCY_CODE`、`DEFAULT_CURRENCY_SYMBOL` 和 `PARENT_ACCOUNT_CURRENCY_PLACEHOLDER` facade，ISO 4217 静态表按代码范围拆到 `consts/currency/aToG.ts`、`hToP.ts`、`qToZ.ts`。
- 源码合同测试已改为读取页面 facade + 外置 template + 功能 helper，继续锁定预算页 `useBudgetStore`、导入导出、forecast、drilldown、`@budget:saved` 以及统计页 `useStatisticsStore`、filter params、chart drilldown、date controls 和 export dialog 接线。

本切片本地验证：

- `Set-Location src/web; npm run test -- --runTestsByPath ../../tests/web/views/desktop/budgets/listPageCategoryIcons.test.ts ../../tests/web/views/desktop/statistics/transactionPageSourceContract.test.ts ../../tests/web/stores/statistics.test.ts ../../tests/web/lib/services.statistics.test.ts ../../tests/web/lib/services.budget.test.ts`：5 suites / 18 tests passed。
- `Set-Location src/web; npm run lint:ci`：通过；仅保留既有 `no-explicit-any` warning。
- `Set-Location src/web; npm run structure:check`：D5 frontend failure 清零；剩余失败为 `src/lib/services.ts`、`src/stores/index.ts`、`src/core/theme.ts`、`src/models/imported_transaction.ts`，均归 D10/shared shell 或非 D5 历史债。

### 7.4 comment-pass/governance-docs/closeout

- 按“导出函数、业务关键函数、复杂私有 helper 必须有中文说明；简单 getter/映射/事件转发不强制”补齐 D5 中文说明。
- 更新 `docs/PROJECT_OVERVIEW.md` 中预算、统计、汇率和资产趋势当前结构事实。
- 收紧 Rust/frontend structure baseline，只处理已完成 D5 项，不能吞并 D6/D10 历史债。
- PR/CI/merge/delete/writeback 完整后，cursor 推进到 D6。

## 8. 关键风险与阻断条件

- 金额必须保持显式 cents/minor units；预算 amount、spent amount、forecast amount、统计聚合、资产余额和汇率换算不得引入浮点金额作为业务存储。
- 预算 period 层级同步会影响父预算、子预算和执行历史；拆分必须先锁 transaction boundary 与 rollback 行为。
- 统计 asset trends 同时依赖账户余额、交易流水、汇率和日期范围；拆分不得改变 transfer/investment/hidden account 处理。
- 汇率 provider fallback 不得在测试中发起不受控 live network；provider parser 与 fallback 只能在可控 fixture 中验证。
- 前端预算页是高交互页面；拆分不得改变筛选、filter preset、历史图、预测面板、导入导出和 drilldown query。
- `services.ts` 和全局 store index 是 D10 shared shell；D5 不得为局部重构顺手改全局 facade。
- PR 自动合并必须证明 PR head SHA 与 CI head SHA 一致、CI passed、base fresh、source branch deleted。

## 9. D5 完成判定

D5 完成后应满足：

- `db/budgets/postgres_reads.rs` 不再由单文件承载 CRUD、导入导出、execution、forecast、history、period sync、row mapping 和 SQL helper。
- `db/budgets.rs` 不再由单文件承载 payload normalization、execution、history、record/value helper 和测试。
- `db/statistics.rs` 不再由单文件承载 category、asset、analyzer、calendar、exchange、loader 和 helper。
- `core/statistics.rs` 不再由单文件承载全部统计 DTO、构建器、Analyzer、exchange provider 和 helper。
- 桌面预算页只负责页面装配；筛选、动作、展示、drilldown、历史和预测逻辑已分离。
- 统计 store、桌面统计页和 currency 常量有清晰 facade + 功能文件夹边界，或有明确治理 baseline。
- 行为锁定覆盖预算 CRUD/import/export/execution/forecast/history、统计 category/trend/asset/analyzer、汇率 custom/provider/fallback 和桌面预算/统计关键交互。
- PR/CI/merge/delete/writeback 证据完整，cursor 推进到 D6。
