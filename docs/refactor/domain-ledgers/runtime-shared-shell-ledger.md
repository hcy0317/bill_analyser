# D10 runtime-shared-shell-final-sweep 结构债 ledger

本 ledger 固化 D10 shared/runtime shell 终扫域的当前结构边界。D10 依赖 D1-D9 全部完成后串行执行，目标是关闭剩余 shared facade、root store、theme/model/router/runtime shell 与结构治理尾债，最终证明 Rust/frontend 结构门禁、doc map、全量 smoke 和 PR closeout evidence 全部通过。

## 1. 覆盖范围

D10 覆盖：

- `src/web/src/lib/services.ts`：全局 axios/auth 拦截、API facade、导出/上传 timeout、URL builder、认证、用户数据、主数据、交易、导入预览、matching、预算、learning、recurring、rule center、LLM/OCR 和 insights endpoint 聚合。
- `src/web/src/stores/index.ts`：root Pinia store、认证/注册/2FA/OAuth/profile/user-data 动作、跨 store reset/invalid state 编排和全局通知。
- `src/web/src/core/theme.ts`：应用 theme enum、主题顺序、Vuetify/F7 颜色变量、theme variant 表、pairing 与 preference helper。
- `src/web/src/models/imported_transaction.ts`：导入预览行模型、matching payload 归一、信号状态 helper、confirm create request 投影和 response DTO。
- `src/web/src/router/desktop.ts` 与 `src/web/src/router/mobile.ts`：桌面 Vue Router 和移动 Framework7 router shell、登录/锁定 guard、主导航注册和 query prop 映射。
- `src/web/scripts/check-frontend-structure.mjs`、`src/web/scripts/frontend-structure-baseline.json`、`scripts/*structure*`：前后端结构 gate 与最终 baseline 收紧。
- `docs/PROJECT_OVERVIEW.md` 与各 domain ledger：最终记录 shared shell 当前结构事实和关闭证据。

D10 不覆盖：

- 已在 D1-D9 关闭的具体业务页面、后端领域模块和 parser/source 运行态；如后续需要触碰这些文件，必须作为 D10 shared lease 或 blocker fix 明确记录。
- UI 视觉重设计；D10 只能保持现有布局、按钮文案、颜色语义和交互合同，不做视觉改版。
- API/DB/金额/导入/认证/统计合同的行为变更；发现合同漂移必须另开 fix/prerequisite slice，不能混入结构拆分。

## 2. 当前结构快照

当前 `Set-Location src/web; npm run structure:check` 失败 4 项，均属于 D10 shared debt：

| 文件 | 当前行数 | gate 状态 | 当前职责密度 |
| --- | ---: | --- | --- |
| `src/web/src/lib/services.ts` | 2342 | 超过 baseline 2214 | axios 初始化/auth refresh、URL builder、REST endpoint facade 和多域 response 适配混在同一 default export |
| `src/web/src/stores/index.ts` | 833 | 超过 baseline 824 | root store 同时承担 auth flow、profile、email、password reset、user-data clear/export 和跨 store invalidation |
| `src/web/src/core/theme.ts` | 796 | 新文件 warning limit 600 | theme enum、基础 Vuetify 颜色、18 个 light/dark variant、mobile F7 config 和 preference helper 同文件 |
| `src/web/src/models/imported_transaction.ts` | 623 | 新文件 warning limit 600 | 导入预览 DTO、matching payload 默认值、信号 helper、有效性校验和 confirm create request 投影同文件 |

辅助结构状态：

- `src/web/src/router/desktop.ts` 当前 284 行，`src/web/src/router/mobile.ts` 当前 366 行，未触发结构 gate 失败，但仍是 D10 shared router shell。
- `src/web/scripts/frontend-structure-baseline.json` 当前 39 个 baseline entry；`src/lib/services.ts` 和 `src/stores/index.ts` 是 legacy oversized baseline，`src/core/theme.ts` 与 `src/models/imported_transaction.ts` 当前没有 baseline entry，按新文件 warning limit 失败。
- `node scripts/check-rust-backend-structure.mjs` 当前通过，扫描 507 个 Rust backend 文件并保留 3 个 baseline entry：`src/backend/db/auth_postgres.rs`、`src/backend/core/smart_dedup.rs`、`src/backend/core/import_pipeline.rs`。D10 final sweep 必须显式决定这些 baseline 是继续作为已治理 ratchet 保留，还是另开 backend shared cleanup；不能在最终验收中忽略。
- `node scripts/check-backend-doc-map.mjs` 当前通过。

