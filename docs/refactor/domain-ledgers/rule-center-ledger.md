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

当前 behavior-lock 切片锁定：

- `tests/backend/core/rule_center_contracts.rs` 覆盖分类规则表达式的括号、OR/AND/NOT、REGEX、转义逗号、空表达式和非法表达式合同。
- `tests/backend/core/rule_center_contracts.rs` 覆盖账户规则的优先级、禁用规则跳过、投资上下文字段、跨字段 fallback、token 边界和 `regex_enabled` 正则匹配。
- `tests/web/views/desktop/pairingcenter/ruleCenterFilters.test.ts` 覆盖规则中心筛选查询的全角符号归一、include/exclude 组合、regex 模式 fail closed 和主分类筛选顺序。

### 6.3 backend-shape

建议顺序：

1. 将 `core/category_rules/mod.rs` 改为 facade，并拆表达式 AST/types、parser、matcher、term escape、regex cache。
2. 将 `core/account_rules/mod.rs` 改为 facade，并拆候选/types、scope normalization、context text preparation、matching、tests。
3. 如结构门禁仍要求，拆 `account_rule_handlers.rs` 的 handler、formatter、compat warning 和 test-context helper；保持 route path 和 response envelope 不变。
4. 运行 focused Rust tests；若改业务源码，最终运行 Rust workspace coverage gate。

当前 backend-shape 切片结果：

- `src/backend/core/category_rules/mod.rs` 已改为 facade；表达式 DTO、parser、matching、term helper 和模块内测试分别进入 `types.rs`、`parser.rs`、`matching.rs`、`terms.rs`、`tests.rs`。
- `src/backend/core/account_rules/mod.rs` 已改为 facade；候选编译、scope 归一、上下文字段准备、匹配解释和模块内测试分别进入 `engine.rs`、`tests.rs`。
- D3 后端结构门禁直接项 `src/backend/core/account_rules/mod.rs` 与 `src/backend/core/category_rules/mod.rs` 已退出 failure 列表；剩余 Rust 结构失败均为非 D3 历史项。

### 6.4 frontend-shape

建议顺序：

1. 先拆 `RuleCenterPanel.vue` 的 template/style，再拆分类规则 state/composable、table toolbar、target grouping、dialog action。
2. 拆 `AccountRulePanel.vue` 的 template/style、分组 view model、form/dialog、service actions。
3. 拆 `ListPage.vue` 的导航/query sync、header actions、pair overview 壳。
4. `LearningCenterPanel.vue` 只做 D3 必需的壳层/结构拆分；LLM/OCR/learning provider 细节进入 D8。
5. 跑 pairingcenter focused tests、lint、coverage；用户可见布局改动补 browser smoke。

已完成第一轮 frontend-shape：

- `RuleCenterPanel.vue` 从 2081 行降到 1626 行；分类规则 overview/types、分类 picker、预设分类兜底、目标分组、payload 构造和 API 错误解析拆到 `categoryRulesModel.ts` 与 `apiResultHelpers.ts`。
- `AccountRulePanel.vue` 从 891 行降到 798 行；编辑/测试/删除弹窗拆到 `AccountRuleDialogs.vue`，父组件保留账户规则表格、分组装配和 service action。
- `LearningCenterPanel.vue` 从 1690 行降到 1573 行；D3 可管的 tab 归一、学习规则筛选、状态色/文案映射和置信度色拆到 `learningCenterPanelModel.ts`，LLM/OCR provider 保存和候选操作仍显式移交 D8。
- `src/web/scripts/frontend-structure-baseline.json` 已只更新 pairingcenter 相关条目；`npm run structure:check` 中 D3 pairingcenter 项已退出 failure/warning，剩余 19 个 failure 均为非 D3 历史项。
- 已通过 `npm run lint:ci`、`npm run test:coverage`（89 suites / 38974 tests / 99.13% total line coverage）与 pairingcenter focused tests：`apiResultHelpers.test.ts`、`categoryRulesModel.test.ts`、`learningCenterPanelModel.test.ts`、`ruleCenterNavigation.test.ts`、`ruleDialogLayout.test.ts`、`learningCenterOcrConfig.test.ts`、`ruleCenterFilters.test.ts`。
- 新增 TS helper 单独覆盖率：`apiResultHelpers.ts` 95.00%、`categoryRulesModel.ts` 91.42%、`learningCenterPanelModel.ts` 100.00%，三者合计 94.56% line coverage。

