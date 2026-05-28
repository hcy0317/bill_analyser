# 日志与运维

## 启动

```powershell
.\start_backend.ps1
.\start_frontend.ps1
```

或使用：

```powershell
.\一键启动.ps1
```

后端脚本会构建并启动 `bill_http_server`，默认监听 `127.0.0.1:5000`。前端脚本在 `src/web` 启动 Vite dev server，默认监听 `127.0.0.1:8081`。

## 停止

```powershell
.\停止服务器.ps1
```

停止脚本按端口定位当前应用进程，避免按全局进程名粗暴清理。

## 本地 CI

```powershell
.\scripts\run_ci_local.ps1
```

该脚本运行 Rust fmt/clippy/test/coverage 和前端 lint/coverage/build。

## PostgreSQL 与 Weaviate

默认本地启动使用 PostgreSQL authority + 必需 Weaviate。`start_backend.ps1` 会设置/要求 `BILL_ANALYSER_DATABASE_BACKEND=postgres`、`BILL_ANALYSER_REQUIRE_POSTGRES_AFTER_CUTOVER=true`、`BILL_ANALYSER_POSTGRES_URL` 和 Weaviate endpoint；一键启动器默认管理缺失的 Postgres 与 Weaviate compose 服务。SQLite 只允许作为 migration tooling 的 legacy 输入或测试 fixture，不能作为正常 HTTP 业务运行态。

需要手动启动本地 Postgres/Weaviate 组合时使用：

```powershell
docker compose -f docker-compose.postgres.yml up -d postgres weaviate
```

Weaviate 默认参与运行时；`BILL_ANALYSER_WEAVIATE_ENABLED=true` 和 `BILL_ANALYSER_WEAVIATE_ENDPOINT=http://127.0.0.1:8088` 是本地默认值。运维命令和重建流程见 [Weaviate derived index](weaviate-derived-index.md)。

一键启动器会尊重 `.env` 或当前环境里的 `BILL_ANALYSER_POSTGRES_URL` / `BILL_ANALYSER_WEAVIATE_ENDPOINT`。未显式配置 Postgres URL 时，如果本机 `5432` 已被其他进程占用，启动器会为本项目 compose Postgres 选择可用本地端口，并把对应 `BILL_ANALYSER_POSTGRES_URL` 传给后端。需要固定端口时可设置 `BILL_ANALYSER_POSTGRES_PORT`。

当前 S1 是 fail-closed policy lock：PostgreSQL/Weaviate 是默认运行态要求，健康检查发现 Postgres 或 Weaviate 不满足时会报告 unhealthy/503，而不是回退到 SQLite。登录、注册、邮箱验证、密码找回/重置、refresh token、token session 管理、2FA status/verify/enable/disable/recovery、Profile GET/更新/头像/cloud settings/profile verification resend/external auth list+unlink、user-data 统计/导出/清空、主数据、账户 CRUD/排序、分类 CRUD/批量创建/导入/导出/排序/统计、分类规则 CRUD/重排/测试/legacy 配置、账户规则 CRUD/重排/测试、标签 CRUD/批量创建/排序、模板 CRUD/排序、交易列表/按月列表/详情、统计金额概览、分类统计/分类趋势/资产趋势/饼图/商户排行、Analyzer/insights 读取、汇率读取与用户自定义汇率设置、matching pairs、净值快照、日历事件、周期建议列表/检测/接受/拒绝、预算列表、预算 CRUD、预算导出、预算导入、预算执行、预算预测、预算历史读取、预算历史快照、备份文件记录、备份任务和备份审计元数据已经走 Postgres 仓储；尚未接管的重业务 route 继续显式失败。运行开关、场景矩阵和本地测试命令见 [导入全链路验收场景](import-full-chain-scenarios.md)。

PostgreSQL cutover 默认设置：

```powershell
$env:BILL_ANALYSER_REQUIRE_POSTGRES_AFTER_CUTOVER = "true"
$env:BILL_ANALYSER_DATABASE_BACKEND = "postgres"
$env:BILL_ANALYSER_POSTGRES_URL = "postgres://bill_analyser:bill_analyser_dev@127.0.0.1:5432/bill_analyser"
```

cutover 开关开启后，业务 route 不再允许 SQLite runtime 作为 fallback；即使有人绕过启动脚本直接把 `BILL_ANALYSER_DATABASE_BACKEND=sqlite` 与 `BILL_ANALYSER_REQUIRE_POSTGRES_AFTER_CUTOVER=false` 传给 Rust HTTP 入口，仓储边界也会以 `sqlite_legacy_disabled` 拒绝业务 SQLite。`/api/health` 会用 `postgres_cutover_status` 标明 backend 未切 Postgres、Postgres URL 缺失、legacy SQLite 被禁用或仓储仍待接管。
