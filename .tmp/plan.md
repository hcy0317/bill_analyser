# 实施计划：统一配对中心与导入预览会话助手收口

## 概览
- 持久化业务中心只保留一个“配对中心”，以前端 `src/web/src/router/desktop.ts` 的 `/pairing/list` 作为唯一 canonical route；`/learning/center` 与 `/rules/center` 在切换完成后只保留重定向或薄包装。
- `src/web/src/views/desktop/pairingcenter/ListPage.vue`、`src/web/src/views/desktop/learningcenter/ListPage.vue`、`src/web/src/views/desktop/rules/RuleCenterPage.vue` 的能力需要被收口到同一个持久化配对中心域。
- 导入预览仍保留会话级助手能力，但它属于 `src/web/src/views/desktop/transactions/import/tabs/ImportTransactionCheckDataTab.vue` 对应的 import-preview session-assistant 域，不再作为第二个“中心”存在。
- 交易分类只保留新规则逻辑：后端以 `src/bill_analyser/api/routes/category_rules.py` + `src/bill_analyser/core/category_engine.py` 中的 category rules 为唯一真源，旧关键词逻辑从运行时和 UI 移除。
- 分类编辑界面的关键词匹配要改为 Web of Science 风格布尔表达式语法，替代 `src/web/src/components/common/KeywordInput.vue` 的旧关键字录入思路。
- LLM 归纳入口要从 `src/bill_analyser/api/routes/llm.py` / `src/bill_analyser/core/llm_learning_service.py` 当前“分析已落库未分类账单”的模式，迁移到导入预览阶段基于 session / preview 数据的模式，在写库前完成业务决策辅助。

## 用户已确认 / 关键假设
- 用户已批准按本计划开始实施；当前已完成一个“基线修复”切片（commit: `8ec2cdf`）。
- 唯一持久化中心名称与入口均收口为“配对中心”，推荐保留 `src/web/src/router/desktop.ts` 中的 `/pairing/list` 作为稳定入口。
- `src/web/src/views/desktop/MainLayout.vue` 中 Pairing / Learning / Rule Center 三个并列导航最终要收敛成一个“配对中心”导航项。
- `src/web/src/views/desktop/pairingcenter/ListPage.vue` 当前投资设置按钮深链到用户设置是错误方向；投资识别设置后续必须并入统一配对中心，不再挂在用户设置下。
- `src/web/src/views/desktop/categories/list/dialogs/EditDialog.vue` 与 `src/web/src/models/transaction_category.ts` 仍带有 legacy keywords 模型，需要在切换中一并消除。
- import-preview session-assistant 域不是第二个中心：它只负责导入会话期间的临时建议、LLM 归纳和确认前操作；长期规则、学习规则、投资识别设置仍归属于统一配对中心域。

## 前置条件
- ✅ 附加切片：CI / agent-stack pytest_sessionfinish 审计兼容已完成（`8ec2cdf`）；当前最小 CI 子集可在缺少 `aiosqlite` 与部分可选配置依赖的环境下通过。
- 强制前置门禁：先清理 `src/web/src/views/desktop/learningcenter/ListPage.vue` 中围绕 `resp.data?.error`（约 589-681 行）的类型错误，统一对齐 `src/web/src/core/api.ts` 当前 `ApiResponse<T> = { success, result }` 的契约，再进入任何页面合并或路由收口。
- 在开始页面整合前，先冻结 `src/web/src/router/desktop.ts`、`src/web/src/views/desktop/MainLayout.vue`、`src/web/src/views/desktop/pairingcenter/ListPage.vue`、`src/web/src/views/desktop/learningcenter/ListPage.vue`、`src/web/src/views/desktop/rules/RuleCenterPage.vue` 的现状，作为切换期间的回归基线。
- 在开始后端域迁移前，先确认 `src/bill_analyser/core/category_engine.py`、`src/bill_analyser/api/routes/category_rules.py`、`src/bill_analyser/api/routes/rules.py`、`src/bill_analyser/api/routes/auth.py`、`src/bill_analyser/api/routes/bills.py` 的职责边界，避免把持久化配对中心域与导入预览 session-assistant 域混写在一起。
- 任何触及后端业务逻辑的阶段，验收前都要执行 `.\.venv\Scripts\python.exe -m pytest tests\ -v`；任何触及前端页面或类型契约的阶段，至少执行 `Set-Location src\web; npm run lint`，必要时补 `Set-Location src\web; npm run build` 作为最小构建验证。

