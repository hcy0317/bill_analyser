# Bill Analyser 项目总览（当前代码基线）

## 1. 项目定位
Bill Analyser 是一个“多来源账单导入 + 智能去重 + 自动分类 + 多维统计分析”的全栈系统，核心目标是统一管理微信、支付宝与多家银行账单，并提供可编辑、可审计、可扩展的数据处理链路。

---

## 2. 总体架构

### 2.1 分层结构
- **API 层（Flask）**：`src/api/`
  - 同步路由处理 HTTP 请求
  - 通过事件循环桥接调用异步服务
- **业务层（Core）**：`src/core/`
  - 账单导入编排、去重、分类、统计、汇率等核心逻辑
- **数据层（Database）**：`src/core/db.py`
  - 基于 `aiosqlite` 的异步数据库访问
- **前端层（Vue3 + TS）**：`src/web/src/`
  - 视图、状态管理（Pinia stores）、服务层（axios）

### 2.2 关键架构模式
- **异步桥接模式**：Flask 路由内创建独立事件循环调用 async 逻辑
- **REST 主链模式**：当前运行态主链统一收口到 REST（`/api/...`）
- **适配器/转换模式**：前后端字段、时间、金额单位统一转换

补充说明（2026-03-07）：
- 当前运行态已无 `/api/v1/*` 路由，也无 WSGI 级 URL rewrite 中间件。
- legacy 兼容主要残留在适配器实现文件与历史快照目录，不再体现在运行态路由表中。

---

## 3. 后端模块分布

### 3.1 API 路由模块（`src/api/routes/`）
- `auth.py`：登录、鉴权、用户资料
- `bills.py`：账单 CRUD、导入、预览、确认、批量操作
- `accounts.py`：账户管理、余额同步
- `categories.py`：分类管理、规则维护
- `tags.py`：标签管理与关联
- `budgets.py`：预算 CRUD、执行统计、导入导出
- `statistics.py`：统计总览、趋势、汇率
- `templates.py`：模板管理
- `backup.py`：备份相关接口

### 3.1.1 API 适配器出口（`src/api/adapters/`）
- 中性实现模块：`transaction_adapter.py`、`account_adapter.py`、`category_adapter.py`
- legacy 兼容壳：`v1_adapter.py`、`v1_account_adapter.py`、`v1_category_adapter.py`
- 当前真实实现已前移到中性模块；legacy 文件仅保留 `V1*Adapter` / `V1ResponseBuilder` 等兼容导出，避免上层继续耦合 `v1_*` 文件名
- 当前新增代码、路由与测试应优先直接引用中性模块；`v1_*` 文件只作为历史兼容别名保留
- 静态回归：`tests/new_ui/test_no_direct_legacy_adapter_usage.py` 会阻止上层代码重新直接引用 `V1*Adapter` 类名或直接导入 `v1_* adapter` 模块

### 3.2 核心业务模块（`src/core/`）
- `db.py`：数据库初始化、迁移、查询、写入、缓存
- `bill_service.py`：导入主流程编排（含 v2 三阶段导入）
- `smart_dedup.py`：智能去重引擎（转账配对、平台银行去重、相似去重、分账去重）
- `category_engine.py`：关键词规则解析与分类匹配（含类型过滤与预编译优化）
- `exchange_rate_providers.py`：多汇率提供者聚合
- `budget.py` / `sync.py` / `analyzer.py` / `report*.py`：预算、同步、分析、报表

### 3.3 解析器模块（`src/parsers/`）
- 已有解析器：`wechat.py`、`alipay.py`、`icbc.py`、`abc.py`、`ccb.py`、`cmbc.py`
- 工厂入口：`factory.py`（自动识别并分发）

---

## 4. 前端模块分布

### 4.1 视图（`src/web/src/views/`）
- `desktop/`：桌面主界面（账户、交易、统计、预算、分类、标签、模板、用户设置）
- `mobile/`：移动端界面（交易、账户、统计、设置等）
- `base/`：公共页面基础

### 4.2 状态管理（`src/web/src/stores/`）
- 业务 store：`transaction.ts`、`account.ts`、`statistics.ts`、`budget.ts`、`transactionCategory.ts`、`transactionTag.ts` 等
- 用户与安全：`user.ts`、`token.ts`、`twoFactorAuth.ts`

### 4.3 服务层（`src/web/src/lib/services.ts`）
- 统一 axios 请求、鉴权头注入、401 刷新 token、REST 主链调用封装

---

## 5. API 对接关系（核心）

