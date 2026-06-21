# D6 auth-profile-user-data-security 结构债 ledger

本 ledger 固化 D6 认证、个人资料、用户数据与安全域的当前结构边界。D6 继续复制 D1-D5 的“功能域 -> 功能文件夹”模板，先补行为与安全合同锁定，再拆后端和前端结构，再补中文说明和治理基线。

## 1. 域边界

D6 覆盖：

- 后端注册、登录、refresh、logout、token session、API/MCP token、密码重置、邮箱验证、OAuth2 callback、2FA、recovery code、step-up 校验和认证审计。
- 后端 profile 读取与更新、头像、默认账户和默认转账分类校验、cloud settings、external auth unlink、user-data statistics/export/clear。
- 注册默认包 opt-in 的认证入口与 PostgreSQL seed 编排，包括默认账户、分类、分类规则和账户规则写入。
- 前端登录态模型、user store、token store、twoFactorAuth store、userExternalAuth store、桌面用户设置页、移动用户资料页、移动 2FA/session/data management 页面和 OAuth2 callback 页面。

D6 不覆盖：

- 备份 zip/Fernet 文件 I/O、backup job、cloud sync 元数据和 backup_records，归 D7。D6 只覆盖 profile cloud settings 的认证侧读写入口。
- LLM/OCR provider 凭据、外部 provider auth refresh 与向量学习凭据，归 D8。
- 全局 `src/web/src/lib/services.ts` axios facade、`src/web/src/stores/index.ts` root store 聚合、router shell、theme 和 imported transaction 共享结构债，归 D10；D6 只能在已有认证/user 调用点范围内记录依赖或申请 shared lease。
- 主数据账户/分类/规则业务语义，归 D2/D3；D6 只校验 profile 中引用的账户/分类属于当前用户且可用。
- UI 视觉重设计；本计划只做结构、注释、测试和安全治理，保留现有用户设置页面视觉与交互。

计划文件中 D6 `owned_paths` 当前只写了 auth 通配，但 D6 目标名包含 profile 和 user-data。执行 D6 时应按本 ledger 扩展实际 owned paths：

- `src/backend/**auth**`
- `src/backend/http/auth_routes/**`
- `src/backend/db/user_data.rs`
- `src/web/src/views/desktop/user/**`
- `src/web/src/views/mobile/users/**`
- `src/web/src/views/base/users/**`
- `src/web/src/views/desktop/OAuth2CallbackPage.vue`
- `src/web/src/stores/user.ts`
- `src/web/src/stores/token.ts`
- `src/web/src/stores/twoFactorAuth.ts`
- `src/web/src/stores/userExternalAuth.ts`
- `src/web/src/lib/userstate.ts`
- `src/web/src/lib/session.ts`
- `src/web/src/lib/webauthn.ts`
- `src/web/src/models/user*.ts`
- `src/web/src/models/token.ts`
- `src/web/src/models/auth_response.ts`
- `src/web/src/models/oauth2.ts`

## 2. 当前结构 gate 快照

采集时间：2026-06-21，基于 `bill_analyser/main` 的 `fabf76fbf36d6aaeceeeb224e0fb744d336e482a`。

Rust 后端结构 gate：

- `node scripts/check-rust-backend-structure.mjs` 失败 3 项，全部属于 D6。
- D6 直接失败项：
  - `src/backend/db/auth_postgres.rs`：结构 gate 计 2247 行，超过 baseline 1830。
  - `src/backend/db/auth_registration_defaults.rs`：结构 gate 计 1078 行，超过新文件限制 600。
  - `src/backend/http/auth_routes/public_auth_handlers.rs`：结构 gate 计 653 行，超过 baseline 625。

前端结构 gate：

- `Set-Location src/web; npm run structure:check` 失败 4 项，当前均不属于 D6：
  - `src/lib/services.ts`
  - `src/stores/index.ts`
  - `src/core/theme.ts`
  - `src/models/imported_transaction.ts`
