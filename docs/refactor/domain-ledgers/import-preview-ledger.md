# D1 导入预览域结构债 Ledger

## 1. 切片边界

- Ultragoal story: `G010-complete-d1-import-preview-pilot-led`
- 功能域: `D1 import-preview-pilot`
- 基线提交: `c0fe2e76301a245d42aed451dd4bb3cafc5f673c`
- 工作分支: `codex/structure-import-preview-ledger`
- 本切片只建立结构债 ledger，不移动业务源码、不改 REST API、不改数据库合同、不改导入语义、不做 UI 视觉重设计。
- 后续切片必须先完成 `behavior-lock`，再进入 `backend-shape` 和 `frontend-shape`。

## 2. 必须保持的业务合同

导入预览域重构时以下合同默认不可变：

- HTTP 运行态仍为 Rust Axum `REST /api/...`，不能增加 sidecar 或旧式透传链路。
- parser-first 上传保持每个文件 exactly-one dedicated parser 决策；未命中或冲突文件不生成标准账单。
- 金额字段进入业务 DTO、preview、confirm、正式账单前必须是整数分或显式 minor units；元单位只保留在 parser/OCR/原始输入边界。
- stage2 顺序保持 parser/standard row -> 同批/跨批 dedup/transfer/history materialization -> 分类/类型规则和 fallback -> recurring -> learning/vector recall -> account_rules -> preview/update/reclassify -> confirm。
- preview 分页、筛选、排序、facets、跨页选择由后端在 `import_preview_rows` 上执行；前端只保留当前页草稿和选择差量。
- `signal=transfer` 只匹配 `preview_matching_feedback.transfer` 等结构化转账匹配反馈，不把普通 `transaction_type=转账` 当作转账匹配信号。
- `category_id`、来源账户、目标账户必须来自当前用户 active 主数据；`0`、不存在、inactive、跨用户、类型不匹配、同账户转账和 sentinel 均不能进入 preview 或正式账单。
- preview update/reclassify 必须落库；普通 update 不能覆盖 server-authoritative `matching_feedback`，只能清除明确 family 的 actionable 建议。
- confirm 必须在事务内写入正式 bills/tags/accounts/learning side effects，并校验 selected preview ids、history rewrite acknowledgement 和当前用户身份。

## 3. 当前结构门禁证据

本切片执行了以下只读扫描：

- `node scripts/check-rust-backend-structure.mjs`
- `Set-Location src/web; npm run structure:check`
- D1 owned paths 行数扫描和函数群 `rg` 扫描
- 导入相关测试入口扫描

当前门禁状态：

- Rust backend structure gate: 失败 21 项，其中 D1 直接相关项包括：
  - `src/backend/db/import_staging.rs`: 6000 lines grew beyond baseline 2791；原始扫描 6024 行。
  - `src/backend/http/import_routes/stage_handlers.rs`: 3672 lines grew beyond baseline 2950；原始扫描 3860 行。
  - `src/backend/http/import_routes/preview_mutation_helpers.rs`: 1243 lines grew beyond baseline 909；原始扫描 1254 行。
  - `src/backend/db/import_staging/preview_drafts.rs`: 655 lines exceeds new-file limit 600。
  - `src/backend/db/import_staging/types.rs`: 611 lines exceeds new-file limit 600。
- Frontend structure gate: 失败 51 项，其中 D1 直接相关项包括：
  - `src/views/desktop/transactions/import/tabs/ImportTransactionCheckDataTab.vue`: gate 5977 行，template 466 行，script 5132 行；原始扫描 5279 行。
  - `src/views/desktop/transactions/import/ImportDialog.vue`: gate 2256 行，template 131 行，script 1703 行，style 82 行；原始扫描 2005 行。
  - `src/views/desktop/transactions/import/checkDataMatching.ts`: gate 1080 行；原始扫描 957 行。
  - `src/views/desktop/transactions/import/importPreviewIndex.ts`: 670 行，超过新文件 600 行限制。
  - `src/models/imported_transaction.ts`: 623 行，属于导入预览模型依赖面，后续改动需要共享路径 lease。

## 4. 后端文件地图

### 4.1 DB repository 层

