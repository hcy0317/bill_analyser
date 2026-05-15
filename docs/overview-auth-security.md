# 认证与安全

认证安全域由 Rust runtime 接管。

## 覆盖范围

- 登录、注册、access/refresh token、logout、token sessions、API/MCP token。
- profile、头像、邮箱验证、cloud settings、external auth list/unlink。
- account recovery、OAuth2 disabled-safe 合同、system version。
- user-data statistics/export/clear。
- 2FA status、TOTP verify、enable/disable、recovery code regenerate/verify。
- step-up action token。
- backup file operations 的额外 step-up 校验。

## 安全约束

- Bearer access token 必须校验签名、过期、session hash、用户启用状态。
- user-scope 数据只允许当前用户读取/写入。
- 当前密码、操作密码回退、TOTP passcode 和 step-up token 的适用范围必须明确。
- 失败认证写入 auth log，并在敏感动作上保留短窗口限流。
- 上传头像和备份文件操作必须验证内容类型、路径安全和当前用户上下文。