### 6.5 comment-pass/governance-docs/closeout

- 按注释策略补齐 D3 导出函数、业务关键函数和复杂私有 helper 中文说明。
- 更新 `docs/PROJECT_OVERVIEW.md` 中规则中心当前结构事实。
- 收紧 Rust/frontend structure baseline，让完成的 D3 项退出 failure/warning。
- PR/CI/merge/delete/writeback 证据完整后，cursor 推进到 D4。

当前 comment-pass 切片结果：

- 后端 `account_rules`、`category_rules`、rule HTTP handler 和 shared audit/rule helper 已补齐公开入口、业务关键函数和复杂私有 helper 的中文说明。
- 前端规则中心导航、筛选、表达式展示、分类规则、账户规则、学习中心、LLM/OCR 配置相关复杂 helper 已补齐中文说明；简单 getter、颜色/label 映射和事件转发未强制补注释。
- 本切片为 comment-only：未改变 API、DB、金额单位、规则表达式语法、UI 视觉或运行逻辑。
- 已通过 `cargo fmt --all -- --check`、`cargo test -p bill-analyser-core --test rule_center_contracts`、`cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 35`、`npm run lint:ci`、`npm run test:coverage`（89 suites / 38974 tests / 99.13% total line coverage）、`node scripts/check-backend-doc-map.mjs` 与 Gitea Actions run 14566。

当前 governance-docs 切片结果：

- `src/web/scripts/frontend-structure-baseline.json` 已对 D3 四个前端大文件记录注释后的当前行数：`RuleCenterPanel.vue` 1659 行、`LearningCenterPanel.vue` 1618 行、`AccountRulePanel.vue` 822 行、`ListPage.vue` 733 行。
- baseline 调整只覆盖 comment-pass 必需中文函数说明带来的 D3 行数变化；非 D3 历史结构失败没有被本切片吞并。
- `node scripts/check-rust-backend-structure.mjs` 仍失败 10 个非 D3 历史项，D3 后端项为 0。
- `node scripts/check-frontend-structure.mjs` 仍失败 19 个非 D3 历史项，D3 pairingcenter 项为 0。

当前 closeout 证据：

- D3 `ledger`：PR #210，commit `4ee496fee603a9292c0e3f904a3ea0d4eaee2647`，CI run 14554 通过，squash merge `fc27146bf3095b6a471b1f677fa6e00141f9aad6`，来源分支已删除。
- D3 `behavior-lock`：PR #211，commit `4d9067ce0a3c5bb250ec34e94abb2d2a2ff55e9e`，CI run 14557 通过，squash merge `98e2685cc2184473b7bcde4c82ba88b4a02253ae`，来源分支已删除。
- D3 `backend-shape`：PR #212，commit `a79d9351b7ec9a716338850cd36d4dcc32506e9d`，CI run 14560 通过，squash merge `71bd5828ed46d3f4bb7be8c5dedb3cd0530358ef`，来源分支已删除。
- D3 `frontend-shape`：PR #213，commit `1e1f3f75bea5a4e37418012a9fe99c551e5068e9`，CI run 14563 通过，squash merge `d37ebdae980440397755891af33244660b3c8b85`，来源分支已删除。
- D3 `comment-pass`：PR #214，commit `8abb6c119583cd666d534ab83316f2fab397619f`，CI run 14566 通过，squash merge `cc13de18a60b205df92ec9e3645bf7b74dc40a0d`，来源分支已删除。
- D3 `governance-docs`：PR #215，commit `fcb3f9f15c09b2f039a721c86d57bed39f07b92a`，CI run 14569 通过，squash merge `fb149f4be0dd167462e5d16678384be7d8aba64e`，来源分支已删除。
- D3 结束时 `node scripts/check-rust-backend-structure.mjs` 仍失败 10 个非 D3 历史项，`node scripts/check-frontend-structure.mjs` 仍失败 19 个非 D3 历史项；规则中心后端和前端直接项均为 0。
- D3 收口后 cursor 应推进到 D4 `budgets-forecast` 的 `ledger` 切片。

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