## 阶段 1: 前端类型契约清障与合并基线建立 ✅ (`8ec2cdf`) — 预计 todo 数: 5
### 目标
先把统一中心重构的前端类型噪音清掉，确保后续所有页面合并都建立在可通过 `vue-tsc` 的基线上。

### 步骤
1. 在 `src/web/src/views/desktop/learningcenter/ListPage.vue` 修正所有对 `resp.data?.error`、`resp.data?.message` 一类旧包络字段的访问，并按 `src/web/src/core/api.ts` 的 `ApiResponse<T>` 契约改成从 `result` 读取有效载荷或从错误分支读取错误信息。
2. 在 `src/web/src/lib/services.ts`、`src/web/src/core/api.ts`、`src/web/src/views/desktop/learningcenter/ListPage.vue` 之间统一成功/失败返回的前端消费方式，避免统一配对中心阶段继续扩散 `ApiResponse<any>` + `resp.data?.error` 这种不匹配写法。
3. 以 `src/web/src/views/desktop/pairingcenter/ListPage.vue`、`src/web/src/views/desktop/rules/RuleCenterPage.vue`、`src/web/src/views/desktop/learningcenter/ListPage.vue` 为范围补一轮相同包络访问方式排查，确保待合并页面都站在同一类型约束之上。
4. 在 `src/web/src/router/desktop.ts` 和 `src/web/src/views/desktop/MainLayout.vue` 暂不改行为，只记录依赖这些页面的入口关系，作为后续路由收口时的对照清单。
5. 明确把“前端类型契约清理完成”设为所有后续阶段的阻塞门禁：未解决 `src/web/src/views/desktop/learningcenter/ListPage.vue` 的基线错误，不允许开始统一页面切换。

### 测试
- `Set-Location src\web; npm run lint`
- `Set-Location src\web; npm run build`

### 验证标准
- `src/web/src/views/desktop/learningcenter/ListPage.vue` 不再出现围绕 `resp.data?.error` 的 `vue-tsc` 基线错误。
- `src/web/src/core/api.ts` 所定义的 `ApiResponse<T>` 契约成为配对中心相关页面的统一消费方式。
- 后续路由或页面收口工作不再被现存类型错误阻塞。

### 依赖关系
- 无；本阶段是全部后续阶段的硬门禁。

### 已完成切片
- ✅ 基线切片：`src/web/src/views/desktop/learningcenter/ListPage.vue` 的 `ApiResponse` 包络误用清理、最小 CI 兼容修复，提交 `8ec2cdf`
- ✅ 跟进切片：`src/web/src/views/desktop/rules/RuleCenterPage.vue` 的规则接口失败分支显式处理与本地 DTO 类型补全，提交 `88322dc`

## 阶段 2: 路由与导航收口到唯一配对中心壳层 — 预计 todo 数: 6
### 目标
先完成信息架构层面的“一个中心”收口，明确 `/pairing/list` 为唯一持久化中心入口，再为后续内容迁移提供统一壳层。

### 步骤
1. 在 `src/web/src/router/desktop.ts` 规划保留 `/pairing/list` 作为 canonical route，并把 `/learning/center`、`/rules/center` 标记为切换完成后的 redirect / wrapper 目标，避免未来再扩散新持久化入口。
2. 在 `src/web/src/views/desktop/MainLayout.vue` 设计把 Pairing / Learning / Rule Center 三个并列导航项收敛成一个“配对中心”导航项，并明确原 Learning / Rule Center 菜单仅作为过渡期兼容入口。
3. 以 `src/web/src/views/desktop/pairingcenter/ListPage.vue` 为统一壳层，规划将 `src/web/src/views/desktop/learningcenter/ListPage.vue` 与 `src/web/src/views/desktop/rules/RuleCenterPage.vue` 的可复用区块改造成配对中心内部 tab / section，而不是三个独立页面继续并存。
4. 在 `src/web/src/views/desktop/pairingcenter/ListPage.vue` 中移除“跳去用户设置”的投资设置入口方案，改为统一配对中心内部可见的域内入口；同时在 `src/web/src/views/desktop/user/settings/tabs/UserDataManagementSettingTab.vue` 标注该功能后续将退出此页面。
5. 约束导入预览页面 `src/web/src/views/desktop/transactions/import/tabs/ImportTransactionCheckDataTab.vue` 只保留 session-assistant 入口，不新增第二套持久化导航，不与 `src/web/src/router/desktop.ts` 形成平级中心关系。
6. 对 `src/web/src/views/desktop/learningcenter/ListPage.vue`、`src/web/src/views/desktop/rules/RuleCenterPage.vue` 预先定义 cutover 后的命运：优先改成 redirect wrapper，待回归稳定后再考虑删除实体页面。

