# Bill Analyser 模块化架构重构 C0 基线

> 状态：`C0_ARTIFACTS_ASSEMBLED / C0_EXIT_BLOCKED`<br>
> 日期：2026-08-15<br>
> 工作流：Cyaness -> Cyanflow `flow / research / affected_surface`<br>
> 当前 HEAD：`345c095d7bd2421532a3c2f74184d408768b925c`<br>
> 范围：只读证据与治理工件；未修改业务源码、数据库 migration、配置或用户数据
> 上游共识：`docs/refactor/cyanflow-project-modular-architecture-consensus-2026-08-15.md`

## 1. C0 结论

底层确实存在结构性问题，但问题不是“Rust、REST 或 PostgreSQL 选错了”，也不需要换语言、拆微服务、引入第二数据库或先建全局框架。当前主要风险是同一业务事实跨 HTTP、core、DB JSON、SQL predicate 和前端 cache 被重复解释：

1. HTTP handler 仍知道连接池、SQL 查询顺序、数据库 row 与兼容 envelope。
2. Import 的 typed column、`preview_payload`、matching feedback、SQL signal function 和前端 selector 共同竞争事实解释权。
3. DB schema 有 version，但普通 preview row mutation 没把 version 暴露到 DTO，也没有 row-level expected version 合同。
4. session、decision、operation 和 learning lifecycle 的若干 `TEXT` 状态缺少数据库约束；child row 的 `user_id` 与 parent ownership 可独立漂移。
5. confirm receipt 仍嵌在 `import_sessions.metadata`，DB 层持有 HTTP status 与 success envelope。
6. 前端 Import 仍有 5,028 行核心 SFC、17 个生产文件使用弱类型，以及人工 revision 驱动的 annotation summary。

因此目标保持为模块化单体：业务深模块拥有 use case 与状态机，PostgreSQL 模块拥有唯一 writer 和事务，HTTP/前端只做 typed adapter。C0 没有发现需要推翻既定架构方向的新证据。

## 2. 证据口径与漂移修正

本基线只把当前工作区和可归因的本地证据写成“现状”。旧计划、旧日志和历史 memory 只能作为定位线索。

| 项目 | 2026-08-15 当前事实 | 处理 |
| --- | --- | --- |
| HTTP 层 SQLx | 8 个生产路径文件有词法命中；其中 `learning_rule_update.rs` 的 7 处均位于 `#[cfg(test)]` 后，运行态文件数为 7 | 基线记为 `runtime=7`，同时保留 `lexical=8/test-only=1`，避免假精确 |
| Core transport | 未直接 import Axum/http crate，但 `core/import_pipeline/responses.rs` 定义 `ImportV2RouteResponse { status_code, body }` | 这是语义上的 transport ownership，不能因关键词门禁为 0 就宣称无债务 |
| DB transport | 未直接 import Axum，但 confirm DB 类型/编排保存 `http_status`、`response_schema_version`、`success_envelope` | 记为 DB 持有 HTTP envelope，而不是“DB transport keyword=0” |
| Import weak DTO | 旧路径 `src/web/src/models/importPreview.ts` 已不存在 | 重新扫描当前 feature：17 个生产文件、67 行 `Record<string, unknown/any>` 命中，另有 13 行 `any` 词法命中 |
| 64 文件性能 | 历史日志仍有 Stage 1 `141,359ms`、Stage 2 `30,798ms`、缺少分类 `2,384` | 日志早于当前 HEAD 且无同次 browser/config provenance，只能作为症状证据 |
| 现有 binary | `target/debug/bill_http_server.exe` 存在，但时间早于当前 HEAD | 不归因到当前源码，不作为性能基线 |

机器可读 provenance：`docs/refactor/evidence/cyanflow-c0-64file-evidence-manifest-2026-08-15.json`。

## 3. 当前所有权矩阵

### 3.1 Route -> use case -> writer/table -> frontend

