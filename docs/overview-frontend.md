# Bill Analyser 前端模块分布

## 视图

- `desktop/`：桌面主界面，覆盖账户、交易、统计、预算、分类、标签、模板、用户设置、规则中心和导入工作台。
- `mobile/`：移动端界面，覆盖交易、账户、统计、预算、设置、账户识别规则和小票 OCR。
- `base/`：公共页面基础。

账户编辑页只展示当前账户字段；账户识别规则由 `/account/rules` 与桌面规则中心统一管理。分类识别规则使用规则表达式，不再展示关键词导入入口。

## 规则中心

`desktop/pairingcenter/ListPage.vue` 是桌面端规则中心：一级导航分为“配对总览”“规则配置”“长期学习”“LLM 识别”。“规则配置”下有“分类识别 / 账户识别 / 周期识别”，分别承接 category rules、account rules 与 recurring matching 入口。

`/pairing/list` 使用 `domain/tab` query 表达当前业务域与二级视图；前端导航和测试只依赖当前 query contract。

## 导入界面

导入 Check Data 信号列把 learning lifecycle 显式映射到颜色和操作：yellow learning 建议显示黄色信号、推荐摘要和接受/拒绝按钮；green/auto-applied learning 建议显示绿色信号，并只保留拒绝按钮用于反馈降级或抑制。缓存签名包含 `recommendation_key`、`lifecycle_status`、`signal_state` 和计数字段。

## 状态管理

- 业务 store：`transaction.ts`、`account.ts`、`statistics.ts`、`budget.ts`、`transactionCategory.ts`、`transactionTag.ts`。
- 用户与安全：`user.ts`、`token.ts`、`twoFactorAuth.ts`。

## 核心工具层

`src/web/src/core/` 承载轻量、偏纯函数的前端领域工具与适配器，例如 `api.ts`、`currency.ts`、`color.ts`、`calendar.ts`、`datetime.ts`、`import_transaction.ts`、`statistics.ts`、`template.ts`。这些模块用于金额/时间/颜色/日历/导入交易等前端内聚转换，避免页面和 store 重复散落格式化逻辑。

## 服务层

`src/web/src/lib/services.ts` 保持统一 axios 请求、鉴权头注入、401 refresh token、REST 主链调用 facade；预算 REST 查询参数、元/分适配与 response mapper 拆在 `src/web/src/lib/services/budget.ts`。

`src/web/scripts/generate-rust-route-fixture.mjs` 会从 Rust route governance 快照生成 `src/web/src/contracts/rustRouteOwnership.manifest.generated.json`，再渲染 `rustRouteOwnership.generated.ts`。前端契约测试扫描 `services.ts` 与 Vue/TS 中的 axios/fetch `/api/...` 调用，要求前端运行态请求只能落到当前 Rust-owned route。

## 分类图标

分类图标统一由 `src/web/src/lib/icon.ts` 的 resolver 归一化，支持数字 preset id、Line Awesome class 与当前图标映射；`ItemIconBase` 只消费 resolver 输出，未知/空图标统一回落到默认分类图标。
