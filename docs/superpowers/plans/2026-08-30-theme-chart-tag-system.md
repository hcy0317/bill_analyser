# 全主题、图表图例与 Tag/Chip 修复计划

计划 ID：`theme-chart-tag-system-20260830`

## 目标

让 9 个主题家族、18 个浅/暗变体在桌面 Vuetify 与移动 Framework7 中共享同一组语义颜色；修复活跃图表的图例、坐标轴、tooltip、禁用态反色和长名称显示；统一交易 Tag/Chip 在列表、编辑、筛选与管理页面中的反色、截断、换行和完整名称可访问性。

## 非目标

- 不改统计、预算、账单或 Tag 的业务数据合同。
- 不重写 ECharts/Vuetify/Framework7，也不引入新的 UI 依赖。
- 不为当前应用未引用的旧 `components/charts/*` wrapper 扩大范围。
- 不改变收入、支出、成功、错误等既有业务颜色语义。

## 全局不变量

- `core/theme` 是应用主题色唯一 owner；页面和图表不得新增固定 Classic 橙色或 `#333/#eee/#ccc` 明暗分支。
- 每个图例和 Tag 即使视觉截断，也必须保留完整 `title`/`aria-label` 或等价 tooltip。
- 移动端暗色状态跟随 `framework7DarkMode` / 当前主题定义，不读取不存在的 `theme-dark` class。
- 图表、Tag 与页面 surface 使用语义 token；金额色与状态色保持独立。

## 依赖顺序

`slice-01-theme-tokens -> slice-02-chart-legends -> slice-03-tag-chip -> slice-04-visual-matrix`

## Slice 1：18 主题语义 token

- 扩展主题定义，派生 primary RGB、surface/on-surface、muted、border、chart、tooltip、tag 与 progress-track token。
- MobileApp 将同一语义合同投影为 Framework7 与 `--ebk-*` 变量。
- desktop/mobile global styles 删除主题相关固定色，datepicker 与 primary tint 跟随当前主题。
- 测试逐一遍历 `APPLICATION_THEME_ORDER`，验证 18 个主题的 token 完整性、配对关系和对比用途。

## Slice 2：活跃图表图例

- 建立共享 chart-theme/legend helper，统一 tooltip、legend、axis、grid、inactive、文本截断和完整名称策略。
- 接入移动 `EChartsPieChart`、`EChartsTrendsChart`，修正暗色 owner 与 id→显示名映射。
- 接入桌面 `PieChart`、`TrendsChart`、`AccountBalanceTrendsChart`、`RadarChart`、`AccountAndCategorySankeyChart`、首页月度图与预算历史图例/标签。
- ECharts legend 明确左右边界、scroll/page 色、formatter、文本宽度与 tooltip；图表容器不得横向溢出。

## Slice 3：Tag/Chip

- mobile/desktop global styles 提供统一 transaction-tag contract：主题背景/文字/边框、flex shrink、label ellipsis、容器 wrap。
- 移动交易列表、编辑、选择与 Tag 管理页面补完整名称和窄屏约束。
- 桌面交易表、筛选、编辑、导入和 Tag 管理页面补弹性列宽、完整名称与统一 chip 内容约束。
- 未知 Tag ID 不渲染空 chip；`None` 保持中性语义。

## Slice 4：验收矩阵

- 单元/行为测试：主题 token、图表 option、图例 formatter、Tag 完整名称和 overflow contract。
- 浏览器：18 个主题；移动 360/390/430px，桌面 600/1024/1440px；短名、长中文、长英文、多图例和多 Tag。
- 断言：无页面横向溢出；图例/Tag 文字可读；被截断内容有完整提示；浅暗主题对比正确。
- 最终门禁：`npm run structure:check`、`npm run lint:ci`、`npm run test:coverage`、`npm run build`，总覆盖率和被改业务代码均 >90%。
