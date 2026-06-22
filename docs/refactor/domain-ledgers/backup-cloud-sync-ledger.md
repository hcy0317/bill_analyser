# D7 backup-cloud-sync 结构债 ledger

本 ledger 固化 D7 备份、云端备份同步与应用设置云同步域的当前结构边界。D7 继续复制 D1-D6 的“功能域 -> 功能文件夹”模板，先锁定文件安全、加密、保留策略、任务、外部 endpoint 与前端设置同步行为，再拆后端和前端结构，再补中文说明和治理基线。

## 1. 域边界

D7 覆盖：

- 后端备份文件 list/create/download/delete/verify/cleanup、zip/Fernet 文件 I/O、公开文件名解析、sidecar metadata、checksum、archive 安全摘要和保留策略。
- 后端 `backup_records`、`backup_jobs`、`backup_audit_logs` 元数据读写、backup job payload normalization 和 best-effort audit 写入。
- 后端 cloud backup sync：OSS、S3、COS、Azure Blob、WebDAV upload config 校验、endpoint allowlist、provider 签名、对象 key 构造、secret redaction 和上传失败投影。
- 备份敏感操作认证：trusted header 与 Bearer session + step-up token 分支、step-up token type/user/expiry 校验。
- 前端应用设置云同步页面：桌面 `AppCloudSyncSettingTab.vue`、移动 `ApplicationCloudSyncSettingsPage.vue`、共享 `AppCloudSyncPageBase.ts`、`stores/user/cloudSettings.ts` 与设置 store 中应用设置同步的创建/应用/禁用流程。

D7 不覆盖：

- 认证注册、登录、2FA、profile、user-data statistics/export/clear 和 profile cloud settings HTTP backend handler 的认证侧职责，已归 D6；D7 只记录应用设置云同步在前端和 store 中的使用边界。
- 全局 `src/web/src/lib/services.ts` axios facade、`src/web/src/stores/index.ts` root store 聚合、router shell、theme 和 imported transaction 共享结构债，归 D10；D7 只能在已有 backup/cloud 调用点范围内记录依赖或申请 shared lease。
- LLM/OCR provider 凭据、外部 AI provider 调用、Weaviate learning 和向量索引，归 D8。
- parser、导入预览、交易、预算统计、规则中心和主数据结构，分别归 D1-D5/D9。
- UI 视觉重设计；本计划只做结构、注释、测试和安全治理，保留现有备份/云同步页面视觉与交互。

计划文件中 D7 `owned_paths` 当前只写了 backup/cloud 通配和前端 backup 目录。执行 D7 时应按本 ledger 扩展实际 owned paths：

- `src/backend/core/ops.rs`
- `src/backend/core/ops/**`
- `src/backend/http/backup_sync.rs`
- `src/backend/http/backup_sync/**`
- `src/backend/http/backup_routes/**`
- `src/backend/db/backup.rs`
- `src/backend/db/backup_postgres.rs`
- `src/backend/db/postgres/migrations/0009_backup_ops.sql`（只读合同锚点，默认不改）
- `src/backend/core/runtime_governance/ownership/auth_taxonomy_backup.rs`（只读合同锚点，除 governance 需要外不改）
- `src/web/src/views/desktop/app/settings/tabs/AppCloudSyncSettingTab.vue`
- `src/web/src/views/mobile/settings/ApplicationCloudSyncSettingsPage.vue`
- `src/web/src/views/base/settings/AppCloudSyncPageBase.ts`
- `src/web/src/stores/user/cloudSettings.ts`
- `src/web/src/stores/setting.ts` 中应用设置云同步相关 helper
- `src/web/src/core/setting.ts` 中 cloud setting 类型/允许 key 合同
- `src/web/src/models/user_app_cloud_setting.ts`

## 2. 当前结构 gate 快照

采集时间：2026-06-21，基于 `bill_analyser/main` 的 `9ee53b8d4ba0f3ba16e7d846afc4697c1c6d30bf`。

Rust 后端结构 gate：

- `node scripts/check-rust-backend-structure.mjs` 通过，扫描 464 个 Rust 后端文件，当前 baseline entries 为 8。
- D7 直接 baseline 对象：
  - `src/backend/core/ops.rs`：structure baseline 计 908 行，物理约 1050 行；当前文件同时承载 backup、user-data 和 report export 合同。
  - `src/backend/http/backup_sync.rs`：structure baseline 计 673 行，物理约 722 行；当前文件集中承载 provider config、endpoint 安全、签名和五类 provider upload。
