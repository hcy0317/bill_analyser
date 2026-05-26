# Bill Analyser

Bill Analyser 是一个面向个人与家庭场景的账单分析系统，支持多来源账单导入、智能去重、自动分类、预算管理和统计分析。

本项目当前后端运行态是 Rust Axum `bill_http_server`，Rust workspace 位于 `src/backend/*`。默认数据库仍为 SQLite WAL；PostgreSQL 迁移骨架已提供配置、schema 和本地 compose，但业务路由在切换前继续走 SQLite。前端使用 Vue 3 + TypeScript + Vite。

## 核心能力

- **多来源账单导入**：微信、支付宝、工商银行、农业银行、建设银行、民生银行等
- **智能去重**：转账配对、平台/银行重复、类似账单、分账单识别
- **自动分类**：支持关键词规则、类型过滤、批量重新分类
- **账户管理**：账户、标签、分类、模板、预算统一管理
- **统计分析**：分类统计、资产趋势、预算执行、汇率支持
- **多端界面**：桌面端（Vuetify）和移动端（Framework7）共用同一后端 API

## 技术栈

### 后端

| 技术 | 用途 |
|------|------|
| Rust stable | 后端运行时 |
| Axum | HTTP API |
| Tokio | 异步运行时 |
| Rusqlite / SQLite WAL | 数据访问 |
| SQLx / PostgreSQL | Postgres 迁移与后续权威库基座 |
| cargo-llvm-cov | 覆盖率门禁 |

### 前端

| 技术 | 用途 |
|------|------|
| Vue 3 + TypeScript | 前端框架 |
| Vite | 构建工具 |
| Vuetify | 桌面端 UI 组件库 |
| Framework7 | 移动端 UI 框架 |
| Pinia | 状态管理 |
| ECharts | 图表可视化 |

## 项目结构

```text
bill_analyser/
├── src/
│   ├── backend/           # Rust 后端 workspace
│   └── web/               # Vue 3 + TypeScript 前端
├── tests/
│   ├── backend/           # Rust 集成与契约测试
│   ├── fixtures/          # 导入样本与契约 fixtures
│   └── web/               # 前端契约与组件测试
├── docs/                  # 项目文档
├── config/                # 配置文件
├── data/                  # 本地数据库与数据文件
├── logs/                  # 运行日志
├── uploads/               # 上传临时文件
├── backup/                # 备份文件
└── mcp-configs/           # MCP 配置与辅助资产
```

## 快速开始

### 环境要求

- Rust stable
- Node.js 22+
- Windows PowerShell

### 安装依赖

```powershell
.\scripts\install.ps1
```

也可以手动安装前端依赖：

```powershell
cd src\web
npm install
cd ..\..
```

### 启动服务

```powershell
.\一键启动.ps1
```

一键启动脚本会按端口清理旧服务，构建并启动 Rust HTTP 后端，启动 Vite 前端，并分别等待后端健康检查和前端首页可访问。

常用参数：

```powershell
.\一键启动.ps1 -BackendOnly   # 只启动 Rust 后端
.\一键启动.ps1 -FrontendOnly  # 只启动前端
.\一键启动.ps1 -NoAutoStop    # 不自动停止旧服务
.\一键启动.ps1 -NoBrowser     # 启动完成后不自动打开浏览器
```

也可以分别启动：

```powershell
.\start_backend.ps1
.\start_frontend.ps1
```

访问地址：

- 前端：`http://127.0.0.1:8081`
- 后端 API：`http://127.0.0.1:5000/api`
- 健康检查：`http://127.0.0.1:5000/api/health`

启动脚本默认配置：

- `BILL_ANALYSER_HTTP_BIND=127.0.0.1:5000`
- `BILL_ANALYSER_SQLITE_DB_PATH=data\bills.db`
- `BILL_ANALYSER_DATABASE_BACKEND=sqlite`
- `BILL_ANALYSER_MIGRATION_MODE=disabled`
- `BILL_ANALYSER_RUST_HTTP_SERVER` 可指定已构建的 `bill_http_server` 可执行文件，未指定时脚本会自动构建 debug 版本

