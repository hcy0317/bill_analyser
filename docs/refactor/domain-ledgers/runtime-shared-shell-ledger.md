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

## 7. Behavior-lock 切片记录

D10 `behavior-lock` 切片只补共享壳行为锁，不移动生产结构、不改 UI 视觉、不调整 API/金额/数据库合同。

本切片新增和扩展的行为锁：

- `tests/web/lib/services.sharedShell.test.ts`：锁定 services 模块初始化、axios request/response interceptor、public no-auth 路径、refresh token unblock、cancel request，以及导入预览 page/confirm facade 的 session encoding、query params 和 payload。
- `tests/web/stores/rootStore.test.ts`：锁定 root store 的 lock/forceLogout reset 范围、profile token/profile/依赖视图 invalidation、账户级清理与全量 user-data 清理的跨 store invalidation 边界。
- `tests/web/router/runtimeShell.test.ts`：用 source contract 锁定桌面 guard redirect、交易/统计 query prop 名、移动 guard redirect options、关键移动路由和 fallback shell。当前 Jest node harness 不直接加载 `.vue` 页面，因此 router 行为锁采用源码合同断言。
- `tests/web/core/theme.test.ts`：补锁 invalid theme preference 到 light/mobile config 的 fallback，以及 paired theme helper 的默认行为。
- `tests/web/models/imported_transaction.test.ts`：补锁 Investment 预览行 `toCreateRequest()` 保留目标账户和目标金额的 payload 合同。

本切片本地验证记录：

- `Set-Location src\web; npm test -- --runTestsByPath ..\..\tests\web\lib\services.sharedShell.test.ts ..\..\tests\web\stores\rootStore.test.ts ..\..\tests\web\router\runtimeShell.test.ts ..\..\tests\web\core\theme.test.ts ..\..\tests\web\models\imported_transaction.test.ts` 通过：5 个 suite、42 个测试。
- `Set-Location src\web; npm run lint:ci` 通过；仅保留既有 `no-explicit-any` warning，无 error。
- `Set-Location src\web; npm run test:coverage` 通过：全量 Jest 99 个 suite、39028 个测试；coverage gate 6 个 suite、81 个测试，line coverage 99.13%。
- `Set-Location src\web; npm run structure:check` 仍按预期失败 4 项，均为 D10 后续 `frontend-shape` 要关闭的 shared debt：`src/lib/services.ts`、`src/stores/index.ts`、`src/core/theme.ts`、`src/models/imported_transaction.ts`。
- `node scripts/check-rust-backend-structure.mjs` 通过，扫描 507 个 Rust backend 文件并保留 3 个 baseline entry。
- `node scripts/check-backend-doc-map.mjs` 通过。
- `git diff --check` 通过，仅提示 Windows LF/CRLF 工作区 warning。

## 8. Backend-shape 切片记录

D10 `backend-shape` 切片关闭 Rust backend structure baseline 的最终尾债。该切片只做物理结构拆分和治理 baseline 收紧，不改变 REST route、PostgreSQL schema、金额单位、导入链路、认证/session 或去重匹配语义。

本切片结构调整：

- `src/backend/core/smart_dedup.rs` 退化为 DTO/public facade 并保留 serde 合同，去重 engine、platform-bank 候选、转账候选、相似重复、拆单候选、merge helper、matching helper 和 reconciliation helper 下沉到 `smart_dedup/**`。
- `src/backend/core/import_pipeline.rs` 保留导入预览 pipeline 合同 facade，preview query、filter index、matching payload、类型映射、route response DTO、response envelope 和 value helper 下沉到 `import_pipeline/**`。
- `src/backend/db/auth_postgres.rs` 保留 PostgreSQL auth facade 和 shared helper，模块内测试下沉到 `auth_postgres/tests.rs`；该文件原本只比 600 行 warning 线高 33 行，测试外置即可关闭 baseline，不移动未覆盖的数据库 I/O helper。
- `scripts/rust-backend-structure-baseline.json` 删除最后 3 个历史 oversized baseline entry；Rust backend structure gate 现在以 0 baseline entry 通过。