- D7 直接非 baseline 但可治理对象：
  - `src/backend/http/backup_routes/archive.rs`：约 497 行，集中承载 zip/Fernet/path/metadata/checksum/list temp cleanup。
  - `src/backend/http/backup_routes/auth.rs`：约 291 行，集中承载 backup auth kind、step-up token 解析和测试。
  - `src/backend/http/backup_routes/sync.rs`：约 279 行，承载 cloud sync route 编排。
  - `src/backend/http/backup_routes/mod.rs`：约 278 行，router facade、runtime enum、route helper 和 PG bridge。
  - `src/backend/db/backup_postgres.rs`：约 283 行，集中承载 records/jobs/audit PG 读写。

前端结构 gate：

- `Set-Location src/web; npm run structure:check` 预期失败 4 项，当前均属于 D10 shared：
  - `src/lib/services.ts`
  - `src/stores/index.ts`
  - `src/core/theme.ts`
  - `src/models/imported_transaction.ts`
- D7 前端入口当前未触发 structure FAIL：
  - `src/web/src/views/desktop/app/settings/tabs/AppCloudSyncSettingTab.vue`：约 205 行。
  - `src/web/src/views/mobile/settings/ApplicationCloudSyncSettingsPage.vue`：约 172 行。
  - `src/web/src/views/base/settings/AppCloudSyncPageBase.ts`：约 193 行。
  - `src/web/src/stores/user/cloudSettings.ts`：约 84 行。
  - `src/web/src/stores/setting.ts`：约 416 行，其中应用设置云同步只是部分职责，D7 frontend-shape 可抽出 cloud setting helper，但不得吸收 D10 root store 债务。

## 3. 后端文件职责地图

### 3.1 `src/backend/core/ops.rs`

当前集中承载：

- 备份文件名规范化、路径穿越和 Windows drive/ADS/control char 拦截。
- zip archive member 安全校验、archive summary、backup file info 合同。
- backup cleanup retention plan、record-first 删除决策和 stray file 保留策略。
- backup job payload normalization、retention 上限、schedule/job_type 字符边界。
- backup encryption secret 配置判定和 Fernet key 派生。
- user-data statistics/export/clear audit、report export format 和 report filename 安全 helper。
- cloud sync provider normalization、prefix normalization、object key 构造、sync config secret redaction。

拆分建议：

1. `ops/backup_types.rs`：备份 DTO、常量和 re-export。
2. `ops/backup_filename.rs`：`secure_backup_filename`、`resolve_backup_filename`、`is_allowed_backup_filename`。
3. `ops/backup_archive.rs`：archive member 安全、archive summary、file info projection。
4. `ops/backup_cleanup.rs`：record-first cleanup plan。
5. `ops/backup_jobs.rs`：job payload normalization 和 retention/schedule helper。
6. `ops/backup_crypto.rs`：encryption secret 和 Fernet key 派生。
7. `ops/backup_sync.rs`：provider/prefix/object key/sync config redaction。
8. `ops/user_data.rs`：user-data statistics/export/clear audit，保持 D6 合同不变。
9. `ops/report_export.rs`：report export format 与 filename 安全 helper，后续如 D10 需要可再归并。

拆分后 `src/backend/core/ops.rs` 保持 public facade 和原导出名，`src/backend/core/lib.rs` re-export 不变。

### 3.2 `src/backend/http/backup_sync.rs`

当前集中承载：

- `CloudBackupUploadResult`、`CloudBackupUploadError` 和 provider status/config 错误。
- upload config 校验：provider、endpoint、bucket/access/secret、Azure secret base64。
- WebDAV、OSS、S3、COS、Azure Blob upload 实现和签名。
- streaming body、SHA256 file hash、AWS/COS/Azure HMAC helper。
- endpoint URL 校验：scheme、credentials/query/fragment、metadata host denylist、private/loopback/link-local 限制、allowlist、本地 HTTP 例外和 provider host 匹配。
- URL path segment 拼接、COS virtual-host/path-style 分支、S3 region 推断。

拆分建议：