- D6 相关超大但当前未列入 failure 的治理对象：
  - `src/web/src/views/mobile/users/UserProfilePage.vue`：约 700 行。
  - `src/web/src/views/desktop/user/settings/tabs/UserBasicSettingTab.vue`：约 617 行。
  - `src/web/src/stores/user.ts`：约 547 行。
  - `src/web/src/views/desktop/user/settings/tabs/UserSecuritySettingTab.vue`：约 469 行。
  - `src/web/src/views/desktop/user/settings/tabs/UserDataManagementSettingTab.vue`：约 317 行。
  - `src/web/src/views/desktop/user/settings/tabs/UserTwoFactorAuthSettingTab.vue`：约 258 行。

## 3. 后端文件职责地图

### 3.1 `src/backend/db/auth_postgres.rs`

当前集中承载 PostgreSQL 认证 repository 主链：

- 用户名、邮箱、账户、分类归属存在性校验。
- login/profile/external auth/cloud settings 查询。
- auth log 创建、事件限流计数、失败次数统计、最近 token 密码失败统计。
- 密码更新、邮箱验证状态、last login 和 failed login 更新。
- token session cleanup、读取、创建、refresh rotate、按 session id/hash 失效和其他 session 失效。
- 2FA 状态读取、启用、禁用、recovery code replace/consume 和 recovery code hash helper。
- profile 更新、profile update 到 metadata JSON 的投影、profile update auth log 事务。
- cloud settings upsert/delete。
- 注册用户事务、默认包 seed、默认账户/分类/规则写入和注册默认包 SQL helper。
- metadata JSON 读取、写入、类型转换和 row projector。

拆分建议：

1. `identity_checks.rs`：username/email/account/category 归属与存在性查询。
2. `login_profile_reads.rs`：login user、token user、profile、external auth 和 cloud settings read projector。
3. `audit_events.rs`：auth log 创建、事件限流、失败计数和事务内 auth log helper。
4. `sessions.rs`：token session cleanup/read/create/rotate/invalidate。
5. `two_factor.rs`：2FA status、enable/disable、recovery code replace/consume 和 recovery code hash。
6. `profile_updates.rs`：profile update、metadata mutation、account/category 引用校验和 auth log 事务。
7. `cloud_settings.rs`：application cloud settings upsert/delete。
8. `registration.rs`：用户注册事务、password/email 初始状态和 default package 调用。
9. `registration_defaults.rs` 或 `registration_defaults_sql.rs`：默认包 seed SQL 写入逻辑。
10. `mapping.rs`：row -> `AuthLoginUserRow`、`AuthUserProfileRow`、`ExternalAuthRow`。
11. `metadata.rs`：metadata JSON get/set/parse 和用户 id 转换 helper。

### 3.2 `src/backend/db/auth_registration_defaults.rs`

当前集中承载 `standard_daily_v1` 注册默认包静态定义和合同测试：

- 默认账户类型、默认账户列表和账户识别规则。
- 默认支出、收入、转账、投资分类树。
- 默认分类规则表达式。
- 默认包名称、版本、摘要和测试样例。
- 默认规则表达式的语法编译与代表性匹配测试。

拆分建议：

1. `registration_defaults/types.rs`：默认包常量、类型别名和公共 re-export。
2. `registration_defaults/expense_categories.rs`：支出分类树。
3. `registration_defaults/income_transfer_investment.rs`：收入、转账和投资分类树。
4. `registration_defaults/accounts.rs`：默认账户和账户类型。
5. `registration_defaults/category_rules.rs`：分类规则表达式。
6. `registration_defaults/account_rules.rs`：账户规则表达式。
7. `registration_defaults/package.rs`：`STANDARD_DAILY_V1_*` 聚合导出。
8. `registration_defaults/tests.rs`：现有表达式编译和代表性匹配测试。

拆分时不得改变 `standard_daily_v1` 的分类/账户/rule id、名称、颜色、icon、表达式或 opt-in 语义。

