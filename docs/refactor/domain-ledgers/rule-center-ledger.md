# D3 规则中心域重构 ledger

## 1. 范围与目标

D3 `rule-center` 接管规则中心相关核心规则语义、HTTP rule handlers 和桌面规则中心页面结构。目标是沿用 D1/D2 模板，按 ledger -> behavior-lock -> backend-shape -> frontend-shape -> comment-pass -> governance-docs -> closeout 顺序推进。

本域不改变 REST 路由、PostgreSQL 表结构、规则表达式语法、账户识别上下文、分类识别语义、settings bundle 字段、金额单位或当前 UI 视觉设计。

## 2. owned paths

计划定义的 D3 owned paths：

- `src/backend/core/account_rules/**`
- `src/backend/core/category_rules/**`
- `src/backend/http/**rule**`
- `src/web/src/views/desktop/pairingcenter/**`

实际实现时需要额外注意以下 shared paths：

- `src/backend/db/taxonomy/**`：D2 已接管 taxonomy repository 和 settings bundle；D3 仅在行为锁定或接口适配必须时申请 lease。
- `src/backend/http/import_routes/**`：导入 stage2 会消费分类/账户规则；D3 不改导入顺序、转账保护或账户规则最后匹配合同。
- `src/web/src/lib/services.ts`、`src/web/src/stores/**`、`src/web/src/models/**`：D10 或对应功能域共享面；D3 只读合同，必要改动先写 shared lease。
- `src/web/src/views/desktop/pairingcenter/components/LearningCenterPanel.vue` 中的 LLM/OCR/learning provider 细节与 D8 重叠；D3 只能拆规则中心页面壳和本地状态边界，不改变外部 provider 调用或 OCR/LLM 配置语义。
- `src/web/src/components/common/CategoryRuleBuilderFields.vue` 是规则表达式编辑共享组件；D3 可作为测试锁定对象，结构改动前需要明确影响面。

## 3. 当前后端结构现状

| 文件 | 当前职责 | 当前行数 | D3 处理建议 |
| --- | --- | ---: | --- |
| `src/backend/core/account_rules/mod.rs` | 账户规则候选、scope 归一、上下文字段包、表达式匹配、跨字段匹配、测试 | 898 | 拆为 `types`、`normalization`、`matching`、`prepared_text`、`tests`；保持 public API facade 不变 |
| `src/backend/core/category_rules/mod.rs` | 分类规则表达式 AST、parser、regex cache、匹配器、escape/unescape、测试 | 646 | 拆为 `types`、`parser`、`matching`、`terms`、`tests`；保持表达式语法和 regex cache 行为不变 |
| `src/backend/http/taxonomy_routes/account_rule_handlers.rs` | 账户规则 list/create/update/delete/reorder/test、compat warning、payload 清洗、测试上下文 | 571 | 尚低于 hard，但职责多；backend-shape 可拆 handler 与 formatter/helper，或保留到 D3 comment-pass 后再治理 |
| `src/backend/http/taxonomy_routes/category_rule_handlers.rs` | 分类规则 list/create/update/delete/reorder/defaults/test/overview handler | 284 | 作为接口合同锁定；仅在拆 handler consistency 时调整 |
| `src/backend/http/taxonomy_routes/audit_and_rules_helpers.rs` | 敏感账户操作、审计日志、rules query helper、JSON helper | 133 | 若 D3 拆 rule handler，避免把账户审计职责误并入规则核心 |

后端结构门禁当前直接 D3 债务：

- `src/backend/core/account_rules/mod.rs` 超过新文件 600 行限制。
- `src/backend/core/category_rules/mod.rs` 超过新文件 600 行限制。
- account/category rule HTTP handlers 不直接触发结构失败，但内部 helper 与 compat warning 需要在 comment-pass 覆盖中文说明。

## 4. 当前前端结构现状