| 阶段/表面 | 当前入口 | 当前 use-case owner | 当前 writer/read owner | 表/权威状态 | 当前前端调用方 | 目标 owner |
| --- | --- | --- | --- | --- | --- | --- |
| C1 Ledger list | `GET /api/bills` -> `bill_routes/crud_handlers.rs::list_bills_handler` | 无独立 use case；handler 打开 runtime、构造 DB filters、调用 query 和 presenter | 只读：`db/bills/postgres_reads/query.rs::query_postgres_bills`；presentation 仍回到 HTTP | `bills`、`categories`，presenter 的关联读取 | `services.ts::getTransactions` -> `stores/transaction/listRequestCoordinator.ts` -> desktop `ListPage.vue` | `LedgerQueries::list(principal, query) -> LedgerEntryPage`；HTTP 只做 mapper |
| C1 missing category | `PUT /api/bills/import/v2/preview/:session_id/update`；`POST .../reclassify/:session_id` | HTTP 从 raw `Value` 构造 patch 并直接打开 import runtime | `db/import_staging/patch_payload_helpers.rs` 行锁、identity validation、typed columns + JSON 同写 | `import_preview_rows.category_id/version/preview_payload`、active `categories` | `ImportTransactionCheckDataTab.vue`、`useImportCheckDataAnnotations.ts`、`importPreviewReviewState.ts` | server issues + draft-only `ProvisionalIssueDelta`；删除长期 revision/cache truth |
| C2 PreviewStateKernel | preview page/index、SQL signal function、前端 signal adapter | Rust projector、SQL projection、前端 view model 并存 | DB query + JSON/SQL 函数读取；多个派生 reader | typed preview facts、`preview_payload.preview_matching_feedback`、decision owners | desktop/mobile 各自 mapping/filter | Rust canonical projector 唯一决定 typed snapshot；旧 SQL/前端仅 shadow oracle |
| C3 ImportStage2 | parse/dedup/reclassify/decision-group | `http/import_routes/stage_handlers/stage2_chain.rs` 与多个 handler/helpers | HTTP 仍有 SQLx；DB staging 执行 persistence | import sources/standard rows/preview/decision groups | `ImportDialog.vue` 与 check-data 流程 | `ImportStage2::evaluate` 深模块；HTTP 不加载规则、不写 SQL |
| C4 Typed Import API | preview/query/mutation/decision/confirm REST | core `ImportV2RouteResponse` + HTTP raw `Value` adapters | DB row DTO 不含 row version；mutation expected state 无 version | DB row `version` 单调增加，但普通 API 未形成 CAS | `lib/services/importPreview.ts` 与 feature 内 17 个弱类型文件 | Rust/TS typed DTO、row version、expected version、409 typed snapshot |
| C5 DB expand/contract | migrations + import session/readers/writers | 状态合法值散在 Rust、SQL 和 JSON | PostgreSQL 是唯一持久 writer，但约束不足 | session/decision/operation/learning `TEXT`；nullable member unique；receipt JSON | 无直接 owner；通过 REST 受影响 | additive constraints + benchmark-selected projection + single writer cutover |
| C6 Confirm | `POST /api/bills/import/v2/confirm` | HTTP 构造 command；DB `confirm_preview_to_bills_with_ack` 执行锁/CAS/plan/effects | DB confirm transaction 唯一业务 writer | bills/history/receipt/session/staging；receipt 在 session metadata | `ImportDialog.vue` 组装 raw confirm payload | `ConfirmIntent -> DB transaction -> ConfirmOutcome`；HTTP 独占 envelope |

### 3.2 Session 与 receipt 所有权