### 3.3 `src/backend/http/auth_routes/public_auth_handlers.rs`

当前集中承载 public auth route handlers：

- OPTIONS/CORS 处理。
- 注册请求 body 解析、密码策略、默认包解析、注册响应和 defaultSeed 投影。
- API/MCP token 生成入口。
- 登录请求解析、密码校验、2FA 分支、失败次数、锁定状态、auth log 和 token session 创建。
- refresh token claim 校验、session rotate 和 refresh 响应。

拆分建议：

1. `public_auth_handlers/cors.rs`：OPTIONS handler、CORS middleware 和 header helper。
2. `public_auth_handlers/register.rs`：register handler、register body 解析、default package 解析和 response projection。
3. `public_auth_handlers/login.rs`：login handler、password 验证、2FA pending response 和 auth log。
4. `public_auth_handlers/refresh.rs`：refresh token handler、claim 校验和 session rotate。
5. `public_auth_handlers/personal_tokens.rs`：API/MCP token handler 调用。
6. `public_auth_handlers/payloads.rs`：request body/object helper 和 register/login response helper。

拆分时不得改变 REST path、CORS header、错误消息、`need2FA`、`defaultSeed`、token 字段或 response envelope。

### 3.4 已拆 route 子模块

以下文件已经按功能分区，D6 backend-shape 默认不优先拆，除非 behavior-lock 或结构 gate 证明需要继续治理：

- `src/backend/http/auth_routes/profile_data_handlers/profile_handlers.rs`
- `src/backend/http/auth_routes/profile_data_handlers/account_recovery_handlers.rs`
- `src/backend/http/auth_routes/profile_data_handlers/user_data_handlers.rs`
- `src/backend/http/auth_routes/two_factor_handlers/manage.rs`
- `src/backend/http/auth_routes/two_factor_handlers/status_step_up.rs`
- `src/backend/http/auth_routes/two_factor_handlers/recovery_verify.rs`
- `src/backend/http/auth_routes/token_session_handlers.rs`
- `src/backend/http/auth_routes/jwt_totp_helpers/**`
- `src/backend/http/auth_routes/profile_payload_helpers/**`
- `src/backend/http/auth_routes/runtime_audit_helpers/**`

这些文件可在 comment-pass 补中文说明，但不应在 D6 backend-shape 中无必要地重切。

## 4. 前端文件职责地图

### 4.1 `src/web/src/stores/user.ts`

当前集中承载：

- 当前用户 basic info 缓存、localStorage 写入和 getter。
- profile 读取、profile 局部更新、头像上传/删除。
- application cloud settings 读取、完整更新和禁用。
- user-data statistics、交易/全量数据导出。
- settings bundle 整包/分 section 导出、预览导入和导入。
- avatar URL cache busting helper。

拆分建议：

1. `stores/user/basicInfo.ts`：basic info 缓存、normalize、getter 和 localStorage。
2. `stores/user/profileActions.ts`：profile 读取、资料更新、头像上传/删除。
3. `stores/user/cloudSettings.ts`：application cloud settings read/update/disable。
4. `stores/user/dataManagement.ts`：statistics、export user data 和 clear data adapter。
5. `stores/user/settingsBundle.ts`：settings bundle export/preview/import。
6. `stores/user/avatar.ts`：avatar URL helper。
7. `stores/user/index.ts`：`useUserStore` facade，保留现有导出名。

### 4.2 桌面用户设置页

`src/web/src/views/desktop/user/settings/tabs/UserBasicSettingTab.vue` 当前集中承载资料表单、默认账户/分类显示、头像上传、保存和邮件验证重发。拆分建议：

1. 外置 template/style，保留 tab facade。
2. `basic/useUserBasicSettings.ts`：初始化、表单状态、保存、头像和 resend email 编排。
3. `basic/accountCategoryLabels.ts`：默认账户、现金账户和现金转账分类显示文本。
4. `basic/profileValidation.ts`：未变更、基础输入、扩展输入和语言区域校验组合。

