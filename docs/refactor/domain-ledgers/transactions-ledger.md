# D4 交易/模板/周期/配对域重构 ledger

## 1. 切片边界

- Ultragoal story: `G017-complete-d4-transactions-templates-recurring-matching`
- 功能域: `D4 transactions-templates-recurring-matching`
- 基线提交: `5fd619de16c5dd9b77f4c4478538e606c118dcd7`
- 工作分支: `codex/structure-transactions-ledger`
- 本切片只建立结构债 ledger，不移动业务源码、不改 REST API、不改数据库合同、不改金额单位、不做 UI 视觉重设计。
- D4 接管正式交易列表、交易编辑、批量手工录入、正式账单 repository、周期建议/绑定、正式账单配对候选、交易 store 与交易页面装配。
- D1 导入预览、D2 身份设置、D3 规则中心已经完成的结构边界默认只读；D4 如需读取这些域的合同，只补测试或记录 shared lease，不回改已收口域。

## 2. 必须保持的业务合同

D4 重构时以下合同默认不可变：

- HTTP 运行态仍为 Rust Axum `REST /api/...`，正式账单、周期、配对、日历和净值相关路由不增加 sidecar 或旧式透传链路。
- 正式账单 DB 读写必须保持 user-scope、分页、筛选、排序、标签、账户、分类、模板来源、周期来源和审计边界。
- 金额字段继续使用整数分或显式 minor units；交易创建、批量创建、转账、投资、对账、导出和 OCR draft 都不能回退到裸元单位。
- 交易类型、分类类型、来源账户、目标账户、投资账户和转账账户的身份校验必须保持当前前后端投影合同。
- 创建、更新、批量更新、删除和批量删除正式账单后，相关账户余额、交易概览、统计缓存和对账状态仍按当前规则失效或同步。
- 交易图片上传、OCR 识别、data URL、文件名清洗和 unused picture 删除合同不变。
- 周期建议 list/detect/accept/reject、账单绑定/解绑周期模板、`next_date` 重算和周期候选匹配分数不变。
- 正式账单配对候选、手工配对、接受/拒绝/清除配对、suppression、manual pair 和 linked pair payload 不变。
- `matching_routes.rs` 同时包含 calendar events 与 networth snapshot；D4 只拆正式交易/周期/配对壳层，不改变预算/统计/资产趋势语义。
- 前端桌面和移动交易列表的 query 参数、筛选器、分页、模板快速新增、导入入口、导出入口、批量手工录入、编辑弹窗、图片 OCR、地理位置和可见布局保持。
- 发现真实 bug 或合同漂移时，当前结构切片写 `blocked:<reason>`，另开 fix slice。

## 3. 当前结构门禁证据

本切片执行了以下只读扫描：

- `node scripts/check-rust-backend-structure.mjs`
- `Set-Location src/web; npm run structure:check`
- D4 owned/shared paths 行数扫描和函数群 `rg` 扫描
- D4 相关测试入口扫描

当前门禁状态：

- Rust backend structure gate: 失败 10 项，其中 D4 直接或必须评估项包括：
  - `src/backend/db/bills/postgres_reads.rs`: gate 1484 行，超过 baseline 1247；原始扫描 1589 行。
  - `src/backend/db/recurring.rs`: gate 1227 行，超过 baseline 1182；原始扫描 1304 行。
  - `src/backend/core/adapters/transaction.rs`: gate 1478 行，超过 baseline 1474；原始扫描 1613 行，属于 D4 实际核心适配器但未在 progress owned paths 中列出。
  - `src/backend/core/matching/session_learning.rs`: gate 607 行，超过 baseline 601；原始扫描 667 行，属于正式配对/learning 候选共享边界。
  - `src/backend/http/matching_routes.rs`: 原始扫描 1655 行，当前未进结构 gate failure，但职责跨周期、matching、calendar、networth，应在 D4 backend-shape 中拆 route facade。
