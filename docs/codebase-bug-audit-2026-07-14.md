# Bill Analyser 全表面缺陷审计报告（2026-07-14）

## Executive summary

本次审计以 base_ref `3fe483207e3ddb073c17cb80732c5ee9ad6eb371` 为识别基线，确认 22 项独立缺陷：P0 0 项、P1 13 项、P2 8 项、P3 1 项。另有 2 个代码级候选及一组动态验证缺口留在 needs corroboration，未提升为 confirmed。最紧迫的风险集中在三处：撤销后的 access token 仍可访问业务 API；账单并发更新/删除可能造成永久余额漂移；前端刷新令牌队列存在旧令牌重放和 Promise 永不 settle。部署与运维层还存在 migration 依赖构建机绝对路径、PostgreSQL health 假绿，以及默认开发数据库 URL 静默兜底。交付工作树未修改业务源码、runtime 或 CI，相关缺陷仍未修复；仅按授权更新 current-state 地图与 README，文档类 finding 在本次 delta 中部分闭合时于对应条目标注。

“全表面”在本报告中的含义是：以 Git-tracked first-party inventory 为确定性输入，覆盖 Rust、前端、跨层合同、数据/并发、测试/CI、脚本/config/docs 六个域，并公开每域的 partial/blocked 状态与残余风险。它不表示 1,629 个 tracked path 均接受了逐行语义审计，也不表示没有未发现缺陷。

本任务不修改业务源码，也不修复业务、runtime 或 CI 缺陷；仅按授权更新 current-state 地图与 README。文档类 finding 在本次 delta 中部分闭合时，于对应条目标注 worktree status。后续方向均为 out-of-scope remediation suggestion。

## Scope、方法与证据等级

- 基线：main，HEAD 3fe483207e3ddb073c17cb80732c5ee9ad6eb371。
- 确定性 inventory：1,629 个 tracked path；Rust backend 524 个 .rs、5 个 backend manifests、18 个 PostgreSQL migrations；前端 301 个 .ts、195 个 .vue、92 个 .scss；21 个 Playwright path、35 个 Rust test .rs、118 个前端 test .ts。
- 方法：route/service 双向追踪、事务和 CAS 边界审阅、认证读写链对照、配置/脚本/文档交叉核验、非修改型静态命令和 targeted tests。
- 执行面披露：当前 Codex App 无可选的 luna high agent/model；本轮使用继承当前高能力模型的原生 subagent lanes，并按 high-reasoning 等价分工执行，未启动 outside-tmux OMX Team。
- Confirmed：至少具备 base_ref `3fe483207e3ddb073c17cb80732c5ee9ad6eb371` 的 path:line、可描述触发、expected/actual、根因与 fresh validation；对本任务改变的文档，同时记录 current worktree 状态与行号。动态环境未运行不自动降级，只要静态控制流已确定产品行为；对可见 UI 表现或极值入口仍不确定者降到 needs corroboration。
- 阴性抽样只约束抽样表面，不外推成全仓安全证明。

## Coverage ledger

| 域 | 状态 | 已覆盖方法与证据 | 未完成原因与 residual risk |
|---|---|---|---|
| Rust runtime | partial | 枚举 524 个 Rust path；审阅 manifests、HTTP/auth/health、bill CRUD、migration、import/money 高风险路径；运行 3 组 targeted tests、Clippy 与 Rust structure gate | 未运行 cargo test --workspace 和 llvm-cov；未逐文件证明 user scope、overflow、事务正确性 |
| Frontend | partial | 审阅 router/guard 装配 views、views 调 stores 和/或 services、stores 调 services 的主链；Axios 共享 interceptor，部分 view-local fetch 绕过该边界；对 import、auth、transaction/statistics、money DTO 抽样；lint:ci | 未运行 Jest coverage、build 或浏览器；未逐 store 审阅并发写回 |
| Cross-boundary | partial | 对 auth、import、bills、taxonomy、statistics、backup 代表路径做双端对照；确认 2 个 live-route mismatch 和 1 个虚假 ownership graph | 未生成全量 route matrix；相邻 legacy route 仍可能漂移 |
| Data & concurrency | partial | 审阅 bill update/delete 事务边界、未接线 CAS、confirm user-scope 与 checked_abs；静态确认三项 PostgreSQL 风险 | 无独立 PostgreSQL 实例；并发余额、revoke replay、outage health 尚无 live reproduction |
| Test/CI/governance | partial | 映射 Gitea steps、Jest whitelist、changed-line normalizer、Playwright、structure gates；执行 structure/doc-map/normalizer/JSON checks | YAML parser 被 active hook 阻塞；无 live Gitea run；coverage 和 E2E 未执行 |
| Scripts/config/docs | partial | 对 start/stop bind、默认 DB URL、AGENTS/README/PROJECT_OVERVIEW、Gitea skill 做锚点核验 | 文档只抽样稳定合同；未做全量 Markdown/link 语义审阅 |