`src/web/src/views/desktop/user/settings/tabs/UserSecuritySettingTab.vue` 当前集中承载密码更新、external auth、API token、session list 和 revoke。拆分建议：

1. `security/usePasswordChange.ts`
2. `security/useExternalAuthLinks.ts`
3. `security/useTokenSessions.ts`
4. `security/sessionPresentation.ts`

`UserTwoFactorAuthSettingTab.vue`、`UserDataManagementSettingTab.vue` 和两个 dialog 当前可保持入口组件，但 D6 frontend-shape 应优先把复杂操作流下沉到相邻 composable/helper，保留按钮、弹窗和 store 调用合同。

### 4.3 移动用户页

`src/web/src/views/mobile/users/UserProfilePage.vue` 当前集中承载移动资料页模板、profile 初始化、保存、密码确认 sheet、账户/语言/格式设置弹窗和更多操作。拆分建议：

1. 外置 template/style，保留页面 facade。
2. `profile/useMobileUserProfilePage.ts`：初始化、保存、密码确认和路由错误处理。
3. `profile/mobileProfileSheets.ts`：sheet/popup 布尔状态。
4. `profile/mobileProfileOptions.ts`：语言、货币、日期时间、数字格式和颜色选项。
5. `profile/mobileProfileLabels.ts`：当前选项展示文本。

`TwoFactorAuthPage.vue`、`SessionListPage.vue`、`DataManagementPage.vue` 当前行数较低，可优先补中文说明和抽出复杂私有 helper，不强制拆成过多文件。

### 4.4 前端共享依赖

D6 可读取但默认不重构：

- `src/web/src/lib/services.ts`：全局 axios facade 和具体 REST adapter 仍归 D10。
- `src/web/src/stores/index.ts`：root store 聚合归 D10。
- `src/web/src/lib/userstate.ts`、`src/web/src/lib/session.ts`、`src/web/src/lib/webauthn.ts`：D6 可在注释和小 helper 拆分范围内治理，不能改变 localStorage key、session restore 或 WebAuthn 数据格式。

## 5. 行为锁定测试锚点

后端现有锚点：

- `tests/backend/core/auth_security_contracts.rs`：Bearer header、refresh claim、token kind、password policy、recovery code normalization。
- `tests/backend/core/bridge_cli_contracts.rs`：auth bridge CLI refresh claim 校验。
- `tests/backend/core/runtime_governance_contracts.rs`：auth/profile/2FA/user-data route ownership 和 Rust runtime contract。
- `tests/backend/db/registration_defaults_postgres.rs`：注册默认包 opt-in/opt-out 和 settings bundle 导出一致性。
- `tests/backend/http/internal/auth_runtime_audit_helpers.rs`：auth runtime audit helper 合同。

前端现有锚点：

- `tests/web/models/user.test.ts`：user model profile fill/request mapping。
- `tests/web/models/token.test.ts`：token type 和 session metadata projection。
- `tests/web/lib/authLoggingRedaction.test.ts`：认证日志脱敏。
- `src/web/e2e/setup/auth.setup.ts`、`src/web/e2e/helpers/testAccount.ts`：注册/登录测试账号流程。
- `src/web/e2e/tests/desktop.route-smoke.spec.ts`、`src/web/e2e/tests/mobile.route-smoke.spec.ts`：未登录重定向和认证路由冒烟。

D6 behavior-lock 应补或确认：

1. 注册：必填字段、密码策略、注册禁用、`defaultPackage` 缺失/null/none/standard_daily_v1 响应和 defaultSeed 摘要。
2. 登录：用户名/邮箱登录、错误密码失败计数、锁定状态、2FA pending response、last login 和 auth log。
3. refresh/logout/session：refresh claim 类型、session rotate、token session list、按 session revoke、revoke others。
4. 2FA：setup request、enable confirm、disable、recovery code regenerate/verify、recovery code normalization 和 hash input。
5. profile：默认账户/现金账户/现金转账分类 user-scope 校验、头像大小限制、邮箱修改验证状态、cloud settings 写入。
6. user-data：statistics、export file type、clear transactions/all 的 step-up 或 password 校验、审计事件和敏感错误不泄漏。
7. 前端：user store facade 导出名、localStorage key、profile 保存 payload、token/session 展示、2FA 页面状态和 data management 操作按钮合同。