- Frontend structure gate: 失败 19 项，其中 D4 直接或必须评估项包括：
  - `src/views/desktop/transactions/ListPage.vue`: template 300 行，超过 baseline 298；原始扫描 2113 行，script 1229 行。
  - `src/views/desktop/transactions/list/dialogs/EditDialog.vue`: 2039 行，超过 baseline 1974；script 1272 行，超过 baseline 1257；style 92 行，超过 baseline 58；原始扫描 2040 行。
  - `src/views/desktop/transactions/list/dialogs/BatchManualEntryDialog.vue`: 原始扫描 1300 行，script 690 行，style 309 行；当前未列入 failure，但超过新文件治理目标。
  - `src/stores/transaction.ts`: 1724 行，超过 baseline 1557；原始扫描 1725 行。
  - `src/views/mobile/transactions/EditPage.vue`: 1633 行，超过 baseline 1602；template 133 行，超过 baseline 123；script 973 行，超过 baseline 955；原始扫描 1634 行。
  - `src/views/mobile/transactions/ListPage.vue`: 1492 行，超过 baseline 1490；template 210 行，超过 baseline 209；script 826 行，超过 baseline 825；原始扫描 1493 行。
  - `src/models/transaction.ts`: 965 行，超过 baseline 964；原始扫描 966 行，属于 D4 实际 DTO/模型边界但未在 progress owned paths 中列出。
- 非 D4 或后续域 failure 不在本域吞并：`desktop/budgets/ListPage.vue`、`src/lib/services.ts`、`src/stores/index.ts`、`src/core/theme.ts`、`src/models/imported_transaction.ts` 等分别归预算/统计、D10 shared shell、D1 或后续治理。

## 4. 后端文件地图

### 4.1 正式账单 repository 与适配器

| 文件 | 行数/状态 | 当前职责 | 后续拆分方向 |
| --- | ---: | --- | --- |
| `src/backend/db/bills.rs` | 158 | `BillRecord`、filter、hash、周期候选 DTO 与 bills facade | 保留 facade；只补必要中文说明 |
| `src/backend/db/bills/postgres_reads.rs` | raw 1589 / gate 1484，超限 | 正式账单查询、详情、标签、账户兜底、分类解析、创建、批量创建、更新、批量更新、删除、批量删除、余额同步、筛选 SQL、row mapping | 拆成 `bills/postgres_reads/{query,mutation,batch_mutation,identity,tags,balance_sync,filters,row_mapping,value_helpers,tests}.rs`，保留 public function facade |
| `src/backend/core/adapters/transaction.rs` | raw 1613 / gate 1478，超限 | 前后端交易类型/金额/时间转换、手工交易 payload、批量创建、图片上传响应、账户余额 delta、对账结果、导出 cell、校验 helper | 需要追加 shared path lease；拆成 `transaction/{types,mutation_payloads,batch_routes,pictures,reconciliation,export_cells,value_helpers,tests}.rs` |
| `src/backend/http/bill_routes/**` | 多小文件 | 正式账单 CRUD、导出、图片、周期绑定 route handler | D4 可读写 route 适配，但不改变 route path、response envelope 或上传安全合同 |
| `src/backend/db/user_data.rs` | shared | 清空用户交易和 tag name 辅助 | D7/账号数据共享；D4 仅在清空交易行为锁定中只读 |

主要函数群：

- 正式账单查询：`query_postgres_bills`、`get_postgres_bill_by_id`、`get_postgres_bill_tags`、`push_bill_filters`、`push_category_filters`、`push_amount_filter_cents`、`bill_record_from_postgres_row`。
- 正式账单写入：`create_postgres_bill`、`batch_create_postgres_bills`、`batch_create_postgres_bills_in_transaction`、`update_postgres_bill`、`batch_update_postgres_bills`、`delete_postgres_bill`、`batch_delete_postgres_bills`。
- 身份和金额：`prepare_postgres_bill_mutation`、`resolve_postgres_category_id_for_fields`、`validate_postgres_bill_account_identity`、`amount_cents_from_record`、`destination_amount_cents`、`canonical_transaction_type`。
- 前后端适配：`frontend_transaction_mutation_to_backend`、`batch_create_transaction_items`、`validate_bill_create_fields`、`frontend_transaction_from_backend`、`transaction_list_type_filter`。
- 图片和 OCR 边界：`transaction_picture_upload_id`、`secure_picture_file_name`、`transaction_picture_mime_type`、`transaction_picture_upload_success_response`、`missing_unused_transaction_picture_id_response`。
- 对账和账户同步：`sync_account_ids_for_bill`、`calculate_account_balance_from_bills`、`parse_reconciliation_query`、`build_reconciliation_filters`、`calculate_reconciliation_summary`、`build_reconciliation_transactions`。

