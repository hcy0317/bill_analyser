# Bill Analyser 日志与运维

## 7.1 日志系统
- 统一日志入口：`src/bill_analyser/utils/logger.py`
- 支持异步写入、滚动清理、等级分离
- 常用日志目录：`logs/`
- 已删除 `src/bill_analyser/utils/advanced_logger.py` 等已确认退出运行态的平行实现，并由回归测试持续阻止回流。
- Rust 运维合同层位于 `crates/bill-analyser-core/src/ops.rs`，当前覆盖日志/运维邻近的备份、backup jobs、加密状态、同步配置和报告导出格式等合同逻辑；Rust HTTP 已接管 backup jobs 读写，Python 仍是实际日志线程、备份文件 I/O、云 SDK 上传与报表文件生成运行时。

## 7.1.1 备份、加密与同步
- 备份 jobs 主链 `GET|POST /api/backup/jobs` 已由 Rust backup ops runtime 提供，按当前 `user_id` 直接读写 `backup_jobs`，同一用户同一 `job_type` 保存会更新既有任务，跨用户或不存在的 `id` 不会被更新，并记录包含 actor 与来源 IP 的 `backup_job_saved` 审计；备份文件列表、创建、恢复预验证、下载、删除、恢复和清理仍由 `src/bill_analyser/api/routes/backup/` 提供。恢复预验证和实际恢复都会拒绝包含绝对路径或 `..` 段的 zip member，且只把包含 `data/` 目录的安全 zip 视为可恢复，Rust 合同层固定安全文件名、`.zip.enc` 加密备份识别、恢复预验证摘要、metadata checksum 投影和 cleanup retention 决策，避免 Python route 与 DB 记录排序/保留语义漂移。
- 本地备份加密继续由 Python `cryptography.Fernet` 执行，Rust 合同层只固定 `BILL_ANALYSER_BACKUP_ENCRYPTION_KEY` 的 SHA-256 URL-safe Fernet key 派生与密钥存在性判断。
- 云同步仍由 Python `SyncManager` 调用 OSS/S3/COS/Azure/WebDAV SDK；对象前缀会先规整为安全相对路径并补齐尾随 `/`，绝对路径、Windows drive、反斜杠、`..` 段和控制字符会被拒绝；Rust 合同层固定 provider 白名单、默认对象前缀和同步配置中的访问密钥/令牌递归脱敏。
- 报告导出真实 PDF/Excel/HTML 文件仍由 `utils.report_export.ReportExporter` 生成，用户传入文件名会先收敛到叶子文件名再写入输出目录；Rust 合同层固定 `pdf`、`excel`/`xlsx`、`html` 三种格式的扩展名与 MIME。

## 7.2 启停脚本
- 一键启动：`一键启动.bat` / `一键启动.ps1`
- 分别启动：`start_backend.ps1`、`start_frontend.ps1`
- 停止服务：`停止服务器.ps1`