| 对象 | 当前事实源 | 当前 writer | 当前 reader | 风险 | 目标 |
| --- | --- | --- | --- | --- | --- |
| import session identity | `import_sessions(id,user_id,session_key)` | DB import session functions | HTTP session/preview/confirm handlers | child table 同时保存 parent id 与 `user_id`，但没有复合 ownership FK | DB 用复合 FK 或移除冗余 user id；session contract version 只在确需多合同并行时引入 |
| session lifecycle | `import_sessions.status TEXT` | 多个 DB staging/confirm 函数 | HTTP/DB 查询 | 缺少 CHECK；合法值由代码约定 | 在历史值盘点后 additive CHECK/受控类型 |
| preview row version | `import_preview_rows.version` | patch/decision/reclassify SQL 单调加一 | DB 内部和部分 decision API | 普通 row DTO 不暴露，普通 update patch 没 expected row version | response additive 暴露；当前 web cutover 后再 enforce CAS |
| confirm receipt | `import_sessions.metadata.confirm_receipt` | `db/import_staging/confirm/persistence.rs` | confirm replay 从 metadata 反序列化 | schema/fingerprint/terminal 唯一性主要靠应用逻辑；DB 存 HTTP envelope | DB 可验证 terminal receipt 合同；typed outcome 与兼容 envelope 分离 |
| confirm effects | 单 DB transaction | `db/import_staging/confirm/orchestration.rs` | terminal replay/正式 bills | 不能双跑；commit 后不能靠 flag 回滚业务 effect | shadow 只比较 pure plan/write-set；真实 effect 永远单路 |

## 4. 架构债务基线

| ID | 当前基线 | 证据 | C0 ratchet / 删除条件 |
| --- | --- | --- | --- |
| D01 HTTP runtime SQLx | 7 文件；另 1 文件仅 test region | `rg 'sqlx::query\|QueryBuilder' src/backend/http` + `#[cfg(test)]` 边界复核 | 新增运行态文件为 0；C1 Ledger handler 不新增；C3 migrated Import handler 降为 0 |
| D02 Ledger handler DB knowledge | handler 打开 runtime、传 pool、解释 DB error、调用 DB-backed presenter | `bill_routes/crud_handlers.rs:6-39` | C1 删除 handler 对 runtime/pool/DB row 的 knowledge 后通过 |
| D03 Core response ownership | 1 文件定义 status code + JSON body | `core/import_pipeline/responses.rs:2` | 不新增；C4/C6 迁完后离开 core |
| D04 DB HTTP envelope ownership | confirm command/orchestration 2 个核心文件持有 `http_status/success_envelope` | `db/import_staging/types/confirm_command.rs:22`、`confirm/orchestration.rs:30` | 不新增；C6 DB 只返回 typed `ConfirmOutcome` |
| D05 DB -> parsers dependency | 1 条直接 Cargo dependency | `src/backend/db/Cargo.toml:15` | 不新增引用；C3/C5 后共享 staging input 迁至 neutral typed module 并删除依赖 |
| D06 Import frontend weak types | 17 生产文件；67 行 Record 弱类型命中；13 行 any 词法命中 | 当前 feature + `lib/services/importPreview.ts` 扫描 | 新增为 0；触及文件只能下降；C4 migrated endpoint 为 0 |
| D07 Preview row CAS 缺口 | DB `version=version+1`，但 `ImportPreviewRow` 无 version，`ImportPreviewExpectedState` 无 version | `types/preview_query.rs:1-31`、`types/preview_patch.rs:105-117`、`patch_payload_helpers.rs:103-105` | C4 先 additive 暴露，再 web cutover；未获 breaking 批准前保留显式 legacy 分支 |
| D08 无约束 lifecycle TEXT | import session status、decision status、confirm operation status、feedback decision、learning signal/status 等 | migration `0001` 的 168/262/305/348/401/417 行 | C5 前先枚举真实历史值；additive validation；不得直接改 enum 破坏存量 |
| D09 child ownership 可漂移 | `import_sources/preview/decision/confirm` 同时引用 session 和 user，但独立 FK | migration `0001:183-205` 等 | C5 使用复合 FK 或删除冗余 user id，并用真实 PG 反例测试 |
| D10 nullable member UNIQUE | `UNIQUE(group_id, preview_row_id, standard_row_id, history_bill_id, member_role)` | migration `0001:271-281` | C5 迁到 member kind + id 或 family partial unique；先查重复/NULL 现状 |
| D11 receipt 约束不足 | receipt JSON 写入 session metadata | `confirm/persistence.rs:91-117` | C5/C6 增加 DB 可验证唯一性、schema、fingerprint/replay 约束 |
| D12 超大前端文件 | check tab 5,028 行；ImportDialog 1,774 行 | 当前物理行计数；已有 frontend structure ratchet | 不按行数机械拆；C1/C4 以状态所有权消失为验收，同时保持文件不增长 |
| D13 signal/issue 多重解释 | Rust projector、SQL JSON function/predicate、前端 adapter/cache 均解释 signal/issue | `core/import_pipeline/filter_index`、migration `0020`、`preview_predicates`、frontend check-data | C2 后 Rust projector 唯一写；旧 SQL/前端仅 read-only oracle，parity=0 后删除 |

