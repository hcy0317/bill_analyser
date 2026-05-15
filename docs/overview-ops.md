# Bill Analyser 日志与运维

## 7.1 日志系统
- 统一日志入口：`src/bill_analyser/utils/logger.py`
- 支持异步写入、滚动清理、等级分离
- 常用日志目录：`logs/`
- 已删除 `src/bill_analyser/utils/advanced_logger.py` 等已确认退出运行态的平行实现，并由回归测试持续阻止回流。
- Rust 运维合同层位于 `crates/bill-analyser-core/src/ops.rs`，当前覆盖日志/运维邻近的备份、backup jobs、加密状态、同步配置和报告导出格式等合同逻辑；Rust HTTP 已接管备份文件 I/O、restore、cleanup、backup jobs 读写与云同步 provider 上传，Python 仍是实际日志线程与报表文件生成运行时。

## 7.1.1 备份、加密与同步
- 备份主链 `GET /api/backup/`、`POST /api/backup/create`、`POST /api/backup/restore/verify`、`GET /api/backup/download/<filename>`、`DELETE /api/backup/delete/<filename>`、`POST /api/backup/restore/<filename>`、`POST /api/backup/cleanup`、`GET|POST /api/backup/jobs` 与 `POST /api/backup/sync` 已由 Rust backup ops runtime 提供。Rust 按当前 `user_id` 读写 `backup_jobs`，创建 zip 备份、公开 `.zip.enc` Fernet-compatible 加密备份名、校验 checksum/metadata、流式下载、删除、恢复前生成唯一 `before_restore_*` 快照、按 `backup_records` 优先做 retention cleanup，并写入 backup 相关审计。Bearer session 调用备份文件类入口需要有效 `step_up` token；可信内部用户头继续作为 runtime bridge。恢复预验证和实际恢复都会拒绝包含绝对路径或 `..` 段的 zip member，且只把包含 `data/` 文件的安全 zip 视为可恢复；当 SQLite DB 位于 `data/` 内时，创建备份先用 `VACUUM INTO` 快照 DB，实际恢复先解压到 staging 目录再替换 `data/`。Rust 创建的新加密备份可在物理层使用 `_encrypted.fernet` 存储名，但 REST 文件名、DB record 和下载/恢复入口保持 `.zip.enc` 兼容。
- 本地备份加密由 Rust `fernet` crate 执行，密钥仍按 `BILL_ANALYSER_BACKUP_ENCRYPTION_KEY` 的 SHA-256 URL-safe Fernet key 派生；既有 Python 生成的 `.zip.enc` 文件仍可由 Rust 识别、验证和恢复。
- 云同步 REST 入口 `POST /api/backup/sync` 由 Rust 创建本地备份后直接上传到 OSS/S3/COS/Azure Blob/WebDAV；对象前缀会先规整为安全相对路径并补齐尾随 `/`，绝对路径、Windows drive、反斜杠、`..` 段和控制字符会被拒绝；响应、审计与 `backup_records` metadata 只保留递归脱敏后的 provider 配置。Python `SyncManager` 仅作为 CLI/兼容残留，不再是默认 backup ops REST 运行时依赖。
- 报告导出真实 PDF/Excel/HTML 文件仍由 `utils.report_export.ReportExporter` 生成，用户传入文件名会先收敛到叶子文件名再写入输出目录；Rust 合同层固定 `pdf`、`excel`/`xlsx`、`html` 三种格式的扩展名与 MIME。

## 7.2 启停脚本
- 一键启动：`一键启动.bat` / `一键启动.ps1`
- 分别启动：`start_backend.ps1`、`start_frontend.ps1`
- 停止服务：`停止服务器.ps1`