| 文件 | 行数/状态 | 当前职责 | 后续拆分方向 |
| --- | ---: | --- | --- |
| `src/backend/db/import_staging.rs` | 6024，严重超限 | session/source/template/standard-row/preview/page query/selection/update/identity validation/confirm/history materialization/learning lifecycle/LLM memory 的混合 repository facade 和实现 | 保留 facade，拆到 `import_staging/{sessions,sources,standard_rows,preview_query,preview_mutations,selection,identity_validation,confirm,history_materialization,learning_lifecycle,llm_memory,row_mapping}.rs` |
| `src/backend/db/import_staging/types.rs` | 611，超限 | 导入 session、preview row、query filters、patch、decision、learning/LLM DTO、confirm acknowledgement 类型 | 拆成 `types/{session,preview_query,preview_patch,decision,learning,llm,confirm}.rs`，`types.rs` 只 re-export |
| `src/backend/db/import_staging/preview_drafts.rs` | 655，超限 | `DedupBill`/历史正式账单到 `ImportPreviewDraft` 的投影，transfer/history/dedup feedback 构造 | 拆为 `preview_drafts/{dedup,history_duplicate,history_transfer,feedback,helpers}.rs` |
| `src/backend/db/import_staging/ledger_types.rs` | 小文件 | import decision/materialization ledger 类型 | 可保留；后续只在 ledger 或 history materialization 拆分时移动 |

主要函数群：

- session 生命周期：`create_import_session`、`update_import_session_status`、`get_import_session`、`clear_session_data`、`clear_user_import_staging_data`。
- source/template/standard row staging：`stage_import_parser_templates_with_sources`、`stage_import_parser_templates`、`insert_parser_templates_batch`、`upsert_import_sources`、`insert_standard_rows_and_parser_payloads`。
- preview 写入与查询：`insert_preview_bills_batch`、`get_preview_page_by_session`、`query_preview_page_by_session`、`build_preview_page_query`、`push_preview_query_predicates`、`query_preview_facets`。
- preview 筛选语义：`push_preview_signal_predicate`、`push_preview_parser_signal_condition`、`push_preview_transfer_signal_condition`、`signal_filter_matches`、`preview_signal_family_matches`。
- selection 和批量更新：`replace_preview_selection_with_patches`、`apply_preview_patches_preserving_selection`、`update_session_preview_selection_by_query`、`batch_update_preview_classification`。
- 身份校验：`load_import_identity_maps`、`apply_identity_validation_to_draft`、`apply_identity_validation_to_preview`、`normalize_identity_values`。
- confirm：`confirm_preview_to_bills`、`confirm_preview_to_bills_with_ack`、`bill_create_fields_from_preview`。
- learning/LLM/history：`apply_preview_learning_decision`、`apply_preview_llm_recommendation`、`review_preview_llm_recommendation`、`record_import_learning_lifecycle_feedback`、`insert_import_history_materializations_batch`。
- row mapping 和 patch：`preview_from_pg_row`、`apply_patch_value_to_preview`、`build_preview_row_update_query`。

### 4.2 HTTP route 层

| 文件 | 行数/状态 | 当前职责 | 后续拆分方向 |
| --- | ---: | --- | --- |
| `src/backend/http/import_routes/stage_handlers.rs` | 3860，严重超限 | dedup runtime、stage2 智能链、confirm、session、preview page/index/selection/update/reclassify、recurring/transfer/learning handlers 混合 | 保留 route facade，拆到 `stage_handlers/{dedup,stage2_intelligence,category_rules,recurring_projection,learning_projection,account_rules,preview_page,preview_selection,preview_update,reclassify,confirm,learning_routes}.rs` |
| `src/backend/http/import_routes/preview_mutation_helpers.rs` | 1254，超限 | preview update payload 解析、patch 构造、category lookup、decision payload、response projection | 拆到 `preview_mutations/{payload,patch_builder,category_lookup,decisions,llm,recurring,response_projection}.rs` |
| `src/backend/http/import_routes/multipart_and_ocr.rs` | 1047，超限但 D1 只读 | multipart 解析、OCR/LLM runtime、provider auth、local OCR、网络 provider 安全 | D1 只读合同；OCR/LLM 属 D8，除 upload contract 测试外不在 D1 重构 |
| `src/backend/http/import_routes/response_payload.rs` | 608 | preview/response JSON 投影 | 可与 `response_payload_ledger.rs`、core response contract 配合收敛 |
| `src/backend/http/import_routes/stage_vector_recall.rs` | 508 | stage2 Weaviate recall 请求和结果接入 | D1 只锁行为，主体归 D8 |
| `src/backend/http/import_routes/stage_account_rule_matchers.rs` | 479 | stage2 account_rules 匹配组合 | D1 可移动到 stage2 子模块，规则核心仍归 D3 |
| `src/backend/http/import_routes/duplicate_materialization.rs` | 440 | 同批/跨批重复和历史 materialization | D1 后端拆分优先对象 |
| `src/backend/http/import_routes/transfer_materialization.rs` | 264 | transfer materialization | D1 后端拆分优先对象 |
| `src/backend/http/import_routes/mod.rs` | 370 | route 注册和 include 聚合 | 后续应减少 `include!` 面，改成显式子模块 re-export |

