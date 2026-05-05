# Bill Analyser

Bill Analyser 是一个面向个人与家庭场景的账单分析系统，支持多来源账单导入、智能去重、自动分类、预算管理和统计分析。

本项目基于 [ezbookkeeping](https://github.com/mayswind/ezbookkeeping) 的前端 UI 代码和设计，后端使用 Python/Flask 重写，数据库使用 SQLite + aiosqlite。

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
| Python 3.14+ | 运行时 |
| Flask | Web 框架，同步路由桥接 async 服务 |
| aiosqlite | 异步 SQLite 访问 |
| pandas | 数据处理与分析 |
| PyJWT / bcrypt | 认证与密码加密 |

### 前端

| 技术 | 用途 |
|------|------|
| Vue 3 + TypeScript | 前端框架 |
| Vite | 构建工具 |
| Vuetify | 桌面端 UI 组件库 |
| Framework7 | 移动端 UI 框架 |
| Pinia | 状态管理 |
| ECharts | 图表可视化 |

### 数据与运行

- SQLite（WAL 模式）
- 本地日志输出到 `logs/`
- 默认数据库位于 `data/`

## 项目结构

```text
bill_analyser/
├── src/
│   ├── bill_analyser/
│   │   ├── api/           # Flask 应用、路由、鉴权
│   │   ├── core/          # 导入、去重、分类、数据库、统计
│   │   ├── parsers/       # 各账单解析器
│   │   └── utils/         # 日志、工具函数、常量
│   └── web/               # Vue 3 + TypeScript 前端
├── tests/                 # pytest 测试
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

- Python 3.14+
- Node.js 18+（建议 20+）
- Windows PowerShell

### 安装后端依赖

```powershell
py -3.14 -m venv .venv
.\.venv\Scripts\python.exe -m pip install -e .
```

### 安装前端依赖

```powershell
cd src\web
npm install
cd ..\..
```

### 启动服务

推荐使用一键启动：

```powershell
.\一键启动.ps1
```

也可以分别启动：

```powershell
.\start_backend.ps1
.\start_frontend.ps1
```

### 访问地址

- 前端：`http://127.0.0.1:8081`
- 后端 API：`http://127.0.0.1:5000/api`
- 健康检查：`http://127.0.0.1:5000/api/health`

### 手动启动

如果不使用脚本：

```powershell
# 终端 1
$env:PYTHONPATH = (Resolve-Path .\src).Path
.\.venv\Scripts\python.exe -m bill_analyser.api.app

# 终端 2
cd src\web
npm run dev
```

### 停止服务

```powershell
.\停止服务器.ps1
```

> **警告**：不要使用 `taskkill /f /im python.exe`，这会杀掉机器上所有 Python 进程。

## 开发指南

### 后端测试

```powershell
.\.venv\Scripts\python.exe -m pytest tests/ -v
.\.venv\Scripts\python.exe -m pytest --cov=src/bill_analyser --cov-report=term-missing tests/ -v
```

### Python 静态检查

```powershell
.\.venv\Scripts\python.exe -m pylint src/bill_analyser/core/*.py src/bill_analyser/api/routes/*.py
```

### 前端检查

```powershell
cd src\web
npm run lint
```

### 前端测试

```powershell
cd src\web
npm run test:coverage
```

### 前端构建

```powershell
cd src\web
npm run build
```

## 导入与处理流程

当前账单导入主链为三阶段：

1. **解析**：识别账单来源并写入临时会话
2. **去重预览**：执行智能去重、分类匹配、账户匹配
3. **确认导入**：用户确认后写入正式账单表

核心模块：

- `src/bill_analyser/core/bills/`
- `src/bill_analyser/core/smart_dedup/`
- `src/bill_analyser/core/category_engine/`

## 重要开发约束

### REST 优先

当前运行态以 REST 为主，不应为新功能重新引入 `/api/v1/*` 作为主链。

### 异步桥接

Flask 路由层保持同步入口，但核心服务和数据库访问必须保持 async。

### 金额单位

- 后端核心通常以元存储
- 前端和部分 API 交互常用分
- 修改接口时必须显式确认元/分转换，不要靠隐式约定

## 前端构建产物说明

`src/web/dist` 是 Vite + PWA 构建输出，包含：

- 多入口 HTML
- hashed JS/CSS 资源
- `sw.js`
- `manifest.json`
- `workbox-*`

这些文件属于部署产物，不是手工维护源码。

## AI 与仓库自动化资产

以下目录属于仓库级 AI / agent 配置资产，应纳入版本控制：

- `.github/` — Copilot 指令、hooks、工作流
- `.claude/` — Claude Code 规则与配置
- `.agents/` — 跨工具共享 skills
- `mcp-configs/` — MCP 配置与辅助资产

这些目录保存了 agent、skills、rules、commands、prompts 和 MCP 配置，不应被当作本地缓存或垃圾文件处理。

## 常见问题

### 端口被占用

先运行 `.\停止服务器.ps1`，再重新启动。

### 数据库锁定

停止所有后端实例和测试进程后重试，避免多个进程同时写 SQLite。

### 导入结果不符合预期

优先检查：

- 对应解析器是否识别正确
- 分类规则是否匹配到正确类型
- 账户别名是否完整
- 金额元/分转换是否一致

## 参考文档

- 项目总览：`docs/PROJECT_OVERVIEW.md`
- Agent 入口：`AGENTS.md`
- Copilot 约束：`.github/copilot-instructions.md`

## 致谢

本项目的前端 UI 代码和设计源自 [ezbookkeeping](https://github.com/mayswind/ezbookkeeping)（原作者：[mayswind](https://github.com/mayswind)）。ezbookkeeping 是一个轻量级、自托管的个人记账应用，采用 MIT 许可证发布。

我们对 mayswind 及 ezbookkeeping 贡献者表示衷心感谢，感谢他们提供的优秀前端架构、UI 组件和设计理念。

**ezbookkeeping 项目信息**：
- 仓库地址：https://github.com/mayswind/ezbookkeeping
- 许可证：MIT
- 官方网站：https://ezbookkeeping.mayswind.net

## 许可证

MIT

本项目基于 MIT 许可证发布，详见 [LICENSE](LICENSE) 文件。

由于本项目使用了 ezbookkeeping 的前端代码，根据 MIT 许可证的要求，我们保留了原项目的版权声明和许可声明。
