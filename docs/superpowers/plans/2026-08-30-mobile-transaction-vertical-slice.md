# 移动端交易核心纵向切片计划

计划 ID：`mobile-transaction-vertical-slice-20260830`

## 编排权威

- 本文件只描述需求、依赖顺序、写集和验证门禁，不持有 workflow 状态。
- 后续每个切片必须重新通过 Cyaness/Cyanflow `prepare start -> start` 取得 frontier；`execution_discipline=tdd` 时只用 `harness mode simple-tdd` 加载 TDD。
- 不使用 `Local-plan-driven-slicing`、`skill-router.js`、OMX、Raft 或 mailbox 作为状态权威。
- 只有 Cyaness frontier 显式选择 `native-subagents` 时，才允许走其受封印的 `prepare-dispatch -> mark-started -> seal-receipt` bridge；否则由 root 单 agent 执行。

## 目标

在不改变 Rust REST、数据库 schema、整数分金额合同和桌面交易语义的前提下，完成移动端“进入新增交易 → 理解必填项 → 提交 → 获得明确结果”的可测试闭环，并为后续移动页面迁移建立一套克制、可复用的 Framework7 视觉基础。

## 明确非目标

- 不在本计划中重写全部移动页面、切换前端框架或引入新的 UI/动画依赖。
- 不修改 `/api/bills`、PostgreSQL、认证、导入或生产部署配置。
- 不使用生产数据库执行写入型 E2E，不自动推送、创建 PR 或合并。
- 不改变 `#c67e48` 品牌主色、金额收入/支出颜色语义和桌面端组件视觉。

## 全局不变量

- 金额始终以整数分进入 store/service；元只存在于用户输入和展示边界。
- 共享校验通过公开 composable 输出观察，不以模板源码字符串代替业务断言。
- 移动端只保留一个主保存动作；禁用或拦截时必须给出可见原因和下一步。
- 浅色、暗色、安全区、长中文文案和 360/390/430px 视口均属于验收面。
- 计划状态仅在 Cyaness 已验证事实后由 root 回写，不从旧 checkpoint 或 worker 摘要反向铸造完成状态。

## 依赖顺序

`slice-01 -> slice-02 -> slice-03`

## 切片 1：交易必填校验行为锁 ✅ (`44d463881`, `6140d6470`)

- id: `slice-01`
- status: `completed`
- depends_on: `[]`
- covered_req_ids: `REQ-MTX-001, REQ-MTX-002`
- read_set: `src/web/src/models/transaction.ts`, `src/web/src/core/transaction.ts`
- write_set / owned_paths:
  - `src/web/src/views/base/transactions/TransactionEditPageBase.ts`
  - `tests/web/transaction-edit-base-coverage/TransactionEditPageBase.behavior.test.ts`
- integration_contract: 共享 composable 继续返回 `inputEmptyProblemMessage` 与 `inputIsEmpty`；桌面和移动调用方无需更换 import 或 store API。
- validation_gates:
  - 目标 Jest 文件先红后绿。
  - 支出、收入、转账、投资、模板正反例均覆盖。
  - 金额字段只按现有 cents 合同判断，不新增元/分转换。

- [x] 把旧测试中“投资缺失字段仍合法”的断言改为四类交易完整行为规格，并证明 RED。
- [x] 用最小共享逻辑补齐投资分类、来源账户、投资账户和非零目标金额必填校验。
- [x] 保持模板名称、支出、收入、转账既有错误优先级不变。
- [x] 运行目标 Jest 并核对改动行覆盖率：15/15，目标文件行/语句/函数 100%，分支 98.87%。

## 切片 2：移动新增入口与 Transaction Composer ✅ (`83ea76bed`)

- id: `slice-02`
- status: `completed`
- depends_on: `[slice-01]`
- covered_req_ids: `REQ-MTX-003, REQ-MTX-004, REQ-MTX-005`
- read_set: `src/web/src/styles/mobile/global.scss`, `src/web/src/MobileApp.vue`, `src/web/src/views/mobile/HomePage.vue`
- write_set / owned_paths:
  - `src/web/src/views/mobile/transactions/ListPage.vue`
  - `src/web/src/views/mobile/transactions/list-page/ListPage.template.html`
  - `src/web/src/views/mobile/transactions/EditPage.vue`
  - `src/web/src/views/mobile/transactions/edit-page/EditPage.template.html`
  - `src/web/src/views/mobile/transactions/edit-page/EditPage.css`
  - `tests/web/mobile-transaction-list-coverage/ListPage.behavior.test.ts`
  - `tests/web/mobile-edit-coverage/EditPage.behavior.test.ts`
  - `tests/web/views/mobile/transactionEditPageInvestment.test.ts`