### 5.1 Flask 蓝图注册（`src/api/app.py`）
- RESTful 蓝图：
  - `/api/bills`
  - `/api/accounts`
  - `/api/categories`
  - `/api/statistics`
  - `/api/tags`
  - `/api/templates`
  - `/api/budgets`
  - `/api/backup`

### 5.1.1 当前 REST 收口进展（2026-03-06）
- 账户域首批 legacy action 已收口到 REST：
  - `PUT /api/accounts/<id>` 支持 `hidden` 可见性更新
  - `PUT /api/accounts/display-orders` 支持批量排序
  - 子账户删除统一通过 `DELETE /api/accounts/<id>`
  - `POST /api/accounts/<id>/transactions/move` 支持账户间批量迁移交易
  - `POST /api/accounts/<id>/transactions/clear` 支持按账户清空交易
- 标签域首批 legacy action 已收口到 REST：
  - `PUT /api/tags/<id>` 同时承担内容更新与 `hidden` 可见性更新
  - `POST /api/tags/batch` 支持批量创建
  - `PUT /api/tags/display-orders` 支持批量排序
- 前端 `services.ts` 与标签 store 已移除对应直连 v1 排序路径，改为统一走 REST 入口
- 模板域已完成契约梳理，但被重新评估为非低风险域：
  - 该域已完成首轮收口：前端模板调用统一走 REST，后端补齐 `templateType` 过滤、双表 DTO 映射、隐藏/排序与 `user_id` 收口；
  - 模板 rewrite 映射、`templates.bp_v1` 注册与实现均已移除；
  - 模板域当前仅保留 REST 主链路与对应回归测试。
- 预算域主链已完成 REST 收口：
  - 前端 `services.ts` 已将列表/详情/创建/更新/删除/执行统计/预测/导入导出统一切换到 `/api/budgets/*`；
  - 服务层新增预算 REST ↔ 前端旧结构兼容映射，避免直接重写 `budget store` 与页面；
  - `db.py` 已为预算 `forecast/import/export` 补齐 `user_id` 隔离；
  - `app.py` 已移除 `budgets.bp_v1` 注册；
  - `src/api/routes/budgets.py` 内预算旧 `bp_v1` 实现已完成物理删除；
  - 已补 `tests/new_ui/test_budgets_rest_api.py` 覆盖 REST 主链与 legacy 404 回归。
- 账户域历史 rewrite 与旧 `/get` `/modify` `/hide` `/delete` `/move` 兼容路由已移除。
- 标签域 `bp_v1` 注册与全部 v1 兼容实现已移除。
- 账单/分类/账户路由层已完成一轮适配器收敛：
  - `bills.py` 已拆分基础上下文与 adapter 上下文，非转换型路由不再默认构造交易适配器；
  - `accounts.py` / `categories.py` / `bills.py` 已统一改从中性适配器模块导入。

### 5.2 导入链路（重点）
- **v2 三阶段导入**（推荐）
  1. `POST /api/bills/import/v2/parse`
  2. `POST /api/bills/import/v2/dedup`
  3. `POST /api/bills/import/v2/confirm`
  - 配套：session 查询、预览查询与更新、reclassify
- **legacy 导入兼容说明**
  - 前端导入弹窗已不再依赖旧 `v1/transactions/import/process.json` 轮询与 `v1/transactions/parse_dsv_file.json`
  - `v1/transactions/parse_import.json` rewrite 已移除，当前应返回 404

### 5.3 统计与汇率
- 统计主链当前统一由 `GET /api/statistics/*` 提供
- 当前前端统计主链已切到：
  - `GET /api/statistics/category-statistics`
  - `GET /api/statistics/category-statistics/trends`
  - `GET /api/statistics/asset-trends`