### 4.2 周期模板与周期建议

| 文件 | 行数/状态 | 当前职责 | 后续拆分方向 |
| --- | ---: | --- | --- |
| `src/backend/db/recurring.rs` | raw 1304 / gate 1227，超限 | 周期建议 count/list/detect/save/accept/reject、账单绑定/解绑周期、周期模板读取、候选匹配、`next_date` 重算、row serialization、helper tests | 拆成 `recurring/{suggestions,detect,bind_unbind,templates,candidates,schedule_dates,row_mapping,value_helpers,tests}.rs` |
| `src/backend/core/matching/recurring.rs` | 314 | 周期模式检测、频率识别、suggestion serialization | D4 可读写检测核心测试；若拆需保持 public API 和导入 stage2 复用合同 |
| `src/backend/db/taxonomy/postgres_reads/templates.rs` | shared | 交易模板 CRUD | D2 已接管 taxonomy/templates；D4 只读模板 CRUD，必要改动需要 lease |
| `src/backend/http/bill_routes/recurring_handlers.rs` | 93 | 正式账单 recurring candidates/bind/unbind route | 可与 D4 backend-shape 一起归入 bill route 子域；保持 API 不变 |

主要函数群：

- 周期建议：`count_postgres_recurring_suggestions`、`list_postgres_recurring_suggestions`、`detect_and_save_postgres_recurring_suggestions`、`accept_postgres_recurring_suggestion`、`reject_postgres_recurring_suggestion`。
- 周期绑定：`get_postgres_bill_recurring_candidates`、`bind_postgres_bill_to_recurring`、`unbind_postgres_bill_from_recurring`、`get_postgres_bills_linked_to_recurring`。
- 周期模板和日期：`list_enabled_postgres_recurring_templates`、`build_postgres_recurring_candidates_for_bill_data`、`pg_recalculate_recurring_next_date_on_tx`、`pg_recurring_due_on_date`、`pg_next_recurring_occurrence_after`。

### 4.3 配对与 matching route

| 文件 | 行数/状态 | 当前职责 | 后续拆分方向 |
| --- | ---: | --- | --- |
| `src/backend/http/matching_routes.rs` | 1655，职责超宽 | calendar events、networth snapshot、recurring suggestions、matching session/bill candidates、manual pairs、accept/reject/clear、reconcile history、response helper 和 payload parser | 拆成 `matching_routes/{router,calendar_networth,recurring_suggestions,candidates,manual_pairs,candidate_actions,reconcile_history,payloads,response_helpers,tests}.rs`，保留 route facade |
| `src/backend/db/matching.rs` | 79 | matching DB facade | 保留 facade |
| `src/backend/db/matching/postgres_reads.rs` | 565 | matching pairs、bill/session candidates、bill feedback、reconciliation candidates、manual pair create/delete、row mapping | 当前低于 600，可保留或随 route 拆分补注释 |
| `src/backend/core/matching/session_learning.rs` | raw 667 / gate 607，超限 | transfer/duplicate/investment/learning candidate 构造、reconciliation query parse、candidate action payload、learning similarity scoring | 需要追加 shared path lease；拆成 `matching/session_learning/{transfer,duplicate,investment,query,actions,learning_similarity,summary}.rs` |

主要函数群：

- HTTP handlers：`matching_session_candidates_handler`、`matching_bill_candidates_handler`、`matching_candidates_handler`、`matching_bill_feedback_handler`、`matching_pairs_handler`、`create_manual_pair_handler`、`delete_manual_pair_handler`、`accept_matching_candidate_handler`、`reject_matching_candidate_handler`、`clear_matching_candidate_handler`、`reconcile_history_handler`。
- Payload 和响应：`matching_session_candidates_response`、`matching_bill_candidates_response`、`matching_candidate_action_response`、`preview_action_request_from_payload`、`expected_state_from_payload`、`recurring_candidate_from_payload`、`reconciliation_filters_from_value`、`matching_history_pair_key`。
- DB matching：`query_postgres_matching_pairs_payload`、`query_postgres_matching_bill_candidates_payload`、`query_postgres_matching_session_candidates_payload`、`query_postgres_matching_bill_feedback_payload`、`query_postgres_reconciliation_candidates_payload`、`create_postgres_manual_matching_pair`、`delete_postgres_manual_matching_pair`。

