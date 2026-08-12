# 运维与启动

本地开发使用 PowerShell 启动脚本：

```powershell
.\一键启动.bat
.\一键结束.bat
.\scripts\dev.ps1 start
.\scripts\dev.ps1 status
.\scripts\dev.ps1 logs
.\scripts\dev.ps1 stop
.\scripts\dev.ps1 check
.\start_backend.ps1
.\start_frontend.ps1
.\停止服务器.ps1
```

`.\一键启动.bat` 要求 PowerShell 7，并以后台日志和进程 manifest 模式调用 `.\一键启动.ps1`。启动器会按端口清理已有服务，启动 `docker-compose.postgres.yml` 中的 PostgreSQL 与 Weaviate，构建并启动 Rust HTTP 后端，再启动 Vite 前端；子进程提前退出时立即回显日志尾部。

`scripts/dev.ps1` 是日常开发的托管入口。`start`、`status`、`logs`、`stop` 共享 `.git/ai/dev-services/services.manifest.json`，停止操作会核对 PID、可执行文件路径、启动时间与监听端口，避免按全局进程名清理。`check` 从真实一键启动链路启动服务，验证后端 `/api/health` 和前端 HTTP，再只清理本次 manifest 拥有的进程。

`.\一键结束.bat` 是 `dev.ps1 stop` 的双击入口，只精确停止 manifest 托管的 Rust 后端和 Vite 前端；PostgreSQL 与 Weaviate 容器继续运行，不删除数据卷。

## Required Runtime

- PostgreSQL: `127.0.0.1:5432`
- Weaviate: `127.0.0.1:8088`
- HTTP backend: `127.0.0.1:5000`
- Frontend: `127.0.0.1:8081`

`start_backend.ps1` 会预检 PostgreSQL 与 Weaviate 端口。任一服务不可达时，后端启动失败；`/api/health` 也会返回 `unhealthy`。

## Backup Ops

备份运行态支持文件 list/create/download/delete/verify/cleanup、job list/save 与 cloud sync 元数据。文件内容由 Rust zip/Fernet 处理，记录写入 PostgreSQL backup tables。
