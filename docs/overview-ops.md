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

Weaviate 默认不参与运行时；需要设置 `BILL_ANALYSER_WEAVIATE_ENABLED=true` 和 `BILL_ANALYSER_WEAVIATE_ENDPOINT` 后才会启用派生向量索引。运维命令和重建流程见 [Weaviate derived index](weaviate-derived-index.md)。

导入主链默认在 SQLite / 规则链路下可验收；开启 PostgreSQL 或 Weaviate 后的运行开关、场景矩阵和本地测试命令见 [导入全链路验收场景](import-full-chain-scenarios.md)。