### 4.1 当前 HTTP runtime SQLx 清单

1. `src/backend/http/router.rs`：PostgreSQL readiness `SELECT 1`，属于 infrastructure exception，但仍需显式 allowlist。
2. `src/backend/http/import_routes/mod.rs`。
3. `src/backend/http/import_routes/llm/preview_helpers.rs`。
4. `src/backend/http/import_routes/llm/rule_synthesis_helpers.rs`。
5. `src/backend/http/import_routes/stage_handlers/stage2_chain.rs`。
6. `src/backend/http/taxonomy_routes/account_handlers/helpers.rs`。
7. `src/backend/http/taxonomy_routes/audit_and_rules_helpers.rs`。

`src/backend/http/import_routes/learning_rule_update.rs` 只有 `#[cfg(test)]` 内 SQL，不能计入 runtime debt，也不能从词法扫描中静默消失。

## 5. Recognition-quality fixture

机器可读 fixture：`docs/refactor/evidence/cyanflow-c0-recognition-quality-fixture-2026-08-15.json`。

| 合同 | 当前覆盖 | 缺口/状态 |
| --- | --- | --- |
| exactly-one dedicated parser / no-match / conflict | Rust parser contract 已覆盖三态 | `covered` |
| generic unmatched + column mapping | detector no-match 与前端 mapping 请求分别存在 | 缺 HTTP -> standardized rows 贯通 fixture，`partial` |
| missing-category derived issue | DB identity/predicate 与前端清理 helpers 有测试 | 缺同 tick/ack/page/refresh/re-enter browser 锁，`partial` |
| manual category ownership | DB 单元 + 真实 PG 已覆盖 active mismatch 与失效分类边界 | `covered` |
| likely-transfer 双 membership | 前端能同时渲染 transfer + learning | 缺 canonical source -> backend counts/filter -> desktop/mobile 贯通 fixture，`partial` |
| transfer accept 字段保护 | accepted lifecycle 已测 | 缺分类、两账户和 manual fields 逐字段前后相等 fixture，`partial` |
| transfer reject 两行重算 | 当前 DB 原子拆两行并保留人工字段 | 非人工字段当前置空并标记 pending reclassification，未满足“同操作重算”，`partial` |
| unknown signal state fail-closed | 找到 canonical status allowlist | 未找到跨 Rust/SQL/frontend 的 unknown fixture，`missing` |

这里必须区分“transfer-only row 不匹配 learning filter”和“likely-transfer recommendation 必须同时有 transfer + learning evidence”。现有 `checkDataMatching.test.ts:507` 只证明前者，不能用来否定或证明后者。

## 6. C1 transition ledger

### 6.1 Ledger list boundary canary