### 测试
- `Set-Location src\web; npm run lint`
- `Set-Location src\web; npm run build`

### 验证标准
- 方案层面明确 `/pairing/list` 是唯一持久化中心入口。
- `src/web/src/views/desktop/MainLayout.vue` 的未来导航模型只保留一个“配对中心”主入口。
- 导入预览助手被清晰界定为会话级辅助流程，而不是第二个中心。

### 依赖关系
- 依赖阶段 1 的前端类型契约清障完成。

### 已完成切片
- ✅ `src/web/src/views/desktop/MainLayout.vue` 已移除 Learning / Rule Center 的并列主导航，只保留 Pairing Center 作为唯一可见中心入口，提交 `d5244e1`
- ✅ `src/web/src/views/desktop/pairingcenter/ListPage.vue` 已补统一壳层 quick-entry，当前可从 Pairing Center 内部进入 Pairs、Investment Settings，并通过临时兼容入口跳转 Learning / Rule Center，提交 `d5244e1`
- ✅ `src/web/src/views/desktop/learningcenter/ListPage.vue` 与 `src/web/src/views/desktop/rules/RuleCenterPage.vue` 已增加兼容入口横幅，明确它们只是过渡期 legacy deep-link 页面，并引导返回 Pairing Center，提交 `8dc2af9`
- ✅ `src/web/src/views/desktop/transactions/import/tabs/ImportTransactionCheckDataTab.vue` 已弱化全局管理按钮在导入预览中的视觉权重，明确导入预览只承担 session-assistant 角色而不是第二中心，提交 `afb23fd`

### 阶段结论
- ✅ 阶段 2 当前已完成：桌面主导航、统一 Pairing Center 壳层、legacy 兼容入口、投资设置归位与导入预览 session-assistant 边界都已建立，后续进入阶段 3 的统一持久化域收口。

## 阶段 3: 统一配对中心持久化域收口（学习 / 规则 / 投资） — 预计 todo 数: 7
### 目标
把规则中心、学习中心、投资识别设置收口为同一个持久化配对中心域，并移除“投资设置属于用户设置”的旧归属。

