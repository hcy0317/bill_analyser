# 浏览器功能自动化

本说明定义 `src/web` Playwright E2E 的本地运行合同。自动化只启动前端 Vite dev server，不隐式启动 Rust 后端、PostgreSQL 或 Weaviate；这些运行时必须由执行者用本地测试配置提前启动。

## 运行时前置

- Rust 后端必须通过 `bill_http_server` 提供 `REST /api/...`。
- `GET /api/health` 必须返回 `status = ok`。
- 前端 Vite 默认监听 `http://127.0.0.1:8081`，并把 `/api` 代理到 `http://127.0.0.1:5000`。
- 后端必须允许注册：`BILL_ANALYSER_AUTH_ENABLE_USER_REGISTRATION=true`。
- 自动化默认要求关闭邮箱强校验：`BILL_ANALYSER_AUTH_REQUIRE_EMAIL_VERIFICATION=false`。
- Weaviate collection prefix 应设置为专用值，例如 `BILL_ANALYSER_WEAVIATE_COLLECTION_PREFIX=BillAnalyserE2E`。
- PostgreSQL 应使用专用本地测试库；`/api/health` 的 `details.postgres_url_redacted` 必须指向本地 PostgreSQL，且数据库名默认需要包含 `e2e` 或 `test`。如果临时复用本地开发库，必须显式设置 `E2E_ALLOW_SHARED_LOCAL_DATABASE=true`，并确认账号只使用 E2E 专用测试数据。

## 环境变量

| 变量 | 默认值 | 说明 |
| --- | --- | --- |
| `E2E_BASE_URL` | `http://127.0.0.1:8081` | Playwright 浏览器访问的前端 URL。 |
| `E2E_API_BASE_URL` | `${E2E_BASE_URL}/api` | REST helper 访问的 API URL，默认走 Vite proxy。 |
| `E2E_ACCOUNT_USERNAME` | `bill-analyser-e2e` | 可复用专用测试账号用户名。 |
| `E2E_ACCOUNT_EMAIL` | `bill-analyser-e2e@example.test` | 可复用专用测试账号邮箱。 |
| `E2E_ACCOUNT_PASSWORD` | `BillAnalyserE2E!2026` | 专用测试账号密码，也用于数据清理的 `password` payload。 |
| `E2E_ACCOUNT_NICKNAME` | `Bill Analyser E2E` | 测试账号昵称。 |
| `E2E_RUN_ID` | 当前 UTC 时间生成 | 本轮创建数据的唯一标记。 |
| `E2E_EXPECTED_WEAVIATE_PREFIX` | `BillAnalyserE2E` | health details 中必须出现的 Weaviate collection prefix。 |
| `E2E_ALLOW_SHARED_LOCAL_DATABASE` | `false` | 允许显式复用本地非 `e2e/test` 命名数据库。 |
| `E2E_HEALTH_TIMEOUT_MS` | `10000` | health preflight 超时时间。 |

## 清理策略

- 开始清理失败时立即中止本轮自动化，避免在脏数据上继续断言。
- 结束清理是 best-effort；失败时保留 Playwright 报告、trace 和运行标记，便于人工追踪残留。
- 自动化只允许对本地 `localhost` 或 `127.0.0.1` API 执行清理。
- 清理 `/api/data/clear/all` 时优先使用账号当前密码 payload；如果未来改为 step-up token，必须在 helper 中显式分支，不能通过页面文案或临时 DOM 状态推断。

## 文件结构

| 路径 | 说明 |
| --- | --- |
| `src/web/playwright.config.ts` | Playwright 配置，包含 setup、desktop-chromium、mobile-chromium 三个 project，并注册 suite 级 `globalTeardown`。 |
| `src/web/e2e/setup/auth.setup.ts` | 健康检查、账号注册/复用、开始清理、登录态写入。 |
| `src/web/e2e/setup/global-teardown.ts` | suite 结束时复用 health 安全门禁并执行 best-effort 清理。 |
| `src/web/e2e/helpers/env.ts` | 环境变量、localhost/PostgreSQL/Weaviate prefix 守卫和 health preflight。 |
| `src/web/e2e/helpers/session.ts` | 有副作用 specs 的 health guard、登录、开始/结束清理封装。 |
| `src/web/e2e/helpers/apiClient.ts` | E2E 生命周期专用 REST client。 |
| `src/web/e2e/helpers/testAccount.ts` | 专用账号登录、注册和 Playwright storage state 写入。 |
| `src/web/e2e/helpers/dataLifecycle.ts` | `/api/data/clear/all` 清理封装。 |
| `src/web/e2e/helpers/routes.ts` | 桌面端 `desktop.html#/...` 与移动端 `mobile.html#!/...` 路由集合。 |
| `src/web/e2e/helpers/domainCrud.ts` | 主数据 CRUD fixture：账户、分类、标签、预算、模板、周期模板、分类规则、账户规则。 |
| `src/web/e2e/helpers/importFlow.ts` | 支付宝样本三阶段导入、确认、账单列表和统计金额查询。 |
| `src/web/e2e/tests/*.spec.ts` | 路由冒烟、主数据 CRUD/linkage、导入预览确认统计联动 specs。 |

## 覆盖范围

| Spec | Project | 覆盖 |
| --- | --- | --- |
| `desktop.route-smoke.spec.ts` | `desktop-chromium` | 桌面首页、账单、统计、账户、分类、标签、模板、周期、预算、规则、汇率、用户设置、应用设置、关于。 |
| `mobile.route-smoke.spec.ts` | `mobile-chromium` | 移动首页、账单、统计、账户、分类、标签、模板、周期、预算、账户规则、汇率、设置、数据管理、关于。 |
| `domain-crud.desktop.spec.ts` | `desktop-chromium` | 账户、分类、标签、预算、模板、周期模板、分类规则、账户规则的确定性创建/更新/删除和桌面页面可见性。 |
| `domain-crud.mobile.spec.ts` | `mobile-chromium` | 同一组主数据在移动页面中的可见性和删除清理。 |
| `import-preview-statistics.spec.ts` | `desktop-chromium` | 支付宝样本 `alipay_statement_sample.csv` 的 v2 parse/dedup/preview/confirm、账单列表和 2026-01 统计金额联动。 |

## 预期命令

```powershell
Set-Location src\web
npm run e2e:install
npm run e2e -- --list
npm run e2e -- --project=desktop-chromium
npm run e2e -- --project=mobile-chromium
npm run e2e
```

Playwright 报告输出到 `src/web/playwright-report/`，trace、截图和视频输出到 `src/web/test-results/`。这些目录不应提交。

## 失败处理

- `--list` 不需要健康后端；它只验证 Playwright 配置和 spec discovery。
- 完整 `npm run e2e` 会先运行 `auth.setup.ts`。如果 health、注册、登录或开始清理失败，后续 specs 不应继续执行。
- 如果 health 中 `weaviate_collection_prefix` 不是 `E2E_EXPECTED_WEAVIATE_PREFIX`，说明当前后端不是专用 E2E 向量命名空间，应先修正环境再运行。
- 如果 health 中 `postgres_url_redacted` 指向非本地 PostgreSQL，或数据库名不含 `e2e/test` 且没有设置 `E2E_ALLOW_SHARED_LOCAL_DATABASE=true`，helper 会在 `data/clear/all` 前失败。
- 如果结束清理失败，报告会保留本轮 `E2E_RUN_ID`、trace 和截图；人工清理时只处理专用测试账号的数据。