## Confirmed findings

### BA-AUTH-001 — P1：logout/revoke 不会阻止既有 access token

- 严重度理由：撤销安全边界失效，所有通用认证业务 API 在 JWT 到期前仍可被盗 token 访问。
- Path:line：src/backend/http/auth.rs:110-192；src/backend/http/auth_routes/token_session_handlers.rs:101-124；src/backend/db/auth_postgres/sessions.rs:88-105。
- Trigger/repro：登录取得 token，调用 logout/revoke，再用同一 Bearer token 访问 bills/budgets/import/statistics。
- Expected / actual：预期同时校验 JWT 与 token_sessions.is_active；实际通用 resolver 只验签名、类型、exp、user_id，session_id 固定为 None。
- Root cause：撤销写侧已落 PostgreSQL，但通用访问校验仍是同步 config-only resolver。
- Blast radius：所有只经 resolve_user_id_from_headers 或 resolve_authenticated_user_from_headers 的业务路由。
- Confidence：high。
- Fresh validation：cargo test -p bill-analyser-http --lib auth::tests::bearer_auth_accepts_configured_hmac_algorithms -- --exact --nocapture，1 passed；测试无 session fixture 仍接受 token。
- Related/duplicates：logout/revoke seed 独立确认；不与 FE app-lock 合并，后者是本地凭据边界。
- 修复方向（任务外）：异步校验 token hash、user、active 与 expiry；补 revoke 后 replay 集成测试。

### BA-BILL-001 — P1：并发 bill update/delete 可造成余额永久漂移

- 严重度理由：正式账单与账户余额的持久一致性可被并发请求破坏，统计和净值继承错误数据。
- Path:line：src/backend/db/bills/postgres_reads/update.rs:11-77,83-178；src/backend/db/bills/postgres_reads/delete.rs:7-60；src/backend/db/bills/postgres_reads/mutation_prepare.rs:78-268。
- Trigger/repro：A、B 同时更新同一账单，或 update/delete、batch/single 交错并改变金额或账户。
- Expected / actual：预期原行读取、锁/CAS、identity validation、余额 delta 同事务；实际 prepare 在事务外，UPDATE/DELETE 无 version 条件，再用陈旧 old mutation 调余额。
- Root cause：正确形状的 FOR UPDATE + version helper 已存在但未接线。
- Blast radius：bills、来源/目标账户 balance_cents、批量 mutation、统计与净值。
- Confidence：high。
- Fresh validation：全仓检索只发现 import confirm 有 real-Postgres concurrency tests；普通 CRUD 无并发回归测试，CAS helper 同时被 Clippy 报 dead_code。
- Related/duplicates：与 BA-TOOL-001 共享“CAS helper 未接线”证据，但产品一致性与交付门禁是两个独立缺陷。
- 修复方向（任务外）：transaction-local prepare + row lock/CAS；增加 version conflict 合同及双事务测试。

### BA-DEPLOY-001 — P1：migration 运行时依赖构建机源码绝对路径

- 严重度理由：复制 release binary、multi-stage 容器或清理 checkout 后服务可能在监听前直接失败。
- Path:line：src/backend/db/postgres.rs:329-345；src/backend/http/bin/bill_http_server.rs:10-20,32-38。
- Trigger/repro：把二进制移动到不存在构建时 CARGO_MANIFEST_DIR 的机器运行。
- Expected / actual：预期 migration 嵌入 binary 或读取显式部署资源目录；实际 env!(CARGO_MANIFEST_DIR) 固化构建时绝对路径，运行时再读文件。
- Root cause：build-tree path 被当成 runtime asset location。
- Blast radius：release artifact、容器、安装包、CI artifact；HTTP 不监听。
- Confidence：high。
- Fresh validation：cargo test -p bill-analyser-db --lib postgres::tests:: -- --nocapture，22 passed；现有测试只证明当前 checkout 路径存在，未覆盖 relocated binary。
- Related/duplicates：合并 backend E3 与 governance GOV-06；单一 root cause。
- 修复方向（任务外）：使用 sqlx compile-time embedding，或显式资源目录并增加 relocated-binary smoke。

### BA-OPS-001 — P1：PostgreSQL 宕机后 /api/health 可假绿

- 严重度理由：readiness/监控可能持续向不可服务实例送流量。
- Path:line：src/backend/http/router.rs:50-61；src/backend/http/runtime.rs:151-180；src/backend/http/database_runtime.rs:31-64。
- Trigger/repro：服务启动后断开 PostgreSQL，同时保持 Weaviate healthy。
- Expected / actual：预期有超时的 pool acquire/SELECT 1 并返回 unhealthy/非 2xx；实际 PostgreSQL 仅检查 URL 是否配置，handler 仍可 200 status=ok。
- Root cause：health state 用配置存在代替连接探测。
- Blast radius：orchestrator readiness、监控、流量切换和故障诊断。
- Confidence：high。
- Fresh validation：cargo test -p bill-analyser-http --lib runtime::tests::health_details_expose_current_postgres_authority_status_only -- --exact --nocapture，1 passed；测试没有 DB fixture。
- Related/duplicates：合并 backend E4 与 governance GOV-05。
- 修复方向（任务外）：区分 liveness/readiness，加入短超时 PostgreSQL probe，readiness 失败返回 503。