主要函数群：

- route handlers：`import_dedup_runtime_handler`、`import_confirm_runtime_handler`、`import_session_runtime_handler`、`import_preview_page_runtime_handler`、`import_preview_index_runtime_handler`、`import_preview_selection_runtime_handler`、`import_preview_update_runtime_handler`、`import_reclassify_runtime_handler`。
- stage2 chain：`apply_import_intelligence_chain`、`apply_category_rule_match_filtered`、`apply_builtin_category_rule_fallback`、`apply_learning_rule_match`、`apply_learning_rule_projection`、`apply_account_rule_match_after_semantic_projection`。
- transfer/learning/recurring invariants：`demote_unauthorized_transfer_preview`、`clear_generated_transfer_feedback`、`enforce_import_preview_invariants`、`best_recurring_candidate_for_draft`。
- preview update payload：`build_preview_patch_from_payload`、`apply_preview_updates_from_payload`、`decision_from_payload`、`preview_decision_result_response`、`llm_decision_result_response`。

### 4.3 Core 合同层

| 文件 | 行数/状态 | 当前职责 | 后续拆分方向 |
| --- | ---: | --- | --- |
| `src/backend/core/import_pipeline.rs` | 1111，结构 gate warning | preview page query normalization、filter index item、matching payload projection、response envelope DTO | 拆到 `import_pipeline/{preview_query,filter_index,matching_payload,response_contract,helpers}.rs`，保留 facade re-export |
| `src/backend/core/import_pipeline_learning.rs` | 小文件 | learning matching payload | 可保留；D8 统一整理 learning 时再处理 |
| `src/backend/core/import_learning.rs`、`import_learning_lifecycle.rs` | 中等 | learning rule/lifecycle 合同 | D1 行为锁定引用，D8 负责结构化 |

## 5. 前端文件地图

### 5.1 页面和大组件

| 文件 | 行数/状态 | 当前职责 | 后续拆分方向 |
| --- | ---: | --- | --- |
| `src/web/src/views/desktop/transactions/import/ImportDialog.vue` | 2005，gate 2256 | 文件选择、配置匹配、列映射、stage2、preview page 请求、confirm、history rewrite ack、清理 session | 页面入口保留 step orchestration；拆 `useImportFiles`、`useImportConfig`、`useImportStageApi`、`useImportPreviewPaging`、`useImportConfirmPayload`、`useImportSessionCleanup` |
| `src/web/src/views/desktop/transactions/import/tabs/ImportTransactionCheckDataTab.vue` | 5279，gate 5977 | preview 表格、筛选、批量选择、编辑草稿、annotation、signal actions、recurring/transfer/learning/LLM 决策、批量分类/账户/标签 | 保留 tab 壳和事件；拆 components、composables、adapters：`PreviewTableToolbar`、`PreviewSignalActions`、`usePreviewSelection`、`usePreviewDrafts`、`usePreviewDecisionActions`、`usePreviewAnnotationIssues`、`usePreviewBatchActions`、`usePreviewFacets` |
| `tabs/ImportTransactionDefineColumnTab.vue` | 545 | 手工列映射 | D1 可作为上传/解析前置页面保留；不优先拆 |
| `tabs/ImportTransactionExecuteCustomScriptTab.vue` | 298 | 自定义脚本导入 | D1 只读 |
| `tabs/ImportPreviewSignalCell.vue` | 440 | preview signal cell 展示和动作入口 | 可保留为展示组件，后续从大 tab 中剥离 action payload |

### 5.2 TS helper 和 dialog

