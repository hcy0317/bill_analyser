# D8 llm-ocr-learning-weaviate 结构债 ledger

本 ledger 固化 D8 LLM/OCR、导入学习、学习中心与 Weaviate 派生索引域的当前结构边界。D8 继续复制 D1-D7 的“功能域 -> 功能文件夹”模板，先锁定 provider 配置、SSRF 防护、secret redaction、OCR 草稿、learning 生命周期、LLM preview memory、Weaviate 派生索引和前端学习中心行为，再拆后端和前端结构，再补中文说明和治理基线。

## 1. 域边界

D8 覆盖：

- 后端 LLM/OCR core：provider 配置、provider alias、SSRF allowlist、secret redaction、prompt/response parser、OCR parser、receipt draft、provider auth refresh 和本地/LLM vision OCR。
- 后端导入学习：recommendation key、feature/sample/token 生成、learning lifecycle、green/blue policy、LLM preview memory apply/reject、learning suggestion/rule route envelope、stage2 learning rules 和 vector recall。
- Weaviate 派生索引：collection prefix、schema/vectorizer none、object id/vector、batch upsert/delete/search、outbox processing、rebuild、health/bootstrap、runtime recall filter 和 CLI。
- PostgreSQL 持久化：LLM saved config、candidate review、memory event、annotation sample、import staging LLM/learning lifecycle 和 vector outbox。
- 前端学习中心与导入预览 LLM/learning/OCR helper：`LearningCenterPanel.vue`、`OcrConfigPanel.vue`、`llmConfigHelpers.ts`、learning store/model、import preview LLM memory 与 learning suggestion dialog。

D8 不覆盖：

- 导入预览表格、session、confirm、通用 preview update、跨页选择和全局 import facade，已归 D1；D8 只治理其中 LLM/learning/OCR hook 和只读依赖。
- 规则中心 shell、分类规则、账户规则、周期规则和配对中心布局，已归 D3；D8 只接管学习中心面板内的 provider、learning 和 OCR 细节。
- 正式交易、matching 核心、账单编辑图片 OCR 的页面壳和交易列表结构，归 D4 或 D10；D8 只记录 OCR/learning 调用边界。
- 全局 `src/web/src/lib/services.ts` axios facade、root store 聚合、theme、router shell 和 `imported_transaction` 共享结构债，归 D10；D8 不吸收这些 shared failures。
- parser dedicated source normalization、parser-first exactly-one 命中和日期/金额标准化主链，归 D9；D8 只能消费 parser 输出和 OCR/LLM 外部边界。
- 未经用户明确授权的 live external-provider 调用；测试必须使用 mock、本地 server 或纯配置/合同断言。
- UI 视觉重设计；本计划只做结构、注释、测试和安全治理，保留现有学习中心、OCR 设置和导入提示视觉与交互。

计划文件中 D8 `owned_paths` 当前只写了 LLM/OCR/learning/Weaviate 通配。执行 D8 时应按本 ledger 扩展实际 owned paths：

- `src/backend/core/ai_ocr_llm/**`
- `src/backend/core/import_learning.rs`
- `src/backend/core/import_learning_lifecycle.rs`
- `src/backend/core/import_pipeline_learning.rs`
- `src/backend/core/weaviate_derived.rs`
- `src/backend/core/matching/session_learning/**` 中 learning candidate 相关合同，只读优先，除非取得 D4 shared lease。
- `src/backend/http/import_routes/multipart_and_ocr.rs`
- `src/backend/http/import_routes/ocr_learning_handlers.rs`
- `src/backend/http/import_routes/learning_runtime.rs`
- `src/backend/http/import_routes/llm_handlers.rs`
- `src/backend/http/import_routes/llm/**`
- `src/backend/http/import_routes/stage_handlers/stage2_learning_rules.rs`
- `src/backend/http/import_routes/stage_handlers/learning_handlers.rs`
- `src/backend/http/import_routes/stage_vector_recall.rs`
- `src/backend/http/import_routes/stage_vector_recall/**`
- `src/backend/http/import_routes/preview_mutation_helpers/llm_decisions.rs`
- `src/backend/http/weaviate.rs`
- `src/backend/http/weaviate_recall.rs`
- `src/backend/http/config_weaviate.rs`
- `src/backend/http/bin/bill_weaviate_derived_index.rs`
- `src/backend/db/llm.rs`
- `src/backend/db/vector_outbox.rs`
- `src/backend/db/import_staging/preview_llm.rs`
- `src/backend/db/import_staging/preview_learning_lifecycle.rs`
- `src/backend/db/import_staging/llm_memory_annotations.rs`
- `src/backend/db/import_staging/types/preview_decision_learning.rs`
- `src/backend/db/postgres/migrations/0010_llm_import_runtime.sql`（只读合同锚点，默认不改）
- `docs/weaviate-derived-index.md`
- `src/web/src/models/learning_center.ts`
- `src/web/src/models/import_learning.ts`
- `src/web/src/stores/learning.ts`
- `src/web/src/views/desktop/pairingcenter/components/LearningCenterPanel.vue`
- `src/web/src/views/desktop/pairingcenter/components/learningCenterPanelModel.ts`
- `src/web/src/views/desktop/pairingcenter/components/llmConfigHelpers.ts`
- `src/web/src/views/desktop/pairingcenter/components/OcrConfigPanel.vue`
- `src/web/src/views/desktop/transactions/import/llmSignalMemory.ts`
- `src/web/src/views/desktop/transactions/import/checkDataLearning.ts`
- `src/web/src/views/desktop/transactions/import/dialogs/ImportLearningSuggestionDialog.vue`