1. `backup_sync/types.rs`：result/error 和常量。
2. `backup_sync/config.rs`：config 字段读取、必填校验、provider config 校验。
3. `backup_sync/endpoint.rs`：endpoint allowlist、restricted host、provider host match、URL 拼接和 COS URL。
4. `backup_sync/body_hash.rs`：streaming body、SHA256/SHA1/hex/HMAC helpers。
5. `backup_sync/webdav.rs`
6. `backup_sync/oss.rs`
7. `backup_sync/s3.rs`
8. `backup_sync/cos.rs`
9. `backup_sync/azure.rs`
10. `backup_sync/tests.rs`：endpoint/region/provider config 合同测试。

拆分时不得改变外部 provider 请求方法、header、签名算法、endpoint allowlist 语义、HTTP timeout 或错误 status 投影。

### 3.3 `src/backend/http/backup_routes/**`

当前 route 已经初步拆分为 facade + 子文件：

- `mod.rs`：route patterns、router、runtime enum、authenticated runtime、DB bridge。
- `handlers.rs`：认证 runtime 构建和 Postgres runtime 获取。
- `archive.rs`：文件创建、zip/Fernet 加密解密、metadata/checksum、path resolve、backup dir/list/temp cleanup。
- `create.rs`、`download.rs`、`delete.rs`、`cleanup.rs`：对应 REST handler 编排。
- `jobs.rs`：list/save backup job。
- `sync.rs`：创建本地备份、构造 sync config、调用 cloud uploader 和 audit。
- `auth.rs`：backup 敏感操作 step-up 校验。
- `audit.rs`：backup create/delete/download/cleanup/job/sync audit helper。
- `payload.rs`、`response.rs`：payload parse 和 success/error response。

拆分建议：

- `archive.rs` 可在 backend-shape 继续拆成 `archive/create_zip.rs`、`archive/encryption.rs`、`archive/metadata.rs`、`archive/path.rs`、`archive/listing.rs`，保持 `mod.rs` facade。
- `mod.rs` 可只保留 route facade、type re-export 和 runtime bridge，把 PG bridge helper 放入 `runtime.rs` 或 `repository_bridge.rs`。
- `auth.rs` 可保留当前体量，但 comment-pass 必须说明 step-up token type/user/expiry 与 trusted header 边界。

### 3.4 `src/backend/db/backup.rs` 与 `backup_postgres.rs`

当前职责：

- `backup.rs`：DTO 和 `BackupJobContract -> BackupJobDraft` 转换。
- `backup_postgres.rs`：backup record list/upsert/update、backup job list/create/update、audit log best-effort 写入、row mapping、metadata merge 和 user id 转换。

拆分建议：

- 体量未超过 gate，可暂不拆；如 backend-shape 触及，可按 `records.rs`、`jobs.rs`、`audit.rs`、`mapping.rs` 拆分。
- 不得改变 `backup_jobs` 的 user-scope 隔离、`backup_audit_logs` best-effort 语义、`backup_records.backup_name` conflict upsert 和 status/version 更新。

## 4. 前端文件职责地图

### 4.1 `AppCloudSyncPageBase.ts`

当前集中承载：

- `ALL_APPLICATION_CLOUD_SETTINGS` 分类常量。
- 桌面/移动共用 loading/enabling/disabling 状态。
- 当前已启用设置 key、全选/全不选/反选、分类半选状态。
- 后端返回 cloud settings 到 settings store 和本地 checkbox map 的应用逻辑。

拆分建议：

1. `app-cloud-sync/cloudSettingCatalog.ts`：分类常量、类型和 desktop/mobile 标记。
2. `app-cloud-sync/useAppCloudSyncSelection.ts`：checkbox map、全选/反选、分类半选和 enabled keys。
3. `AppCloudSyncPageBase.ts`：保留 `useAppCloudSyncBase` facade 和原导出名。

### 4.2 桌面与移动页面

`AppCloudSyncSettingTab.vue` 当前承载 Vuetify template、snackbar、初始化、enable/update/disable。拆分建议：

1. 外置 template/style，保留 tab facade。
2. `app-cloud-sync/useDesktopAppCloudSyncActions.ts`：初始化、enable/update/disable 和 snackbar 错误投影。

`ApplicationCloudSyncSettingsPage.vue` 当前承载 Framework7 template、初始化、enable/update/disable、toast/preloader。拆分建议：