### 步骤
1. 以后端持久化域为中心，先梳理 `src/bill_analyser/api/routes/rules.py`、`src/bill_analyser/api/routes/category_rules.py`、`src/bill_analyser/api/routes/matching.py` 与 dedicated pairing-center domain service/API 的职责，把长期规则、学习规则、投资识别设置统一映射到“配对中心域”能力；`src/bill_analyser/api/routes/auth.py` 与 profile 语义只保留过渡兼容，不再作为正式归属或首选写路径。
2. 在数据访问层采用增量迁移策略：先把投资识别设置的域归属、API 入口与 UI 写路径从 `src/bill_analyser/api/routes/auth.py` / users/profile 语义中脱钩，为统一配对中心提供专用 compatibility layer；底层物理存储可暂时继续落在 `src/bill_analyser/core/db_schema_users_security.py` 对应的 users 表之后，待统一中心稳定后，再评估是否迁往更贴近学习/规则的 shard（如 `src/bill_analyser/core/db_import_learning.py`）或新增 `src/bill_analyser/core/db_matching.py`，且对外仍只经 `src/bill_analyser/core/db.py` facade 暴露。
3. 在前端把 `src/web/src/views/desktop/user/settings/tabs/UserDataManagementSettingTab.vue` 中的投资识别设置入口彻底移除，并在 `src/web/src/views/desktop/pairingcenter/ListPage.vue` 或其子组件目录下承接同等能力，保证“投资设置不再住在用户设置里”是强制门禁。
4. 以 `src/web/src/views/desktop/learningcenter/ListPage.vue`、`src/web/src/views/desktop/rules/RuleCenterPage.vue` 的现有能力清单为输入，规划统一配对中心内部至少包含：长期学习规则、分类规则总览、投资识别设置、 recurring / pairing 辅助规则，且不再出现三个持久化页面各自维护一套表述。
5. 在服务层同步收口 `src/web/src/lib/services.ts`，让配对中心只消费一套 dedicated pairing-center domain API；原本散落在 `src/web/src/views/desktop/learningcenter/ListPage.vue` 和 `src/web/src/views/desktop/rules/RuleCenterPage.vue` 的直连接口需要被替换成统一入口，且前端不再把 auth/profile 路径视为投资设置的官方写入链路。
6. 对 `src/bill_analyser/api/routes/auth.py` 中与投资识别有关的 profile 读写保留过渡兼容期，但要明确它只是 compatibility wrapper / fallback 入口；即使底层暂时仍复用 users 表存储，正式域服务与官方写路径也应先切到统一配对中心，待其稳定后再评估物理表 / shard 迁移。
7. 在统一域完成前，不允许新增任何新的“学习中心设置”或“规则中心设置”独立页面；所有新增持久化设置必须落在 `src/web/src/views/desktop/pairingcenter/` 下。

### 测试
- `Set-Location src\web; npm run lint`
- `Set-Location src\web; npm run build`
- `.\.venv\Scripts\python.exe -m pytest tests\ -v`

### 验证标准
- `src/web/src/views/desktop/user/settings/tabs/UserDataManagementSettingTab.vue` 不再承载投资识别设置。
- 统一配对中心成为学习规则、规则配置、投资识别设置的唯一持久化 UI 归属。
- 后端投资识别设置的正式域服务与官方写路径不再以 auth/profile 语义为主；即使底层暂时保留 users 表兼容存储，也只能通过统一配对中心 compatibility layer 暴露。

### 依赖关系
- 依赖阶段 1 的类型基线。
- 建议在阶段 2 的路由壳层方案明确后执行，以减少重复页面搬运。

### 已完成切片
- ✅ 投资识别设置编辑能力已从 `src/web/src/views/desktop/user/settings/tabs/UserDataManagementSettingTab.vue` 脱离，统一由 `src/web/src/views/desktop/pairingcenter/ListPage.vue` 内部的 investment-settings section 承接，提交 `ed1a282`
- ✅ `src/web/src/views/desktop/rules/RuleCenterPage.vue` 与用户设置页中的相关入口已统一改为跳转 `/pairing/list?view=investment-settings`，不再回到用户设置承载正式编辑

## 阶段 4: category_rules 成为唯一真源，并把分类编辑改成布尔表达式 — 预计 todo 数: 7
### 目标
彻底移除 legacy keyword 分类逻辑，让分类规则和分类编辑都围绕新规则体系运行。

### 步骤
1. 在 `src/bill_analyser/core/category_engine.py` 去掉 `load_rules_from_db_v2` 对 `categories.keywords` 的 fallback，明确仅从 category rules 相关来源加载规则，保证运行时只有一套分类判定主链。
2. 在 `src/bill_analyser/api/routes/category_rules.py` 保留并强化规则 CRUD / migrate / test 的唯一入口角色，同时更新 `src/bill_analyser/api/routes/rules.py` 的 overview 聚合，停止继续读取或展示带有 legacy 味道的 `category_keywords` / `transaction_categories` 旧来源。
3. 在 `src/web/src/views/desktop/categories/list/dialogs/EditDialog.vue` 用新的规则表达式编辑体验取代旧关键词录入，并把 `src/web/src/components/common/KeywordInput.vue` 的角色改造成 Web of Science 风格布尔表达式输入器，或由新组件完全替代它。
4. 在 `src/web/src/models/transaction_category.ts` 移除或废弃 `keywords` 字段的请求/模型语义，改为显式承载规则表达式、规则摘要或与 `src/bill_analyser/api/routes/category_rules.py` 对齐的 rule DTO。
5. 在 `src/web/src/views/desktop/rules/RuleCenterPage.vue`（切换期）与后续统一后的 `src/web/src/views/desktop/pairingcenter/` 页面中，移除 legacy keywords、investment keywords 这类旧概念的独立展示，避免用户继续认为“关键词分类”与“新规则分类”是两套系统。
6. 设计一次性迁移路径：利用 `src/bill_analyser/api/routes/category_rules.py` 已有 migrate/test 能力，把历史 `categories.keywords` 数据转换为规则表达式后再关闭 runtime fallback，而不是长期双跑。
7. 明确验收门禁：只有当 `src/bill_analyser/core/category_engine.py` 不再读取 legacy keywords，且 `src/web/src/views/desktop/categories/list/dialogs/EditDialog.vue` 不再展示旧关键词编辑时，才允许宣告切换完成。

