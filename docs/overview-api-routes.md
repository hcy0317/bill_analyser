# API 路由与 REST 收口

当前运行态 API 主链为 Rust Axum `REST /api/...`。所有业务路由都在 `src/backend/http/` 的 route modules 中注册，未知 `/api/...` 请求由 Rust 返回结构化 404。

## 主要 Route Modules

- `auth_routes/`：login/register/token/profile/cloud/external-auth/system/user-data/2FA/step-up/OAuth2 disabled-safe 合同。
- `bill_routes/`：账单 CRUD、批量写入、图片、导出、recurring、reconciliation、分类 quick actions。
- `import_routes/`：parser-first 上传、JSON parse、未匹配文件列映射、session、preview、dedup、confirm、learning、LLM/OCR 入口。
- `taxonomy_routes/`：账户、账户识别规则、标签、分类、分类规则、模板、设置包、settings encryption status。
- `budget_routes.rs` facade + `budget_routes/`：预算 CRUD/export/execution/forecast/snapshot/import。
- `statistics_routes/`：分类统计、资产趋势、饼图、top merchants、amounts、Analyzer、洞察、汇率。
- `matching_routes.rs`：formal matching candidates、feedback、manual pairs、candidate actions、reconcile data、recurring/calendar/networth。
- `backup_routes/`：backup list/create/verify/download/delete/cleanup、jobs、sync。

## Contract Notes

- 认证路由按 Bearer access token 和 session 状态解析当前用户。
- 账户、标签、分类、模板、预算、账单和导入 preview 写入必须保持 user-scope。
- 金额相关路由必须显式处理元/分边界。
- 前端路由使用 `src/web/src/contracts/rustRouteOwnership.generated.ts` 做 contract guard。
- 新增或调整 route 时，同步检查 `docs/backend-map.md` 的路径索引和开发路径是否仍准确。