## 3. 行为不变式

- `services.ts` 拆分时必须保留现有 import path 兼容：既有 `import services from '@/lib/services.ts'` 和 `ApiResponsePromise` 类型不能在同一切片内破坏。
- axios base URL、timeout、token header、refresh token、request blocking、cancel request 和 public no-auth endpoint 语义必须先行为锁定，再做 service module 拆分。
- root store 拆分时必须保留 `useRootStore` 导出名、登录/注册/2FA/OAuth/profile/user-data action 名、localStorage/sessionStorage key 和跨 store reset/invalid state 顺序。
- theme 拆分不得改变 `ThemeType` 值、`APPLICATION_THEME_ORDER` 顺序、paired theme、Vuetify theme name、Framework7 CSS variable、meta theme color 或现有显示文案 key。
- imported transaction 模型拆分不得改变金额 cents 字段、`TransactionType.Transfer`/Investment destination account 校验、matching signal helper、parser tag/dedup summary 或 `toCreateRequest()` payload。
- router 拆分不得改变桌面/移动 path、guard 行为、query prop 名、history 模式或移动 route options。
- 结构 baseline 只能收紧已完成的 D10 项；不得把未关闭的 shared debt 加入 baseline 以制造 gate 通过。

## 4. 后续切片计划

- `behavior-lock`：补足 shared shell 行为锁。优先锁定 services axios/auth refresh 与 endpoint facade、root store auth/profile/user-data flow、theme preference/paired theme helper、imported transaction matching/validation/toCreateRequest、desktop/mobile router guard 与 query props。
- `backend-shape`：复核 Rust 结构 baseline 3 项。若 final acceptance 要求全仓无 oversized backend core file，则把 `smart_dedup.rs`、`import_pipeline.rs` 和 `auth_postgres.rs` 作为 D10 backend shared cleanup 子切片；若保持 ratchet，必须在治理文档写明理由和后续 owner。
- `frontend-shape`：按 shared shell 拆分 `services.ts`、`stores/index.ts`、`theme.ts`、`imported_transaction.ts` 和必要 router helper。入口应退化为 facade/barrel，功能文件夹按 API 域、auth/root-store flow、theme tokens/variants/helper、import preview DTO/matching/helper 拆分。
- `comment-pass`：按用户确认标准补齐导出函数、业务关键函数和复杂私有 helper 的中文说明；简单 getter、映射和事件转发不强制。
- `governance-docs`：收紧 frontend/Rust structure baseline，更新 `docs/PROJECT_OVERVIEW.md` 的 shared shell 当前事实，并确认 `npm run structure:check` 不再失败。
- `closeout`：汇总 D10 PR 链、CI、source branch 删除、one-click startup/smoke、全域 ledger 完整性和最终 ultragoal quality gate。

## 5. 验收门槛

D10 完成前必须满足：

- `Set-Location src/web; npm run structure:check` 通过，不再出现 `services.ts`、`stores/index.ts`、`theme.ts`、`imported_transaction.ts` failure。
- `node scripts/check-rust-backend-structure.mjs` 通过，并对 3 个 Rust baseline entry 的最终处置有明确证据。
- `node scripts/check-backend-doc-map.mjs` 通过。
- 前端改动切片必须通过 `npm run lint`、`npm run test:coverage` 和必要的 focused tests；被改业务代码覆盖率按计划门槛复核。
- Rust 业务改动切片必须通过 focused cargo tests、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 35` 和 changed-line coverage gate。
- 最终 closeout 前执行一键启动或等价 runtime smoke，覆盖登录/导入预览/交易列表/统计或计划指定主流程。
- 全域 PR/CI/squash merge/delete branch/writeback evidence 完整，最终 ultragoal quality gate 通过。

## 6. Ledger 切片记录

D10 `ledger` 切片只建立 shared/runtime shell 终扫边界，不修改运行时代码、测试、路由、结构 baseline 或 UI。

本切片本地验证记录：

- `Set-Location src/web; npm run structure:check` 失败 4 项，均为 D10 shared debt：`src/lib/services.ts`、`src/stores/index.ts`、`src/core/theme.ts`、`src/models/imported_transaction.ts`。
- `node scripts/check-rust-backend-structure.mjs` 通过，当前 3 个 Rust baseline entry 已在本 ledger 作为 final-sweep 决策项记录。
- `node scripts/check-backend-doc-map.mjs` 通过。
- 通过文件行数与源码扫描确认 router shell 未触发当前结构 failure，但属于 D10 后续治理面。