## 6. 安全审查门禁

D6 必须在 behavior-lock、backend-shape 和 closeout 至少记录一次 security-reviewer 或等价独立安全审查证据。审查清单：

- Secrets：不得新增硬编码 token、password、secret、JWT key、OAuth credential。
- Input validation：注册、登录、profile、2FA、user-data clear/export 请求必须继续做显式字段校验。
- SQL injection：新增或搬迁 SQL 必须保留 `sqlx::query` bind 参数，不拼接用户输入。
- Authentication：Bearer access token、refresh token、session id、step-up token 和 recovery code 不得改变签名、hash 或过期语义。
- Authorization：profile、session、external auth、cloud settings、user-data 和 default account/category 校验必须 user-scope。
- Sensitive data exposure：日志、错误响应、前端 toast/snackbar 不输出密码、token、recovery code、operation password 或 stack trace。
- Rate limiting：登录失败、token 密码失败、profile resend verification、step-up 失败和 sensitive auth 失败的现有限流常量不得丢失。
- File upload：头像上传继续保留大小和格式边界，不扩大可上传内容。
- XSS/CSRF：前端不新增未清洗 HTML 注入；当前 token/localStorage 合同若需改变，必须作为单独安全改造切片，而不是混入结构搬迁。

本 ledger 切片只新增文档，不修改运行时代码；安全审查初始结论为“未引入新攻击面，后续 D6 行为锁定和拆分必须逐项复核上述合同”。

## 7. 切片执行顺序

1. `ledger`：提交本文件，记录结构 gate 快照、域边界、测试锚点和安全审查门禁。
2. `behavior-lock`：只新增/补强测试和必要测试 helper，不移动生产代码；覆盖注册、登录、refresh/session、2FA、profile、user-data 和前端 user store 合同。
3. `backend-shape`：拆 `auth_postgres.rs`、`auth_registration_defaults.rs`、`public_auth_handlers.rs`；保持 public facade、REST route、SQL、user-scope 和 security contract 不变。
4. `frontend-shape`：拆 user store、桌面用户设置 tabs、移动用户资料页；保留 `useUserStore` 等导出、路由、localStorage key、服务调用和现有视觉布局。
5. `comment-pass`：按用户确认的注释标准补中文说明：导出函数、业务关键函数、复杂私有 helper 必须有中文说明；简单 getter、映射、事件转发不强制。
6. `governance-docs`：收紧 D6 后端结构 baseline，必要时更新前端 structure baseline；同步 `docs/PROJECT_OVERVIEW.md` 的认证与安全现状。
7. `closeout`：确认 D6 PR 链、CI、merge、branch delete、结构 gate D6 清零和 security-review evidence，进度 cursor 推进到 D7/ledger。

## 8. 验收门槛

D6 完成前必须满足：

- D6 直接后端结构 gate 失败项清零。
- D6 前端拆分不新增新的 frontend structure failure，且 D6 超大页面/store 有明确 facade + 功能文件夹边界。
- `cargo fmt --all -- --check`、相关 auth/profile/user-data 测试、`cargo clippy --workspace --all-targets -- -D warnings` 和 `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 35` 通过。
- `Set-Location src/web; npm run lint:ci` 和 `npm run test:coverage` 通过，必要时补桌面/移动用户页或 store 单测。
- 安全审查结论不含 blocker；如发现真实安全缺陷，必须拆出独立 fix/prerequisite slice，不能混入结构搬迁。
- PR CI 通过、自动 squash merge、来源分支删除、进度 JSON 与 ultragoal ledger 回写完成。

## 9. Behavior-lock 记录