| 文件 | 当前职责 | 当前行数 | D3 处理建议 |
| --- | --- | ---: | --- |
| `src/web/src/views/desktop/pairingcenter/components/RuleCenterPanel.vue` | 分类规则 tab、learning/recurring overview、分类规则表格、筛选、批量操作、编辑/测试/删除 dialog、API 调用 | 2081 | 优先拆模板/样式，再拆 category rule composable、dialog、table/toolbar、filter state |
| `src/web/src/views/desktop/pairingcenter/components/AccountRulePanel.vue` | 账户识别规则列表、分组展示、编辑/测试/删除、settings JSON 导入导出 | 891 | 拆模板/样式、账户规则 form/dialog、service actions、group view model |
| `src/web/src/views/desktop/pairingcenter/components/LearningCenterPanel.vue` | learning rules、LLM 规则候选、LLM config、OCR config 入口、批量操作 | 1690 | D3 仅拆规则中心壳层或状态组织；LLM/OCR/learning provider 语义留给 D8 |
| `src/web/src/views/desktop/pairingcenter/ListPage.vue` | 规则中心主页面、一级/二级导航、配对概览、刷新/删除 pair、query sync | 721 | 拆导航状态、header actions、pairing overview 壳；保持路由 query 合同 |
| `rule_center_navigation.ts` | 规则中心 query/domain/tab 归一 | 86 | 已有测试，作为行为锁定入口 |
| `ruleCenterFilters.ts` | 分类规则筛选解析与匹配 | 135 | 作为行为锁定入口；后续若移动要保持导出函数名 |

前端结构门禁当前直接 D3 债务：

- `RuleCenterPanel.vue`、`LearningCenterPanel.vue`、`ListPage.vue` 在 legacy baseline 中仍超阈值或增长。
- `AccountRulePanel.vue` 当前超过新文件限制但不在旧 baseline 中。
- D3 frontend-shape 不应触碰 `services.ts` 全量拆分；规则中心调用可先通过局部 composable 封装现有 service 方法。

## 5. 现有行为锁定入口

后端测试：

- `tests/backend/db/category_rules_postgres.rs`
- `tests/backend/db/account_rules_postgres.rs`
- `tests/backend/http/import_runtime_contract.rs`
- `src/backend/core/account_rules/mod.rs` 内部单元测试
- `src/backend/core/category_rules/mod.rs` 内部单元测试

前端测试：

- `tests/web/views/desktop/pairingcenter/ruleCenterNavigation.test.ts`
- `tests/web/views/desktop/pairingcenter/ruleDialogLayout.test.ts`
- `tests/web/views/desktop/pairingcenter/learningCenterOcrConfig.test.ts`
- `tests/web/views/mobile/accountRuleListPage.test.ts`
- `tests/web/models/account_rule.test.ts`
- `tests/web/views/desktop/settingsJsonPerPageImportExport.test.ts`
- `tests/web/locales/i18nKeyContract.test.ts`

Behavior-lock 切片至少应补强：

- 分类规则表达式 parser/matcher：OR/AND/NOT/REGEX、转义字符、括号组合、空表达式、regex disabled。
- 账户规则匹配：完整 token 语义、跨字段匹配、source/destination/payment method 上下文、投资/转账上下文、deprecated scope warning 不进入持久化。
- 桌面规则中心：一级域、二级规则配置 tab、分类/账户规则列表、settings JSON import/export、筛选/分页/批量操作、编辑/测试/删除 dialog。
- D8 重叠面：Learning/LLM/OCR 入口只锁当前可见导航和不触发 live provider 调用。

## 6. 切片计划

### 6.1 ledger

- 本文件记录边界、文件地图、测试入口、风险和 D3 完成判定。
- 只允许文档和 progress writeback，不移动源码。
- 最小验证：
  - `node scripts/check-rust-backend-structure.mjs`（预期仍失败；记录 D3 直接项）
  - `Set-Location src/web; npm run structure:check`（预期仍失败；记录 D3 直接项）
  - `node scripts/check-backend-doc-map.mjs`