1. 外置 template/style，保留页面 facade。
2. `app-cloud-sync/useMobileAppCloudSyncActions.ts`：初始化、enable/update/disable 和 route/toast 错误投影。

拆分时不得改变按钮文案、禁用状态、settings key 列表、选中状态、store 调用、桌面/移动路由或现有视觉布局。

### 4.3 Store 与模型

`src/web/src/stores/user/cloudSettings.ts` 当前承载：

- profile/cloud-settings 读取，false/数组结果投影到 settings store。
- full update 将 enabled setting keys 转为 `ApplicationCloudSetting[]`。
- disable 后清空 settings store 同步状态。

`src/web/src/stores/setting.ts` 当前承载应用设置同步相关职责：

- `syncedAppSettings` 和 `enableApplicationCloudSync`。
- `createUserApplicationCloudSetting`、`updateUserApplicationCloudSettingValue`。
- `createApplicationCloudSettings`、`setApplicationSettingsFromCloudSettings`。
- 各业务设置 setter 中调用 `updateUserApplicationCloudSettingValue`。

拆分建议：

1. `stores/setting/cloudSync.ts`：cloud setting creation、value application、synced map helper。
2. `stores/user/cloudSettings.ts` 保留 user store action facade，不改变导出名。
3. `core/setting.ts` 中允许 key 和 type enum 默认保留；若拆分，必须保留原 import path re-export。

## 5. 行为锁定测试锚点

后端现有锚点：

- `tests/backend/core/ops_contracts.rs`：backup filename/archive/file info、cleanup retention、job payload、Fernet key、cloud sync provider/prefix/object key/secret redaction 合同。
- `tests/backend/core/runtime_governance_contracts.rs`：`/api/backup/*` route Rust-owned、download content type、backup sync route ownership。
- `src/backend/http/backup_routes/auth.rs` 模块内测试：trusted header、Bearer step-up、wrong user/type/expired token。
- `src/backend/http/backup_routes/cleanup.rs`、`response.rs` 模块内测试：cleanup decision 和 response envelope 边界。
- `src/backend/http/backup_sync.rs` 模块内测试：S3 region endpoint 推断。

前端现有锚点：

- `tests/web/stores/user.test.ts`：user store cloud settings 读取、full update、disable 和 settings bundle 周边合同。
- `src/web/src/contracts/rustRouteOwnership.generated.ts`：backup route ownership 前端生成合同。
- 当前缺少专门覆盖 `AppCloudSyncPageBase.ts`、桌面 cloud sync tab、移动 cloud sync page 的 focused unit tests。

D7 behavior-lock 应补或确认：

1. 备份文件名与 path：拒绝路径穿越、Windows drive、ADS、control char、非 backup 前缀和非 zip/zip.enc 后缀。
2. Archive：拒绝 unsafe zip member，空 zip、无 data/、加密 key 缺失和 checksum mismatch 投影保持稳定。
3. Cleanup：record-first retention、missing record mark deleted、stray file 保留槽位和 audit event 语义保持稳定。
4. Job：job_type、schedule_expr、retention days/count、id、enabled/default 和 user-scope update/insert 合同保持稳定。
5. Cloud sync：provider allowlist、endpoint SSRF 防护、metadata IP denylist、local allowlist 例外、prefix/object key、secret redaction 和 provider status 502 投影保持稳定。
6. Backup auth：trusted header 只在可信 header 形态下跳过 step-up；Bearer session 必须提供 type/user/expiry 正确的 step-up token。
7. 前端：`ALL_APPLICATION_CLOUD_SETTINGS` key 列表、全选/半选/反选、enabled keys、false response 清空、enable/update/disable store 调用和桌面/移动按钮禁用状态保持稳定。

## 6. 安全审查门禁

D7 涉及文件 I/O、加密派生、外部 endpoint 和云存储 secret，必须在 behavior-lock、backend-shape 和 closeout 至少记录一次 security-reviewer 或等价独立安全审查证据。审查清单：

