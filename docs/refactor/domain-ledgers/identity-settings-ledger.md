# D2 账户分类标签与设置域结构债 Ledger

## 1. 切片边界

- Ultragoal story: `G015-complete-d2-identity-settings-domain`
- 功能域: `D2 identity-settings`
- 基线提交: `ed99bb7b44d084421505072c62f6bceed555928e`
- 工作分支: `codex/structure-identity-settings-ledger`
- 本切片只建立结构债 ledger，不移动业务源码、不改 REST API、不改数据库合同、不改金额单位、不做 UI 视觉重设计。
- D2 覆盖账户、分类、标签、模板与 settings bundle 的身份主数据边界；规则中心页面与规则编排体验归 D3，但 taxonomy route/db 中的 account/category rule 基础读写属于共享边界，后续切片必须保持 D3 可继续接管。

## 2. 必须保持的业务合同

D2 重构时以下合同默认不可变：

- HTTP 运行态仍为 Rust Axum `REST /api/...`，账户、分类、标签、模板、settings bundle 路由不增加 sidecar 或旧式透传链路。
- 所有 taxonomy DB 读写必须按当前 `user_id` 过滤；handler 不直接复制 SQL 或事务细节。
- 账户金额字段必须使用整数分或显式 minor units；前端账户余额展示可格式化为元，但 API/store/model 机器字段保持 `*Cents`。
- 账户类型、账户分类、父子账户关系、隐藏状态、display order 和 metadata 归一化必须保持当前恢复兼容逻辑。
- 分类 canonical id、父子分类、虚拟主分类、隐藏状态、图标颜色、分类规则同步字段必须保持当前前后端投影合同。
- 标签 CRUD、display order、隐藏状态和 settings bundle 导入导出字段保持当前 REST payload。
- Settings bundle 导入必须保持 dry-run rollback、真实导入事务提交、引用重映射、幂等 upsert 和 unsupported section warning。
- `transactionTemplates` 与 `scheduledTransactions` 在 settings bundle 中仍需在账户、分类、标签引用重映射后写入；不允许回退为 unsupported warning。
- 旧式 account rule scope 字段只能在 API/导入边界产生兼容 warning 并被忽略，不能重新持久化或向前端传播。
- 发现真实 bug 或合同漂移时，当前重构切片写 `blocked:<reason>`，另开修复切片。

## 3. 当前结构门禁证据

本切片执行了以下只读扫描：

- `node scripts/check-rust-backend-structure.mjs`
- `Set-Location src/web; npm run structure:check`
- D2 owned paths 行数扫描和函数群 `rg` 扫描
- D2 相关测试入口扫描

当前门禁状态：

- Rust backend structure gate: 失败 16 项，其中 D2 直接相关项包括：
  - `src/backend/db/taxonomy/postgres_reads.rs`: gate 3042 行，超过 baseline 2894；原始扫描 3045 行。
  - `src/backend/db/taxonomy/settings_bundle/postgres_import_export.rs`: gate 1532 行，超过 baseline 940；原始扫描 1544 行。
  - `src/backend/http/taxonomy_routes/account_handlers.rs`: gate 760 行，超过 baseline 759；原始扫描 793 行。
  - `src/backend/http/taxonomy_routes/account_category_formatters/accounts_and_tags.rs`: gate 683 行，超过新文件 600 行限制；原始扫描 688 行。
  - `src/backend/core/account_rules/mod.rs`: 898 行，属于 D3 规则域但被 D2 settings/account rule 边界引用。
  - `src/backend/core/category_rules/mod.rs`: 646 行，属于 D3 规则域但被 D2 分类规则同步引用。
- Frontend structure gate: 失败 42 项，其中 D2 直接相关项包括：
  - `src/stores/account.ts`: gate 1248 行，超过 baseline 1246；原始扫描 1038 行。
  - `src/views/desktop/accounts/ListPage.vue`: gate 837 行，template 145 行，script 413 行；原始扫描 730 行。
  - `src/views/desktop/accounts/list/dialogs/EditDialog.vue`: 727 行，超过新文件 500 行限制；原始扫描 644 行。
  - `src/views/mobile/accounts/EditPage.vue`: gate 797 行，超过 baseline 779；原始扫描 714 行。
  - `src/views/desktop/categories/ListPage.vue`: gate 562 行，template 91 行，script 339 行；原始扫描 488 行。
  - `src/views/desktop/tags/ListPage.vue`: gate 528 行，template 75 行，script 246 行；原始扫描 462 行。
  - `src/lib/services.ts` 与 `src/stores/index.ts` 是共享壳层债务；D2 只能按账户/分类/标签/settings bundle API 申请 lease，不能全量拆服务壳。

