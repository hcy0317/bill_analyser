# Bill Analyser 认证与安全

- 认证入口主链已统一为 `POST /api/auth/login`、`POST /api/auth/register`、`POST /api/auth/logout`；旧 `authorize.json`、`register.json`、`logout.json` 已停止使用，并由 legacy 404 回归保护
- 认证辅助入口已继续收口到 REST：`POST /api/auth/email/verify`、`POST /api/auth/email/resend-verification`、`POST /api/auth/password/forgot`、`POST /api/auth/password/reset`；旧 `verify_email/*.json` 与 `forget_password/*.json` 已停止使用，并由 legacy 404 回归保护
- 用户资料主链已扩展到头像 REST：`GET|PUT /api/profile`、`POST|DELETE /api/profile/avatar`；旧 `v1/users/avatar/update.json` 与 `v1/users/avatar/remove.json` 已停止使用，并由 legacy 404 回归保护
- 用户资料主链已扩展到验证邮件重发 REST：`POST /api/profile/email/resend-verification`；旧 `v1/users/verify_email/resend.json` 已停止使用，并由 legacy 404 回归保护
- 用户资料主链已扩展到第三方登录 REST：`GET /api/profile/external-auths`、`POST /api/profile/external-auths/unlink`；旧 `v1/users/external_auth/*.json` 已停止使用，并由 legacy 404 回归保护
- 用户资料主链已扩展到应用云同步设置 REST：`GET|PUT|DELETE /api/profile/cloud-settings`；旧 `v1/users/settings/cloud/*.json` 已停止使用，并由 legacy 404 回归保护
- 2FA 主链已扩展到完整 REST 写接口：`GET /api/2fa/status`、`POST /api/2fa/enable/request`、`POST /api/2fa/enable/confirm`、`POST /api/2fa/disable`、`POST /api/2fa/recovery/regenerate`；旧 `v1/users/2fa/*.json` 写接口已停止使用，并由 legacy 404 回归保护
- 2FA 登录验证链路已补齐 REST：`POST /api/2fa/verify`、`POST /api/2fa/recovery/verify`；`/api/auth/login` 在用户启用 2FA 时返回 `need2FA` 与待验证 token，旧 `/api/2fa/authorize.json`、`/api/2fa/recovery.json` 已停止使用，并由 legacy 404 回归保护
- 2FA 恢复码当前已从纯内存缓存升级为数据库持久化哈希存储：`db.py` 新增 `user_two_factor_recovery_codes` 表，启用 2FA / 重生成恢复码时会整体替换当前批次，恢复码登录校验按哈希一次性消费，旧批次在重生成后立即失效
- 安全中心当前已补齐 step-up 验证基础：`POST /api/security/step-up/verify` 可通过当前密码或 TOTP passcode 签发短期 `step_up` token，`POST /api/2fa/disable`、`POST /api/2fa/recovery/regenerate`、`POST /api/data/clear/transactions`、`POST /api/data/clear/all` 已支持使用 `stepUpToken` 替代重复提交密码
- 备份主链当前除创建 / 下载 / 删除 / 恢复外，还支持恢复前预验证与可选本地加密：`GET /api/backup/` 与 `POST /api/backup/create` 会返回备份 `checksum`、压缩包结构检查结果与 `ready_to_restore` 摘要；当存在 `BILL_ANALYSER_BACKUP_ENCRYPTION_KEY` 时，创建接口会产出 `.zip.enc` 本地加密备份并在记录中标记 `encrypted=true`；`POST /api/backup/restore/verify` 与 `POST /api/backup/restore/<filename>` 会自动识别并解密本地加密备份；`POST /api/backup/cleanup` 现已同时覆盖 `.zip` / `.zip.enc`，并优先基于 `backup_records` 决定 retention 淘汰项与回写 `deleted` 状态
- 认证域敏感安全动作当前会同时写认证日志与操作审计：`2fa_enabled`、`2fa_disabled`、`2fa_recovery_regenerated`、`2fa_recovery_code_used` 通过 `audit_logs` 持久化关键安全事件上下文
- 备份域关键动作当前也会写入 `audit_logs`：`backup_created`、`backup_deleted`、`backup_restored`、`backup_restore_verified` 会记录备份文件名、校验值、预验证结果或失败原因，便于后续追踪恢复链路与异常排查
- OAuth2 callback authorize 已收口到 `POST /api/auth/oauth2/authorize`；旧 `/api/oauth2/authorize.json` 已停止使用，并由 legacy 404 回归保护。当前后端仅提供 disabled-safe / not-implemented 语义，待后续真实 OAuth2 provider exchange 主链补齐
- token 会话主链已切到认证域 REST：`GET|DELETE /api/tokens`、`POST /api/tokens/api`、`POST /api/tokens/mcp`、`POST /api/tokens/refresh`、`DELETE /api/tokens/<id>`；旧 `v1/tokens/generate*.json`、`v1/tokens/revoke*.json` 与 `v1/tokens/refresh.json` 已收口到新主链，并由 legacy 回归保护
- 认证错误契约已补齐：`POST /api/auth/login` 与 `POST /api/tokens/refresh` 在请求体为空、`null` 或其他非对象 JSON 时返回 `400 Invalid request`，不再落入 `500`。
- 统计时间范围契约已收紧：`GET /api/statistics/category-statistics`、`GET /api/statistics/category-statistics/trends`、`GET /api/statistics/asset-trends` 在起始时间/年月晚于结束时间时返回 `400`，避免把反向区间静默视为空结果。