- File safety：backup filename/path/zip member 必须继续拒绝路径穿越、绝对路径、Windows drive、ADS、control char 和 unsafe archive member。
- Encryption：Fernet key 派生和 plaintext temp cleanup 不得改变；未配置 secret 时不得伪装成可解密。
- SSRF：cloud sync endpoint 不得允许 metadata host、loopback/private/link-local，除非命中显式 allowlist 且符合本地 HTTP 例外。
- Secrets：不得记录或返回 access_key、secret_key、password、token、Authorization、credential 等 secret；redaction 必须递归保留。
- Auth：backup create/delete/download/sync/cleanup 的敏感路径必须保留 Bearer + step-up 或 trusted header 语义。
- SQL：`backup_jobs` 必须按 user_id 隔离，backup metadata SQL 继续使用 bind 参数。
- External calls：测试不得对真实云服务发起 live upload；需要 mock/local server 或只测 config/signature/endpoint 合同。
- Frontend：不得在 toast/snackbar 或日志中显示 secret；UI 只传 enabled setting keys，不传业务设置具体值给 cloud sync 开关页。

本 ledger 切片只新增文档，不修改运行时代码；安全审查初始结论为“未引入新攻击面，后续 D7 行为锁定和拆分必须逐项复核上述合同”。

## 7. 切片执行顺序

1. `ledger`：提交本文件，记录结构 gate 快照、域边界、测试锚点和安全审查门禁。
2. `behavior-lock`：只新增/补强测试和必要测试 helper，不移动生产代码；覆盖 backup ops core、route auth、cloud sync endpoint/provider 和前端应用设置云同步行为。
3. `backend-shape`：拆 `core/ops.rs`、`http/backup_sync.rs`，必要时继续拆 `backup_routes/archive.rs`/`mod.rs`；保持 public facade、REST route、SQL、文件安全、加密、endpoint 防护和 response envelope 不变。
4. `frontend-shape`：拆应用设置云同步前端 shared base、桌面/移动页面和 settings store cloud helper；保留 import path、store action、路由、按钮文案、禁用状态和现有视觉布局。
5. `comment-pass`：按用户确认的注释标准补中文说明：导出函数、业务关键函数、复杂私有 helper 必须有中文说明；简单 getter、映射、事件转发不强制。
6. `governance-docs`：收紧 D7 后端结构 baseline，必要时更新前端 structure baseline；同步 `docs/PROJECT_OVERVIEW.md` 的备份与云同步现状。
7. `closeout`：确认 D7 PR 链、CI、merge、branch delete、结构 gate D7 清零和 security-review evidence，进度 cursor 推进到 D8/ledger。

## 8. 验收门槛

D7 完成前必须满足：

- D7 直接后端结构 baseline 对象清零或明显收紧：`src/backend/core/ops.rs` 和 `src/backend/http/backup_sync.rs` 退出 oversized baseline 或降到新的 facade 基线。
- D7 前端拆分不新增新的 frontend structure failure；D10 shared 四项不得被 D7 吸收或误标为完成。
- `cargo fmt --all -- --check`、相关 backup/ops/runtime governance 测试、`cargo clippy --workspace --all-targets -- -D warnings` 和 `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 35` 通过。
- `Set-Location src/web; npm run lint:ci` 和 `npm run test:coverage` 通过，必要时补桌面/移动云同步页或 store 单测。
- 安全审查结论不含 blocker；如发现真实安全缺陷，必须拆出独立 fix/prerequisite slice，不能混入结构搬迁。
- PR CI 通过、自动 squash merge、来源分支删除、进度 JSON 与 ultragoal ledger 回写完成。

## 9. Behavior-lock 记录

D7 `behavior-lock` 切片新增测试范围：

- 后端 `src/backend/http/backup_sync/tests.rs` 锁定云备份同步 endpoint 安全合同：OSS/S3/COS/Azure provider host 匹配、非 HTTPS provider endpoint 拒绝、凭据/query/fragment 拒绝、metadata host 与 link-local/loopback 拒绝、显式 allowlist 对本地 HTTP 的例外、公开 HTTP 仍拒绝、WebDAV 本地 allowlist 和 provider-specific 必填字段校验。
- 后端保留 `backup_sync.rs` public/private 行为不变，只把原模块内 region 测试外置到 `backup_sync/tests.rs`；生产上传、签名、endpoint、防 SSRF 逻辑没有移动。
- 后端 `ops_contracts` 和 `runtime_governance_contracts` 继续锁定 backup filename/archive/file info、cleanup retention、job/encryption、secret redaction 与 `/api/backup/*` Rust ownership。
- 前端 `tests/web/views/base/settings/appCloudSyncPageBase.test.ts` 锁定应用设置云同步 catalog 与允许 key 合同一致、移动/桌面标记、初始同步 key 复制、全选/全不选/反选、分类半选、server settings 应用和 false response 清空。
- 前端 `tests/web/stores/user.test.ts` 补充 user store 云同步读取、full update payload、成功后刷新本地 synced keys 和 disable 后清空 synced keys 的合同。

