# 运维与启动

本地开发使用 PowerShell 启动脚本：

```powershell
.\一键启动.ps1
.\start_backend.ps1
.\start_frontend.ps1
.\停止服务器.ps1
```

`.\一键启动.ps1` 会按端口清理已有服务，启动 `docker-compose.postgres.yml` 中的 PostgreSQL 与 Weaviate，构建并启动 Rust HTTP 后端，再启动 Vite 前端。

## Required Runtime

- PostgreSQL: `127.0.0.1:5432`
- Weaviate: `127.0.0.1:8088`
- HTTP backend: `127.0.0.1:5000`
- Frontend: `127.0.0.1:8081`

`start_backend.ps1` 会预检 PostgreSQL 与 Weaviate 端口。任一服务不可达时，后端启动失败；`/api/health` 也会返回 `unhealthy`。

## Backup Ops

备份运行态支持文件 list/create/download/delete/verify/cleanup、job list/save 与 cloud sync 元数据。文件内容由 Rust zip/Fernet 处理，记录写入 PostgreSQL backup tables。