## 2. 当前结构 gate 快照

采集时间：2026-06-22，基于 `bill_analyser/main` 的 `55a043d46623e5d861354e4ad774d9be6c83e9ec`。

Rust 后端结构 gate：

- `node scripts/check-rust-backend-structure.mjs` 通过，扫描 484 个 Rust 后端文件，当前 baseline entries 为 6。
- D8 直接 baseline 对象：
  - `src/backend/http/import_routes/multipart_and_ocr.rs`：structure baseline 计 1031 行，physical 1105；当前集中承载 multipart parse、OCR input/rate limit/provider、runtime LLM config、provider auth refresh 和 LLM vision OCR。
  - `src/backend/core/import_learning.rs`：structure baseline 计 827 行，physical 999；当前集中承载 recommendation key、features、lifecycle/model、LLM preview memory、route envelope 和 helper。
  - `src/backend/db/llm.rs`：structure baseline 计 677 行，physical 792；当前集中承载 saved config、candidate review、memory event、annotation sample 和 row mapping。
- D8 直接非 baseline 但应治理对象：
  - `src/backend/http/import_routes/ocr_learning_handlers.rs`：约 607 行，集中承载 OCR config/recognition、learning suggestion、learning rule handler 和上下文加载。
  - `src/backend/http/weaviate.rs`：约 552 行，集中承载 health、bootstrap、batch、search、outbox、rebuild、apply event 和 retry helper。
  - `src/backend/http/import_routes/llm/provider_runtime.rs`：约 550 行，集中承载 provider runtime config、request 构造和外部调用边界。
  - `src/backend/http/import_routes/llm/rule_synthesis_helpers.rs`：约 549 行，集中承载 rule synthesis prompt/response helper。

前端结构 gate：

- `Set-Location src/web; npm run structure:check` 预期失败 4 项，当前均属于 D10 shared：
  - `src/lib/services.ts`
  - `src/stores/index.ts`
  - `src/core/theme.ts`
  - `src/models/imported_transaction.ts`
- D8 直接 baseline 对象：
  - `src/web/src/views/desktop/pairingcenter/components/LearningCenterPanel.vue`：frontend baseline 计 1618 行，template 424、script 635、style 183；当前仍集中承载学习规则、LLM 配置、候选审核、OCR 设置和面板交互。
- D8 直接非 baseline 但可治理对象：
  - `src/web/src/views/desktop/pairingcenter/components/OcrConfigPanel.vue`：约 430 行，集中承载 OCR config UI、provider 参数和保存/测试交互。
  - `src/web/src/views/desktop/pairingcenter/components/llmConfigHelpers.ts`：约 360 行，集中承载 provider form、payload、candidate 和 runtime helper。
  - `src/web/src/models/learning_center.ts`：约 275 行，承载学习中心 response/model 合同。
  - `src/web/src/stores/learning.ts`：约 206 行，承载 learning center store action 和 state。

## 3. 后端文件职责地图

### 3.1 `src/backend/core/ai_ocr_llm/**`

当前职责：

