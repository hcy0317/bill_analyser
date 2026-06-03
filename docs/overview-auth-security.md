# 认证与安全

认证运行态直接读写 PostgreSQL：

- `users`
- `token_sessions`
- `user_two_factor_recovery_codes`
- `user_external_auths`
- `business_audit_events`
- `auth_audit_events`

Bearer access token 必须校验签名、过期时间和 token session。refresh、logout、token session 管理、2FA code 管理、profile、cloud settings、external auth、user-data statistics/export/clear 都通过 PostgreSQL 表约束。

敏感动作通过当前密码、操作密码或签名 step-up token 校验，并写入审计事件。备份文件动作按当前用户和操作类型校验，不暴露跨用户文件路径或密钥。

安全实现重点：

- 不记录请求体、文件名、交易描述、prompt、token 或密钥。
- 密钥字段只返回脱敏状态。
- OAuth provider exchange 在当前 workspace 构建中以 disabled-safe 响应收口。
- user-data clear 删除当前用户业务表数据并写入业务审计。