D6 behavior-lock 切片补充了拆分前合同测试，仍不移动生产代码：

- `tests/backend/core/auth_security_contracts.rs`：新增 `AuthRestError` JSON 对外字段合同，锁定不暴露 password/token/stack；补充 `TokenKind::from_str`、`as_str`、API/MCP/session user-agent 和 token type 推断合同。
- `tests/web/stores/user.test.ts`：新增 user store 合同测试，覆盖 basic info localStorage key、profile 读取归一化、profile 更新缓存写回、user-data statistics fallback 计数、导出 content-type 校验、settings bundle section export/preview 和 avatar token URL helper。
- `tests/web/stores/authSecurity.test.ts`：新增认证安全 store 合同测试，覆盖 refresh token 轮换写回、旧 token best-effort revoke、cloud settings/user profile 写回、API token generation、2FA status/confirm/recovery code、external auth list/unlink。

本切片本地验证：

- `cargo fmt --all -- --check` 通过。
- `cargo test -p bill-analyser-core --test auth_security_contracts` 通过，7 tests。
- `cargo test -p bill-analyser-db --test registration_defaults_postgres` 通过，1 test。
- `Set-Location src/web; npm run test -- --runTestsByPath ../../tests/web/stores/authSecurity.test.ts ../../tests/web/stores/user.test.ts ../../tests/web/models/user.test.ts ../../tests/web/models/token.test.ts` 通过，20 tests。
- `Set-Location src/web; npm run lint:ci` 通过，仅有既有 `no-explicit-any` warnings。
- `Set-Location src/web; npm run test:coverage` 通过，93 suites / 38996 tests，All files line coverage 99.13%。
- `node scripts/check-backend-doc-map.mjs` 通过。
- `node scripts/check-rust-backend-structure.mjs` 预期失败仍为 D6 三项：`auth_postgres.rs`、`auth_registration_defaults.rs`、`public_auth_handlers.rs`。
- `Set-Location src/web; npm run structure:check` 预期失败仍为 D10 shared 四项：`services.ts`、`stores/index.ts`、`core/theme.ts`、`models/imported_transaction.ts`。

## 10. Backend-shape 记录

D6 backend-shape 切片按现有后端 facade 模板完成纯结构拆分，不改变 SQL、REST path、请求/响应字段、JWT/refresh/session、2FA/recovery code、注册默认包内容或 user-scope 校验。

- `src/backend/db/auth_postgres.rs`：保留 repository facade、共享 metadata/row mapping/helper 和模块内测试；实现下沉到 `auth_postgres/identity_checks.rs`、`read_models.rs`、`audit_events.rs`、`sessions.rs`、`two_factor.rs`、`profile_updates.rs`、`cloud_settings.rs`、`registration.rs`、`registration_defaults_seed.rs`。
- `src/backend/db/auth_registration_defaults.rs`：保留默认包 facade；默认包常量和测试下沉到 `auth_registration_defaults/types.rs`、`expense_categories.rs`、`income_transfer_investment.rs`、`categories.rs`、`accounts.rs`、`category_rules.rs`、`account_rules.rs`、`tests.rs`。
- `src/backend/http/auth_routes/public_auth_handlers.rs`：保留 public auth handler facade；CORS、注册、API/MCP token、登录和 refresh handler 下沉到 `public_auth_handlers/cors.rs`、`register.rs`、`personal_tokens.rs`、`login.rs`、`refresh.rs`。

本切片本地验证：