### BA-HTTP-001 — P2：部分 bill route 将 malformed JSON 映射为 500

- 严重度理由：客户端输入错误污染 5xx SLO，并诱导无意义重试。
- Path:line：src/backend/http/bill_routes/payload_helpers.rs:117-129；对照 src/backend/http/taxonomy_routes/common_helpers.rs:88。
- Trigger/repro：向复用 required_json_object_from_body 的 bill route 发送截断 JSON，如单个左花括号。
- Expected / actual：预期 400；实际 serde_json 失败返回 INTERNAL_SERVER_ERROR 和 JSON parse error。
- Root cause：局部 helper 的错误映射与仓库其他 JSON helper 不一致。
- Blast radius：quick-add category 等复用该 helper 的 bill routes。
- Confidence：high。
- Fresh validation：当前工作区检索未发现该 helper 的 malformed-body 回归测试，并确认其他 helper 映射 400。
- Related/duplicates：JSON 500 seed 独立确认。
- 修复方向（任务外）：统一 bad_request mapping，补 router-level malformed JSON test。

### BA-TOOL-001 — P2：workspace Clippy 被 5 个 dead_code 错误阻断

- 严重度理由：仓库要求的 -D warnings 门禁 exit 1，阻断交付；其中四项还暴露 BA-BILL-001 的未接线实现。
- Path:line：src/backend/db/bills/postgres_reads/update.rs:83；mutation_prepare.rs:78,160,189；src/backend/db/import_staging/preview_decisions.rs:95。
- Trigger/repro：cargo clippy --workspace --all-targets -- -D warnings。
- Expected / actual：预期 exit 0；实际 exit 1、5 个 dead_code。
- Root cause：未接线 CAS/transaction helpers 和遗留 transfer-decision helper。
- Blast radius：workspace Clippy、CI/交付基线及并发修复可发现性。
- Confidence：high。
- Fresh validation：本轮 fresh Clippy 正好报告上述 5 项。
- Related/duplicates：与 BA-BILL-001 相关但不合并；seed 以一个 gate finding 处置。
- 修复方向（任务外）：接入所需 helper、删除真正遗留代码，不以 blanket allow 掩盖。

### BA-FE-AUTH-001 — P1：应用锁未保护 refresh token

- 严重度理由：本地操作者可在未解锁时铸造新 access token，绕过应用锁边界。
- Path:line：src/web/src/lib/userstate.ts:131-144,288-297；src/web/src/lib/services.ts:668-695。
- Trigger/repro：启用并锁定应用，读取 ebk_user_refresh_token，直接调用 POST /api/tokens/refresh。
- Expected / actual：预期 refresh credential 与 access token 同受锁状态保护；实际 refresh token 明文读写 localStorage，refresh 为 noAuth。
- Root cause：加密生命周期只覆盖 access token。
- Blast radius：桌面/浏览器本地 app-lock 威胁模型；不等同防御完全同源 XSS。
- Confidence：high。
- Fresh validation：当前源码双锚点确认存储与消费链，无 unlock gate。
- Related/duplicates：与 BA-AUTH-001 不同层、不同 root cause。
- 修复方向（任务外）：统一 token 锁生命周期；补锁定态 refresh 不可用的 browser/unit test。

### BA-API-001 — P1：前端 import-config 调用在 live Rust router 中不存在

- 严重度理由：保存、匹配、建议、更新、删除列映射模板均会落入 unknown API 404。
- Path:line：src/web/src/lib/services/importPreview.ts:403-473；src/web/src/views/desktop/transactions/import/ImportDialog.vue:780,823,866,965,1031,1045；src/backend/http/import_routes/mod.rs:206-360。
- Trigger/repro：打开 import dialog 并执行模板加载、匹配、建议或 CRUD。
- Expected / actual：预期 Rust 注册五类 method/path；实际只有前端调用和 ownership metadata，无 handler/route。
- Root cause：旧 import route package 移除后，前端和治理声明残留。
- Blast radius：桌面导入模板管理与自动匹配。
- Confidence：high。
- Fresh validation：当前 live router 全段核验无 /api/bills/import/configs* 注册。
- Related/duplicates：与 BA-GOV-ROUTE-001 同源邻接但症状不同；一个是 runtime 404，一个是治理假绿。
- 修复方向（任务外）：实现或移除合同；为全部 method/path 添加 router oneshot 与 UI flow test。