## 4. 后端文件地图

### 4.1 DB taxonomy repository 层

| 文件 | 行数/状态 | 当前职责 | 后续拆分方向 |
| --- | ---: | --- | --- |
| `src/backend/db/taxonomy/postgres_reads.rs` | 3045，超限 | 账户、分类、分类规则、规则总览、账户规则、标签、模板的 PostgreSQL CRUD、row mapping、payload normalization、测试 | 拆成 `taxonomy/postgres/{accounts,categories,category_rules,account_rules,tags,templates,rule_overview,row_mapping,value_helpers,tests}.rs`，保留 facade re-export |
| `src/backend/db/taxonomy/accounts.rs` | 20 | `AccountRecord`、display order DTO | 可保留为类型文件；后续补充中文说明 |
| `src/backend/db/taxonomy/categories.rs` | 16 | `CategoryRecord`、统计 DTO | 可保留为类型文件；分类 row mapping 下沉到 postgres 子模块 |
| `src/backend/db/taxonomy/tags.rs` | 19 | `TagRecord`、display order DTO | 可保留为类型文件 |
| `src/backend/db/taxonomy/templates.rs` | 8 | `TemplateRecord`、display order DTO | 可保留为类型文件；模板 CRUD 由 postgres 子模块接管 |
| `src/backend/db/taxonomy/account_rules.rs` / `category_rules.rs` | 3/3 | rule record facade | D3 接管规则核心；D2 只保持 settings bundle/account/category 引用兼容 |

主要函数群：

- 账户主数据：`list_postgres_accounts`、`get_postgres_account_by_id`、`get_postgres_sub_accounts`、`create_postgres_account`、`update_postgres_account`、`delete_postgres_account`、`update_postgres_account_display_orders`。
- 分类主数据：`list_postgres_categories`、`get_postgres_category_by_id`、`get_postgres_category_by_name`、`create_postgres_category`、`update_postgres_category`、`delete_postgres_category`、`update_postgres_category_display_order`、`update_postgres_main_category_name`。
- 标签和模板：`list_postgres_tags`、`create_postgres_tag`、`update_postgres_tag`、`delete_postgres_tag`、`update_postgres_tag_display_orders`、`list_postgres_templates`、`create_postgres_template`、`update_postgres_template`、`delete_postgres_template`。
- 规则共享边界：`list_postgres_category_rules`、`create_postgres_category_rule`、`list_postgres_account_rules`、`create_postgres_account_rule`、`query_postgres_rules_overview_payload`。
- row/value helpers：`account_from_postgres_row`、`category_from_postgres_row`、`tag_from_postgres_row`、`template_from_postgres_row`、`account_metadata_from_payload`、`strict_minor_units`、`normalize_template_transaction_type`。

### 4.2 Settings bundle repository 层

| 文件 | 行数/状态 | 当前职责 | 后续拆分方向 |
| --- | ---: | --- | --- |
| `src/backend/db/taxonomy/settings_bundle/postgres_import_export.rs` | 1544，超限 | PostgreSQL settings bundle 导入、dry-run、引用重映射、账户/分类/标签/模板/规则 upsert、minor units 校验和内部测试 | 拆成 `settings_bundle/postgres_import/{orchestrator,accounts,categories,tags,templates,category_rules,account_rules,ref_maps,minor_units,tests}.rs` |
| `types_and_normalization.rs` | 328 | settings bundle section normalization、导出 taxonomy sections、账户/分类/标签/template payload normalize | 可保留或按 normalize/export 拆分 |
| `export_formatters.rs` | 300 | settings bundle 导出格式化 | 可保留，后续补充导出函数说明 |
| `value_helpers.rs` | 196 | ref、safe value、minor units、category name 等 helper | 可保留为公共 helper，但复杂 helper 需要中文说明 |
| `mod.rs` | 112 | settings bundle facade 和 include 聚合 | 后续作为明确 re-export facade |

主要函数群：

