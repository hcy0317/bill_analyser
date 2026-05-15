# Bill Analyser 前端模块分布

## 4.1 视图（`src/web/src/views/`）
- `desktop/`：桌面主界面（账户、交易、统计、预算、分类、标签、模板、用户设置）
- 前端结构现在有 `src/web/scripts/check-frontend-structure.mjs` 作为文件体量 ratchet gate，并用 `src/web/scripts/frontend-structure-baseline.json` 记录历史超大文件。新增或触达的 Vue/TS/样式文件不能超过对应阈值，历史超限文件允许继续拆小但不能反向变大；`npm run structure:check` 是本地与 CI 可复用的结构检查入口。
- 可复用页面骨架收敛在 `src/web/src/components/common/templates/`、`src/web/src/components/desktop/templates/`、`src/web/src/components/mobile/templates/`。这些模板只承载标题栏、内容区、桌面 panel、移动 page shell 等轻量结构，继续复用 Vuetify / Framework7 与现有全局样式，不引入第二套视觉系统。
- 账户、交易分类、交易标签、交易模板、定时交易、规则中心分类识别、LLM 配置与 OCR 配置页面各自提供设置 JSON 导入入口：页面标题栏只显示一个 `Import` 按钮，悬停 1.5 秒后在同一按钮菜单卡片中显示 `Export Settings JSON`；导入会读取本地 JSON、调用对应 section 的预览接口汇总新增/更新/跳过数量，确认后只导入当前页面对应的设置 section 并刷新本页数据。预算管理页也使用同款单按钮入口，导出预算 JSON 收在 hover 菜单中。数据管理页不再作为这些设置 JSON 的唯一入口。
- `desktop/pairingcenter/ListPage.vue` 当前承载桌面端 canonical 规则中心：一级导航分为"配对总览""规则配置""长期学习""LLM 识别"。"配对总览"下有"转账配对 / 投资配对"，读取并管理历史正式账单 pair；"规则配置"下有"分类识别 / 周期识别"，分别承接 category rules 与 recurring matching 入口，投资类识别规则也作为投资分类下的 category rule 表达式在"分类识别"中维护；"长期学习"下保留自动建议与学习规则；"LLM 识别"下有 `LLM Recognition`、`LLM Config`、`OCR Config` 三个二级入口，其中 OCR Config 单独维护小票 OCR provider/lang 配置。候选主链当前收口到 `rule_synthesis` 人工审核流：服务端先把 durable learning rules / suggestions、concept stats、active model metadata 与近期 `llm_memory_events(feedback)` 汇总成 `KnowledgeSummaryPack`，再由 LLM 归纳出兼容现有 category rule grammar 的候选表达式，前端只展示这类待审核候选，accept/reject 不依赖实时 provider。页面以预算管理式纵向导航 + 二级 tabs 展示当前域，右侧标题栏统一承载当前二级标题、刷新/新增/生成建议/筛选等动作，正文子组件不再重复渲染同名标题；分类识别表格的过滤选项在 `ruleCenterFilters.ts` 维护，规则表达式展示由 `RuleExpressionDisplay.vue` 和 `ruleExpressionDisplay.ts` 承载，主 panel 保持路由 facade。
- `/pairing/list` 是规则中心运行态主路由，并使用 `domain/tab` query 表达当前业务域与二级视图；UI 的一级导航由 `domain/tab` 派生：`domain=transfer|investment&tab=overview` 属于"配对总览"，`domain=transfer&tab=rules` 属于"规则配置"，`domain=learning` 属于长期学习，`domain=llm` 属于 LLM 识别。旧深链 `pairType` / `view` / `tab` query 会被归一化到新 domain/tab；旧投资识别设置深链会改写到 `domain=transfer&tab=rules` 的分类识别页；`/rules/center` 与 `/learning/center` 只重定向到同一规则中心页面，不再保有独立持久化业务中心语义。
- `desktop/budgets/ListPage.vue` 的预算管理页现把周期入口收口为标题栏 `月度 / 季度 / 年度` 切换 + 左侧 `当前周期 / 上一周期` 快捷切换，切换会真实改变预算列表/执行请求语义；预算 reload 会与预算数据并行加载分类元数据，一级/二级预算图标优先使用预算返回的分类图标并回退到分类 store，不再依赖其他页面预热。预算列表、预测面板、预测设置、历史图例、历史面板和历史周期列表拆在 `desktop/budgets/components/` 下，主页面保留路由 facade 与数据编排。预算列表中的进度条 drilldown 统一跳转到 `/transaction/list` 的 canonical query contract：分类走 `categoryIds`，时间范围走 `dateType=Custom + minTime/maxTime`，账户/标签上下文复用 `accountIds/tagIds`。同页的历史预算执行视图继续通过 `historyPolarChart.ts` facade 生成往期预算执行图，具体动画、几何、图例、金额轴和 option 构造拆在 `desktop/budgets/history-polar/`；分类隐藏/显示通过 legend selection 同步到柱状预算金额、圆环分组、标签和下方往期预算列表，剩余可见分类会重新填满极坐标布局。往期预算列表按月/季/年聚类，周期面板和一级分类下的二级分类分别可折叠，分类图标可直接联动图例筛选。
- `mobile/`：移动端界面（交易、账户、统计、预算、设置等）。移动端添加/编辑账单在交易表单内常态显示图片控件；添加账单上传图片后会调用小票 OCR 识别并回填金额、时间与描述，支付宝 / 微信支付截图会额外识别支付平台和更稳定的商户/备注文本。移动端预算管理按一级分类聚类展示，一级分类行显示分类图标、汇总进度和金额，二级分类预算缩进在对应一级分类下并支持折叠/展开。移动首页不提供异常洞察、周期配对或配对中心的独立入口；这些能力的桌面规则中心与后端 matching/learning 主链仍保留。
- `base/`：公共页面基础