### BA-API-002 — P1：learning-rule update 仍调用已移除 legacy route

- 严重度理由：规则编辑保存稳定 404，而 list/toggle/delete 使用当前 route family。
- Path:line：src/web/src/lib/services.ts:1548-1550；src/web/src/views/desktop/pairingcenter/components/LearningCenterPanel.vue:308-320；src/backend/http/import_routes/mod.rs:350-359。
- Trigger/repro：在 learning center 修改 matchValue 或 learnedType 并保存。
- Expected / actual：预期 PUT /api/learning/rules/:id；实际前端 PUT /api/bills/import/learning-rules/:id。
- Root cause：单个 update service 遗留旧路径。
- Blast radius：学习规则编辑。
- Confidence：high。
- Fresh validation：live router 和 ocr_learning_handlers 当前字段合同与新路径互证。
- Related/duplicates：BA-GOV-ROUTE-001 的虚假 ownership 会遮蔽本项。
- 修复方向（任务外）：切换到 live route，增加 service URL + old/new router contract test。

### BA-GOV-ROUTE-001 — P1：route ownership graph 将不存在路由标成 verified

- 严重度理由：代码库图谱和 gate 对真实 404 给出假绿，直接遮蔽 BA-API-001/002。
- Path:line：src/backend/core/runtime_governance/ownership/import_ai.rs:231-299；src/web/src/contracts/rustRouteOwnership.generated.ts:99-106；src/backend/http/import_routes/mod.rs:206-360。
- Trigger/repro：生成/消费 route ownership manifest。
- Expected / actual：预期 verified entry 必须解析到 assembled Axum router；实际 import-config 和 legacy learning route 被标 RustOwnedVerified 但无注册。
- Root cause：手写 metadata 被视为权威，未与 live registration 双向校验。
- Blast radius：API 地图、治理 gate、前端生成合同和审查判断。
- Confidence：high。
- Fresh validation：metadata、generated graph 与 live router 三向比对。
- Related/duplicates：保留为独立治理 finding；runtime 症状见 BA-API-001/002。
- 修复方向（任务外）：从 live routes 生成 ownership，或加入双向一致性 gate。

### BA-FE-AUTH-002 — P1：刷新队列在新 token 落盘前恢复请求

- 严重度理由：refresh 成功后，被阻塞请求仍可能带旧 token 再次 401。
- Path:line：src/web/src/lib/services.ts:394-423,701-719；src/web/src/stores/token.ts:53-65。
- Trigger/repro：refresh 期间让多个认证请求进入 blocked queue。
- Expected / actual：预期先 updateCurrentToken，再恢复队列；实际 refresh service 同步调用 closures，store 仅在下游 .then 落盘。
- Root cause：Promise continuation 顺序与 token ownership 分离。
- Blast radius：所有并发 401/refresh 场景。
- Confidence：high。
- Fresh validation：当前控制流与 microtask 顺序双锚点确认。
- Related/duplicates：与 BA-FE-AUTH-003 同模块但 outcome/root cause 可独立修复。
- 修复方向（任务外）：由单一 refresh coordinator 原子提交 token 后再 resolve waiters；补 deferred-promise test。

### BA-FE-AUTH-003 — P1：refresh 失败会让 blocked Promise 永不 settle

- 严重度理由：网络失败、无 refresh token 或响应缺 newToken 时调用方可永久悬挂。
- Path:line：src/web/src/lib/services.ts:394-423,674-683,701-725。
- Trigger/repro：队列存在时让 refresh reject 或返回缺少 result.newToken 的 nominal response。
- Expected / actual：预期所有 waiter 确定性 reject；实际队列只有 resolve closure，失败分支清空数组或只取消 blocking，不调用也不 reject。
- Root cause：resolve-only callback queue，没有共享 reject/completion primitive。
- Blast radius：等待中的 API、loading 状态和 UI 操作。
- Confidence：high。
- Fresh validation：全部失败分支与 Promise 构造点静态闭环。
- Related/duplicates：与 BA-FE-AUTH-002 相关但不合并。
- 修复方向（任务外）：队列保存 resolve/reject 或共享 refresh Promise；补 timeout-race regression。

### BA-FE-STATE-001 — P2：transaction/statistics 可提交过期响应

- 严重度理由：快速切换筛选时旧请求可覆盖新状态，展示与 URL/filter 不一致。
- Path:line：src/web/src/stores/transaction.ts:768-842；src/web/src/views/desktop/transactions/ListPage.vue:553-593,758-760,1221-1226；src/web/src/stores/statistics.ts:1661-1773；src/web/src/views/desktop/statistics/TransactionPage.vue:378-410,569-684。
- Trigger/repro：请求 A 后改变筛选发请求 B，先 resolve B、后 resolve A。
- Expected / actual：预期只提交当前 generation；实际无 request key、abort 或 response/filter 比较，均无条件写 Pinia。
- Root cause：异步加载缺少 latest-request-wins 协议。
- Blast radius：交易列表/counts 和多个统计 dataset。
- Confidence：high。
- Fresh validation：principal loaders 与重复 reload 入口静态交叉确认。
- Related/duplicates：前端第 7 项，未外推到所有 stores。
- 修复方向（任务外）：generation/AbortController，按 dataset 增加乱序 deferred tests。