- 导入编排：`import_postgres_settings_bundle`、`skip_postgres_settings_section`。
- 分段导入：`import_postgres_settings_accounts`、`import_postgres_settings_categories`、`import_postgres_settings_tags`、`import_postgres_settings_templates`、`import_postgres_settings_category_rules`、`import_postgres_settings_account_rules`。
- 引用和金额：`add_account_refs`、`add_category_refs`、`settings_template_values_from_item`、`settings_template_tag_ids`、`settings_strict_minor_units`、`strict_minor_units`。
- 导出/normalize：`normalize_settings_bundle_sections`、`export_taxonomy_sections`、`normalize_account_import`、`normalize_category_import`、`normalize_tag_import`、`resolve_template_payload`。

### 4.3 HTTP taxonomy route 层

| 文件 | 行数/状态 | 当前职责 | 后续拆分方向 |
| --- | ---: | --- | --- |
| `src/backend/http/taxonomy_routes/mod.rs` | 361 | Axum route 注册、route pattern 常量、共享 imports 和 facade | 保留 route facade，减少跨文件 mega-import；D10 可再整理 route shell |
| `account_handlers.rs` | 793，超限 | 账户 CRUD、display order、sync balances、move/clear transactions、余额 delta helper | 拆成 `account_handlers/{crud,display_order,balance_sync,move_transactions,clear_transactions,helpers}.rs` |
| `category_handlers.rs` | 553 | 分类 CRUD、all/flat/tree/export/statistics、move/batch/update-all | 可拆 `category_handlers/{crud,tree_export,bulk_mutation,statistics}.rs` |
| `tag_template_handlers.rs` | 490 | 标签和模板 CRUD/display order | 可拆 `tag_handlers.rs` 与 `template_handlers.rs` |
| `settings_bundle_handlers.rs` | 410 | settings bundle export/import/preview/section import/export | 可拆 `settings_bundle_handlers/{full_bundle,sections,authorization}.rs` |
| `account_category_formatters/accounts_and_tags.rs` | 688，超限 | account/tag/template 前后端投影、legacy account category/type 归一、cents 校验、测试 | 拆成 `account_category_formatters/{accounts,tags,templates,legacy_account_normalization,balance_projection,tests}.rs` |
| `account_rule_handlers.rs` / `category_rule_handlers.rs` | 571/284 | account/category rule API；D3 规则中心会继续拆分 | D2 只记录共享边界；若为 settings bundle 或账户编辑必须改动，需保持 D3 接口稳定 |

主要函数群：

- 账户 handler：`list_accounts_handler`、`get_account_handler`、`create_account_handler`、`update_account_handler`、`delete_account_handler`、`sync_account_balances_handler`、`move_account_transactions_handler`、`clear_account_transactions_handler`。
- 分类 handler：`list_categories_handler`、`flat_categories_handler`、`all_categories_handler`、`create_category_handler`、`update_category_handler`、`delete_category_handler`、`move_categories_handler`、`batch_create_categories_handler`、`export_categories_handler`、`category_statistics_handler`。
- 投影与兼容：`frontend_account_to_backend`、`backend_account_to_frontend`、`normalize_frontend_account_type`、`normalize_frontend_account_category`、`backend_tag_to_frontend`。
- Settings bundle handler：`export_settings_bundle_handler`、`import_settings_bundle_handler`、`preview_import_settings_bundle_handler`、`export_settings_bundle_section_*`、`import_settings_bundle_section_handler`。

## 5. 前端文件地图

### 5.1 账户页面与 store

| 文件 | 行数/状态 | 当前职责 | 后续拆分方向 |
| --- | ---: | --- | --- |
| `src/web/src/stores/account.ts` | raw 1038 / gate 1248，超限 | 账户缓存、category map、余额合计、显隐/排序、CRUD、同步余额、move/clear account transactions、服务请求去重 | 拆成 `stores/accounts/{state,derivedBalances,visibilityOrdering,serviceActions,mutations}.ts`，保留 `useAccountsStore` facade |
| `src/web/src/views/desktop/accounts/ListPage.vue` | raw 730 / gate 837，超限 | 账户列表页面、分类侧栏、余额概览、排序、显隐、对账、move/clear dialog 接线 | 拆成 `accounts/list/{AccountCategoryNav,AccountOverview,AccountListToolbar,AccountCardList,useAccountListPage}.ts/vue` |
| `src/web/src/views/desktop/accounts/list/dialogs/EditDialog.vue` | raw 644 / gate 727，超限 | 账户创建/编辑 dialog、父子账户、规则匹配编辑、余额和分类字段 | 拆成 `edit-dialog/{AccountBasicFields,SubAccountFields,AccountRuleEditor,useAccountEditDialog}.ts/vue` |
| `src/web/src/views/mobile/accounts/EditPage.vue` | raw 714 / gate 797，超限 | 移动端账户编辑页面 | 与 base `AccountEditPageBase.ts` 抽 shared composable，避免桌面/移动重复业务逻辑 |
| `src/web/src/views/mobile/accounts/ReconciliationStatementPage.vue` | 644 | 移动端对账明细、图表、虚拟列表、增删改交易入口 | D4 交易域会接管交易编辑面；D2 只拆账户对账页壳层 |

