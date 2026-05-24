# Bill Analyser 项目总览

Bill Analyser 是一个多来源账单导入、智能去重、自动分类、预算与统计分析全栈系统。当前运行态已经完成 Rust-only 切换：`bill_http_server` 是唯一 HTTP 服务入口，默认监听 `BILL_ANALYSER_HTTP_BIND=127.0.0.1:5000`，业务 API 通过 Rust Axum `REST /api/...` 进入后端；未知 `/api/...` 返回 Rust 侧结构化 404。

## 运行态入口

- 后端主入口：`src/backend/http/bin/bill_http_server.rs`
- 后端导航图：`docs/backend-map.md`
- 静态阅读入口：`docs/backend-map.html`
- 前端工程：`src/web`
- 当前 API 主链：`REST /api/...`

## 后端分层

- `src/backend/http`：Axum 路由、认证上下文、上传处理、response envelope、structured error 和各业务 route facade。
- `src/backend/core`：金额、时间、分类、统计、预算、导入、matching、LLM/OCR、认证与迁移治理等共享业务合同。
- `src/backend/db`：SQLite WAL/FK 连接、schema 初始化、事务 helper、user-scope repository、导入 staging、auth、budget、matching、taxonomy/settings 等仓储模块。
- `src/backend/parsers`：微信、支付宝、工商银行、农业银行、建设银行、民生银行等 dedicated parser，以及 `RawBill` 到 `StandardBill` 的统一标准化。

详细的代码阅读路径、请求生命周期、导入管线、repository 数据流和验证矩阵见 `docs/backend-map.md`。

## 关键业务链路

### 导入

导入链路由 Rust runtime 完成 parser-first 上传、JSON parse、session/template staging、dedup、账户别名匹配、分类规则、transfer/recurring/learning decision、preview page、preview update/reclassify、confirm 和 cleanup。混合来源上传按文件保留 parser id / tags；Check Data 首屏读取分页 preview page；确认、取消和失败后新建 session 都会清理当前用户的 import staging。

### Matching

历史正式账单的转账候选与导入既有账单转账配对保持同一核心约束：同日、5 分钟内、金额绝对值在 1 分容差内且方向相反、来源账户不同，并排除已配对或已 suppressed 的账单。账单详情的历史匹配还会对同用户、未配对且日期、类型、金额、账户、对方、说明、支付方式和分类完全一致的正式账单返回只读 `duplicate` 候选，用于提示人工创建的重复账单。

### 金额

数据库核心金额通常按元存储，部分前端/API 交互使用分。账单、账户、预算、统计或导入金额字段发生变化时，必须显式复核元/分转换。

### 预算与统计

预算按月/季/年层级同步，删除主预算或最后一个子预算时会清理派生父周期预算。统计链路由 Rust 读取账户、分类、汇率和资产趋势数据，并保持前端图表契约。

### 认证与备份

Rust auth runtime 校验 Bearer access token，2FA、step-up、user-data clear、backup file 操作按当前用户和动作类型执行额外校验，并写入认证或业务审计。备份 runtime 负责本地 zip、加密备份公开名、下载、删除、恢复、cleanup、jobs 与 cloud sync。

### LLM/OCR

LLM 临时配置保存在 Rust 进程内 user-scoped map，saved config 落库并脱敏；provider 生成保留 allowlist/SSRF 防护、响应体上限、候选截断和 rate limit。OCR recognition 默认 disabled，配置后通过 provider 返回结构化交易草稿。

### 前端主题

前端主题由 `src/web/src/core/theme.ts` 的统一注册表管理。桌面 Vuetify、移动端 Framework7、应用设置页与系统主题自动解析都从同一注册表读取主题名称、明暗属性、色板、移动端 CSS 变量和 meta theme-color；当前保留 `auto` / `light` / `dark` 兼容值，并为 Halloween、forest、wireframe、black、dracula、business、night、dim 等经典预设提供浅色/深色配对。桌面表格条纹、表头和主题切换按钮使用主题 token 与配对关系，避免深色主题下出现硬编码浅色条纹。金额收入/支出颜色仍由独立 amount-color 体系管理，不并入主题状态色。

## 文档入口

- [后端导航图](backend-map.md) — Rust 后端分层、请求生命周期、导入管线、repository 数据流和验证矩阵
- [总体架构](overview-architecture.md) — Rust 后端、前端、SQLite 与 REST 主链模式
- [后端模块](overview-backend.md) — HTTP/Core/DB/Parser 责任边界
- [API 路由](overview-api-routes.md) — `/api/...` route modules 和合同约束
- [导入链路](overview-import.md) — v2 三阶段导入、preview、learning、LLM/OCR
- [数据库与数据流](overview-database.md) — repository、事务、user-scope 和 staging 生命周期
- [Matching 域](overview-matching.md) — transfer/investment/learning/recurring 配对候选
- [统计与汇率](overview-statistics.md) — 统计主链、汇率 REST 和用户数据管理
- [认证与安全](overview-auth-security.md) — 认证、2FA、token、backup、step-up 和审计
- [测试结构](overview-testing.md) — Rust、前端和治理测试基线

## 验证基线

后端业务改动以 Rust gate 为准：

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 90
```

前端改动至少运行：

```powershell
Set-Location src\web
npm run lint
npm run test:coverage
npm run build
```