- `cargo fmt --all` 通过。
- `cargo test -p bill-analyser-db auth_postgres` 通过，5 tests。
- `cargo test -p bill-analyser-db --test registration_defaults_postgres` 通过，1 test。
- `cargo test -p bill-analyser-core --test auth_security_contracts` 通过，7 tests。
- `cargo test -p bill-analyser-http auth_routes` 通过，7 tests。
- `node scripts/check-rust-backend-structure.mjs` 通过；D6 三项后端结构 gate 红灯清零，且新子文件均低于 600 行。
- `cargo fmt --all -- --check` 通过。
- `cargo clippy --workspace --all-targets -- -D warnings` 通过。
- `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 35` 通过，生成 `workspace.lcov`。
- 独立 `security-reviewer` 审查结论 `APPROVE`：未发现安全 blocker；确认未新增硬编码 secret，SQL 仍使用 bind 参数，token/session/2FA/recovery-code/profile/cloud-settings/external-auth 语义和 user-scope 校验保持不变，CORS/preflight 只做文件移动。

## 11. Frontend-shape 记录

D6 frontend-shape 切片按前端 facade + 功能文件夹模板完成认证/用户资料前端结构拆分，不改变 `useUserStore` 导出名、localStorage key、profile 保存 payload、头像 URL 生成、settings bundle/user-data 调用、桌面/移动用户设置入口或现有视觉布局。

- `src/web/src/stores/user.ts`：保留 `useUserStore` facade；basic info/localStorage getter、profile/avatar action、cloud settings action、user-data statistics/export action、settings bundle import/export action 分别下沉到 `stores/user/basicInfo.ts`、`profileActions.ts`、`cloudSettings.ts`、`dataManagement.ts`、`settingsBundle.ts`、`avatar.ts`。
- `src/web/src/views/desktop/user/settings/tabs/UserBasicSettingTab.vue`：保留桌面基本信息 tab 入口；template/style 下沉到 `basic/UserBasicSettingTab.template.html` 和 `basic/UserBasicSettingTab.css`，账户/现金账户/现金转账分类展示文本下沉到 `basic/accountCategoryLabels.ts`。
- `src/web/src/views/mobile/users/UserProfilePage.vue`：保留移动用户资料页入口；template 下沉到 `profile/UserProfilePage.template.html`，当前语言和星期展示文本下沉到 `profile/mobileProfileLabels.ts`。

本切片本地验证：

- `Set-Location src/web; npm run lint:ci` 通过，仅有既有 `no-explicit-any` warnings。
- `Set-Location src/web; npm run test -- --runTestsByPath ../../tests/web/stores/user.test.ts ../../tests/web/stores/authSecurity.test.ts ../../tests/web/models/user.test.ts ../../tests/web/models/token.test.ts` 通过，4 suites / 20 tests。
- `Set-Location src/web; npm run structure:check` 预期失败仅剩 D10 shared 四项：`src/lib/services.ts`、`src/stores/index.ts`、`src/core/theme.ts`、`src/models/imported_transaction.ts`；D6 `UserBasicSettingTab.vue`、`UserProfilePage.vue` 和 `stores/user.ts` 均只剩 line-reduction warning。
- `Set-Location src/web; npm run test:coverage` 通过，93 suites / 38996 tests，All files line coverage 99.13%。
- `Set-Location src/web; npm run build` 通过；仅出现既有 Sass `@import` deprecation、`server_settings.js` 非 module、Framework7 空 CSS 和 chunk size warnings。
- `git diff --check` 通过；仅有 Windows checkout 换行提示。

## 12. Comment-pass 记录

D6 comment-pass 切片按用户确认的注释标准补齐中文说明：导出函数、业务关键函数、复杂私有 helper 必须说明意图、边界或安全语义；简单 getter、映射和事件转发不强制扩写。该切片只补文档注释，不改变 SQL、REST path、请求/响应字段、token/session/2FA/recovery code 语义、localStorage key、profile/user-data/settings bundle 调用或页面视觉。

后端补充范围：

- `src/backend/db/auth_postgres/**`：身份存在性校验、登录/profile 读取、审计事件、session 生命周期、2FA/recovery code、profile/cloud settings 更新、注册与默认包 seed helper。
- `src/backend/db/user_data.rs`：user-data statistics/export/clear/audit 入口和导出/统计私有 helper。
- `src/backend/http/auth_routes/**`：public auth、token/session、profile/user-data、account recovery、2FA status/manage/recovery verify handler 和响应 helper。