### 5.2 分类与标签页面

| 文件 | 行数/状态 | 当前职责 | 后续拆分方向 |
| --- | ---: | --- | --- |
| `src/web/src/views/desktop/categories/ListPage.vue` | raw 488 / gate 562，超限 | 分类列表、主分类切换、排序、隐藏、preset、settings section import/export | 拆成 `categories/list/{CategoryTypeNav,CategoryTable,CategoryToolbar,useCategoryListPage}.ts/vue` |
| `src/web/src/views/desktop/categories/list/dialogs/EditDialog.vue` | 538 | 分类编辑、分类规则 builder 同步 | 分类编辑保留 D2 身份字段，规则 builder 同步边界交给 D3；后续拆 `category-edit-dialog/**` |
| `src/web/src/views/desktop/tags/ListPage.vue` | raw 462 / gate 528，超限 | 标签列表、排序、隐藏、CRUD、settings section import/export | 拆成 `tags/list/{TagTable,TagToolbar,useTagListPage}.ts/vue` |
| `src/web/src/views/mobile/categories/ListPage.vue` / `src/web/src/views/mobile/tags/ListPage.vue` | 338 / 371 | 移动端分类/标签列表、排序和 CRUD | 与桌面 list composable 共享核心动作，保留移动 UI 壳 |
| `src/web/src/stores/transactionCategory.ts` / `transactionTag.ts` | 500 / 313 | 分类和标签缓存、CRUD、请求动作 | 当前不在 D2 progress owned paths 中，但属于 identity-settings 实际依赖；frontend-shape 前必须追加 owned paths 或记录 shared lease |

### 5.3 Settings 与共享路径

| 文件 | 行数/状态 | 当前职责 | 后续拆分方向 |
| --- | ---: | --- | --- |
| `src/web/src/lib/services.ts` | 2342，shared shell | 所有 REST service 方法；D2 使用账户、分类、标签、模板、settings bundle endpoints | D2 如需拆服务，只能提取 `services/taxonomy.ts` 或局部 adapter，并保持 default facade 兼容；全量拆分归 D10 |
| `src/web/src/views/base/settings/*SettingPageBase.ts` | 25-194 | 设置页过滤、cloud sync、app settings base 逻辑 | 账户/分类/标签 filter settings 可作为 D2 行为锁定只读依赖；cloud sync 归 D7 |
| `src/web/src/components/desktop/SettingsJsonImportExportButton.vue` | shared | per-section settings import/export 入口 | D2 可测试，不默认改组件 |
| `src/web/src/models/account.ts`、`transaction_category.ts`、`transaction_tag.ts`、`account_rule.ts` | shared models | 账户/分类/标签/rule DTO 与 request converter | D2 可补测试或注释；若拆模型需写 shared path lease |

## 6. 共享路径和 lease

D2 当前 progress owned paths：

- `src/backend/db/taxonomy/**`
- `src/backend/http/taxonomy_routes/**`
- `src/web/src/views/**/accounts/**`
- `src/web/src/views/**/categories/**`
- `src/web/src/views/**/tags/**`
- `src/web/src/stores/account.ts`

默认只读或需要单独 lease 的路径：