### 本地 PostgreSQL 骨架

SQLite 仍是默认运行路径。需要验证 Postgres 配置和 migration skeleton 时，可先启动本地数据库：

```powershell
docker compose -f docker-compose.postgres.yml up -d
$env:BILL_ANALYSER_DATABASE_BACKEND = "postgres"
$env:BILL_ANALYSER_POSTGRES_URL = "postgres://bill_analyser:bill_analyser_dev@127.0.0.1:5432/bill_analyser"
$env:BILL_ANALYSER_MIGRATION_MODE = "validate"
```

`/api/health` 会显示 `database_backend`、`postgres_configured`、`postgres_url_redacted`、`migration_mode`、`migration_status` 和 `weaviate_status`，其中 Postgres URL 只输出脱敏形式。当前切片不切换业务 repository。

SQLite 到 PostgreSQL 的迁移工具当前支持 dry-run、export 和 import-check：

```powershell
cargo run -p bill-analyser-db --bin bill_sqlite_to_postgres_migrate -- --mode dry-run --sqlite data\bills.db --output migration-report.json
cargo run -p bill-analyser-db --bin bill_sqlite_to_postgres_migrate -- --mode export --sqlite data\bills.db --output migration-bundle.json
cargo run -p bill-analyser-db --bin bill_sqlite_to_postgres_migrate -- --mode import-check --bundle migration-bundle.json --output migration-import-check.json
cargo run -p bill-analyser-db --bin bill_sqlite_to_postgres_migrate -- --mode import --bundle migration-bundle.json --postgres-url $env:BILL_ANALYSER_POSTGRES_URL --output migration-import-report.json
```

详见 `docs/postgres-migration.md`。

### 手动启动

```powershell
# 终端 1
$env:BILL_ANALYSER_SQLITE_DB_PATH = "data\bills.db"
$env:BILL_ANALYSER_HTTP_BIND = "127.0.0.1:5000"
cargo run -p bill-analyser-http --bin bill_http_server

# 终端 2
cd src\web
npm run dev
```

### 停止服务

```powershell
.\停止服务器.ps1
```

## 开发指南

### 后端检查

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 90
```

### 前端检查

```powershell
cd src\web
npm run lint
npm run test:coverage
npm run build
```

### 本地 CI

```powershell
.\scripts\run_ci_local.ps1
```

## 导入与处理流程

当前账单导入主链为三阶段：

1. **解析**：识别账单来源并写入临时会话
2. **去重预览**：执行智能去重、分类匹配、账户匹配
3. **确认导入**：用户确认后写入正式账单表

核心 Rust 模块：

- `src/backend/parsers/`
- `src/backend/db/`
- `src/backend/core/`
- `src/backend/http/`

## 重要开发约束

### REST 优先

Rust `bill_http_server` 是唯一 HTTP 运行时入口。当前运行态以 `REST /api/...` 为主，不应为新功能重新引入 `/api/v1/*`、Python/Flask sidecar 或旧式代理兜底作为主链。

### 金额单位

- 后端核心通常以元存储
- 前端和部分 API 交互常用分
- 修改接口时必须显式确认元/分转换，不要靠隐式约定

## 前端构建产物说明

`src/web/dist` 是 Vite + PWA 构建输出，属于部署产物，不是手工维护源码。

## AI 与仓库自动化资产

以下目录属于仓库级 AI / agent 配置资产，应纳入版本控制：

- `.github/` — Copilot 指令、hooks、工作流
- `.claude/` — Claude Code 规则与配置
- `.agents/` — 跨工具共享 skills
- `.codex/` — Codex 薄适配器
- `mcp-configs/` — MCP 配置与辅助资产

## 常见问题

### 端口被占用

先运行 `.\停止服务器.ps1`，再重新启动。`.\一键启动.ps1` 默认也会按后端和前端端口清理旧服务；如果手动设置了 `BILL_ANALYSER_HTTP_BIND`，脚本会从该地址解析后端端口。

### 数据库锁定

停止所有后端实例和测试进程后重试，避免多个进程同时写 SQLite。
