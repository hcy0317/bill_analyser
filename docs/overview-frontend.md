# Bill Analyser 前端模块分布

## 4.1 视图（`src/web/src/views/`）
- `desktop/`：桌面主界面（账户、交易、统计、预算、分类、标签、模板、用户设置）
- `desktop/pairingcenter/ListPage.vue` 当前承载桌面端 canonical 规则中心：一级导航分为"配对总览""规则配置""长期学习""LLM 识别"。"配对总览"下有"转账配对 / 投资配对"，读取并管理历史正式账单 pair；"规则配置"下有"分类识别 / 周期识别"，分别承接 category rules 与 recurring matching 入口，投资类识别规则也作为投资分类下的 category rule 表达式在"分类识别"中维护；"长期学习"下保留自动建议与学习规则；"LLM 识别"下保留 LLM 规则候选与 LLM 配置，其中候选主链当前收口到 `rule_synthesis` 人工审核流：服务端先把 durable learning rules / suggestions、concept stats、active model metadata 与近期 `llm_memory_events(feedback)` 汇总成 `KnowledgeSummaryPack`，再由 LLM 归纳出兼容现有 category rule grammar 的候选表达式，前端只展示这类待审核候选，accept/reject 不依赖实时 provider。页面以预算管理式纵向导航 + 二级 tabs 展示当前域，右侧标题栏统一承载当前二级标题、刷新/新增/生成建议/筛选等动作，正文子组件不再重复渲染同名标题；分类识别表格的批量选择、表头筛选、每页数量与分页统计都收敛在表头/分页区，分类表头筛选按收入 / 支出 / 转账 / 投资聚类并在视口内滚动，避免选项过多时撑高页面。
- `/pairing/list` 是规则中心运行态主路由，并使用 `domain/tab` query 表达当前业务域与二级视图；UI 的一级导航由 `domain/tab` 派生：`domain=transfer|investment&tab=overview` 属于"配对总览"，`domain=transfer&tab=rules` 属于"规则配置"，`domain=learning` 属于长期学习，`domain=llm` 属于 LLM 识别。旧深链 `pairType` / `view` / `tab` query 会被归一化到新 domain/tab；旧投资识别设置深链会改写到 `domain=transfer&tab=rules` 的分类识别页；`/rules/center` 与 `/learning/center` 只重定向到同一规则中心页面，不再保有独立持久化业务中心语义。
- `desktop/budgets/ListPage.vue` 的历史预算执行视图使用 `historyPolarChart.ts` 生成往期预算执行图：分类隐藏/显示通过 legend selection 同步到柱状预算金额、圆环分组与标签，剩余可见分类会重新填满极坐标布局；一级/二级分类标签保留上一帧角度状态，跨 0° 时按最短圆弧平滑过渡。图表构建会在空历史、无可见分类或非法几何时返回安全空态/兜底布局，避免往期预算执行图不可见。预算列表中的进度条 drilldown 现统一跳转到 `/transaction/list` 的 canonical query contract：分类走 `categoryIds`，时间范围走 `dateType=Custom + minTime/maxTime`，账户/标签上下文复用 `accountIds/tagIds`。
- `mobile/`：移动端界面（交易、账户、统计、设置等）
- `base/`：公共页面基础

## 4.2 状态管理（`src/web/src/stores/`）
- 业务 store：`transaction.ts`、`account.ts`、`statistics.ts`、`budget.ts`、`transactionCategory.ts`、`transactionTag.ts` 等
- 用户与安全：`user.ts`、`token.ts`、`twoFactorAuth.ts`

## 4.3 服务层（`src/web/src/lib/services.ts`）
- 统一 axios 请求、鉴权头注入、401 刷新 token、REST 主链调用封装