### BA-GOV-STRUCT-001 — P1：结构 ratchet 未进 CI 且当前已失败

- 严重度理由：已提交的复杂度/体量约束不阻止 merge，且当前 Rust 9 项、前端 7 项已越界。
- Path:line：scripts/check-rust-backend-structure.mjs；src/web/package.json；.gitea/workflows/ci.yml；scripts/run_ci_local.ps1。
- Trigger/repro：分别运行 Rust structure checker 和 src/web 的 npm run structure:check。
- Expected / actual：预期 ratchet 受 CI 强制且当前绿；实际两个命令 exit 1，CI/local CI 均不调用。
- Root cause：gate 脚本与 merge pipeline 断链，baseline/阈值持续漂移。
- Blast radius：import staging、preview、services 和大型 SFC 可继续无约束增长。
- Confidence：high。
- Fresh validation：Rust 9 个 >600 行文件；前端 7 个 baseline/threshold violation。
- Related/duplicates：structure seed 单一 finding。
- 修复方向（任务外）：先拆分或审核 baseline，再把两个 non-mutating gate 接入 CI。

### BA-GOV-COV-001 — P1：前端“global >90%”只覆盖 curated subset

- 严重度理由：大量业务源码不在 denominator，aggregate green 不能证明全前端覆盖合同。
- Path:line：src/web/package.json:20；src/web/jest.config.ts:11-37,52-86；AGENTS.md:122。
- Trigger/repro：在 collectCoverageFrom 外新增未测业务模块，现 gate 可保持 green。
- Expected / actual：预期全前端及改动业务代码 >90%；实际 coverage mode 限定 test list 与少量 import/budget source。
- Root cause：门禁阈值高，但输入集合是 whitelist。
- Blast radius：未列入的 store、service、route 和 UI 逻辑。
- Confidence：high。
- Fresh validation：当前 package/Jest config 静态互证；未用 lint pass 替代 coverage。
- Related/duplicates：coverage whitelist seed；与 changed-line gate 分开。
- 修复方向（任务外）：扩大 denominator 或明确分层阈值，并对 changed business files 单独验收。

### BA-GOV-COV-002 — P1：changed executable business-line >90% 未由 CI 执行

- 严重度理由：仓库硬性审计合同在 PR 合并路径中没有实际 acceptance gate。
- Path:line：AGENTS.md:130-132；.gitea/workflows/ci.yml；scripts/governance-normalizers.mjs。
- Trigger/repro：提交低覆盖业务 diff；CI 只跑 workspace Rust 35% 与 curated frontend coverage。
- Expected / actual：预期 PR diff + LCOV 计算并阻断 <=90%；实际 normalizer 只自测，未接当前 diff/LCOV。
- Root cause：计算 primitive 存在，但 orchestration 未接线。
- Blast radius：全部新增/修改业务可执行行。
- Confidence：high。
- Fresh validation：CI step mapping 与 normalizer invocation 当前源码核对。
- Related/duplicates：changed-line seed；不与 BA-GOV-COV-001 合并，前者是 denominator，后者是缺少 diff gate。
- 修复方向（任务外）：在 CI 生成 coverage 后以 base/head diff 执行 changed-line acceptance。

### BA-GOV-E2E-001 — P2：Playwright E2E 未受 Gitea CI 保护

- 严重度理由：desktop/mobile 关键跨层交互可在 Jest/build 通过时回归。
- Path:line：src/web/package.json:22；src/web/playwright.config.ts；docs/overview-testing.md:22；.gitea/workflows/ci.yml。
- Trigger/repro：引入仅浏览器交互可见的 regression 并走 Gitea CI。
- Expected / actual：预期安装/发现/运行 smoke；实际前端 CI 只有 lint、Jest coverage、build。
- Root cause：E2E runner 已配置但未进入 pipeline。
- Blast radius：auth shell、import、desktop/mobile navigation 与真实 browser contracts。
- Confidence：high。
- Fresh validation：package/playwright config 与 CI steps 静态互证；本轮 browser run blocked。
- Related/duplicates：Playwright CI seed。
- 修复方向（任务外）：引入可重复服务 fixture 和最小 smoke shard，再逐步扩展。

### BA-CONFIG-001 — P2：缺失 PostgreSQL env 时静默使用开发 URL