本切片本地验证记录：

- `node scripts/check-rust-backend-structure.mjs` 通过，扫描 523 个 Rust backend 文件，baseline entries 为 0。
- `cargo test -p bill-analyser-core --test import_pipeline_contracts` 通过：14 个测试。
- `cargo test -p bill-analyser-core --test smart_dedup_contracts` 通过：12 个测试。
- `cargo test -p bill-analyser-db auth_postgres` 通过：5 个 auth postgres 单元测试。
- `cargo fmt --all -- --check` 通过。
- `cargo clippy --workspace --all-targets -- -D warnings` 通过。
- `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 35` 通过，生成完整 Rust 工作区 coverage 报告。
- `node scripts/governance-normalizers.mjs changed-coverage --lcov workspace.lcov --diff .omx\ultragoal\evidence\d10-backend-shape-rust.diff --threshold 90` 通过：1830 个可执行 changed lines，1716 行覆盖，changed-line coverage 93.77%。
- `node scripts/check-backend-doc-map.mjs` 通过。
- `Set-Location src\web; npm run structure:check` 仍按预期失败 4 项，全部属于 D10 后续 `frontend-shape`：`src/lib/services.ts`、`src/stores/index.ts`、`src/core/theme.ts`、`src/models/imported_transaction.ts`。

## 9. Frontend-shape 切片记录

D10 `frontend-shape` 切片关闭剩余 shared runtime shell 前端结构门禁。该切片只做 facade + 功能文件夹拆分，不改变现有 UI 视觉、REST/API 调用、金额字段、认证/session、导入预览、theme preference 或 router 合同。

本切片结构调整：

- `src/web/src/lib/services.ts` 保留 axios/auth interceptor、通用 services facade 和 `ApiResponsePromise` 兼容导出，HTTP 类型/response envelope helper 下沉到 `lib/services/http.ts`，导入预览、导入学习、matching candidate 和导入配置 endpoint 下沉到 `lib/services/importPreview.ts`。
- `src/web/src/stores/index.ts` 保留 `useRootStore` facade，跨 store reset 编排下沉到 `stores/root/reset.ts`，OAuth URL helper 下沉到 `stores/root/oauthUrls.ts`。
- `src/web/src/core/theme.ts` 保留兼容导出，theme 类型/常量、基础 Vuetify/F7 色板、主题变体表和 preference/paired helper 分别下沉到 `core/theme/types.ts`、`base.ts`、`variants.ts` 与 `registry.ts`。
- `src/web/src/models/imported_transaction.ts` 保留 `ImportTransaction` 模型和响应接口，matching payload 归一化、dedup source id 与信号 helper 下沉到 `models/imported_transaction/matching.ts`。

本切片本地验证记录：

- `Set-Location src\web; npm run structure:check` 通过，扫描 596 个文件，保留 39 个 baseline entry；原 D10 失败项 `src/lib/services.ts`、`src/stores/index.ts`、`src/core/theme.ts`、`src/models/imported_transaction.ts` 均退出 failure。
- `Set-Location src\web; npm test -- --runTestsByPath ..\..\tests\web\lib\services.sharedShell.test.ts ..\..\tests\web\stores\rootStore.test.ts ..\..\tests\web\router\runtimeShell.test.ts ..\..\tests\web\core\theme.test.ts ..\..\tests\web\models\imported_transaction.test.ts` 通过：5 个 suite、42 个测试。
- `Set-Location src\web; npm test -- --runTestsByPath ..\..\tests\web\styles\desktopTableTheme.test.ts` 通过；该 source-contract 测试已改为读取拆分后的 `src/core/theme/base.ts`。
- `Set-Location src\web; npm run lint:ci` 通过；仅保留既有 `no-explicit-any` warning，无 error。
- `Set-Location src\web; npm run test:coverage` 通过：全量 Jest 99 个 suite、39028 个测试；coverage gate 6 个 suite、81 个测试，line coverage 99.13%，branch coverage 91.48%。