本切片本地验证记录：

- `cargo fmt --all -- --check` 通过。
- `cargo test -p bill-analyser-http backup_sync` 通过，9 个 `backup_sync` 测试全部通过。
- `cargo test -p bill-analyser-core --test ops_contracts backup` 通过，3 个 backup core 合同测试全部通过。
- `cargo test -p bill-analyser-core --test runtime_governance_contracts backup` 通过，backup route ownership 合同通过。
- `node scripts/check-rust-backend-structure.mjs` 通过，`src/backend/http/backup_sync.rs` 较 baseline 减少 22 行。
- `git diff --check` 通过。
- `node scripts/check-backend-doc-map.mjs` 通过。
- `Set-Location src/web; npm run test -- --runTestsByPath ../../tests/web/views/base/settings/appCloudSyncPageBase.test.ts ../../tests/web/stores/user.test.ts` 通过，2 个 suite、14 个测试全部通过。
- `Set-Location src/web; npm run lint:ci` 通过，保留历史 `no-explicit-any` warnings，无 errors。
- `Set-Location src/web; npm run test:coverage` 通过，94 个 suite、39003 个测试通过，coverage gate 为 99.13% lines、91.48% branches。
- `cargo clippy --workspace --all-targets -- -D warnings` 通过。
- `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 35` 通过。
- `Set-Location src/web; npm run structure:check` 仍预期失败 4 项，均属于 D10 shared：`src/lib/services.ts`、`src/stores/index.ts`、`src/core/theme.ts`、`src/models/imported_transaction.ts`。

## 10. Backend-shape 记录

D7 `backend-shape` 切片完成后端备份与云同步结构拆分：

- `src/backend/core/ops.rs` 改为 public facade，原 `bill_analyser_core::ops::*` 导出名保持不变；备份文件名、archive 摘要、cleanup plan、job payload、Fernet key、云同步合同、user-data audit 和 report export 分别落到 `src/backend/core/ops/**` 功能文件。
- `src/backend/http/backup_sync.rs` 改为 cloud upload facade，原 `upload_backup_to_cloud`、`validate_sync_upload_config`、`CloudBackupUploadError` 和 `CloudBackupUploadResult` 对外合同保持不变；provider 配置、endpoint/SSRF 防护、body/hash、WebDAV/OSS/S3/COS/Azure 上传实现分别落到 `src/backend/http/backup_sync/**` 功能文件。
- 后端结构 gate 显示 `src/backend/core/ops.rs` 较 baseline 减少 870 行，`src/backend/http/backup_sync.rs` 较 baseline 减少 613 行，两个 D7 直接 oversized 对象已退化为 facade。
- 行为保持不变：REST route、public exports、备份文件安全、zip member 安全、Fernet 派生、云同步 endpoint allowlist、provider host 匹配、签名算法、secret redaction、provider status 502 投影和 user-data/report 合同均沿用行为锁定切片的测试锚点。
- 按用户确认的注释标准，本切片为导出函数、业务关键函数和复杂私有 helper 补充中文说明；简单 getter、字段映射和事件转发不强制补注释。

本切片本地验证记录：

- `cargo fmt --all -- --check` 通过。
- `node scripts/check-rust-backend-structure.mjs` 通过，`src/backend/core/ops.rs` 较 baseline 减少 870 行，`src/backend/http/backup_sync.rs` 较 baseline 减少 613 行。
- `node scripts/check-backend-doc-map.mjs` 通过。
- `cargo test -p bill-analyser-core --test ops_contracts` 通过，5 个 ops 合同测试全部通过。
- `cargo test -p bill-analyser-http backup_sync` 通过，9 个云同步测试全部通过。
- `cargo test -p bill-analyser-core --test runtime_governance_contracts backup` 通过，backup route ownership 合同通过。
- `git diff --check` 通过。
- `cargo clippy --workspace --all-targets -- -D warnings` 通过。
- `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 35` 通过。

