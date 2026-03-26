```````instructions
# Bill Analyser - Copilot Instructions

## 项目定位

Bill Analyser 是一个账单导入、去重、自动分类、预算与统计分析系统。

- 后端运行入口：`src/bill_analyser/api/app.py`
- 前端工程目录：`src/web`
- 当前运行态主链：REST (`/api/...`)

## 必须遵守的架构约束

1. Flask 路由层保持同步入口，通过独立事件循环桥接 async 服务。
2. 数据库访问保持 `async def` + `aiosqlite`，不要把核心数据库逻辑改回同步。
3. 新实现优先走 REST，不要重新引入 `/api/v1/*` 作为运行时主链。
4. 金额单位必须显式处理：后端核心存元，很多前端/API 交互用分。
5. 时间、金额、账户、分类的前后端转换优先放在适配器/转换层，不要在路由里重复散落。

## 目录与职责

- `src/bill_analyser/api/`: Flask 应用、路由、鉴权、中间件
- `src/bill_analyser/core/`: 导入、去重、分类、统计、数据库
- `src/bill_analyser/parsers/`: 各账单解析器与工厂
- `src/web/src/`: Vue 3 + TypeScript 前端
- `tests/`: pytest 回归与接口测试

## 开发原则

- 修改导入链路时，优先检查 `bill_service.py`、`smart_dedup.py`、`category_engine.py` 的调用顺序是否仍然一致。
- 修改 API 时，先确认前端 `src/web/src/lib/services.ts` 和相关 store 的契约是否受影响。
- 尽量使用中性适配器模块，不要新增对 legacy `v1_*` 适配器文件的直接依赖。
- 保持变更聚焦，不顺手改无关历史问题。

## 运行与验证

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
cd src\web
npm run lint
```

## 操作禁令

- 不要使用 `taskkill /f /im python.exe`。
- 不要提交本地数据库、日志、上传文件或密钥。
- 不要在没有充分理由的情况下改动构建产物、历史快照目录或第三方参考代码。

## 提交前最低检查

1. 受影响的 pytest 用例通过。
2. Python 改动至少通过对应模块的 pylint。
3. 前端改动至少通过 `npm run lint` 或最小构建验证。
4. 如修改接口或金额字段，人工复核一次元/分转换。
```````