## 5. 前端文件地图

### 5.1 Store、模型与 service 边界

| 文件 | 行数/状态 | 当前职责 | 后续拆分方向 |
| --- | ---: | --- | --- |
| `src/web/src/stores/transaction.ts` | raw 1725 / gate 1724，超限 | 交易列表/对账状态、筛选、分页、CRUD、批量创建/更新/删除、导出、图片 OCR、receipt draft normalization、错误映射、缓存失效 | 拆成 `stores/transactions/{state,filters,serviceActions,mutations,receiptRecognition,reconciliation,errors}.ts`，保留 `useTransactionsStore` facade |
| `src/web/src/models/transaction.ts` | raw 966 / gate 965，超限 | 交易 DTO、page wrapper、amount request、默认交易、category/account/tag/picture 投影、日期/金额 helper | 需要 shared path lease；拆成 `models/transaction/{types,factory,amountRequests,responseMapping,pictures,helpers}.ts` |
| `src/web/src/models/bill_matching.ts` | 低于阈值 | matching candidates 和 linked pair payload 归一 | 作为 behavior-lock 入口，结构移动前先扩测试 |
| `src/web/src/stores/transactionTemplate.ts`、`src/web/src/models/transaction_template.ts` | shared | 交易模板 store/model | D2 taxonomy/templates 已接管 CRUD；D4 只消费模板快速新增和周期绑定字段，改动前写 shared lease |
| `src/web/src/stores/recurring.ts`、`src/web/src/models/recurring_suggestion.ts` | shared | 周期建议 store/model | D4 可接管周期建议消费侧；若拆 store/model 需要追加 owned paths |
| `src/web/src/lib/services.ts` | shared shell failure | 交易、模板、周期、matching REST service 聚合 | D10 负责全量 services 拆分；D4 不直接全量拆，只可局部 adapter + facade 兼容 |

主要函数群：

- `transaction.ts` store：receipt OCR error 映射、draft normalization、`useTransactionsStore` 内交易列表加载、创建/更新/删除、批量操作、导出、对账、图片识别与缓存失效。
- `transaction.ts` model：交易创建、响应投影、金额请求类型、page wrapper 和图片信息投影。

### 5.2 桌面交易页面

| 文件 | 行数/状态 | 当前职责 | 后续拆分方向 |
| --- | ---: | --- | --- |
| `src/web/src/views/desktop/transactions/ListPage.vue` | raw 2113；template 125；script 1229；gate template 300 warning | 桌面交易列表、tab、模板快速新增、筛选菜单、分页、批量入口、导入导出、图片识别入口、编辑弹窗装配 | 拆成 `desktop/transactions/list/{template,style,useTransactionListPage,filters,toolbar,pagination,dialogs}.ts/vue`；保持当前布局和 query 合同 |
| `src/web/src/views/desktop/transactions/list/dialogs/EditDialog.vue` | raw 2040 / gate 2039，超限 | 桌面交易新增/编辑/复制/详情、金额、分类、账户、标签、图片、OCR、地理位置、周期候选、matching panel、只读背景 | 拆成 `edit-dialog/{template,style,useTransactionEditDialog,amounts,categories,accounts,pictures,geolocation,recurring,billMatching}.ts/vue` |
| `src/web/src/views/desktop/transactions/list/dialogs/BatchManualEntryDialog.vue` | raw 1300，超限 | 批量手工录入表格、行复制/填充、键盘导航、目标金额同步、校验、提交 | 拆成 `batch-manual-entry/{template,style,useBatchManualEntryDialog,rows,validation,keyboard,fieldSync}.ts` |
| `src/web/src/views/desktop/transactions/list/dialogs/BillMatchingPanel.vue` | 492 | 正式账单 matching 候选展示、接受/拒绝/清除 | 当前低于阈值；作为 behavior-lock 和 EditDialog 子组件边界 |
| `src/web/src/views/desktop/transactions/import/**` | D1 已接管 | 导入预览 | D4 只保留 ListPage 入口调用，不改导入预览结构 |

主要函数群：