- 严重度理由：运维以为配置缺失会 fail-closed，实际可能连接本机开发实例或暴露默认凭据合同。
- Path:line：base_ref `src/backend/http/config.rs:29-30,99,308`；current worktree `docs/PROJECT_OVERVIEW.md:55` 已在本次 delta 同步为 current-state。
- Trigger/repro：删除或置空 BILL_ANALYSER_POSTGRES_URL 后启动。
- Expected / actual：生产或通用 runtime 对缺失数据库 URL 应 fail-closed，或只在显式 dev mode/profile 下启用开发默认；实际通用 config 对缺失或空值静默 fallback 到带开发账号的 localhost URL。
- Root cause：开发便利默认值进入通用 runtime config。
- Blast radius：部署配置错误、health 语义和错误数据库连接。
- Confidence：high。
- Fresh validation：default construction 与 env fallback 两处静态确认。
- Worktree status：PROJECT_OVERVIEW 地图已更新为当前 runtime 行为；runtime config 未修改，本 finding 仍未修复。
- Related/duplicates：DB default URL seed；会放大 BA-OPS-001 的配置存在假绿。
- 修复方向（任务外）：生产 fail-closed；开发默认仅由显式 dev mode/profile 启用。

### BA-OPS-002 — P3：custom backend bind 未被 health/stop 脚本一致尊重

- 严重度理由：自定义端口时启动健康探测错误，停止脚本遗留运行进程；影响局部运维而非核心数据。
- Path:line：start_backend.ps1:333-334；停止服务器.ps1:18；一键启动.ps1 的 custom-bind parsing。
- Trigger/repro：设置 BILL_ANALYSER_HTTP_BIND 为非 5000 并启动、检查、停止。
- Expected / actual：预期所有脚本解析同一 bind；实际 health 和 stop 固定 port 5000。
- Root cause：运行参数与辅助脚本常量分叉。
- Blast radius：本地/自托管 custom-bind 操作。
- Confidence：high。
- Fresh validation：三个脚本当前路径交叉核验；未修改 dirty 的 一键启动.ps1。
- Related/duplicates：bind/stop seed 合并为一个 finding。
- 修复方向（任务外）：共享 bind parser，按解析端口精确探测与停止。

### BA-DOC-001 — P2：canonical AGENTS 后端入口已过时

- 严重度理由：AI/开发者从权威入口进入不存在的旧 crates 路径，影响审计、修复与导航。
- Path:line：AGENTS.md:47；src/backend/http/Cargo.toml:47；docs/PROJECT_OVERVIEW.md:7。
- Trigger/repro：按 AGENTS 的“后端主入口”定位。
- Expected / actual：预期指向 src/backend/http/bin/bill_http_server.rs；实际指向 crates/bill-analyser-http/src/bin/bill_http_server.rs。
- Root cause：仓库布局迁移后 canonical guidance 未同步。
- Blast radius：跨工具 agent 和人工维护流程。
- Confidence：high。
- Fresh validation：Cargo bin target 与 PROJECT_OVERVIEW 提供两个独立当前来源。
- Related/duplicates：governance unique，未与 runtime map 合并。
- 修复方向（任务外）：修正 canonical AGENTS，保持平台 adapter 薄。

### BA-DOC-002 — P2：base-ref 的 README/AGENTS 元/分合同与 canonical Money 冲突（当前部分闭合）

- 严重度理由：金额单位错误会诱发 100 倍数据错误，且冲突位于权威协作说明。
- Path:line：base_ref `AGENTS.md:60`、`README.md:214-215`、`src/backend/core/primitives/money.rs:9-13`、`docs/PROJECT_OVERVIEW.md:55`；current worktree `AGENTS.md:60`、`README.md:214-215`、`docs/PROJECT_OVERVIEW.md:61`。
- Trigger/repro：按 base-ref README/AGENTS，或按 current worktree 的 AGENTS.md，实现新金额字段并将 backend core 当“元”。
- Expected / actual：预期说明 core/DB/API 使用整数分或显式 minor units；base-ref README 与 AGENTS 均声称后端核心存元。current worktree README 已改为整数分合同，AGENTS.md:60 仍保留旧表述。
- Root cause：旧金额迁移语义残留在顶层文档。
- Blast radius：parser、预算、统计、导入和 API DTO 的未来改动。
- Confidence：high。
- Fresh validation：Money primitive、StandardBill/current DTO 形状与 current `README.md:214-215`、`docs/PROJECT_OVERVIEW.md:61` 同向；`AGENTS.md:60` 仍反向。
- Worktree status：README 与 current-state 地图已在本次 delta 同步；AGENTS.md 不在授权修改范围内，本 finding 仅部分闭合并保持原计数。
- Related/duplicates：sampled formal transaction DTO 未发现实际 double conversion；本项是文档缺陷。
- 修复方向（任务外）：在获得规则文件修改授权后同步 AGENTS.md，统一为“外部输入/展示可用元，进入业务 DTO 前归一整数分”。