### 6.2 behavior-lock

- 只允许新增/调整测试、fixture、smoke 和必要合同文档。
- 后端优先补 core matcher 单元测试和 HTTP rule handler contract。
- 前端优先补 pairingcenter source guards 与 focused unit tests，不做结构移动。
- 若发现真实 bug，当前切片写 `blocked:<reason>`，另开 fix slice。

### 6.3 backend-shape

建议顺序：

1. 将 `core/category_rules/mod.rs` 改为 facade，并拆表达式 AST/types、parser、matcher、term escape、regex cache。
2. 将 `core/account_rules/mod.rs` 改为 facade，并拆候选/types、scope normalization、context text preparation、matching、tests。
3. 如结构门禁仍要求，拆 `account_rule_handlers.rs` 的 handler、formatter、compat warning 和 test-context helper；保持 route path 和 response envelope 不变。
4. 运行 focused Rust tests；若改业务源码，最终运行 Rust workspace coverage gate。

### 6.4 frontend-shape

建议顺序：

1. 先拆 `RuleCenterPanel.vue` 的 template/style，再拆分类规则 state/composable、table toolbar、target grouping、dialog action。
2. 拆 `AccountRulePanel.vue` 的 template/style、分组 view model、form/dialog、service actions。
3. 拆 `ListPage.vue` 的导航/query sync、header actions、pair overview 壳。
4. `LearningCenterPanel.vue` 只做 D3 必需的壳层/结构拆分；LLM/OCR/learning provider 细节进入 D8。
5. 跑 pairingcenter focused tests、lint、coverage；用户可见布局改动补 browser smoke。

### 6.5 comment-pass/governance-docs/closeout

- 按注释策略补齐 D3 导出函数、业务关键函数和复杂私有 helper 中文说明。
- 更新 `docs/PROJECT_OVERVIEW.md` 中规则中心当前结构事实。
- 收紧 Rust/frontend structure baseline，让完成的 D3 项退出 failure/warning。
- PR/CI/merge/delete/writeback 证据完整后，cursor 推进到 D4。

## 7. 关键风险与阻断条件

- 规则表达式语法是业务合同；拆 parser/matcher 时不得改变 OR/AND/NOT/REGEX、括号、转义和 regex disabled 语义。
- 账户规则已迁移掉旧 role/type/field scope 持久列；旧 payload/query/bundle 只能保留兼容 warning 和忽略，不得重新引入持久化 scope。
- 导入链路要求账户规则在稳定后的类型/分类/recurring/learning 投影之后运行；D3 不得调整 stage2 顺序。
- 分类规则和账户规则 settings JSON import/export 是 D2 已锁合同；D3 不得改变 section key 或引用重映射。
- LearningCenterPanel 与 D8 LLM/OCR/learning 域重叠；D3 不得新增 live provider 调用或更改 provider 配置保存语义。
- Rule center UI 只做结构拆分，不做视觉重设计；按钮、筛选、分页、分组、dialog 交互必须保持。

## 8. D3 完成判定

D3 完成后应满足：

- `core/category_rules/mod.rs` 和 `core/account_rules/mod.rs` 不再由单文件承载 parser、matcher、normalization、helper 和测试。
- 规则中心 HTTP handler 的 compat warning、payload 清洗、formatter 和 test context helper 有清晰边界。
- `RuleCenterPanel.vue`、`AccountRulePanel.vue`、`ListPage.vue` 只负责装配；复杂状态、筛选、分组、dialog 和 service action 已分离。
- `LearningCenterPanel.vue` 的 D3 结构债已按壳层处理或显式移交 D8。
- D3 直接相关结构门禁项不回涨，并从 failure/warning 列表退出或被明确移交后续域。
- 行为锁定覆盖分类/账户规则 CRUD、reorder、test、表达式语法、settings import/export、规则中心导航与可见交互。
- PR/CI/merge/delete/writeback 证据完整，cursor 推进到 D4。