- `ListPage.vue`: `init`、`reload`、`changePageType`、`changeDateFilter`、`changeTypeFilter`、`changeCategoryFilter`、`changeAccountFilter`、`changeTagFilter`、`changeAmountFilter`、`add`、`addByRecognizingImage`、`batchAdd`、`importTransaction`、`exportTransactions`、`show`、`scrollMenuToSelectedItem`。
- `EditDialog.vue`: 交易打开/保存/复制/删除、周期候选刷新/绑定/解绑、图片上传/OCR、地理位置、matching panel 更新与只读状态。
- `BatchManualEntryDialog.vue`: `createTransactionRow`、`createRowFromTransaction`、`applyCommonFields`、`fillEmptyCommonFields`、`isRowValid`、`getRowIssues`、`onCellKeydown`、`submit`。

### 5.3 移动交易页面

| 文件 | 行数/状态 | 当前职责 | 后续拆分方向 |
| --- | ---: | --- | --- |
| `src/web/src/views/mobile/transactions/ListPage.vue` | raw 1493 / gate 1492，超限 | 移动交易列表、筛选 popover/sheet、分页加载、add/duplicate/edit/remove、月份折叠和滚动 | 拆成 `mobile/transactions/list/{template,style,useMobileTransactionListPage,filters,actions,scroll}.ts/vue` |
| `src/web/src/views/mobile/transactions/EditPage.vue` | raw 1634 / gate 1633，超限 | 移动交易新增/编辑/复制、金额/分类/账户 sheet、周期字段、图片 OCR、地理位置、page lifecycle | 拆成 `mobile/transactions/edit-page/{template,style,useMobileTransactionEditPage,amounts,categories,pictures,geolocation,lifecycle}.ts/vue` |
| `src/web/src/views/mobile/transactions/AmountFilterPage.vue` | 170 | 移动金额筛选页 | 当前低于阈值，作为筛选合同测试入口 |
| `src/web/src/views/mobile/transactions/ImportPreviewPage.vue` | D1 共享 | 移动导入预览 parity | D4 不改导入预览结构，只读 |

主要函数群：

- `mobile/ListPage.vue`: `init`、`reload`、`loadMore`、`changePageType`、`changeDateFilter`、`changeCustomDateFilter`、`changeTypeFilter`、`changeCategoryFilter`、`changeAccountFilter`、`changeTagFilter`、`changeAmountFilter`、`add`、`duplicate`、`edit`、`remove`、`collapseTransactionMonthList`、`onScroll`。
- `mobile/EditPage.vue`: `parseStrictQueryCents`、`getPageTypeNameMode`、`addDefaultCategoriesAndOpenSheet`、`init`、`save`、`updateGeoLocation`、`clearGeoLocation`、`uploadPicture`、`recognizeUploadedPicture`、`applyReceiptRecognitionResult`、`duplicate`、page lifecycle hooks。

## 6. 共享路径和 lease

D4 当前 progress owned paths：

- `src/backend/db/bills/**`
- `src/backend/http/matching_routes.rs`
- `src/backend/db/recurring.rs`
- `src/web/src/views/**/transactions/**`
- `src/web/src/stores/transaction.ts`

默认只读或需要单独 lease 的路径：

- `src/backend/core/adapters/transaction.rs`: D4 实际核心适配器，structure gate 已失败；backend-shape 前必须追加 shared path lease 或更新 progress owned paths。
- `src/backend/core/matching/session_learning.rs`、`src/backend/core/matching/recurring.rs`: matching/recurring 核心合同；D4 可读写前需明确 shared lease。
- `src/backend/db/matching/postgres_reads.rs`、`src/backend/db/matching.rs`: matching repository；当前未在 owned paths 中，若拆应追加 lease。
- `src/backend/http/bill_routes/**`: 正式账单 route helper；D4 backend-shape 需要时应追加 owned paths。
- `src/web/src/models/transaction.ts`、`src/web/src/models/bill_matching.ts`: D4 前端模型边界；拆分前追加 lease 并覆盖模型测试。
- `src/web/src/stores/transactionTemplate.ts`、`src/web/src/stores/recurring.ts`、`src/web/src/models/transaction_template.ts`、`src/web/src/models/recurring_suggestion.ts`: 模板/周期 store/model，共享 D2 taxonomy 与 D4 消费侧。
- `src/web/src/lib/services.ts`、`src/web/src/stores/index.ts`: D10 shared shell；D4 不做全量拆分。
- `src/backend/db/taxonomy/**`、`src/backend/http/taxonomy_routes/**`: D2 已收口的模板、账户、分类、标签 CRUD；D4 只读或申请 lease。
- `src/web/src/views/desktop/templates/**`、`src/web/src/views/mobile/templates/**`: 模板管理页面；当前不在 D4 owned paths，若后续模板页面结构债需要单独 steering 或 shared lease。
- `src/web/src/views/desktop/transactions/import/**`、`src/web/src/models/imported_transaction.ts`: D1 导入预览；D4 只保留入口调用。