| 文件 | 行数 | 当前职责 | 后续拆分方向 |
| --- | ---: | --- | --- |
| `checkDataMatching.ts` | 957，gate 1080 | parser/dedup/transfer/learning/LLM/history signal view model 和 filter matching | 拆 `signals/{parser,dedup,transfer,learning,llm,history,filters}.ts` |
| `importPreviewIndex.ts` | 591，gate 670 | server-paged index/facet/filter/sort/selection helpers | 拆 `previewIndex/{filters,facets,selection,sort}.ts` |
| `importPreview.ts` | 153 | category id/path resolution | 可保留，增加注释时标明 canonical id 约束 |
| `importPreviewTransaction.ts` | 188 | backend preview record -> `ImportTransaction` | 可保留，后续移入 adapter 目录 |
| `importPreviewUpdates.ts` | 111 | update payload 构造 | 可保留或移入 `adapters/previewUpdates.ts` |
| `importPreviewDrafts.ts` | 113 | draft deep clone | 可保留 |
| `importPreviewReviewState.ts` | 133 | review/annotation 状态清理 | 可保留或合并到 annotation composable |
| `checkDataFilters.ts` | 235 | 本地筛选和日期 preset | 可保留，后续和 server query adapter 分层 |
| `checkDataSelection.ts` | 107 | selection summary | 可保留 |
| `checkDataLearning.ts` | 63 | learning expected-state/payload | 可保留 |
| `checkDataCandidateReview.ts` | 51 | transfer/learning expected state | 可保留 |
| `checkDataAnnotation.ts` | 21 | annotation filter | 可保留 |
| `importDialogApi.ts` | 52 | fetch/timeout/error helper | 可保留或上移到 import API adapter |
| `strictCents.ts` | 26 | strict cents parsing | 可保留；所有金额改动必须引用该边界 |
| `BatchReplace*.vue`、`BatchCreateDialog.vue`、`ImportLearningSuggestionDialog.vue` | 162-416 | 批量替换/创建和 learning 建议弹窗 | D1 frontend-shape 只在大 tab 释放后再拆 |

## 6. 共享路径和 lease

D1 默认 owned paths：

- `src/backend/http/import_routes/**`
- `src/backend/core/**import**`
- `src/backend/db/import_staging.rs`
- `src/backend/db/import_staging/**`
- `src/web/src/views/desktop/transactions/import/**`

默认只读或需要单独 lease 的路径：

- `src/backend/parsers/**`: D1 只读 parser-first 合同，不重构 parser。
- `src/backend/db/taxonomy/**`、`src/backend/core/account_rules/**`、`src/backend/core/category_rules/**`: 只读身份和规则合同；结构重构归 D2/D3。
- `src/web/src/lib/services.ts`: 如果需要拆 import API facade，必须在 `frontend-shape` 或 D10 中申请 shared-path lease。
- `src/web/src/models/imported_transaction.ts`: 导入预览模型依赖面，若移动或拆分必须覆盖前端模型测试。
- `src/web/src/views/mobile/transactions/ImportPreviewPage.vue`: 移动端复用桌面 helper；D1 可加 parity 测试，但改组件需单独记录 lease。

## 7. 现有测试覆盖地图

后端已存在测试入口：

- `tests/backend/db/import_staging.rs`: import staging schema、materialization edge、Postgres staging/confirm 相关测试。
- `tests/backend/core/import_pipeline_contracts.rs`: import stage response、preview page/filter/matching payload 合同。
- `tests/backend/core/import_learning_contracts.rs`: learning rule/lifecycle 合同。
- `tests/backend/http/import_runtime_contract.rs`: Rust REST import runtime contract。
- `src/backend/db/import_staging/preview_drafts.rs` 内部测试：历史转账、金额分、history duplicate draft。

前端已存在测试入口：

- `tests/web/views/desktop/transactions/import/importPreview.test.ts`: category identity、server-paged reset、signal/annotation 合同源码测试。
- `tests/web/views/desktop/transactions/import/importPreviewIndex.test.ts`: 全局 index、facet、server-paged sort/filter/selection。
- `tests/web/views/desktop/transactions/import/checkDataMatching.test.ts`: parser/dedup/transfer/learning/LLM/history signal view model。
- `tests/web/views/desktop/transactions/import/importPreviewTransaction.test.ts`: backend preview record 到 `ImportTransaction` 投影。
- `tests/web/views/desktop/transactions/import/importPreviewUpdates.test.ts`: preview update payload、manual annotation、suggestion clear。
- `tests/web/views/desktop/transactions/import/importDialogHistoryAck.test.ts`: confirm history rewrite acknowledgement payload。
- `tests/web/views/desktop/transactions/import/importStageTimeout.test.ts`: long-running import stage timeout。
- `tests/web/views/mobile/importPreviewPage.test.ts`: mobile import preview parity。

G011 `behavior-lock` 必须补强或明确复用以下场景：

