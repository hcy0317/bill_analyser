# Bill Analyser Workspace Instructions

## Project Focus

Bill Analyser 是一个账单导入、去重、自动分类、预算与统计分析系统。

- 后端入口：src/bill_analyser/api/app.py
- 前端工程：src/web
- 当前运行态主链：REST /api/...

## Architecture

- Flask 路由层保持同步入口，通过独立事件循环桥接 async 服务；不要把路由层改成直接 async 运行模型。
- 数据库访问保持 async def + aiosqlite；不要把核心数据库逻辑改回同步。
- 新实现优先走 REST /api/...，不要重新引入 /api/v1/* 作为运行时主链。
- 时间、金额、账户、分类的前后端转换优先放在适配器/转换层，不要在路由或 store 中重复散落。

## Critical Conventions

- 金额单位必须显式处理：后端核心存元，很多前端/API 交互用分；改动金额字段时必须人工复核一次元/分转换。
- 修改导入链路时，优先检查 bill_service.py、smart_dedup.py、category_engine.py 的调用顺序是否仍然一致。
- 修改 API 契约时，先确认 src/web/src/lib/services.ts 和相关 store 是否需要同步更新。
- 新增适配器优先使用中性模块命名，不要新增对 legacy v1_* 适配器文件的直接依赖。
- 保持变更聚焦，不顺手改无关历史问题。

## Reviewer Subagent Contract

- 调用 `code-reviewer`、`python-reviewer`、`security-reviewer` 时，必须显式传入 `Review Context`，不要假设 subagent 一定能自己拿到正确 diff。
- `Review Context` 至少应包含：`base_ref`、`head_ref`、`changed_files`、`diff_text`（或代表性 patch hunk）。
- 如果 diff 过大，至少传入关键 hunk + refs；如果既没有 diff 也没有可用 refs，就应明确报告 review 被 scope 阻塞，而不是做泛化全仓库审查。

## Workspace Boundaries

- 不要使用 taskkill /f /im python.exe。
- 不要提交本地数据库、日志、上传文件或密钥。
- 不要在没有充分理由的情况下改动构建产物、历史快照目录或第三方参考代码。

## Build And Test

```powershell
# 启动
.\一键启动.ps1
.\start_backend.ps1
.\start_frontend.ps1

# 停止
.\停止服务器.ps1

# 测试
.\.venv\Scripts\python.exe -m pytest tests/ -v

# Python 静态检查
.\.venv\Scripts\python.exe -m pylint src/bill_analyser/core/*.py src/bill_analyser/api/routes/*.py

# 前端检查
Set-Location src\web
npm run lint
```

提交前最低检查：受影响的 pytest 用例通过；Python 改动至少通过对应模块的 pylint；前端改动至少通过 npm run lint 或最小构建验证；接口或金额字段变更时人工复核一次元/分转换。

## Reference Docs

- 详细架构与模块地图见 docs/PROJECT_OVERVIEW.md
- 多工具代理入口与定制资产说明见 AGENTS.md
- 通用语言与测试规则见 .github/instructions/ecc/