## 7. 现有测试覆盖地图

后端已存在测试入口：

- `tests/backend/db/bills_postgres.rs`: 正式账单 query/create/batch/identity/cents/user-scope 合同。
- `tests/backend/db/matching_postgres.rs`: matching manual pair、feedback 和 materialization payload round trip。
- `tests/backend/core/transaction_adapter_contracts.rs`: 前后端交易适配、金额、对账、图片和 route response 合同。
- `tests/backend/core/matching_contracts.rs`: matching candidate id、pair feedback、recurring/learning 等核心合同。
- `src/backend/http/matching_routes_contract_tests.rs`: matching route envelope、query validation 和 candidate action contract。
- `src/backend/http/bill_routes/**` 内部 tests：bill route success/error envelope。
- `tests/backend/db/settings_bundle_postgres.rs`: transactionTemplates/scheduledTransactions 的 settings bundle 引用重映射合同，D4 只读。

前端已存在测试入口：

- `tests/web/stores/transaction.core.test.ts`: 交易 store load/save/move/delete/cache invalidation 合同。
- `tests/web/stores/transaction.recognizeReceiptImage.test.ts`: receipt OCR draft normalization、候选字段和错误映射。
- `tests/web/lib/services.transaction.test.ts`、`tests/web/lib/services.transactionFacade.test.ts`: 交易 service facade 合同。
- `tests/web/models/bill_matching.test.ts`: matching candidate、linked pair、explicit cents 和 bill snapshot 投影。
- `tests/web/models/transaction_picture_info.test.ts`、`tests/web/models/recurring_suggestion.test.ts`: 图片和周期建议模型。
- `tests/web/views/desktop/transactions/editDialogPictureOcr.test.ts`: 桌面编辑弹窗图片 OCR 源码合同。
- `tests/web/views/desktop/transactions/editDialogReadonlyBackground.test.ts`: 编辑弹窗只读背景合同。
- `tests/web/views/desktop/transactions/aiImageRecognitionDialog.test.ts`: AI 图片识别入口。
- `tests/web/views/mobile/transactionEditPagePictureOcr.test.ts`: 移动编辑页图片 OCR 和 cents 转换。
- `tests/web/views/mobile/transactionEditPageInvestment.test.ts`: 移动编辑页投资字段和 reset watcher。
- `tests/web/views/mobile/transactionListPageInvestment.test.ts`: 移动列表投资展示和金额筛选。

D4 `behavior-lock` 必须补强或明确复用以下场景：

- 正式账单 CRUD/batch：收入、支出、转账、投资、标签、分类、来源账户、目标账户、模板来源、周期来源、删除余额同步。
- 交易列表筛选：日期、月份、类型、分类、账户、标签、金额、关键词、分页和 query 参数双向同步。
- 桌面编辑弹窗：新增/编辑/复制/只读、金额同步、分类默认值、图片 OCR、地理位置、周期候选绑定/解绑、matching panel action。
- 批量手工录入：行复制、向下填充、空字段填充、键盘导航、目标金额同步、有效性判断和一次提交事务。
- 移动列表/编辑 parity：筛选、add/duplicate/edit/remove、OCR cents、投资和转账字段。
- 正式账单 matching：transfer/duplicate/investment/learning candidates、manual pair、accept/reject/clear、suppression 和 linkedPair payload。
- 周期建议：detect/list/accept/reject、`next_date` 重算、active date/due date 规则和候选序列化。

## 8. 后续切片执行顺序

### 8.1 behavior-lock