| 字段 | 冻结内容 |
| --- | --- |
| Current owner | HTTP `list_bills_handler` 同时持有认证、DB runtime、filter lookup、query、error 与 presenter orchestration |
| Target owner | `LedgerQueries::list(principal, LedgerListQuery) -> Result<LedgerEntryPage, LedgerQueryError>`；实现留在 PostgreSQL bills query module |
| Old read | handler -> `postgres_filters_from_query(pool,...)` -> `query_postgres_bills(pool,...)` -> `page_to_frontend_postgres(pool,...)` |
| New read | handler mapper -> typed use case -> PostgreSQL query module -> typed page -> HTTP mapper |
| Writer | 无；此 canary 是只读，不新增 repository writer/trait |
| Compatibility | `/api/bills` path、query aliases、分页上限、amount cents、排序、success envelope 保持 |
| Rollback | 单独 feature/caller cutover；恢复旧 handler read path，不涉及 schema/data rollback |
| Delete gate | route contract fixture parity=0；handler 不打开 runtime、不接触 pool/DB row；新 abstraction 确实减少 knowledge，否则删除 canary abstraction |

### 6.2 Missing-category containment

| 字段 | 冻结内容 |
| --- | --- |
| Current owner | DB identity facts + JSON issue；前端 `useImportCheckDataAnnotations` revision/cache + `importPreviewReviewState` 又做一次解释 |
| Target owner | server snapshot issues 决定 filter/count/selection/confirm；draft-only `ProvisionalIssueDelta` 只影响当前 frame |
| Old read | effective transaction + cached summary by index + persisted matching annotation |
| New read | typed server row + local draft overlay -> effective row -> provisional delta；server ack 替换整行并清 overlay |
| Writer | DB patch transaction 仍是唯一持久 writer；C1 不改 schema、不创建第二套 issue writer |
| Compatibility | 现有 endpoint/envelope 保持；desktop/mobile 可分批读同一 adapter，但不能新增 raw JSON 推导 |
| Rollback | 前端 caller 级独立回退；DB/data 不变；不得通过额外 refresh/revision 作为“修复” |
| Delete gate | `importTransactionSelectionRevision` 与长期 annotation cache writer 为 0；合法分类后同 tick、ack、翻页、刷新、重进均消失；任何 browser case skip 即失败 |

### 6.3 C2-C6 后续 ledger

| 阶段 | Old read | New read | 唯一 writer | Compatibility/rollback | Delete gate |
| --- | --- | --- | --- | --- | --- |
| C2 PreviewStateKernel | SQL JSON function + Rust projector + desktop/mobile parser | typed canonical snapshot；旧路径 shadow-only | Rust projector 决定新/修改行的 projection | old reader 保留，parity diff 非 0 时回退 new reader；禁止 old writer | fixture + real shadow parity=0；desktop/mobile raw JSON signal reader=0 |
| C3 ImportStage2 | 三个 caller 各加载 context/规则并含 HTTP SQL | `ImportStage2::evaluate(ImportContextSnapshot, drafts)` | DB staging persistence；Stage2 只返回 typed drafts/effects | caller-by-caller cutover；资源超限 fail-closed | initial/reclassify/decision-group 只进深模块；相关 HTTP business SQL=0 |
| C4 Typed API/frontend | raw `Value`、weak TS DTO、无普通 row CAS | typed DTO + row version + expected version/409 | DB mutation command 单路 | additive response；legacy missing-version 分支可观测；breaking 需另批 | migrated endpoint weak type=0；当前 web 竞争 mutation 全发 version |
| C5 DB expand/contract | unconstrained TEXT、JSON signal read、receipt metadata | additive constraint + benchmark winner projection | canonical Rust/DB writer；旧投影只 compatibility | expand/backfill/shadow/cutover/contract；forward-fix/backup，不做假 DOWN | parity=0；public create/mutation/confirm 原子开放；legacy writer=0 |
| C6 Confirm | DB 返回 HTTP-shaped receipt/envelope | typed `ConfirmIntent/Outcome`；内部 pure plan/write-set | 单 DB transaction，真实 effect 禁止双跑 | terminal receipt 永远优先 replay；shadow 只比 plan | failure injection 无部分 effect；receipt replay/restore；active legacy session=0 后删旧 reader |