## 4.2 状态管理（`src/web/src/stores/`）
- 业务 store：`transaction.ts`、`account.ts`、`statistics.ts`、`budget.ts`、`transactionCategory.ts`、`transactionTag.ts` 等
- 用户与安全：`user.ts`、`token.ts`、`twoFactorAuth.ts`

## 4.3 前端核心工具层（`src/web/src/core/`）
- `core/` 承载轻量、偏纯函数的前端领域工具与适配器，例如 `api.ts`、`currency.ts`、`color.ts`、`calendar.ts`、`datetime.ts`、`import_transaction.ts`、`statistics.ts`、`template.ts` 等；这些模块用于金额/时间/颜色/日历/导入交易等前端内聚转换，避免页面和 store 重复散落格式化逻辑

## 4.4 服务层（`src/web/src/lib/services.ts`）
- 统一 axios 请求、鉴权头注入、401 刷新 token、REST 主链调用封装
- `src/web/scripts/generate-rust-route-fixture.mjs` 会从 Rust `migration_governance.rs::OWNERSHIP_MATRIX` 生成 `src/web/src/contracts/rustRouteOwnership.generated.ts`；`tests/web/contracts/frontendRustRouteContract.test.ts` 扫描 `services.ts` 与 Vue/TS 中的 axios/fetch `/api/...` 调用，要求前端运行态请求只能落到 Rust `RustOwnedVerified` 或 `PythonDeleted` 路由，并阻止重新引用 `/api/v1/*`。

## 4.5 分类图标
- 分类图标统一由 `src/web/src/lib/icon.ts` 的 resolver 归一化，支持数字 preset id、Line Awesome class 与历史 `mdi-*` 分类图标值；`ItemIconBase` 只消费 resolver 输出，未知/空图标统一回落到默认分类图标。
- 规则中心和预算管理不再用 `mdiCloseCircle` 作为正常分类缺失图标兜底；分类筛选、规则行、预算分组与二级预算行都通过同一分类图标解析链路渲染。