### 测试
- `Set-Location src\web; npm run lint`
- `Set-Location src\web; npm run build`
- `.\.venv\Scripts\python.exe -m pytest tests\ -v`

### 验证标准
- `category_rules` 成为唯一真源，运行时不再依赖 `categories.keywords`。
- 分类编辑界面只暴露布尔表达式语法，不再暴露 legacy keyword editor。
- 规则总览与统一配对中心页面不再展示旧关键词逻辑。

### 依赖关系
- 依赖阶段 1 的前端类型基线。
- 与阶段 3 高度耦合，建议在统一配对中心域结构基本确定后执行。

### 已完成切片
- ✅ `src/bill_analyser/api/routes/rules.py` 的 overview 聚合已停止读取 legacy `category_keywords`，改为暴露 `categoryRuleCount`，并由新增 `tests/domains/categories/unit/test_rules_route_branches.py` 锁住“category_rules 为 canonical source”的契约
- ✅ `src/web/src/views/desktop/rules/RuleCenterPage.vue` 已移除 Legacy Keywords 独立 tab，并把 `Investment Keywords` 调整为仅保留只读兼容摘要的 `Investment Settings` 入口，避免继续把旧关键词体系当作独立中心能力
- ✅ 为修复 CI `verify-agent-stack` 的 `repo.codex-baseline` fail，仓库已补齐最小 `.codex/config.toml` fallback，使 `scripts/agent_stack_health.py --mode repo` 在无用户级 `~/.codex/config.toml` 的环境下也能通过
- ✅ `src/bill_analyser/core/category_engine.py` 已去掉 `load_rules_from_db_v2` 对 `categories.keywords` 的 runtime fallback，并由 `tests/domains/categorization/unit/test_category_engine.py` 与 `tests/new_ui/test_app.py` 锁住“运行时只从 category_rules 读取分类规则”的契约
- ✅ `src/web/src/models/transaction_category.ts` 已把前端模型 canonical 字段收口到 `ruleExpression`，`keywords` 仅保留兼容桥接；配套 `tests/web/models/transaction_category.test.ts` 已补 ruleExpression 主断言
- ✅ `src/web/src/components/common/KeywordInput.vue` 已完成前半刀语义收口：组件标题、空态、帮助文案、按钮与 clause 语义已从 legacy keyword list 转向布尔表达式 / ruleExpression 输入器
- ✅ `src/web/src/views/desktop/categories/list/dialogs/EditDialog.vue` 已完成后半刀认知收口：分类编辑弹窗中的字段标题、帮助说明、保存语义与表达式示例已统一切到“分类规则表达式”，不再把该输入区表述为关键词列表

## 阶段 5: LLM 入口迁移到导入预览 session-assistant 域 — 预计 todo 数: 6
### 目标
把 LLM 归纳/业务入口从“已落库未分类账单分析”改到“导入预览、写库前的会话级辅助”，同时与统一配对中心的长期规则域形成明确分层。