- 只允许测试、fixture、smoke、测试辅助和必要合同文档。
- 禁止移动业务源码。
- 优先补强正式账单 DB、transaction adapter、matching routes、transaction store、桌面/移动页面源码合同。
- 最小验证：
  - `cargo fmt --all -- --check`
  - `cargo test -p bill-analyser-db --test bills_postgres`
  - `cargo test -p bill-analyser-db --test matching_postgres`
  - `cargo test -p bill-analyser-core --test transaction_adapter_contracts`
  - `cargo test -p bill-analyser-core --test matching_contracts`
  - `cargo test -p bill-analyser-http matching`
  - `Set-Location src/web; npm run test -- --runTestsByPath ../../tests/web/stores/transaction.core.test.ts ../../tests/web/stores/transaction.recognizeReceiptImage.test.ts ../../tests/web/models/bill_matching.test.ts ../../tests/web/views/desktop/transactions/editDialogPictureOcr.test.ts ../../tests/web/views/mobile/transactionEditPagePictureOcr.test.ts ../../tests/web/views/mobile/transactionListPageInvestment.test.ts`
  - `Set-Location src/web; npm run lint:ci`
- 若发现真实 bug，当前切片写 `blocked:<reason>`，另开 fix slice。

本次 behavior-lock 切片执行证据（2026-06-21）：

- 新增后端 adapter 合同：`tests/backend/core/transaction_adapter_contracts.rs` 锁定转账和投资写入时 `sourceAmountCents` 转负、`destinationAmountCents` 原样保留、来源/目标账户和分类/tag metadata 不丢失。
- 新增前端 store 合同：`tests/web/stores/transaction.core.test.ts` 锁定交易列表 URL query 与导出请求共享同一筛选合同，覆盖自定义日期、投资类型、分类、账户、标签、tag filter、金额区间和关键词编码。
- 新增桌面批量录入源码合同：`tests/web/views/desktop/transactions/batchManualEntryDialog.test.ts` 锁定批量录入的 destination 字段键盘网格、转账/投资目标金额同步、非空行校验、批量 `saveTransactions` 提交、批量覆盖/填空分工，以及桌面列表把当前筛选上下文传给批量录入弹窗。
- 通过验证：
  - `cargo fmt --all -- --check`
  - `cargo test -p bill-analyser-db --test bills_postgres`
  - `cargo test -p bill-analyser-db --test matching_postgres`
  - `cargo test -p bill-analyser-core --test transaction_adapter_contracts`
  - `cargo test -p bill-analyser-core --test matching_contracts`
  - `cargo test -p bill-analyser-http matching`
  - `Set-Location src/web; npm run test -- --runTestsByPath ../../tests/web/stores/transaction.core.test.ts ../../tests/web/stores/transaction.recognizeReceiptImage.test.ts ../../tests/web/models/bill_matching.test.ts ../../tests/web/views/desktop/transactions/editDialogPictureOcr.test.ts ../../tests/web/views/desktop/transactions/batchManualEntryDialog.test.ts ../../tests/web/views/mobile/transactionEditPagePictureOcr.test.ts ../../tests/web/views/mobile/transactionListPageInvestment.test.ts`
  - `Set-Location src/web; npm run lint:ci`（0 errors，保留既有 `no-explicit-any` warnings）
- 覆盖率门禁：本切片只改测试和 ledger 文档，未改业务运行时代码；Rust full-runtime coverage 与被改业务源码 90% 核算不适用。

### 8.2 backend-shape

建议顺序：

1. 先追加或记录 `core/adapters/transaction.rs`、`core/matching/session_learning.rs`、`db/matching/postgres_reads.rs`、`http/bill_routes/**` 的 shared lease，避免实际 D4 结构债落在未授权路径。
2. 拆 `db/bills/postgres_reads.rs`，先保留 public repository function facade，再按 query、mutation、identity、tags、balance sync、filter SQL 和 row mapping 拆分。
3. 拆 `core/adapters/transaction.rs`，优先分离 mutation payload、batch route、picture、reconciliation、export cell、value helper；保持 route response payload 字段不变。
4. 拆 `db/recurring.rs`，把 suggestions、detect/save、bind/unbind、template/candidate、schedule date helper 和 row mapping 分开。
5. 拆 `http/matching_routes.rs`，以 router facade 聚合 recurring suggestions、matching candidates、manual pairs、candidate actions、reconcile history、calendar/networth shell。
6. 如 `core/matching/session_learning.rs` 仍触发结构门禁，拆 transfer/duplicate/investment/learning candidate builder 和 query/action helper。
7. 每一步跑 focused Rust tests；最终运行 fmt、clippy、coverage 门禁。