### BA-GOV-SKILL-001 — P2：Gitea cache skill 保留已移除 Python-era 验证

- 严重度理由：维护者会执行不存在的测试并偏离 Rust-only source-tree policy，造成错误 gate 结论。
- Path:line：.agents/skills/gitea-ci-cache-discipline/SKILL.md；scripts/trim_ci_caches.ps1。
- Trigger/repro：按 skill 执行 tests/test_gitea_workflows.py、tests/test_local_ci_scripts.py 或 .venv pytest。
- Expected / actual：预期 skill 指向 tracked、当前有效 gate；实际两个测试不存在，仓库无 tracked Python，脚本仍带 pip cache 参数。
- Root cause：Python-era guidance/tooling 清理不完整。
- Blast radius：Gitea workflow/cache 维护和跨工具 skill 执行。
- Confidence：high。
- Fresh validation：git ls-files '*.py' 为空；两个 prescribed path 不存在。
- Related/duplicates：governance unique。
- 修复方向（任务外）：按当前 Node/Rust/PowerShell gate 重写 skill，评估并清理真正无用的 pip cache 选项。

## Needs corroboration

### BA-MONEY-SUS-001 — 裸 i64::abs 对 i64::MIN 的极值风险

src/backend/db/bills/postgres_reads/mutation_prepare.rs:9,86 和 src/backend/db/import_staging/patch_payload_helpers.rs:39,130-138 等存在裸 abs；src/backend/db/import_staging/confirm.rs:725-727,1134-1142 已使用 checked_abs 并返回 domain error。触发候选是公开 API、恢复数据或坏 DB 行传入 -9223372036854775808。预期为稳定 400/domain error；潜在实际是 debug panic/500，或 release 下负极值继续传播。静态不一致明确，但尚未证明每个裸 abs 的公开可达路径、入口上限和 debug/release 动态结果，因此不计 confirmed。后续需 HTTP fixture、DB fixture 与 debug/release 极值测试。

### BA-FE-ROUTER-SUS-001 — desktop 缺 catch-all，但可见影响未动态确认

src/web/src/router/desktop.ts:276-284 无 /:pathMatch(.*)*；src/web/src/router/mobile.ts:360-363 明确 redirect。结构不对称已确认，但未知 desktop path 最终是空白、保留 shell、错误页还是其他 boot 行为依赖运行态，故不把“用户必见空白页”写成 confirmed。后续需 authenticated/unauthenticated router unit 与 browser E2E，再决定 home、login 或 404 合同。

### 动态 PostgreSQL / browser / runner gaps

- 未启动独立 PostgreSQL：BA-AUTH-001 的 revoke replay、BA-BILL-001 的双事务余额、BA-OPS-001 的 post-startup outage 尚无 live E2E。
- 未运行 browser：BA-FE-AUTH-001 的锁定态 refresh、BA-API-001/002 的实际 response envelope、刷新队列调度、desktop unknown route visible impact 均缺动态证据。
- 未运行 cargo test --workspace、cargo llvm-cov、npm run test:coverage、npm run build 或 npm run e2e；这些是验收缺口，不自动否定已由控制流确认的 finding。
- .gitea/workflows/ci.yml 的 parser-backed YAML 验证被 active Conductor hook 阻塞；Node fallback 因未安装 yaml module exit 1，未新增依赖。

## Rejected / negative sampled evidence

| 候选 | 处置 | 抽样证据与边界 |
|---|---|---|
| Backup path traversal | rejected in sampled routes | backup_filename 拒绝绝对路径、盘符、分隔符、冒号和控制字符；archive canonicalize 后检查 starts_with backup_dir；未证明 symlink race/zip bomb 安全 |
| Obvious redirect-based SSRF | rejected | 云备份和 LLM client 禁止 redirect，endpoint 有 scheme/credential/private/metadata/host checks；未覆盖 DNS rebinding、proxy、mapped IPv6 |
| Critical sampled SQL IDOR | rejected in sampled paths | bill/account/category/tag、import patch/confirm SQL 带 user scope；不是全仓形式化证明 |
| Confirm i64 extreme | rejected for confirm path | confirm 使用 checked_abs；BA-MONEY-SUS-001 只保留其他不一致路径 |
| Import preview paging mismatch | rejected | 前端 page/page_size/selected_only/preview_ids/signal 与 Rust aliases/response 对齐 |
| Formal transaction money DTO mismatch | rejected | TS cents contract、Rust mutation adapter 和 presenter 对齐，未见 sampled path 双重元/分转换 |
| Desktop/mobile auth guard mismatch | rejected | 两端均覆盖 unauthenticated、locked、already-authenticated 三态；不包含 unknown-route |
| doc-map / normalizer pass 推导全同步 | rejected inference | 两个脚本均 exit 0，但仅证明静态 anchor 与 self-tests，不证明全地图或 live changed-line gate |