## 7. Architecture ratchet 设计

C0 只冻结规则，不在本切片修改 CI 脚本。实施 ratchet 时扩展现有 structure gates，而不是另建平行治理系统。

1. `http-sqlx-runtime-files`：当前 7；新增文件必须为 0。唯一基础设施 allowlist 是 readiness query，需精确到文件/符号。`#[cfg(test)]` SQL 单独计数。
2. `core-http-shaped-response-files`：当前 1；不得增加；C4/C6 后目标 0。
3. `db-http-envelope-files`：当前至少 2 个核心文件；不得增加；C6 后目标 0。
4. `db-to-parsers-direct-dependency`：当前 1；不得增加引用；C3/C5 完成后目标 0。
5. `import-weak-type-files/occurrences`：当前 `17/67`；新增为 0，触及文件必须不增长，C4 migrated surface 为 0。`any` 最终用 TypeScript AST/ESLint 口径替代词法数。
6. `preview-row-version-contract`：DB version 存在不能算通过；必须同时检查 Rust response field、mutation expected field、TS field 与 409 contract test。
7. `schema-invariant-ledger`：每个新 lifecycle TEXT、nullable multi-member unique、parent+user child、terminal receipt 变更必须附 migration ADR、合法值/存量查询和真实 PG rejection test。
8. 继续使用现有 `scripts/check-rust-backend-structure.mjs` 与 `src/web/scripts/check-frontend-structure.mjs`；baseline 只能下降，禁止用重生成 baseline 接受增长。
9. 每个 migrated endpoint 都要有 ownership assertion：handler 不开 runtime、不收 pool、不解析 DB row；application 不返回 HTTP status/JSON envelope；DB 不接收 raw web payload。

## 8. C0 Gate

| Gate | 状态 | 证据/阻断 |
| --- | --- | --- |
| Ownership matrix | PASS | C1-C6 当前/目标 owner、reader、writer、rollback/delete gate 已记录 |
| Architecture debt baseline | PASS | 当前 HEAD 重新计数，已区分词法、语义与 test-only |
| 64-file corpus identity | PASS | 64 文件、3,678,128 bytes、三类集合 SHA 已固化且不泄露文件名 |
| Historical symptom evidence | PASS | 日志 hash 与 Stage 1/2/缺少分类计数已固化 |
| Current reproducible performance baseline | BLOCKED | 服务 down；binary 早于 HEAD；没有当前 build -> corpus -> config -> browser 同次链 |
| Recognition fixture inventory | PASS | 8 个合同已登记，缺口显式 fail-closed |
| Recognition fixture executable completeness | BLOCKED | 6 个 scenario 仍 partial/missing，不能用于 C2 cutover |
| Browser marker | BLOCKED | 未发现可归因到当前 HEAD 与本 corpus 的 marker/trace |
| User/runtime config provenance | BLOCKED | 未保存脱敏 user/PostgreSQL/Weaviate/LLM snapshot |
| C1 authorization | NOT GRANTED | 本轮只授权 C0 evidence/governance，不进入业务实现 |

## 9. 下一动作

下一动作不是直接进入 C1，而是一个独立、可回滚的 `C0-runtime-baseline` 切片：从当前 HEAD 构建，启动仓库唯一运行入口，使用本 manifest 的 64 文件 corpus 做一次真实浏览器导入，记录同次 binary SHA、脱敏配置状态、阶段日志、browser marker 和 operable-first-screen 时间，然后停止服务并复核没有用户数据或配置被写入仓库。

该动作会启动服务、执行真实导入并产生数据库/本地运行态写入，超出本轮只读/治理授权。得到明确授权和可清理测试用户/session 方案前，C0 Exit 保持 `BLOCKED`，C1 不放行。