前端补充范围：

- `src/web/src/stores/user/**`、`src/web/src/stores/token.ts`、`src/web/src/stores/twoFactorAuth.ts`、`src/web/src/stores/userExternalAuth.ts`：user/token/2FA/external auth store facade 与 action helper。
- `src/web/src/lib/userstate.ts`、`src/web/src/lib/session.ts`、`src/web/src/lib/webauthn.ts`：token、app lock、user info、session metadata 和 WebAuthn 边界 helper。
- `src/web/src/models/user.ts`、`src/web/src/models/token.ts`、`src/web/src/views/base/users/**`、桌面/移动用户资料 label helper：模型和页面组合函数说明。

本切片本地验证：

- 自定义 D6 注释扫描通过，覆盖选定 D6 Rust/TypeScript 文件的 `pub fn`、`export function/class`、`export const use*` 注释前置检查。
- `cargo fmt --all -- --check` 通过。
- `git diff --check` 通过；仅有 Windows checkout 换行提示。
- `node scripts/check-backend-doc-map.mjs` 通过。
- `Set-Location src/web; npm run lint:ci` 通过，仅有既有 `no-explicit-any` warnings。
- `cargo test -p bill-analyser-core --test auth_security_contracts` 通过，7 tests。
- `cargo test -p bill-analyser-db --test registration_defaults_postgres` 通过，1 test。
- `cargo test -p bill-analyser-http auth_routes` 通过，7 tests。
- `Set-Location src/web; npm run test -- --runTestsByPath ../../tests/web/stores/user.test.ts ../../tests/web/stores/authSecurity.test.ts ../../tests/web/models/user.test.ts ../../tests/web/models/token.test.ts` 通过，4 suites / 20 tests。
- `Set-Location src/web; npm run test:coverage` 通过，93 suites / 38996 tests，All files line coverage 99.13%。
- `cargo clippy --workspace --all-targets -- -D warnings` 通过。
- `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 35` 通过，生成 `workspace.lcov`。
- `node scripts/check-rust-backend-structure.mjs` 通过；仅剩 baseline line-reduction warnings。
- `Set-Location src/web; npm run structure:check` 预期失败仍为 D10 shared 四项：`src/lib/services.ts`、`src/stores/index.ts`、`src/core/theme.ts`、`src/models/imported_transaction.ts`；D6 相关文件没有新增 FAIL。

## 13. Governance-docs 记录

D6 governance-docs 切片只收紧结构治理基线和记录现状，不改变认证、profile、2FA、user-data 的运行时代码、REST 合同、SQL、token/session 语义、localStorage key 或页面视觉。

治理基线更新：

- `scripts/rust-backend-structure-baseline.json`：将 `src/backend/db/auth_postgres.rs` 从旧 1830 行基线收紧到当前 633 行，移除已低于 Rust warning 阈值的 `src/backend/http/auth_routes/public_auth_handlers.rs` baseline；D6 backend-shape 后端结构债不再以 line-reduction warning 形式残留。
- `src/web/scripts/frontend-structure-baseline.json`：移除已拆分且低于新文件阈值的 D6 前端入口 baseline：`src/views/mobile/users/UserProfilePage.vue`、`src/views/desktop/user/settings/tabs/UserBasicSettingTab.vue`、`src/stores/user.ts`。
- `docs/PROJECT_OVERVIEW.md` 已在 backend-shape/frontend-shape 切片同步为当前 facade + 功能文件夹事实；本切片不重复改写项目总览。

本切片本地验证：

- `node scripts/check-rust-backend-structure.mjs` 通过，扫描 464 个 Rust 后端文件，baseline entries 从 9 收紧到 8。
- `Set-Location src/web; npm run structure:check` 预期失败仍为 D10 shared 四项：`src/lib/services.ts`、`src/stores/index.ts`、`src/core/theme.ts`、`src/models/imported_transaction.ts`；D6 已清理三项不再出现 line-reduction warning。
