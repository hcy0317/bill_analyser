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

本地 Postgres/Weaviate 组合使用：

```powershell
docker compose -f docker-compose.postgres.yml up -d postgres weaviate
```

Weaviate 默认参与运行时；`BILL_ANALYSER_WEAVIATE_ENABLED=true` 和 `BILL_ANALYSER_WEAVIATE_ENDPOINT=http://127.0.0.1:8088` 是本地默认值。运维命令和重建流程见 [Weaviate derived index](weaviate-derived-index.md)。

导入主链默认在 PostgreSQL + Weaviate 运行态下验收；SQLite 仅用于迁移来源、legacy fixture 和显式兼容测试。运行开关、场景矩阵和本地测试命令见 [导入全链路验收场景](import-full-chain-scenarios.md)。

PostgreSQL cutover 验证时设置：

```powershell
$env:BILL_ANALYSER_REQUIRE_POSTGRES_AFTER_CUTOVER = "true"
$env:BILL_ANALYSER_DATABASE_BACKEND = "postgres"
$env:BILL_ANALYSER_POSTGRES_URL = "postgres://bill_analyser:bill_analyser_dev@127.0.0.1:5432/bill_analyser"
```

cutover 开关开启后，业务 route 不再允许 SQLite runtime 作为 fallback；`/api/health` 会用 `postgres_cutover_status` 标明 backend 未切 Postgres、Postgres URL 缺失或仓储仍待接管。