- `llm_config.rs`、`llm_provider.rs`、`llm_prompts.rs`、`llm_responses.rs` 承载 provider alias/default、LLM config、prompt 构造、JSON array parser 和 response projection。
- `ocr_config.rs`、`ocr_parser.rs`、`receipt_draft.rs` 承载 OCR config、支付截图解析、结构化交易草稿、金额元到分边界和 provenance/context 映射。
- `provider_auth.rs` 承载 provider auth refresh、token endpoint 校验和 secret redaction 边界。
- `secret_redaction.rs`、`value_helpers.rs` 承载递归脱敏、字段读取和安全投影 helper。

拆分建议：

1. 保持 `ai_ocr_llm/mod.rs` 为 facade，只 re-export public 合同。
2. 将 OCR parser 的支付平台解析、金额/时间提取和 draft projection 保持在 core 层，不上移到 HTTP route。
3. 将 provider auth refresh 与 base URL/token endpoint allowlist 保持隔离，避免和业务 handler 混合。
4. comment-pass 必须说明外部 provider 输入、secret redaction、SSRF allowlist、response cap 和金额单位转换边界。

### 3.2 `src/backend/http/import_routes/multipart_and_ocr.rs`

当前集中承载：

- multipart 字节读取、上传字段解析、文件输入构造和导入 parser 入口周边 helper。
- OCR recognition 输入校验、rate limit、provider config 读取、Tesseract/local JSON OCR 分支和 LLM vision OCR 外部调用。
- runtime LLM config get/put、safe response 投影、provider auth refresh 和 token endpoint 验证。
- LLM/OCR 外部请求 body、header、timeout、response limit 和错误投影。

拆分建议：

1. `multipart_and_ocr/multipart_reader.rs`：multipart 字节读取和文件输入构造。
2. `multipart_and_ocr/ocr_request.rs`：OCR payload、输入校验、rate limit 和 response envelope。
3. `multipart_and_ocr/ocr_local.rs`：Tesseract/local JSON OCR 分支。
4. `multipart_and_ocr/ocr_llm_vision.rs`：LLM vision OCR 调用、response cap 和错误投影。
5. `multipart_and_ocr/runtime_config.rs`：runtime LLM/OCR config get/put 与 safe response。
6. `multipart_and_ocr/provider_auth.rs`：auth refresh route helper 和 token endpoint 验证。
7. 保持原文件为 route facade，不改变 REST path、错误 status、provider defaults 或 secret redaction。

### 3.3 `src/backend/core/import_learning.rs`

当前集中承载：

- recommendation key、composite match hash、feature schema/model metadata 和 learning sample/token 生成。
- learning lifecycle threshold、green/blue policy、transfer signal state 和 auto-apply 判定。
- LLM preview memory apply/reject/revert、memory events、route envelope、paging 和错误 shape。

拆分建议：

1. `import_learning/types.rs`：公共 DTO、常量和 route envelope。
2. `import_learning/recommendation_key.rs`：key/hash/token 生成。
3. `import_learning/features.rs`：feature schema、sample、model metadata。
4. `import_learning/lifecycle.rs`：threshold、green/blue policy、transfer signal state。
5. `import_learning/llm_memory.rs`：LLM preview memory apply/reject/revert 和 event projection。
6. `import_learning/route_envelope.rs`：list/paging/error response helper。
7. 保持 `core::import_learning::*` public facade 与测试合同不变。

### 3.4 `src/backend/db/llm.rs` 与 import staging LLM 子文件

当前职责：

- user-scoped saved config、provider config safe projection、candidate review、memory event、annotation sample 和 row mapping。
- import staging preview LLM decision、learning lifecycle、LLM memory annotation 和 preview decision learning 类型。

拆分建议：

1. `db/llm/configs.rs`：saved config CRUD、secret redaction 和 row mapping。
2. `db/llm/candidates.rs`：candidate review list/accept/reject。
3. `db/llm/memory_events.rs`：memory event append/list 和 paging。
4. `db/llm/annotation_samples.rs`：sample list/update 和 lifecycle 字段。
5. import staging 子文件保持在 `import_staging/**` 下，只拆职责过大的文件，不改变 staging schema 和 confirm 语义。
6. SQL 必须继续使用 bind 参数和 user_id 条件，不允许引入跨用户读取。

### 3.5 Weaviate HTTP、core 与 outbox

当前职责：