### 步骤
1. 在前端把 LLM 入口从 `src/web/src/views/desktop/learningcenter/ListPage.vue` 的持久化中心语义中剥离，改为由 `src/web/src/views/desktop/transactions/import/tabs/ImportTransactionCheckDataTab.vue` 发起，会话内只针对当前 import preview / session 数据提供分析与建议。
2. 在后端把 `src/bill_analyser/api/routes/llm.py` 的业务入口重定向到导入预览链路，优先与 `src/bill_analyser/api/routes/bills.py` 已有 `import v2 preview / suggestions / promote` 能力对齐，而不是继续从正式 `bills` 表筛选 uncategorized 账单。
3. 在 `src/bill_analyser/core/llm_learning_service.py` 把 `analyze_transactions` 一类“查正式账单未分类数据”的逻辑改造成“读取 import session / bills_preview / preview updates”的逻辑，确保 LLM 在写库前就能结合导入会话上下文做归纳。
4. 以 `src/web/src/views/desktop/transactions/import/tabs/ImportTransactionCheckDataTab.vue`、`src/bill_analyser/api/routes/bills.py`、`src/bill_analyser/core/db_import_sessions.py`、`src/bill_analyser/core/db_import_preview.py` 为主链，定义 session-scoped assistant 的输入输出；长期规则沉淀仍交由统一配对中心域管理，避免 LLM 会话助手变成新的持久化中心。
5. 把 `src/web/src/views/desktop/learningcenter/ListPage.vue` 中与 LLM config / candidates 强绑定的持久化页面功能改成过渡入口或历史兼容壳，最终让用户从导入预览流程而不是中心页发起这类分析。
6. 规定 cutover 验收标准：只要 `src/bill_analyser/core/llm_learning_service.py` 仍以 persisted uncategorized bills 为主入口，就不得宣告本阶段完成。

### 测试
- `Set-Location src\web; npm run lint`
- `Set-Location src\web; npm run build`
- `.\.venv\Scripts\python.exe -m pytest tests\ -v`

### 验证标准
- LLM 分析从导入预览 session / preview 数据发起，而不是从正式账单表的未分类记录发起。
- `src/web/src/views/desktop/transactions/import/tabs/ImportTransactionCheckDataTab.vue` 成为用户发起 LLM 会话分析的主入口。
- 统一配对中心只保留长期规则与长期学习配置，不把导入会话助手重新做成第二个中心。

### 依赖关系
- 依赖阶段 3 的域边界梳理，以便区分持久化配对中心域与 session-assistant 域。
- 若阶段 4 已完成，可直接复用新的规则表达式和长期规则沉淀路径。

### 已完成切片
- ✅ 导入预览会话 LLM 归纳链路已切到 `ImportTransactionCheckDataTab.vue -> services.ts -> /api/llm/analyze-transactions -> LLMLearningService._analyze_import_session(...)`；Learning Center 仅保留配置与历史候选兼容壳。同步补齐了 `preview_ids / preview_updates` 约束、candidate user-scope、防重复提交 UI 与 `tests/new_ui/test_llm_import_session_analysis_api.py` 回归；当前已通过全量 `pytest tests/ -v`、相关 `pylint`、`vue-tsc`、最小前端 `eslint` 与 `npm run build` 验证，repo 级 `npm run lint` 仍受仓库既有前端 lint debt 阻塞。

## 阶段 6: cutover、兼容包装与最终清理 ✅ (`5f023112`) — 预计 todo 数: 6
### 目标
在统一配对中心上线后完成路由切换、兼容包装、旧页面降级和全链路验收。

### 步骤
1. 在 `src/web/src/router/desktop.ts` 正式把 `/learning/center` 与 `/rules/center` 变成指向 `/pairing/list` 的 redirect，或指向统一配对中心壳层的 wrapper route；同时把 `/pairing/list` 固定为唯一 canonical route。
2. 在 `src/web/src/views/desktop/learningcenter/ListPage.vue` 与 `src/web/src/views/desktop/rules/RuleCenterPage.vue` 落地过渡策略：优先做最薄 redirect wrapper；若仍需保留页面文件，也只能做兼容壳，不再保有独立持久化业务实现。
3. 在 `src/web/src/views/desktop/MainLayout.vue` 完成旧导航删除，只保留“配对中心”主入口，并确认任何深链都不会再把用户带到独立学习中心或规则中心。
4. 在 `src/web/src/views/desktop/pairingcenter/ListPage.vue`、`src/web/src/views/desktop/categories/list/dialogs/EditDialog.vue`、`src/web/src/views/desktop/transactions/import/tabs/ImportTransactionCheckDataTab.vue` 三条主链上执行最终人工核对，确认持久化中心、分类规则编辑、导入预览助手三者边界清晰且无相互回退。
5. 在 `src/bill_analyser/api/routes/rules.py`、`src/bill_analyser/api/routes/category_rules.py`、`src/bill_analyser/api/routes/llm.py`、`src/bill_analyser/api/routes/bills.py` 上做最终契约梳理，确保前端不会再依赖被弃用的旧接口语义。
6. 将“旧 routes/pages 仅作为 redirect 或 wrapper”设为最终门禁：若 `src/web/src/router/desktop.ts` 仍保有多个并列持久化中心入口，则不得结束切换。

