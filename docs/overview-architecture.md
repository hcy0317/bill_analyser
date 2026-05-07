# Bill Analyser 总体架构

## 2.1 分层结构
- **API 层（Flask）**：`src/bill_analyser/api/`
  - 同步路由处理 HTTP 请求
  - 通过事件循环桥接调用异步服务
- **业务层（Core）**：`src/bill_analyser/core/`
  - 账单导入编排、去重、分类、统计、汇率等核心逻辑
- **数据层（Database）**：`src/bill_analyser/core/db.py` + `src/bill_analyser/core/database/`
  - `db.py` 只暴露公共 `Database` façade
  - 真实持久化能力按 runtime / schema / 业务域 mixin 拆分；实现模块统一收口到 `core/database/**`，业务域使用无 `db_` 前缀的语义 package
  - `database/runtime.py`、`database/shared.py`、`database/time.py`、`database/encryption.py` 与 `database/schema/` 维护连接生命周期、共享协议、时间、加密和 schema 编排内核
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
- **Rust 内部库边界**：Rust 迁移当前保持 Flask REST 外壳作为运行时入口；`bill-analyser-core` 提供 runtime identity、health、error、API response envelope、共享 primitives、auth/security foundation、分类规则表达式 AST 编译、AI/OCR/LLM 合同层以及 backup/ops 安全合同层，`bill-analyser-db` 提供 SQLite runtime foundation；`POST /api/tokens/refresh` 将 decoded refresh claims 形状校验委托给预构建的 `bill_auth_bridge`，新分类规则表达式编译委托给 `bill_category_rule_bridge`，其余业务 API、备份文件操作、云 SDK 上传与数据库写入仍由 Python 拥有。

补充说明（2026-03-07）：
- 当前运行态已无 `/api/v1/*` 路由，也无 WSGI 级 URL rewrite 中间件。
- legacy 兼容已退出运行时代码树，当前仅残留在历史快照目录与针对遗留路径的回归测试约束中，不再体现在运行态路由表或适配器实现中。