- `src/backend/core/weaviate_derived.rs` 承载 collection prefix validation、schema class、deterministic object id/vector、batch/delete/graphql payload 和 recall query。
- `src/backend/http/weaviate.rs` 承载 health、bootstrap、batch upsert/delete/search、outbox process、rebuild、apply event、retry 和 ensure_success。
- `src/backend/http/import_routes/stage_vector_recall/**` 承载导入 stage vector recall worker、query chain、results 和测试。
- `src/backend/db/vector_outbox.rs` 承载 outbox enqueue/list/mark/retry 等持久化。

拆分建议：

1. `http/weaviate/types.rs`：response、event、request 和错误类型。
2. `http/weaviate/health.rs`：ready probe、health projection 和 config summary。
3. `http/weaviate/schema.rs`：bootstrap schema 和 class payload。
4. `http/weaviate/batch.rs`：upsert/delete/search HTTP calls。
5. `http/weaviate/outbox.rs`：outbox process、retry 和 apply event。
6. `http/weaviate/rebuild.rs`：rebuild CLI/handler 编排。
7. Weaviate metadata 只能作为 evidence，不能单独触发 auto-apply；PostgreSQL lifecycle 仍是权威。

## 4. 前端文件职责地图

### 4.1 `LearningCenterPanel.vue`

当前集中承载：

- 学习规则列表、筛选、状态展示、启停/更新/删除操作。
- LLM provider config、saved config、runtime config、candidate review 和 review memory 设置。
- OCR config tab、provider 参数、识别开关和保存/测试动作入口。
- 大量面板状态、dialog 状态、helper 映射、template 和局部样式。

拆分建议：

1. `learning-center/LearningRulesSection.vue`：学习规则列表、筛选和操作入口。
2. `learning-center/LlmProviderSection.vue`：LLM provider config、runtime config 和 saved config 展示。
3. `learning-center/LlmCandidatesSection.vue`：candidate review、accept/reject 和 memory config。
4. `learning-center/OcrSection.vue`：OCR tab shell 和 `OcrConfigPanel` 装配。
5. `learning-center/useLearningCenterActions.ts`：加载、刷新、保存、候选审核和错误投影。
6. `learning-center/useLearningCenterDialogs.ts`：dialog/form state。
7. `LearningCenterPanel.vue` 保持页面 facade、tab 入口和现有视觉布局。

### 4.2 `OcrConfigPanel.vue` 与 `llmConfigHelpers.ts`

`OcrConfigPanel.vue` 当前职责：

- OCR provider/endpoint/base URL 参数 UI。
- 配置保存、测试、错误提示和 provider-specific form helper。
- 与 learning center shell 的 prop/emit 合同。

拆分建议：

1. `ocr-config/OcrProviderForm.vue`：provider-specific 表单。
2. `ocr-config/useOcrConfigForm.ts`：payload 构造、校验和保存动作。
3. `ocr-config/useOcrConfigMessages.ts`：错误/成功提示投影。
4. `OcrConfigPanel.vue` 保持 facade、props/emits 和视觉布局。

`llmConfigHelpers.ts` 拆分建议：

1. `llm-config/providerOptions.ts`：provider 列表、默认值和 label。
2. `llm-config/configPayload.ts`：form 到 API payload。
3. `llm-config/candidateHelpers.ts`：candidate review 和 safe display helper。
4. 保留 `llmConfigHelpers.ts` 作为兼容 facade，原导出名不变。

### 4.3 Store、模型与导入预览 helper

当前职责：

- `src/web/src/models/learning_center.ts` 和 `import_learning.ts` 承载 learning center 与 import learning response/model。
- `src/web/src/stores/learning.ts` 承载 learning rule、suggestion、LLM config、OCR config 和 candidate action。
- `llmSignalMemory.ts`、`checkDataLearning.ts` 和 `ImportLearningSuggestionDialog.vue` 承载导入预览页内的 LLM memory 与 learning suggestion UI 边界。

拆分建议：

1. `models/learning-center/**`：rules、suggestions、llm config、ocr config、candidate response 类型。
2. `stores/learning/**`：rule actions、suggestion actions、llm config actions、ocr actions 和 candidate review actions。
3. 导入预览 helper 保持 D1 facade 路径不变，只拆 LLM/learning 专属 helper，不改变 preview table、selection、confirm 或 services 全局路径。

## 5. 行为锁定测试锚点

后端现有锚点：