- 旧 `transaction-statistics*` 兼容子路径已删除，当前通过 legacy 404 回归测试防止恢复
- 汇率主链已切到 `GET /api/statistics/exchange-rates`
- 用户自定义汇率写接口已收口到统计域 REST：`PUT /api/statistics/exchange-rates/custom`、`DELETE /api/statistics/exchange-rates/custom/<currency>`；旧 `v1/exchange_rates/user_custom/update.json` 与 `v1/exchange_rates/user_custom/delete.json` 已停止使用，并由 legacy 404 回归保护
- `GET /api/statistics/exchange-rates` 在用户存在自定义汇率时优先返回 `dataSource=user_custom` 的持久化结果，否则继续返回实时/回退汇率
- 用户资料页“数据统计”已切到认证域 REST 主链 `GET /api/data/statistics`，旧 `v1/data/statistics.json` 已删除并由 legacy 404 回归保护
- 用户数据管理已继续收口到认证域 REST：`GET /api/data/export.csv`、`GET /api/data/export.tsv`、`POST /api/data/clear/transactions`、`POST /api/data/clear/all`；旧 `v1/data/export.csv`、`v1/data/export.tsv`、`v1/data/clear/transactions.json`、`v1/data/clear/all.json` 已停止使用，并由 legacy 404 回归保护
- 系统版本检查已切到 `GET /api/system/version`；旧 `v1/systems/version.json` 已停止使用，并由 legacy 404 回归保护
- 导入解析辅助已切到 `POST /api/bills/parse_import`；前端 `services.ts` 不再使用旧 `v1/transactions/parse_import.json`
- 账单写入/导入辅助旧 rewrite 已移除：`v1/transactions/add.json`、`v1/transactions/modify.json`、`v1/transactions/delete.json`、`v1/transactions/import.json`、`v1/transactions/reconciliation_statements.json` 当前应返回 404
- 分类写入旧 rewrite 已移除：`v1/transaction/categories/list.json`、`v1/transaction/categories/add.json`、`v1/transaction/categories/add_batch.json` 当前应返回 404
- 导入提交前端已统一走 `POST /api/bills/import/v2/confirm`；旧 `v1/transactions/import/process.json` 与 `v1/transactions/parse_dsv_file.json` 已停止使用，并由 legacy 404 回归保护
- AI 小票识图前端已切到 `POST /api/ml/receipt-recognition`；当前 Python 后端提供 disabled-safe `501 Not Implemented` 占位语义，旧 `v1/llm/transactions/recognize_receipt_image.json` 已停止使用，并由 legacy 404 回归保护
- 交易列表前端主链已切到 `GET /api/bills/`；按月列表主链已切到 `GET /api/bills/by-month`
- 旧 `v1/transactions/list.json` 与 `v1/transactions/list/by_month.json` 已移除，并由 legacy 404 回归保护
- 交易图片前端主链已切到 `POST /api/bills/pictures` 与 `POST /api/bills/pictures/unused`
- 旧 `v1/transaction/pictures/upload.json` 与 `v1/transaction/pictures/remove_unused.json` 已停止使用，并由 legacy 404 回归保护
- 当前运行态前端源码（`src/web/src`）已不再直接引用 `v1/` 或 `/api/v1/` 路径；legacy 兼容仅保留在后端兼容层与历史 `ezbookkeeping/` 快照中
- `src/api/app.py` 中历史 `URLRewriteMiddleware` 已移除；当前运行态不再通过 WSGI rewrite 兼容任何 `/api/v1/*` 路径
- 认证入口主链已统一为 `POST /api/auth/login`、`POST /api/auth/register`、`POST /api/auth/logout`；旧 `authorize.json`、`register.json`、`logout.json` 已停止使用，并由 legacy 404 回归保护
- 认证辅助入口已继续收口到 REST：`POST /api/auth/email/verify`、`POST /api/auth/email/resend-verification`、`POST /api/auth/password/forgot`、`POST /api/auth/password/reset`；旧 `verify_email/*.json` 与 `forget_password/*.json` 已停止使用，并由 legacy 404 回归保护
- 用户资料主链已扩展到头像 REST：`GET|PUT /api/profile`、`POST|DELETE /api/profile/avatar`；旧 `v1/users/avatar/update.json` 与 `v1/users/avatar/remove.json` 已停止使用，并由 legacy 404 回归保护
- 用户资料主链已扩展到验证邮件重发 REST：`POST /api/profile/email/resend-verification`；旧 `v1/users/verify_email/resend.json` 已停止使用，并由 legacy 404 回归保护
- 用户资料主链已扩展到第三方登录 REST：`GET /api/profile/external-auths`、`POST /api/profile/external-auths/unlink`；旧 `v1/users/external_auth/*.json` 已停止使用，并由 legacy 404 回归保护
- 用户资料主链已扩展到应用云同步设置 REST：`GET|PUT|DELETE /api/profile/cloud-settings`；旧 `v1/users/settings/cloud/*.json` 已停止使用，并由 legacy 404 回归保护
- 2FA 主链已扩展到完整 REST 写接口：`GET /api/2fa/status`、`POST /api/2fa/enable/request`、`POST /api/2fa/enable/confirm`、`POST /api/2fa/disable`、`POST /api/2fa/recovery/regenerate`；旧 `v1/users/2fa/*.json` 写接口已停止使用，并由 legacy 404 回归保护
- 2FA 登录验证链路已补齐 REST：`POST /api/2fa/verify`、`POST /api/2fa/recovery/verify`；`/api/auth/login` 在用户启用 2FA 时返回 `need2FA` 与待验证 token，旧 `/api/2fa/authorize.json`、`/api/2fa/recovery.json` 已停止使用，并由 legacy 404 回归保护
- OAuth2 callback authorize 已收口到 `POST /api/auth/oauth2/authorize`；旧 `/api/oauth2/authorize.json` 已停止使用，并由 legacy 404 回归保护。当前后端仅提供 disabled-safe / not-implemented 语义，待后续真实 OAuth2 provider exchange 主链补齐
- token 会话主链已切到认证域 REST：`GET|DELETE /api/tokens`、`POST /api/tokens/api`、`POST /api/tokens/mcp`、`POST /api/tokens/refresh`、`DELETE /api/tokens/<id>`；旧 `v1/tokens/generate*.json`、`v1/tokens/revoke*.json` 与 `v1/tokens/refresh.json` 已收口到新主链，并由 legacy 回归保护

