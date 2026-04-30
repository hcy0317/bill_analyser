# Bill Analyser 日志与运维

## 7.1 日志系统
- 统一日志入口：`src/bill_analyser/utils/logger.py`
- 支持异步写入、滚动清理、等级分离
- 常用日志目录：`logs/`
- 已删除 `src/bill_analyser/utils/advanced_logger.py` 等已确认退出运行态的平行实现，并由回归测试持续阻止回流。

## 7.2 启停脚本
- 一键启动：`一键启动.bat` / `一键启动.ps1`
- 分别启动：`start_backend.ps1`、`start_frontend.ps1`
- 停止服务：`停止服务器.ps1`
