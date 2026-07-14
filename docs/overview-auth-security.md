# 认证与安全

认证运行态直接读写 PostgreSQL：

- `users`
- `token_sessions`
- `user_two_factor_recovery_codes`
- `user_external_auths`
- `business_audit_events`
- `auth_audit_events`

通用业务 Bearer access token 当前由 HTTP 认证层校验 JWT 算法、HMAC 签名、`type=access`、过期时间和 `user_id`；该路径不查询 `token_sessions`，认证上下文中的 `session_id` 为空。

PostgreSQL `token_sessions` 承载 refresh token hash、access token hash、有效状态、过期时间和设备信息。refresh 会查询有效 refresh session 并轮换 access/refresh hash；logout 按当前 access token hash 失效 session；token 管理页可列出和撤销 session。通用业务 Bearer 校验不读取 session active 状态，因此 session 失效与已签发 access JWT 的通用路由验签是两个独立边界。

前端应用锁 guard 在桌面和移动 router 中把已登录但未解锁的页面导航重定向到 `/unlock`。`userstate.ts` 对 localStorage 中的 access token 加密，并在解锁会话中保存明文 access token；refresh token 由独立 key 直接保存在 localStorage，`services.ts` 的 refresh 请求以 `noAuth` 方式携带该 refresh token。应用锁当前保护的是页面导航和 access-token 读取边界，不改变后端 refresh session 合同。

2FA code 管理、profile、cloud settings、external auth、user-data statistics/export/clear 都通过 PostgreSQL 表约束。

敏感动作通过当前密码、操作密码或签名 step-up token 校验，并写入审计事件。备份文件动作按当前用户和操作类型校验，不暴露跨用户文件路径或密钥。

安全实现重点：

- 不记录请求体、文件名、交易描述、prompt、token 或密钥。
- 密钥字段只返回脱敏状态。
- OAuth provider exchange 在当前 workspace 构建中以 disabled-safe 响应收口。
- user-data clear 删除当前用户业务表数据并写入业务审计。