- `src/web/src/lib/services.ts`: 提取 D2 service adapter 必须独占 lease，并继续保留 default `services` facade。
- `src/web/src/stores/index.ts`: 只允许 D10 或明确 store facade 切片改动。
- `src/web/src/stores/transactionCategory.ts`、`src/web/src/stores/transactionTag.ts`: D2 frontend-shape 实际需要时应追加 owned paths 或记录 shared lease。
- `src/web/src/models/account.ts`、`transaction_category.ts`、`transaction_tag.ts`、`account_rule.ts`: DTO/请求转换测试可改，模型拆分需 lease。
- `src/backend/core/account_rules/**`、`src/backend/core/category_rules/**`: D3 规则域负责结构拆分；D2 只读规则匹配合同和 settings bundle 兼容。
- `docs/PROJECT_OVERVIEW.md`、structure baseline JSON: governance-docs/closeout 独占写入。

## 7. 现有测试覆盖地图

后端已存在测试入口：

- `tests/backend/db/settings_bundle_postgres.rs`: settings bundle export/import/upsert、minor units、模板 refs、dry-run/真实导入合同。
- `tests/backend/db/account_rules_postgres.rs`: account rule 与 account/template payload 兼容、scope 字段忽略、规则匹配。
- `tests/backend/db/category_rules_postgres.rs`: category rule 默认/列表合同。
- `src/backend/db/taxonomy/postgres_reads.rs` 内部测试：template transaction type normalization、rule helper normalization。
- `src/backend/http/taxonomy_routes/account_category_formatters/accounts_and_tags.rs` 内部测试：legacy account category/type 归一、explicit cents、balance discrepancy response。

前端已存在测试入口：

- `tests/web/stores/account.test.ts`: account store load 去重、parent/sub-account id map、sync balance 请求去重和缓存刷新。
- `tests/web/models/transaction_category.test.ts`: 分类模型、嵌套、request converter、icon resolver、collection helpers。
- `tests/web/models/transaction_tag.test.ts`: 标签模型和 request converter。
- `tests/web/models/account_rule.test.ts`: 当前 expression-only payload、旧 scope 字段不泄漏、账户规则分组。
- `tests/web/views/desktop/accounts/listPageAddButton.test.ts`: 桌面账户页 Add 行为合同。
- `tests/web/views/desktop/accounts/editDialogLayout.test.ts`: 账户编辑弹窗布局和 account matching editor 紧凑合同。
- `tests/web/views/desktop/accounts/reconciliationStatementDialog*.test.ts`: 对账弹窗 promise/options 合同。
- `tests/web/views/desktop/categories/listPageAddButton.test.ts`: 桌面分类页 Add 行为合同。
- `tests/web/views/desktop/categories/editDialog.test.ts`: 分类编辑与规则 builder 同步保护。
- `tests/web/views/desktop/settingsJsonPerPageImportExport.test.ts`: 账户、分类、标签、settings section import/export 入口和 service endpoint 合同。
- `tests/web/views/mobile/accountRuleListPage.test.ts`: 移动端账户规则列表，属于 D3 规则中心依赖边界。

G015 的 `behavior-lock` 必须补强或明确复用以下场景：

- 账户 CRUD：主账户/子账户、隐藏/显示、排序、余额同步、move/clear transactions 的 API 和 store 合同。
- 分类 CRUD：一级/二级分类、虚拟主分类、隐藏/显示、排序、preset、分类规则字段同步不漂移。
- 标签 CRUD：新增、编辑、隐藏、排序、批量接口和 settings section 导入导出。
- Settings bundle：accounts、transactionCategories、transactionTags、transactionTemplates、scheduledTransactions、categoryRecognitionRules、accountRecognitionRules 的 export/import/preview、引用重映射和 dry-run rollback。
- 金额合同：账户余额、期初余额、模板 amount/minor units、balance discrepancy 全部使用 cents/minor units。
- 浏览器或 API smoke：账户/分类/标签桌面页至少一条列表/新增/编辑/导入导出入口 smoke；settings bundle 可用 API smoke 兜底。

## 8. 后续切片执行顺序

### 8.1 behavior-lock

- 只允许测试、fixture、smoke、测试辅助和必要的合同文档。
- 禁止移动业务源码。
- 最小验证：
  - `cargo test -p bill-analyser-db --test settings_bundle_postgres`
  - `cargo test -p bill-analyser-db --test account_rules_postgres`
  - `cargo test -p bill-analyser-db --test category_rules_postgres`
  - `Set-Location src/web; npm run test -- --runTestsByPath ../../tests/web/stores/account.test.ts ../../tests/web/models/transaction_category.test.ts ../../tests/web/models/transaction_tag.test.ts ../../tests/web/views/desktop/settingsJsonPerPageImportExport.test.ts`
  - 一条账户/分类/标签或 settings bundle API/browser smoke 证据。