- `tests/backend/core/ai_ocr_llm_contracts.rs`：OCR 默认 disabled、provider normalization、safe disabled error、secret redaction、LLM vision SSRF allowlist、支付截图 parser、OCR draft provenance、LLM provider config、Azure endpoint、advanced settings isolation、candidate review live-provider boundary、prompt 和 JSON parser。
- `tests/backend/core/import_learning_contracts.rs`：recommendation key、feature schema、sample/token、lifecycle threshold、green/blue policy、transfer signal state、LLM preview memory、route envelope、paging 和 error shape。
- `tests/backend/core/weaviate_derived_contracts.rs`：collection prefix、schema vectorizer none、object id/vector、batch/delete/graphql payload、recall query filter 和 limit。
- `tests/backend/core/import_pipeline_contracts.rs`：learning/LLM 在 stage2 chain 中的顺序、转账保护和 preview 投影。
- `tests/backend/core/runtime_governance_contracts.rs`：LLM/OCR/learning route ownership、移除旧 route 和运行态治理策略。
- `tests/backend/http/weaviate_runtime_contract.rs`：Weaviate runtime health、config、outbox、rebuild 和 recall 行为。
- `tests/backend/db/llm_postgres.rs` 与 `tests/backend/db/vector_outbox.rs`：PostgreSQL LLM 和 vector outbox 持久化合同。

前端现有锚点：

- `tests/web/views/desktop/pairingcenter/learningCenterPanelModel.test.ts`：学习中心 tab、筛选、状态色和匹配类型展示模型。
- `tests/web/views/desktop/pairingcenter/learningCenterOcrConfig.test.ts`：OCR config panel 行为。
- `tests/web/models/learning_center.test.ts`：学习中心模型合同。
- `tests/web/views/desktop/transactions/import/llmSignalMemory.test.ts` 与 `services.llmMemory.test.ts`：导入预览 LLM memory 行为和 service 调用合同。
- `tests/web/views/desktop/transactions/import/checkDataLearning.test.ts`：导入预览 learning suggestion 行为。
- `tests/web/views/desktop/transactions/editDialogPictureOcr.test.ts` 与 `tests/web/views/mobile/transactionEditPagePictureOcr.test.ts`：交易编辑图片 OCR 调用边界。

D8 behavior-lock 应补或确认：

1. Provider config：alias/default、base URL、Azure endpoint、token endpoint、advanced settings 和 safe response redaction 保持稳定。
2. SSRF：LLM vision、provider auth refresh 和 Weaviate endpoint 不允许 metadata/internal host、credentials、control chars、非允许 scheme 或未经授权本地地址。
3. Secret redaction：API key、access token、refresh token、Authorization、credential 等字段不得进入 response、toast、日志或测试快照。
4. OCR：默认 disabled、配置缺失错误、rate limit、local JSON/Tesseract/LLM vision 分支、金额元到分、provenance/context 映射保持稳定。
5. Learning：recommendation key、feature schema、green/blue threshold、transfer signal state、LLM preview memory apply/reject/revert 和 route envelope 保持稳定。
6. Weaviate：vectorizer none、自带 vector、userId/schema/type/ruleState filter、max limit、outbox retry、rebuild 和 Postgres authoritative policy 保持稳定。
7. 前端：学习中心 tab/filter/status、LLM/OCR config 表单、candidate accept/reject、memory config、导入预览 LLM signal 和 learning suggestion dialog 保持稳定。

## 6. 安全审查门禁

D8 涉及外部 provider、OCR 输入、LLM response、provider auth、Weaviate 派生索引和 secret 管理，必须在 behavior-lock、backend-shape 和 closeout 至少记录一次 security-reviewer 或等价独立安全审查证据。审查清单：

- SSRF：LLM/OCR vision base URL、provider auth token endpoint 和 Weaviate endpoint 必须继续执行 allowlist、scheme、credential、metadata host、private/link-local/loopback 和 control char 校验。
- Secrets：LLM/OCR provider API key、access token、refresh token、Authorization、credential 和 config parameters 必须递归脱敏，不能进入 response、日志、toast 或 docs 示例。
- Auth 与 user-scope：saved config、candidate review、memory event、annotation sample、learning rule/suggestion 和 vector outbox 必须绑定当前 user_id。
- SQL：LLM、learning、outbox 和 import staging 读写必须继续使用 bind 参数，不得拼接未验证输入。
- Prompt/response：provider response 必须限制体积、候选数量和 JSON 解析边界；错误信息不得透传完整外部响应或 secret。
- Rate limit 与 payload cap：OCR/LLM 输入、图片、multipart 和 provider response 继续保留大小、超时和并发限制。
- Weaviate authority：Weaviate metadata 只能作为派生 evidence，不能越过 PostgreSQL lifecycle 自动启用 auto-apply。
- Frontend：provider secret 不在表单之外持久化或展示；candidate/review 操作不得绕过服务端 user-scope 校验。
- External calls：本地测试不得调用真实 LLM/OCR/cloud/vector provider；需要 mock/local server 或纯 payload/config 合同。