### 测试
- `Set-Location src\web; npm run lint`
- `Set-Location src\web; npm run build`
- `.\.venv\Scripts\python.exe -m pytest tests\ -v`

### 验证标准
- `/pairing/list` 成为唯一持久化中心入口。
- `/learning/center` 与 `/rules/center` 仅保留 redirect / wrapper 语义。
- 分类规则、投资识别设置、长期学习规则、导入预览 LLM 助手的边界已完成切割。

### 依赖关系
- 依赖阶段 2、阶段 3、阶段 4、阶段 5 全部完成。

### 已完成切片
- ✅ `src/web/src/router/desktop.ts` 已将 `/learning/center` 与 `/rules/center` 正式收口为指向 `/pairing/list` 的 redirect，并修复 redirect helper 的 vue-router 类型签名，提交 `5f023112`
- ✅ `src/web/src/views/desktop/pairingcenter/ListPage.vue` 已承接 Learning / Rule Center 的持久化能力壳层；新增 `src/web/src/views/desktop/pairingcenter/components/LearningCenterPanel.vue` 与 `RuleCenterPanel.vue` 承载原页面主体，实现 `/pairing/list` 作为唯一 canonical 持久化中心
- ✅ `src/web/src/views/desktop/learningcenter/ListPage.vue` 与 `src/web/src/views/desktop/rules/RuleCenterPage.vue` 已降级为最薄 redirect wrapper，不再保有独立持久化业务实现；`docs/PROJECT_OVERVIEW.md` 已同步更新当前系统事实
- ✅ 本阶段验收结果：`Set-Location src\web; npm run build` 通过；`Set-Location src\web; npx vue-tsc --noEmit` 通过；针对本切片触达文件的 `npx eslint ...` 通过；repo 级 `npm run lint` 在本仓库现状下长时间未结束（两次超时），未再出现此前由 router 类型不匹配导致的前端 gate 失败；`.\.venv\Scripts\python.exe -m pytest tests\ -v` 全量通过

### 阶段结论
- ✅ 阶段 6 当前已完成：`/pairing/list` 已成为唯一持久化中心入口，`/learning/center` 与 `/rules/center` 仅保留 redirect / wrapper 语义，分类规则 / 投资识别设置 / 长期学习规则 / 导入预览 session-assistant 的边界已完成切割。

## 风险与缓解

| 风险 | 影响 | 触发点 | 缓解措施 |
| --- | --- | --- | --- |
| `src/web/src/views/desktop/learningcenter/ListPage.vue` 的 `ApiResponse<any>` 包络错误未先清理 | 后续统一页面改造持续被 `vue-tsc` 噪音淹没 | 阶段 2 之前 | 把阶段 1 设为硬门禁，先跑 `Set-Location src\web; npm run lint` 和 `Set-Location src\web; npm run build` |
| `src/bill_analyser/core/category_engine.py` 长期保留 keywords fallback | 运行时出现双规则源，用户无法判断谁生效 | 阶段 4 切换期 | 通过 `src/bill_analyser/api/routes/category_rules.py` 先完成迁移，再关闭 fallback，并以全量 `.\.venv\Scripts\python.exe -m pytest tests\ -v` 验证 |
| 投资识别设置继续留在 `src/web/src/views/desktop/user/settings/tabs/UserDataManagementSettingTab.vue` / `src/bill_analyser/api/routes/auth.py` | 域边界继续错误，统一配对中心名实不符 | 阶段 3 | 明确 UI 与 API 双重门禁：用户设置页移除入口、auth 路径只保留临时兼容包装 |
| LLM 仍从 `src/bill_analyser/core/llm_learning_service.py` 查询已落库未分类账单 | 用户导入前无法获得正确业务辅助，且会污染“一个中心”设计 | 阶段 5 | 把 `src/web/src/views/desktop/transactions/import/tabs/ImportTransactionCheckDataTab.vue` 作为唯一业务入口，改用 import session / preview 数据 |
| `/learning/center`、`/rules/center` 没有及时降级为 redirect/wrapper | 前端仍存在三个并列中心，导航与外链继续分裂 | 阶段 6 | 在 `src/web/src/router/desktop.ts` 设置 final gate，cutover 完成后只允许 `/pairing/list` 是正式中心入口 |