- API/runtime smoke：upload -> stage2 -> preview page -> update -> reclassify -> selection -> confirm。
- 金额分单位矩阵：parser 元边界、preview cents、destination cents、confirm bills cents。
- 身份校验矩阵：合法/非法 category id、source account、destination account、inactive、sentinel、转账同账户。
- cross-page selection：all/valid/needs-review、invert、selected invalid count。
- visible signal family：parser/platform duplicate/transfer/history/learning/LLM/annotation。
- history rewrite acknowledgement：selected preview ids、operation id、history bill id/version、ack token。
- parser 只读边界：D1 不修改 dedicated parser，行为锁仅验证 exactly-one 和 source parser tags。
- 浏览器或 API smoke：至少一次真实导入样本预览确认链路。

## 8. 后续切片执行顺序

### 8.1 G011 behavior-lock

- 只允许测试、fixture、smoke、测试辅助和必要的合同文档。
- 禁止移动业务源码。
- 最小验证：
  - 后端 focused import tests。
  - 前端 import helper tests。
  - 一条 API/browser smoke 证据。
- 若发现真实 bug，当前切片写 `blocked:<reason>`，另开 fix slice。

### 8.2 G012 backend-shape

建议顺序：

1. `src/backend/db/import_staging.rs` facade 先稳定 re-export，再拆 preview query/selection/identity/confirm。
2. 拆 `preview_mutation_helpers.rs`，保持 patch builder 输出 byte/JSON 等价。
3. 拆 `stage_handlers.rs` 中 preview page/selection/update/reclassify/confirm handlers。
4. 最后拆 stage2 intelligence 子模块，避免和 behavior-lock 同时扩大。
5. 每一步都运行 focused Rust tests；最终运行 fmt/clippy/coverage 门禁。

### 8.3 G013 frontend-shape

建议顺序：

1. 先从 `ImportDialog.vue` 抽 API/paging/confirm composables，页面模板和视觉结构不变。
2. 再从 `ImportTransactionCheckDataTab.vue` 抽 selection、decision actions、annotation、batch actions、facet/query adapter。
3. `checkDataMatching.ts` 按 signal family 拆分，保留 barrel/facade 兼容旧 import。
4. 每一步保留现有 prop/emit 合同和 UI 视觉，不做布局/配色重设计。
5. 运行 `npm run lint`、相关 vitest、`npm run test:coverage`；可见流程补 browser smoke。

### 8.4 G014 comment-pass/governance-docs/closeout

- 对导出函数、业务关键函数、复杂私有 helper 补中文说明。
- 简单 getter、字段映射、事件转发不强制。
- 更新结构门禁 baseline/ratchet，确保 D1 相关文件不回涨。
- 若稳定业务事实改变，更新 `docs/PROJECT_OVERVIEW.md`；纯结构移动不写会话日志。
- 完成 PR CI、squash merge、删除分支、写回 progress，并把 cursor 推进到 D2。

## 9. 关键风险与阻断条件

- `include!` 文件混杂会让 Rust 模块移动产生路径和可见性连锁问题；拆分时先建立明确 `mod` 和 re-export。
- `import_staging.rs` 同时承担 SQL、事务、row mapping 和业务校验；任何拆分都要先锁 user-scope、事务 rollback 和 cents 字段。
- `stage_handlers.rs` 的 stage2 顺序是业务合同，不允许因拆模块改变调用顺序。
- `ImportTransactionCheckDataTab.vue` 同时有视图状态和服务端状态；拆分时必须区分当前页草稿、server-paged selection、baseline decision state。
- `checkDataMatching.ts` 的 signal family 是当前筛选合同；拆分时必须保持 filter 和 cell view model 同源。
- `services.ts`、`src/models/imported_transaction.ts`、移动端 import preview 是共享面；没有 lease 不改。
- PR 自动合并必须证明 PR head SHA 与 CI head SHA 一致、CI passed、base fresh、terminal review passed、source branch deleted。

## 10. D1 完成判定

D1 完成后应满足：

- 导入预览后端无单文件同时承载 route、SQL、identity、confirm、learning/LLM 多组职责。
- 导入预览页面入口只负责装配；复杂状态、payload、filter、selection、decision action 已分离。
- D1 直接相关结构门禁项不回涨，目标是移除 `import_staging.rs`、`stage_handlers.rs`、`preview_mutation_helpers.rs`、`ImportDialog.vue`、`ImportTransactionCheckDataTab.vue`、`checkDataMatching.ts` 的超限状态。
- 行为锁定覆盖 upload/stage/preview/update/reclassify/confirm、金额、身份校验、cross-page selection 和 visible signals。
- PR/CI/merge/delete/writeback 证据完整，cursor 推进到 D2。
