# Bill Analyser 总体架构

## 2.1 分层结构
- **API 层（Flask）**：`src/bill_analyser/api/`
  - 同步路由处理 HTTP 请求
  - 通过事件循环桥接调用异步服务
- **业务层（Core）**：`src/bill_analyser/core/`
  - 账单导入编排、去重、分类、统计、汇率等核心逻辑
- **数据层（Database）**：`src/bill_analyser/core/db.py` + `src/bill_analyser/core/db_*.py`
  - `db.py` 只暴露公共 `Database` façade
  - 真实持久化能力按 runtime / schema / 业务域 mixin 拆分到多个 `db_*.py` 模块
  - 底层仍保持基于 `aiosqlite` 的异步数据库访问
- **前端层（Vue3 + TS）**：`src/web/src/`
  - 视图、状态管理（Pinia stores）、服务层（axios）

补充说明（2026-03-28）：
- 当前后端唯一源码根为 `src/bill_analyser/`。
- 仓库级 `src/api`、`src/core`、`src/parsers`、`src/utils`、`src/data`、`src/uploads` 等顶层阴影目录不再承载运行时代码或数据。

## 2.2 关键架构模式
- **异步桥接模式**：Flask 路由内创建独立事件循环调用 async 逻辑
- **REST 主链模式**：当前运行态主链统一收口到 REST（`/api/...`）
- **适配器/转换模式**：前后端字段、时间、金额单位统一转换

补充说明（2026-03-07）：
- 当前运行态已无 `/api/v1/*` 路由，也无 WSGI 级 URL rewrite 中间件。
- legacy 兼容已退出运行时代码树，当前仅残留在历史快照目录与针对遗留路径的回归测试约束中，不再体现在运行态路由表或适配器实现中。