本 ledger 切片只新增文档，不修改运行时代码；安全审查初始结论为“未引入新攻击面，后续 D8 行为锁定和拆分必须逐项复核上述合同”。

## 7. 切片执行顺序

1. `ledger`：提交本文件，记录结构 gate 快照、域边界、测试锚点、安全审查门禁和 D8 shared path 排除项。
2. `behavior-lock`：只新增/补强测试和必要测试 helper，不移动生产代码；覆盖 provider config/SSRF/secret redaction、OCR draft、learning lifecycle、LLM preview memory、Weaviate authority 和学习中心前端行为。
3. `backend-shape`：拆 `multipart_and_ocr.rs`、`import_learning.rs`、`db/llm.rs`，必要时继续拆 `ocr_learning_handlers.rs`、`weaviate.rs` 和 LLM route helper；保持 public facade、REST route、SQL、provider 防护、Weaviate policy 和 response envelope 不变。
4. `frontend-shape`：拆 `LearningCenterPanel.vue`、`OcrConfigPanel.vue`、`llmConfigHelpers.ts`、learning store/model 和导入预览 LLM/learning helper；保留 import path、store action、props/emits、按钮文案、禁用状态和现有视觉布局。
5. `comment-pass`：按用户确认的注释标准补中文说明：导出函数、业务关键函数、复杂私有 helper 必须有中文说明；简单 getter、映射、事件转发不强制。
6. `governance-docs`：收紧 D8 后端结构 baseline 和前端 structure baseline；同步 `docs/PROJECT_OVERVIEW.md` 的 LLM/OCR、learning 和 Weaviate 当前结构事实。
7. `closeout`：确认 D8 PR 链、CI、merge、branch delete、结构 gate D8 清零或收紧、security-review evidence 和无 live provider 调用证据，进度 cursor 推进到 D9/ledger。

如 security-reviewer 在任意阶段发现真实安全缺陷，必须先拆独立 `security-fix` prerequisite slice，不能混入结构搬迁。

## 8. 验收门槛

D8 完成前必须满足：

- D8 直接后端 structure baseline 对象清零或明显收紧：`src/backend/http/import_routes/multipart_and_ocr.rs`、`src/backend/core/import_learning.rs`、`src/backend/db/llm.rs` 退化为 facade 或进入新的更小 ratchet。
- D8 直接前端 structure baseline 对象清零或明显收紧：`LearningCenterPanel.vue` 退化为 facade 或进入新的更小 ratchet。
- `ocr_learning_handlers.rs`、`weaviate.rs`、`provider_runtime.rs` 和 `rule_synthesis_helpers.rs` 不得因为拆分新增 oversized debt；如仍超过阈值，必须记录保守 ratchet 与后续责任。
- `Set-Location src/web; npm run structure:check` 仍只允许 D10 shared 四项失败；D8 不得新增或吸收 shared structure failure。
- `cargo fmt --all -- --check`、相关 LLM/OCR/import learning/Weaviate DB 与 runtime 合同测试、`cargo clippy --workspace --all-targets -- -D warnings` 和 `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 35` 通过。
- `Set-Location src/web; npm run lint:ci` 和 `npm run test:coverage` 通过，必要时补 LearningCenterPanel、OcrConfigPanel、llmConfigHelpers、learning store/model 和 import preview LLM/learning 单测。
- 安全审查结论不含 blocker；如发现真实安全缺陷，必须拆出独立 fix/prerequisite slice，不能混入结构搬迁。
- 全程不得发起未授权 live external-provider 调用；验证证据必须说明 mock/local/payload-only 边界。
- PR CI 通过、自动 squash merge、来源分支删除、进度 JSON 与 ultragoal ledger 回写完成。
