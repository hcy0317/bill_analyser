# 认证与安全

认证安全域由 Rust runtime 接管。HTTP `auth_routes/` 保持 facade 注册路由，内部按 public auth、profile/user-data、2FA/token、JWT/TOTP、payload、audit/response helper 分片；DB `auth.rs` 保持 SQLite repository facade，PostgreSQL cutover 入口由 `auth_postgres.rs` 与 `user_data.rs` 读取 `users`、`token_sessions`、`user_two_factor_recovery_codes`、`user_external_auths`、`accounts`、`categories`、`settings` 与 `business_audit_events`。PostgreSQL 模式下登录、注册、邮箱验证、密码找回/重置、refresh token、token session 管理、2FA status/verify/enable/disable/recovery、Profile GET/更新/头像/cloud settings/profile verification resend/external auth list+unlink、user-data statistics/export/clear、Bearer JWT 解析和 logout 不再打开 SQLite session fallback；普通 Bearer API 请求以签名和过期时间有效的 JWT 作为身份边界，token 管理和 refresh 通过 PostgreSQL session 表约束。

## 覆盖范围

- 登录、注册、access/refresh token、logout、token sessions、API/MCP token。
- profile、头像、邮箱验证、cloud settings、external auth list/unlink。
- account recovery、OAuth2 disabled-safe 合同、system version。
- user-data statistics/export/clear。
- 2FA status、TOTP verify、enable/disable、recovery code regenerate/verify。
- step-up action token。
- backup file operations 的额外 step-up 校验。

## 安全约束

- Bearer access token 必须校验签名和过期时间；legacy SQLite test-only runtime 继续校验 session hash 与用户启用状态，PostgreSQL cutover runtime 的登录/注册/邮箱验证/密码重置/refresh/token session/2FA/Profile/external auth 与 user-data 统计/导出/清空直接读写 PostgreSQL，不能回退到 SQLite session 表。
- user-scope 数据只允许当前用户读取/写入。
- 当前密码、操作密码回退、TOTP passcode 和 step-up token 的适用范围必须明确；PostgreSQL user-data clear 使用 PostgreSQL 用户密码、环境操作密码、user-scoped `settings.operation_password` 或签名 step-up token，不读取 SQLite session 表。
- 失败认证写入 auth log，并在敏感动作上保留短窗口限流。
- 上传头像和备份文件操作必须验证内容类型、路径安全和当前用户上下文。