## Seed disposition（12/12）

| Seed | Disposition | Finding |
|---|---|---|
| logout/revoke | confirmed | BA-AUTH-001 |
| bill update/delete concurrency | confirmed | BA-BILL-001 |
| migration path | confirmed；与 GOV-06 合并 | BA-DEPLOY-001 |
| PostgreSQL health probe | confirmed；与 GOV-05 合并 | BA-OPS-001 |
| malformed JSON 500 | confirmed | BA-HTTP-001 |
| default DB URL | confirmed | BA-CONFIG-001 |
| frontend coverage whitelist | confirmed | BA-GOV-COV-001 |
| structure ratchet | confirmed | BA-GOV-STRUCT-001 |
| changed-line coverage | confirmed | BA-GOV-COV-002 |
| Playwright CI | confirmed | BA-GOV-E2E-001 |
| custom bind/stop | confirmed；合并 health/stop 两症状 | BA-OPS-002 |
| five Clippy dead_code | confirmed | BA-TOOL-001 |

## Commands and fresh results

| Command/check | Result |
|---|---|
| cargo clippy --workspace --all-targets -- -D warnings | exit 1；5 个 dead_code，见 BA-TOOL-001 |
| node scripts/check-rust-backend-structure.mjs | exit 1；9 个 Rust 文件越过 600 行阈值 |
| npm run structure:check（src/web） | exit 1；7 个 baseline/threshold violation |
| npm run lint:ci（src/web） | exit 0；vue-tsc、E2E tsc、ESLint 0 errors，145 个既有 no-explicit-any warnings |
| auth bearer targeted Rust test | 1 passed；无 session fixture 仍接受签名 JWT |
| bill-analyser-db postgres::tests:: | 22 passed；仅证明 checkout migration path |
| health targeted Rust test | 1 passed；无真实 DB probe |
| node scripts/check-backend-doc-map.mjs | exit 0；只证明选定 anchors |
| node scripts/check-governance-normalizers.mjs | exit 0；只证明 normalizer self-tests |
| active adapter JSON parse | exit 0；仅语法 |
| YAML parse | blocked；Python 命令被 hook 拦截，Node fallback 无 yaml module |

一次非 --lib Rust test 因正在运行的 target/debug/bill_http_server.exe 被 Windows 锁定而无法替换；审计未按全局进程名强杀，随后以 lib-only targeted test 获得有效结果。该锁定不是产品 finding。

## Dirty / generated baseline

- 初始 dirty tracked files 仅 tests/backend/core/runtime_governance_contracts.rs 与 一键启动.ps1；初始 untracked、non-ignored 集合为空。
- 前者 SHA-256 为 EF9B3CE8ABFEE08D406BFF1BAD6C24C2C5498C29E8134497C30926F3F9EDB708，后者为 C8F261AACAF87C58205B4C16B1D5725E8FC808C4DB13950236DA553B96849B40；本次审计未编辑、恢复、暂存或清理它们。
- 既有 ignored/generated baseline：target 332,958 files、workspace.lcov 3,762,501 bytes、src/web/dist 114 files、src/web/coverage 507 files、node_modules 40,630 files、test-results 9 files、playwright-report 23 files、tests/.runtime 245 files。Cargo 定向测试只刷新既有 target cache；不能用 git status clean 推导 generated surface 未变化。

## 后续修复排序（不在本任务修复）

1. 先修安全与数据完整性：BA-AUTH-001、BA-BILL-001、BA-FE-AUTH-001/002/003。
2. 再修不可用合同：BA-API-001/002、BA-DEPLOY-001、BA-OPS-001，并让 BA-GOV-ROUTE-001 成为双向 gate。
3. 收紧交付门禁：BA-GOV-COV-001/002、BA-GOV-STRUCT-001、BA-TOOL-001、BA-GOV-E2E-001。
4. 修复局部 correctness 与运维：BA-HTTP-001、BA-FE-STATE-001、BA-CONFIG-001、BA-OPS-002。
5. 最后同步权威文档/skill：BA-DOC-001/002、BA-GOV-SKILL-001，并动态裁决两个 needs-corroboration 候选。

每批修复应另立 PRD、先补 regression tests，并按业务代码门禁运行完整 Rust/frontend coverage；本报告不授权任何源码、测试、配置或脚本修改。

## Residual risk

本轮没有对 524 个 Rust 文件、588 个前端 TS/Vue/SCSS 文件做逐行语义证明；parser 供应商格式矩阵、LLM/OCR live provider、Weaviate consistency、全量 user-scope、DNS rebinding、备份 TOCTOU/zip bomb、全 store race 和真实浏览器交互仍可能包含未发现缺陷。结论应被理解为“确定性表面已建账、关键风险已佐证、未完成部分可追踪”，而不是“代码库无其他 bug”。