## 11. Frontend-shape 记录

D7 `frontend-shape` 切片完成应用设置云同步前端结构拆分：

- `src/web/src/views/base/settings/AppCloudSyncPageBase.ts` 改为 shared facade，原 `ALL_APPLICATION_CLOUD_SETTINGS` 和 `useAppCloudSyncBase` 导出保持不变；catalog/type 下沉到 `src/web/src/views/base/settings/app-cloud-sync/cloudSettingCatalog.ts`，选择状态、全选/全不选/反选、分类半选和服务端 settings 应用逻辑下沉到 `useAppCloudSyncSelection.ts`。
- 桌面 `AppCloudSyncSettingTab.vue` 保留 template、style、按钮文案和原路由入口；加载、启用/更新、禁用和 snackbar 成功/错误投影下沉到 `src/web/src/views/desktop/app/settings/tabs/app-cloud-sync/useDesktopAppCloudSyncActions.ts`。
- 移动 `ApplicationCloudSyncSettingsPage.vue` 保留 Framework7 template、style、路由和现有视觉布局；加载、启用/更新、禁用、toast、loading overlay 和 `routeBackOnError` 下沉到 `src/web/src/views/mobile/settings/app-cloud-sync/useMobileAppCloudSyncActions.ts`。
- `src/web/src/stores/setting.ts` 保留 Pinia store 导出名与 action 名；应用设置云同步序列化、云端 setting 反序列化、增量写回和 synced key map helper 下沉到 `src/web/src/stores/setting/cloudSync.ts`。
- 新增 `tests/web/stores/settingCloudSync.test.ts` 锁定 cloud sync helper 合同：enabled map 判定、root/nested setting 序列化、typed value 应用、synced key map 和仅已启用 key 才增量写回。
- 新增功能文件体量：`cloudSettingCatalog.ts` 93 行、`useAppCloudSyncSelection.ts` 108 行、桌面 action 68 行、移动 action 81 行、`stores/setting/cloudSync.ts` 190 行；原 facade 文件明显缩小且未新增 D7 前端 structure failure。
- 行为保持不变：按钮文案、禁用状态、桌面/移动路由、catalog key 集合、全选/全不选/反选、分类半选、settings store action 名、full update payload、disable 清空 synced key、toast/snackbar 文案和现有视觉布局均沿用 behavior-lock 测试锚点。

本切片本地验证记录：

- `Set-Location src/web; npm run test -- --runTestsByPath ../../tests/web/views/base/settings/appCloudSyncPageBase.test.ts ../../tests/web/stores/user.test.ts ../../tests/web/stores/settingCloudSync.test.ts` 通过，3 个 suite、18 个测试全部通过。
- `Set-Location src/web; npm run lint:ci` 通过，保留历史 `no-explicit-any` warnings，无 errors。
- `Set-Location src/web; npm run test:coverage` 通过，95 个 suite、39007 个测试通过，coverage gate 为 99.13% lines、91.48% branches。
- `Set-Location src/web; npm run structure:check` 仍预期失败 4 项，均属于 D10 shared：`src/lib/services.ts`、`src/stores/index.ts`、`src/core/theme.ts`、`src/models/imported_transaction.ts`；D7 前端入口未新增 failure。

## 12. Comment-pass 记录

D7 `comment-pass` 切片按用户确认的注释标准补齐中文说明：导出函数、业务关键函数、复杂私有 helper 必须说明；简单 getter、字段映射和事件转发不强制。

- 后端 `src/backend/db/backup_postgres.rs` 补充备份记录、备份任务、审计日志和 Postgres 行转换/元数据合并 helper 的中文说明。
- 后端 `src/backend/http/backup_routes/**` 补充认证、step-up token、归档创建/加密/解密/元数据、审计脱敏、创建/下载/删除/清理/同步/任务响应、handler、payload、response 和 runtime DB bridge helper 的中文说明。
- 后端 `src/backend/core/ops/helpers.rs` 补充 ops 合同字段解析、secret redaction 和敏感 key 识别 helper 的中文说明。
- 后端 `src/backend/http/backup_sync/body_hash.rs` 与 `config.rs` 补充 hex 编码和可选 provider 配置读取说明。
- 前端 `src/web/src/stores/user/cloudSettings.ts`、`src/web/src/stores/setting/cloudSync.ts`、`AppCloudSyncPageBase` 拆分后的 shared/desktop/mobile composable 补充云同步加载、启用/更新、禁用、全选/反选、服务端 settings 应用和 synced key map 刷新说明。
- 本切片只增加注释和 ledger 文档，不修改运行时代码、REST/API 合同、数据库 schema、测试逻辑或 UI 视觉布局。