- integration_contract: 路由仍为 `/transaction/add`；保存仍调用 `transactionsStore.saveTransaction`；聚合账户筛选仍不能作为正式账单账户，但用户必须得到明确解释。
- validation_gates:
  - 列表新增入口在聚合账户筛选下显示原因，不再静默失效。
  - 编辑页移除重复顶栏保存动作，只保留填充式底部主 CTA。
  - 页面内展示当前必填问题；无可用账户时提供新增账户入口。
  - CTA、图标动作和页面拥有稳定 test id / accessible name，触控目标不小于 44px。
  - 视觉只使用 Framework7/theme variables 和现有品牌色。

- [x] 先新增/调整移动组件行为锁，覆盖聚合账户提示、唯一 CTA、inline validation 和账户恢复入口，并证明 RED。
- [x] 修改列表新增动作：拦截时提示 `Select Account`、打开账户筛选并保持原 query 不导航。
- [x] 把交易编辑页收敛为单一底部 fill CTA、可见 validation banner 和明确账户空状态。
- [x] 为金额区、类型选择、内容 surface、CTA 和 pressed/focus 状态落地局部现代样式。
- [x] 运行受影响 Jest、模板合同、lint、目标覆盖率与 build：49/49；Edit 99.77%，List 100%；build 通过。

## 切片 3：安全浏览器闭环与覆盖率验收 ✅ (`ae0644819`, `a747dfc5c`, `ad6fd1a62`, `68a1ea140`, `4f68f13e5`)

- id: `slice-03`
- status: `completed`
- depends_on: `[slice-02]`
- covered_req_ids: `REQ-MTX-006, REQ-MTX-007`
- read_set: `src/web/e2e/helpers/session.ts`, `src/web/e2e/helpers/env.ts`, `src/web/playwright.config.ts`
- write_set / owned_paths:
  - `src/web/e2e/tests/transaction-create.mobile.spec.ts`
  - `src/web/src/locales/*.json`
  - `tests/web/locales/i18nKeyContract.test.ts`
  - `docs/PROJECT_OVERVIEW.md`
- integration_contract: E2E 必须复用 dedicated account、storage state 与 cleanup；数据库名不含 `e2e|test` 时失败关闭，不得放宽安全门禁。
- validation_gates:
  - 新 E2E 覆盖“进入新增 → 填写支出 → 保存 → 列表可见”。
  - `npm run lint` 与 `npm run test:coverage` 通过，总覆盖率及被改业务代码覆盖率 > 90%。
  - 只有专用 E2E PostgreSQL/Weaviate 环境可运行写入型 Playwright；否则记录明确 open gate。
  - 360/390/430px 与浅/暗主题完成浏览器视觉审查；不可用环境不得冒充通过。
  - `docs/PROJECT_OVERVIEW.md` 只写当前稳定行为。

- [x] 添加移动新增交易 E2E 规格，并确认其复用 dedicated account、失败关闭 health 与 finally cleanup。
- [x] 在 task-owned PostgreSQL/Weaviate 与独立后端运行目标 Playwright：2/2 通过，创建 12,345 分支出、返回移动列表并由 strict cleanup 清理。
- [x] 完整前端 coverage：314/314 suites、41,545/41,545 tests；总行 94.37%、分支 91.20%，Edit 99.77%、List 100%、新增路由 helper 100%、图片面板行/语句/函数 100%、分支 92.85%。
- [x] 浏览器检查 360/390/430px、浅暗主题、44px 触控目标、单行类型切换、水平溢出、长中文业务文案与安全区底部 CTA，并保留 Playwright 截图证据。
- [x] 更新 15 locale 合同、E2E typecheck/list 与当前态架构文档；计划仅回写事实，不拥有 Cyaness 状态。

## 交付与停止条件

- 每个实现切片独立通过 Cyanflow frontier、TDD、验证、review 和必要的本地 commit；下一切片重新请求 Cyanflow，不复用旧编排状态。
- 用户已在实现完成后追加授权提交、PR、CI、合并与分支清理；网络写入仍必须逐项经过 Cyaness `harness-git` 与 ExternalWrite lease。
- 任一切片验证失败时由 Cyaness 重新裁决；不得由本文件或旧 Skill 自动重试、跳过或派发。
- 全部切片完成后继续完成精确 head CI、双审、squash merge 与来源分支清理；任一未关闭门禁必须如实保留。
