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