本切片本地验证记录：

- D7 注释缺口扫描通过：`src/backend/db/backup_postgres.rs`、`src/backend/http/backup_routes/**`、`src/backend/core/ops/**`、`src/backend/http/backup_sync/**`、`src/web/src/stores/**cloudSync**` 和应用云同步 composable 范围内，`pub`/`pub(super)`/`export function`/返回给页面的关键函数前 8 行均有中文说明；简单 getter/映射/事件转发不作为强制项。
- `cargo fmt --all` 通过。
- `cargo fmt --all -- --check` 通过。
- `node scripts/check-rust-backend-structure.mjs` 通过，保留 D7 facade 行数减少 WARN：`src/backend/core/ops.rs` 减少 870 行，`src/backend/http/backup_sync.rs` 减少 613 行。
- `node scripts/check-backend-doc-map.mjs` 通过。
- `git diff --check` 通过。
- `cargo test -p bill-analyser-core --test ops_contracts` 通过，5 个 ops 合同测试全部通过。
- `cargo test -p bill-analyser-http backup_sync` 通过，9 个云同步测试全部通过。
- `cargo test -p bill-analyser-core --test runtime_governance_contracts backup` 通过，backup route ownership 合同通过。
- `cargo test -p bill-analyser-http backup_routes` 通过，7 个备份路由认证/响应/清理测试全部通过。
- `Set-Location src/web; npm run test -- --runTestsByPath ../../tests/web/views/base/settings/appCloudSyncPageBase.test.ts ../../tests/web/stores/user.test.ts ../../tests/web/stores/settingCloudSync.test.ts` 通过，3 个 suite、18 个测试全部通过。
- `Set-Location src/web; npm run lint:ci` 通过，保留历史 `no-explicit-any` warnings，无 errors。
- `Set-Location src/web; npm run structure:check` 仍预期失败 4 项，均属于 D10 shared：`src/lib/services.ts`、`src/stores/index.ts`、`src/core/theme.ts`、`src/models/imported_transaction.ts`。
- `cargo clippy --workspace --all-targets -- -D warnings` 通过。
- `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 35` 通过。
- `Set-Location src/web; npm run test:coverage` 通过，95 个 suite、39007 个测试通过，coverage gate 为 99.13% lines、91.48% branches。

## 13. Governance-docs 记录

D7 `governance-docs` 切片完成备份与云同步域结构治理收口：

- `scripts/rust-backend-structure-baseline.json` 移除 `src/backend/core/ops.rs` 与 `src/backend/http/backup_sync.rs` 两个历史 oversized 豁免项；这两个文件在 D7 backend-shape 后已退化为 facade，当前行数低于 600，不再需要结构债 baseline 保护。
- `docs/PROJECT_OVERVIEW.md` 已在 D7 backend/frontend shape 中同步当前事实：后端 `ops.rs`、`backup_sync.rs` 为 facade + 功能子文件，前端应用设置云同步为 facade + 功能文件夹；本切片复核后不再制造纯措辞 churn。
- 前端 structure baseline 未调整；D7 前端拆分没有新增 oversized failure，当前剩余 structure failure 仍全部归属 D10 shared 终扫域。
- 本切片只修改结构治理 baseline 与域 ledger，不改运行时代码、REST/API 合同、数据库 schema、测试逻辑或 UI 视觉布局。

本切片本地验证记录：

- `node scripts/check-rust-backend-structure.mjs` 通过，D7 已拆分后端 facade 不再出现在 oversized baseline 中。
- `node scripts/check-backend-doc-map.mjs` 通过。
- `Set-Location src/web; npm run structure:check` 仍预期失败 4 项，均属于 D10 shared：`src/lib/services.ts`、`src/stores/index.ts`、`src/core/theme.ts`、`src/models/imported_transaction.ts`。
- `cargo fmt --all -- --check` 通过。
- `git diff --check` 通过。
