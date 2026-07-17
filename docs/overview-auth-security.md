# 认证与安全

认证运行态直接读写 PostgreSQL：

- `users`
- `token_sessions`
- `user_two_factor_recovery_codes`
- `user_external_auths`
- `business_audit_events`
- `auth_audit_events`

通用业务 Bearer access token 先由 HTTP 认证层校验 JWT 算法、HMAC 签名、`type=access`、过期时间和 `user_id`，随后以 token hash、用户、`is_active` 和 `expires_at` 查询 PostgreSQL `token_sessions`。有效 session id 写入认证上下文；session 不存在或已失效返回 401，session authority 超时或不可用返回 503，不回退为 JWT-only 放行。可信本地 header 仍走独立受控边界，不伪造数据库 session id。

PostgreSQL `token_sessions` 承载 refresh token hash、access token hash、有效状态、过期时间和设备信息。refresh 会消费有效 refresh session，并在同一事务内失效旧 session、轮换 access/refresh hash；logout 按当前 access token hash 失效 session，token 管理页可列出和撤销 session。因为受保护路由每次都读取 session authority，logout、指定 session revoke 与 refresh 轮换会立即阻止旧 access token 继续访问业务 API。

前端应用锁 guard 在桌面和移动 router 中把已登录但未解锁的页面导航重定向到 `/unlock`。`userstate.ts` 保留应用锁 credential/session facade，`userstate/credentials.ts` 承载 storage key、凭据格式识别、CryptoJS 加解密、旧明文 refresh 清理/迁移与解锁恢复；localStorage 中的 access 与 refresh credential 仍使用同一已验证锁状态分别加密，只有当前解锁会话在 sessionStorage 中保存明文，损坏密文或部分解锁会 fail-closed 并清理会话明文。`services.ts` 的 refresh 请求仍以 `noAuth` 方式发送当前已解锁的 refresh token，但 refresh 成功后先持久化并复核新 refresh credential，再提交新 access token，避免部分轮换。

2FA code 管理、profile、cloud settings、external auth、user-data statistics/export/clear 都通过 PostgreSQL 表约束。

敏感动作通过当前密码、操作密码或签名 step-up token 校验，并写入审计事件。备份文件动作按当前用户和操作类型校验，不暴露跨用户文件路径或密钥。

安全实现重点：

- 不记录请求体、文件名、交易描述、prompt、token 或密钥。
- 密钥字段只返回脱敏状态。
- OAuth provider exchange 在当前 workspace 构建中以 disabled-safe 响应收口。
- user-data clear 删除当前用户业务表数据并写入业务审计。