- 若发现真实 bug，当前切片写 `blocked:<reason>`，另开 fix slice。

### 8.2 backend-shape

建议顺序：

1. 先把 `postgres_reads.rs` 改为 taxonomy Postgres facade，再按 accounts/categories/tags/templates/rules-overview 拆生产子文件。
2. 拆 `settings_bundle/postgres_import_export.rs`，先保留 orchestrator 顺序，再按 section importer 和 ref/minor units helper 拆分。
3. 拆 `account_handlers.rs` 和 `account_category_formatters/accounts_and_tags.rs`，保持 route patterns、response envelope、legacy normalization 和 cents 字段不变。
4. `account_rule_handlers.rs`、`category_rule_handlers.rs` 只做 D2 必需边界整理；规则中心结构留给 D3。
5. 每一步运行 focused Rust tests；最终运行 fmt/clippy/coverage 门禁。

### 8.3 frontend-shape

建议顺序：

1. `src/web/src/stores/account.ts` 保留 `useAccountsStore` facade，拆 state/derived balance/mutation/service action。
2. 桌面账户页先拆 toolbar/category nav/overview/card list，再拆 edit dialog。
3. 分类和标签列表页拆 UI 壳与 composable，保留现有按钮、菜单、import/export 入口和布局。
4. 需要 category/tag store 或 model 改动前先写 shared lease；不在同一 PR 中全量拆 `services.ts`。
5. 跑前端 focused tests、lint、coverage；用户可见页面改动补 browser smoke。

### 8.4 comment-pass/governance-docs/closeout

- 按用户确认的注释策略补齐 D2 导出函数、业务关键函数和复杂私有 helper 中文说明；简单 getter、字段映射、事件转发不强制。
- 更新 `docs/PROJECT_OVERVIEW.md` 的 taxonomy/settings 当前事实。
- 收紧 Rust/frontend structure baseline，让已完成 D2 项退出 failure/warning。
- PR/CI/merge/delete/writeback 证据完整后，cursor 才能推进到 D3。

## 9. 关键风险与阻断条件

- `postgres_reads.rs` 同时包含主数据 CRUD、规则 CRUD、模板 CRUD、row mapping 和兼容 helper，拆分时必须先锁 route/API 和 settings bundle 行为。
- Settings bundle 导入顺序是业务合同：accounts -> categories -> tags -> templates -> scheduled -> category rules -> account rules；不能因拆模块改变引用重映射顺序。
- 账户余额和模板金额字段存在元/分历史风险；任何 D2 金额字段改动都必须人工复核 cents/minor units。
- `account_handlers.rs` 的 move/clear transactions 会触碰正式交易和审计日志，D4/D6 有共享风险；D2 只重构账户侧编排，不改变交易语义。
- 分类编辑 dialog 与分类规则 builder 共享字段，D3 会继续接管规则中心；D2 不得把规则表达式合同改成页面局部格式。
- `services.ts`、category/tag stores、models 和 settings import/export button 是共享面；没有 lease 不改。
- PR 自动合并必须证明 PR head SHA 与 CI head SHA 一致、CI passed、base fresh、terminal review passed、source branch deleted。

## 10. D2 完成判定

D2 完成后应满足：

- Taxonomy DB repository 不再由单个 `postgres_reads.rs` 承载账户、分类、标签、模板、规则和 helper 的全部实现。
- Settings bundle Postgres import/export 不再由单个大文件承载全部 section upsert、引用重映射和金额 helper。
- Account HTTP handler 和 account formatter 大文件拆成职责清晰的 route/helper 子模块。
- 账户 store 和账户/分类/标签页面入口只负责装配；复杂状态、余额派生、排序、显隐、CRUD/service action 分离。
- D2 直接相关结构门禁项不回涨，目标是移除 `postgres_reads.rs`、`settings_bundle/postgres_import_export.rs`、`account_handlers.rs`、`accounts_and_tags.rs`、`stores/account.ts`、账户/分类/标签大页面的超限状态。
- 行为锁定覆盖账户/分类/标签 CRUD、settings bundle export/import/preview、引用重映射、金额 minor units、legacy account/category normalization。
- PR/CI/merge/delete/writeback 证据完整，cursor 推进到 D3。