### 8.3 frontend-shape

建议顺序：

1. 先拆 `src/web/src/stores/transaction.ts` 为 facade + state/filter/service action/receipt/reconciliation/error 子文件，保持 `useTransactionsStore` 导出名不变。
2. 拆桌面 `ListPage.vue` 的 template/style/filter/menu/pagination/dialog orchestration；保留当前桌面交易列表视觉和 query 合同。
3. 拆桌面 `EditDialog.vue` 的 template/style、金额/分类/账户、图片 OCR、地理位置、周期候选和 matching panel 接线。
4. 拆 `BatchManualEntryDialog.vue` 的 template/style、row model、validation、keyboard navigation 和 submit 编排。
5. 拆移动 `ListPage.vue` 与 `EditPage.vue` 的 template/style/composable；保持移动交互和投资/转账字段 parity。
6. `models/transaction.ts` 若仍触发结构门禁，作为 shared lease 拆 DTO/types/factory/response mapping/helper。
7. 跑前端 focused tests、lint、coverage；页面结构变化补 browser smoke 或源码合同说明。

### 8.4 comment-pass/governance-docs/closeout

- 按用户确认策略补齐 D4 导出函数、业务关键函数和复杂私有 helper 中文说明；简单 getter、字段映射和事件转发不强制。
- 更新 `docs/PROJECT_OVERVIEW.md` 中正式交易、周期、matching 当前 facade + 功能文件夹事实。
- 收紧 Rust/frontend structure baseline，让已完成 D4 项退出 failure/warning；非 D4 历史债不得在 D4 governance-docs 中吞并。
- PR/CI/merge/delete/writeback 证据完整后，cursor 才能推进到 D5。

## 9. 关键风险与阻断条件

- `db/bills/postgres_reads.rs` 同时处理 SQL、事务、身份校验、标签、余额同步和 row mapping；拆分前必须先锁 user-scope、transaction rollback、cents 字段和账户余额副作用。
- `core/adapters/transaction.rs` 是前后端金额和类型转换核心；任何金额字段改动都必须人工复核元/分转换，默认不改字段含义。
- `matching_routes.rs` 同时混入 calendar/networth；D4 结构拆分不得改变统计、资产趋势或预算依赖的响应语义。
- 周期建议依赖 `transaction_templates` 和正式账单历史；D2 已接管模板 CRUD，D4 不能重写 settings bundle 或 taxonomy template 合同。
- 桌面交易列表仍承载导入入口；D1 导入预览已收口，D4 不得改导入预览流程。
- 交易图片 OCR 与 LLM/OCR provider 属 D8 共享边界；D4 只处理正式交易编辑侧的调用和合同测试，不新增 provider 语义。
- `services.ts`、`stores/index.ts`、transaction template/recurring stores 是共享面；没有 lease 不改。
- PR 自动合并必须证明 PR head SHA 与 CI head SHA 一致、CI passed、base fresh、terminal review passed、source branch deleted。

## 10. D4 完成判定

D4 完成后应满足：

- 正式账单 repository 不再由单个 `postgres_reads.rs` 承载查询、写入、批量、身份、标签、余额同步、筛选和 row mapping。
- `core/adapters/transaction.rs` 不再由单文件承载交易 DTO 转换、批量 route、图片、对账、导出和 helper。
- `db/recurring.rs` 不再由单文件承载周期建议、检测、绑定、模板候选、日期调度和 serialization。
- `matching_routes.rs` 被拆成 route facade + recurring/matching/manual pair/action/reconcile/calendar-networth 子模块，route path 和 response envelope 保持。
- 桌面交易列表、编辑弹窗、批量手工录入、移动列表和移动编辑页只负责装配；复杂状态、筛选、表单、图片、周期和 matching action 已分离。
- `stores/transaction.ts` 和必要的交易模型共享文件退出 D4 结构 failure 或有明确治理 baseline。
- 行为锁定覆盖正式交易 CRUD/batch、金额 cents、身份校验、账户余额同步、周期建议/绑定、matching candidate action、桌面/移动交易页面关键交互。
- PR/CI/merge/delete/writeback 证据完整，cursor 推进到 D5。