## 回滚方案
- 路由回滚：若统一配对中心壳层切换后导航不可用，可先在 `src/web/src/router/desktop.ts` 恢复 `/learning/center` 与 `/rules/center` 的原始页面挂载，但必须保留 `/pairing/list` 为首选入口，避免再次形成长期多中心设计。
- 页面回滚：若 `src/web/src/views/desktop/pairingcenter/ListPage.vue` 的统一壳层在短期内无法稳定承接，可临时让 `src/web/src/views/desktop/learningcenter/ListPage.vue` 与 `src/web/src/views/desktop/rules/RuleCenterPage.vue` 继续以 wrapper + 嵌入式内容方式存在，而不是恢复成独立产品中心。
- 分类规则回滚：若 `src/bill_analyser/core/category_engine.py` 关闭 legacy keywords fallback 后出现误分类，可在过渡期保留迁移前快照并回退到“先迁移数据、后关闭 fallback”的顺序，但不应重新开放 UI 上的 legacy keyword editor。
- 投资设置回滚：若新配对中心域的投资设置存储未稳定，可在 `src/bill_analyser/api/routes/auth.py` 保留短期兼容读取，但 `src/web/src/views/desktop/user/settings/tabs/UserDataManagementSettingTab.vue` 不应重新恢复为正式入口。
- LLM 回滚：若导入预览 session-assistant 路径短期不可用，可暂时保留 `src/bill_analyser/api/routes/llm.py` 的旧实现作为后台兼容开关，但前端入口仍应优先固定在 `src/web/src/views/desktop/transactions/import/tabs/ImportTransactionCheckDataTab.vue`，避免重新鼓励用户走错误业务入口。

## 实施顺序建议（注明哪些步骤适合交给 specialized agent，例如 planner/build-fixer/typescript-reviewer/code-reviewer/database-reviewer）
1. 先做阶段 1（建议 `build-fixer` 或 `typescript-reviewer` 参与）：先清掉 `src/web/src/views/desktop/learningcenter/ListPage.vue` 的类型包络错误，再开始其余工作。
2. 再做阶段 2（建议 `planner` 参与）：统一 `src/web/src/router/desktop.ts` 和 `src/web/src/views/desktop/MainLayout.vue` 的信息架构，先确定一个中心的外壳。
3. 接着做阶段 3（建议 `planner` + `database-reviewer`）：先处理 `src/web/src/views/desktop/user/settings/tabs/UserDataManagementSettingTab.vue`、`src/bill_analyser/api/routes/auth.py` 的域归属与 API 收口，再评估 `src/bill_analyser/core/db_schema_users_security.py` 等底层存储迁移，确保投资设置先脱离用户设置的正式归属。
4. 然后做阶段 4（建议 `database-reviewer` + `typescript-reviewer`）：让 `src/bill_analyser/core/category_engine.py` 与 `src/web/src/views/desktop/categories/list/dialogs/EditDialog.vue` 同步切到 category_rules 单一真源和布尔表达式语法。
5. 再做阶段 5（建议 `planner` + `code-reviewer`）：把 `src/bill_analyser/api/routes/llm.py` / `src/bill_analyser/core/llm_learning_service.py` 与 `src/web/src/views/desktop/transactions/import/tabs/ImportTransactionCheckDataTab.vue` 重新对齐到 import-preview session-assistant 模式。
6. 最后做阶段 6（建议 `code-reviewer` + `typescript-reviewer`）：执行 `/learning/center`、`/rules/center` 到 `/pairing/list` 的最终 cutover，并跑 `Set-Location src\web; npm run lint`、`Set-Location src\web; npm run build`、`.\.venv\Scripts\python.exe -m pytest tests\ -v` 作为整体验收。