---

## 6. 数据库与数据流

### 6.1 主要业务表（`db.py` 初始化）
- 交易域：`bills`
- 分类域：`categories`
- 账户域：`accounts`、`account_types`、`account_transfers`
- 标签域：`tags`、`bill_tags`
- 模板域：`bill_templates`、`recurring_bills`
- 预算域：`budgets`、`budget_history`
- 用户与安全：`users`、`sessions`、`auth_logs`、`audit_logs`
- 导入三阶段：`import_sessions`、`bills_parser_template`、`bills_preview`

### 6.1.1 模板域当前漂移清单
- 前端期望字段：`templateType/categoryId/sourceAccountId/destinationAccountId/sourceAmount/destinationAmount/hideAmount/tagIds/displayOrder/hidden/scheduled*`。
- 后端当前字段：`category/account/tag/description/is_favorite/use_count/last_used_at` 等旧模板语义。
- 结果：已通过 DTO → 数据库映射完成首轮收口，但模板双表语义仍比账户/标签复杂，兼容层清理应继续分阶段推进。

### 6.2 导入处理顺序（核心）
1. 解析器识别并标准化账单
2. 数据验证
3. 智能去重
4. 分类匹配（规则+类型过滤）
5. 账户匹配（源/目标账户）
6. 预览写入或正式入库

---

## 7. 日志与运维

### 7.1 日志系统
- 统一日志入口：`src/utils/logger.py`
- 支持异步写入、滚动清理、等级分离
- 常用日志目录：`logs/`
- 当前仍保留重复实现候选：`src/utils/advanced_logger.py`（待后续收口确认无引用后清理）

### 7.2 启停脚本
- 一键启动：`一键启动.bat` / `一键启动.ps1`
- 分别启动：`start_backend.ps1`、`start_frontend.ps1`
- 停止服务：`停止服务器.ps1`

---

## 8. 测试结构与质量门禁

### 8.1 测试分布（`tests/`）
- 核心测试：`test_import.py`、`test_smart_dedup.py`、`test_category_engine_v2.py`、`test_db.py`
- API 测试：`test_v1_routes.py`、`test_statistics_*`、`new_ui/` 下接口测试
- 回归脚本/诊断脚本：`check_*`、`diagnose_*`、`debug_*`

### 8.2 推荐验证命令
- 单元/集成：
  - `C:/Users/hcy/OneDrive/Github/bill_analyser/.venv/Scripts/python.exe -m pytest tests/ -v`
- 代码质量：
  - `C:/Users/hcy/OneDrive/Github/bill_analyser/.venv/Scripts/python.exe -m pylint src/core/*.py src/api/routes/*.py`

---

## 9. 当前已知现状（本次会话）

1. 已修复统计汇率模块的导入路径不一致问题：
   - `src/api/routes/statistics.py` 中 `_fetch_exchange_rates_from_providers` 现与同文件统一使用 `src.core.exchange_rate_providers`。
2. 针对性回归显示：
  - `tests/new_ui/test_accounts_tags_rest_api.py`
  - `tests/new_ui/test_categories_api.py`
  - `tests/new_ui/test_bills_api.py`
  - `tests/new_ui/test_transaction_list_rest_api.py`
  当前组合回归结果为：`24 passed, 6 skipped`。
   - 现有 `statistics` 相关测试存在既有 401/404 夹具与鉴权依赖问题（非本次导入路径修复引入）。
3. 当前代码质量：
   - `statistics.py` Pylint 评分维持在高分区间（本次修复未引入新增严重告警）。

---

## 10. 后续建议（可执行）

1. 为 `statistics` 测试补齐统一鉴权 fixture（避免 401/404 误报掩盖业务回归）。
2. 建立“导入链路”和“统计链路”两套最小可用回归集，作为每次改动必跑基线。
3. 按模块分批重构测试（优先 `bills/category/statistics`），避免一次性重写导致不可控风险。
