# Bill Analyser 项目总览

Bill Analyser 是一个多来源账单导入、智能去重、自动分类、预算与统计分析全栈系统。当前运行态由 Rust Axum `bill_http_server` 独立承载，业务 API 统一通过 `REST /api/...` 进入后端；未知 `/api/...` 由 Rust 返回结构化 404。

## 运行态入口

- HTTP 服务入口：`bill_http_server`（`src/backend/http/bin/bill_http_server.rs`）；它是唯一 HTTP 运行时入口。
- Weaviate 运维 CLI：`bill_weaviate_derived_index`（`src/backend/http/bin/bill_weaviate_derived_index.rs`），提供 health、schema bootstrap、outbox 处理和 rebuild。
- 认证合同桥接 CLI：`bill_auth_bridge`（`src/backend/core/bin/bill_auth_bridge.rs`）。
- 分类规则桥接 CLI：`bill_category_rule_bridge`（`src/backend/core/bin/bill_category_rule_bridge.rs`）。
- 运行态治理清单 CLI：`bill_runtime_manifest`（`src/backend/core/bin/bill_runtime_manifest.rs`）。
- 后端导航图：`docs/backend-map.md`
- 静态阅读入口：`docs/backend-map.html`
- 前端工程：`src/web`
- 当前 API 主链：`REST /api/...`
- HTTP bind：Rust 与 `start_backend.ps1`、`一键启动.ps1`、`停止服务器.ps1` 共享 IP/`localhost` + 非零端口解析合同；wildcard 监听使用 loopback 做健康探测，停止脚本只定位配置端口的监听 PID。
- 本地托管入口：`scripts/dev.ps1` 提供 `start/status/logs/stop/check`；后台启动复用 `一键启动.ps1`，以 manifest 记录 wrapper、监听 PID、路径和启动时间，停止时校验进程身份。`一键启动.bat` 是 PowerShell 7 双击启动入口，`一键结束.bat` 是双击托管停止入口；两者共享同一套日志与 manifest，停止默认保留 PostgreSQL 和 Weaviate 容器。

本地启动要求 PostgreSQL 与 Weaviate 同时可达。PostgreSQL 是唯一业务数据库；Weaviate 是导入 learning recall 与派生向量索引的必需服务。

## 后端分层

- `src/backend/http`：Axum 路由、认证上下文、上传处理、response envelope、structured error 与各业务 route facade。
- `src/backend/core`：金额、时间、分类、统计、预算、导入、matching、LLM/OCR、认证、安全校验与运行态治理合同。
- `src/backend/db`：PostgreSQL schema scripts、SQLx pool、user-scope repository、导入 staging、auth、budget、matching、taxonomy/settings、backup metadata 与 vector outbox。
- `src/backend/parsers`：微信、支付宝、工商银行、农业银行、建设银行、民生银行等 dedicated parser，以及 `RawBill` 到 `StandardBill` 的统一标准化。

解析来源标准化后端当前以 facade + 功能子文件组织：`src/backend/parsers/lib.rs` 聚合 parser public API，registry、DTO、parser tag、金额 serde、类型/金额归一和 post-process 分别下沉到 `registry.rs`、`types.rs`、`tags.rs`、`money_serde.rs`、`normalization.rs` 与 `post_process.rs`；`src/backend/parsers/dedicated/mod.rs` 聚合 dedicated parser dispatcher facade，来源 registry、filename/content hint、selection、candidate evidence 与选择 DTO 分别下沉到 `dedicated/registry.rs`、`hints.rs`、`selection.rs`、`evidence.rs` 与 `types.rs`；`src/backend/parsers/spreadsheet.rs` 聚合 spreadsheet preview/validation API，HTML 与 XLSX archive/XML/worksheet 的资源预算实现下沉到 `spreadsheet/**`。dedicated selection 对 HTML/XLS/XLSX 在每个文件内只创建一次受资源预算约束的 immutable prepared spreadsheet，所有候选 parser 只读复用同一份完整 rows，禁止各来源 parser 自行重新打开、解包或物化工作簿。该拆分不改变 parser registry 顺序、exactly-one/no_match/conflict 决策、金额元/分兼容边界、日期标准化或导入 staging 前的 `StandardBill` 合同。

导入预览后端当前以 facade + 功能子文件组织：`src/backend/db/import_staging.rs` 聚合 session、parser/source staging、preview query/predicate/selection/write/identity/confirm/materialization/row mapping/filter matching 等子文件；`src/backend/http/import_routes/stage_handlers.rs`、`preview_mutation_helpers.rs` 与 `stage_vector_recall.rs` 聚合 dedup/stage2、preview page/selection/update/reclassify、confirm、learning、vector recall 等子文件；`src/backend/core/import_pipeline.rs` 聚合导入预览 pipeline 合同，preview page query、filter index、matching payload、类型映射、route response DTO、response envelope 和 value helper 下沉到 `import_pipeline/**`。该拆分只改变物理结构，公开函数名、REST 路由、PostgreSQL 合同与导入语义保持不变。

身份设置后端当前以 facade + 功能子文件组织：`src/backend/db/taxonomy/postgres_reads.rs` 聚合账户、分类、分类规则、账户规则、标签、模板、row mapping 与 helper 子文件；`settings_bundle/postgres_import_export.rs` 聚合 settings bundle 导入编排、账户/分类/标签/模板/规则 section importer 与金额/ref helper；`src/backend/http/taxonomy_routes/account_handlers.rs` 聚合账户 CRUD、排序、余额同步、交易迁移和清理；`account_category_formatters/accounts_and_tags.rs` 聚合账户子账户、响应格式化、请求 payload、legacy 类型分类归一化、标签和模板格式化。该拆分不改变 REST route、user-scope SQL、settings bundle 顺序、legacy 恢复兼容或 cents/minor units 金额合同。

正式交易、周期和 matching 后端当前以 facade + 功能子文件组织：`src/backend/db/bills/postgres_reads.rs` 聚合正式账单查询、标签/账户/对账读取、创建、更新、删除、mutation 准备、标签替换、余额同步、筛选 SQL、row mapping 和 helper 子文件；`src/backend/core/adapters/transaction.rs` 聚合交易 DTO、写入 payload、批量 route 响应、图片、对账、前端投影和 value helper 子文件；`src/backend/db/recurring.rs` 聚合周期建议、账单绑定、检测、accept/reject、linked bills、row mapping、模板候选、日期调度和 helper 子文件；`src/backend/core/matching/session_learning.rs` 聚合 transfer、duplicate、investment、session candidate、candidate action 与 learning candidate 子文件；`src/backend/http/matching_routes.rs` 保留 router facade，并聚合 calendar/networth、recurring suggestions、matching candidates、manual pairs、reconcile history、payload、filter、value helper 和 response 子文件。该拆分不改变 REST route、PostgreSQL user-scope、金额 cents 字段、正式账单 CRUD、周期建议/绑定、matching candidate action 或 response envelope。

正式交易前端当前以入口 facade + 功能子文件组织：`src/web/src/stores/transaction.ts` 保留 `useTransactionsStore` facade，筛选/month-list 类型、receipt draft 归一化 helper 和交易列表/按月请求代次协调分别下沉到 `stores/transaction/types.ts`、`receiptDraft.ts` 与 `listRequestCoordinator.ts`；`src/web/src/models/transaction.ts` 保留 `Transaction`/`TransactionGeoLocation` facade，REST request/response、统计、overview 和空结果合同下沉到 `models/transaction/contracts.ts`；桌面交易列表、桌面编辑弹窗、批量手工录入、移动列表和移动编辑页保留原 `.vue` 入口，template/style 及无状态展示 helper 下沉到相邻功能文件夹。该拆分不改变 Pinia store 导出名、模型 import 路径、桌面/移动交易入口、投资/转账字段、图片 OCR、批量录入、latest-request-wins 或筛选 URL 合同。

身份设置前端当前以页面 facade + 功能文件夹组织：桌面账户、分类、标签列表页和移动账户编辑页保留页面入口，模板与样式下沉到相邻功能文件夹；账户编辑弹窗保留脚本入口并外置模板/样式；桌面分类列表的页面状态、表单、导入导出和操作回调收敛到 `useCategoryListPage.ts` composable。该拆分不改变按钮入口、导入导出控件、REST/store 调用、账户余额聚合、分类/标签编辑合同或现有视觉布局。

导入预览桌面前端当前以页面 facade + 功能子文件组织：`ImportDialog.vue` 保留导入流程编排和模板入口，`import-dialog/**` 承载导入源选择、进度条、配置匹配、preview page 查询参数和样式；`ImportTransactionCheckDataTab.vue` 保留预览表格入口和事件合同，`check-data-tab/**` 承载筛选菜单和批量动作；`checkDataMatching.ts` 与 `importPreviewIndex.ts` 作为兼容 facade 聚合 `check-data-matching/**` 和 `import-preview-index/**` 的信号视图模型、历史改写、筛选分组、查询过滤与 mapping helper。该拆分不改变 prop/emit、REST 调用、金额/身份合同或导入预览视觉布局。

前端运行链由 router/guard 装配并进入 desktop/mobile views；views 调用 Pinia stores 和/或 services，stores 也通过 services 访问后端。Axios 请求共享 `src/web/src/lib/services.ts` 的 API base URL、Bearer、response envelope、401 refresh 与请求阻塞 interceptor；若干 view-local 原生 `fetch` 调用绕过该 Axios interceptor 边界。桌面未知路径由 catch-all 回到受登录/解锁 guard 约束的首页：未登录进入 `/login`，已登录且解锁后进入 `/`。交易列表、月度列表以及分类、趋势、资产趋势统计分别以请求代次执行 latest-request-wins，旧成功或旧失败不会覆盖较新的页面状态。

运行时共享前端当前以 facade + 功能子文件组织：`src/web/src/lib/services.ts` 保留 axios/auth interceptor、通用 endpoint facade 和 `ApiResponsePromise` 兼容导出，HTTP 类型/response envelope helper 下沉到 `lib/services/http.ts`，导入预览、导入学习、matching candidate 和导入配置 endpoint 下沉到 `lib/services/importPreview.ts`；`src/web/src/stores/index.ts` 保留 `useRootStore` facade，跨 store reset 编排和 OAuth URL helper 下沉到 `stores/root/**`；`src/web/src/core/theme.ts` 保留 theme 兼容导出，theme 类型、基础 Vuetify/F7 色板、主题变体表和 preference/paired helper 分别下沉到 `core/theme/**`；`src/web/src/models/imported_transaction.ts` 保留 `ImportTransaction` 模型和响应接口，matching payload 归一化、dedup source id 与信号 helper 下沉到 `models/imported_transaction/matching.ts`；桌面和移动 router shell 继续由 `src/web/src/router/desktop.ts` 与 `src/web/src/router/mobile.ts` 承载，路由 guard、query prop 映射和主导航合同由 shared shell 行为锁测试覆盖。该拆分不改变 import path、default services API、Pinia root store action 名、ThemeType/preference/paired theme 合同、导入预览金额/目标账户 payload、路由 path/guard/query prop 或现有 UI 视觉。

## 数据库与健康检查

默认本地运行态使用：

- `BILL_ANALYSER_DATABASE_BACKEND=postgres`
- `BILL_ANALYSER_POSTGRES_URL=postgres://bill_analyser:bill_analyser_dev@127.0.0.1:5432/bill_analyser`
- `BILL_ANALYSER_WEAVIATE_ENABLED=true`
- `BILL_ANALYSER_WEAVIATE_ENDPOINT=http://127.0.0.1:8088`

`/api/health` 的 details 暴露 PostgreSQL 配置状态、脱敏 URL、route repository backend、Weaviate ready 状态、脱敏 endpoint、API key 是否配置与 collection prefix。readiness 会并行执行有界 PostgreSQL `SELECT 1` 与 Weaviate `/v1/.well-known/ready` 探测，任一依赖未配置、超时或不可用均返回 unhealthy/503；liveness 独立表示 Rust 进程本身仍可响应。

Rust HTTP 主入口在开始监听前会运行 PostgreSQL migrations。DB build script 把 migration manifest 与 SQL 内容嵌入 binary，运行时使用 embedded migrator，不依赖构建机源码绝对路径。`HttpShellConfig::from_env` 只在 `BILL_ANALYSER_RUNTIME_PROFILE=dev|development|local` 时允许缺失 PostgreSQL URL 并使用文档化本地默认值；其他 profile 缺失或空 `BILL_ANALYSER_POSTGRES_URL` 会在监听前失败关闭。

## API 与业务域

当前 HTTP route 覆盖登录、注册、邮箱验证、密码重置、refresh token、token session、logout、2FA、profile、user-data 统计与导出、交易清空、账户/分类/标签/模板主数据、分类规则、账户规则、设置包导入导出、账单列表与详情、手工交易、批量交易、账单导出、分类 quick-add/refresh、周期模板候选与绑定、matching pairs、净值快照、日历事件、预算 CRUD/导出/导入/执行/预测/快照、统计金额概览、分类统计、分类趋势、资产趋势、分类饼图、商户排行、Analyzer/insights、汇率读取与用户自定义汇率、备份文件 list/create/download/delete/verify/cleanup、备份任务与 cloud sync 元数据。

金额在数据库、Rust core/DB/HTTP DTO、REST API、前端模型、services、stores 与测试中统一使用整数分或显式 minor units 字段。后端 JSON 使用 `*_cents` / `*_minor_units`，前端 JSON 和模型使用 `*Cents` / `*MinorUnits`；预算、账户、交易、导入预览、统计、对账、matching、交易模板、周期模板与设置包都不再用裸 `amount`、`balance`、`sourceAmount`、`destinationAmount` 表达金额。元单位只存在于用户输入/展示格式化，以及 parser、OCR、LLM、原始导入源等外部边界；进入业务 DTO 前立即归一化为整数分。账单 single/batch 与导入 preview/confirm 在 HTTP 边界拒绝无法安全取绝对值的 `i64::MIN`，DB 层继续以 `checked_abs` 失败关闭；复用 raw-body helper 的账单路由把 malformed JSON 固定映射为不含 serde 细节的 400。

## 导入链路

导入链路由 Rust 完成 parser-first 上传、JSON parse、session/source/template/standard-row staging、dedup、转账 materialization、分类规则、recurring/learning/LLM decision、账户规则匹配、preview page、preview update/reclassify、confirm 与 cleanup。多文件 staging 与 preview row 落库使用分块批量写入；导入首屏只等待 preview row 和历史改写物化完成，decision group 证据在 preview 可操作后后台 best-effort 物化，不阻塞用户进入预览。preview 分页、筛选、排序、facets 和跨页选择由 PostgreSQL 在 `import_preview_rows` 上执行，跨页选择会保留“全部/有效/无效/需标注”子集语义，前端只保留当前页交互草稿。preview page metadata 的选择计数、无效选择计数、六类信号计数和完整选择快照在一次 core conditional aggregate 中计算，三类 facets 各自做有界聚合，不再为每个 core 指标重复扫描相同 preview scope；`selection_hash` 始终按当前用户、当前 session 的完整已选 preview id 有序快照计算，不受当前页或筛选缩小。前端表头与菜单筛选共用 trim/空值省略后的 canonical page query 签名，等价 watcher 触发只提交一次请求；服务端分页模式会在字段变化时立即缓存当前行草稿，迟到的分页响应先恢复草稿并把编辑器重绑到当前行对象，跨页选择 mutation 直接消费响应 metadata，不再额外请求同一页；“选有效/选无效/需标注”会在同一次 mutation 中先落库已浏览页面中影响有效性判定的草稿，再按更新后的数据库状态执行集合选择。三项 LLM/学习动作会先 flush 跨页选择差量：存在选择时携带同一非空 `selection_hash` 且只处理选择集，无选择时携带当前 canonical filters 的 `all_matching` 快照。预览信号筛选按信号列可见 family 计算：parser 只匹配没有平台重复、转账匹配、历史改写、learning 和 LLM 可见信号的 parser 行；`signal=transfer` 只匹配 `preview_matching_feedback.transfer` 这类转账匹配反馈，不把普通 `transaction_type=转账` 或仅有 transfer-like dedup 文本的行当作转账匹配信号；其他信号筛选分别匹配对应可见 family。预览分类的 canonical `category_id` 会贯穿 draft、payload、mutation、筛选、confirm 和最终 bill 创建；前端预览以当前用户可见分类表中的 canonical id 为合法性依据，不再因为本地类型缓存不一致把已存在分类显示为非法；用户明确写入当前用户 active 分类后，SQL 的 needs-review 与 missing-category 投影会尊重 `manual_fields.category_id` 归属，不会仅因修改前的类型投影再次提示缺少分类，缺失、不存在或 inactive 分类仍然无效。reclassify 携带非空 `preview_updates` 时只读取、重算、写回并返回目标 preview id；空更新继续保留整 session 重新分类语义，非分页前端按目标 preview id 原位合并局部响应。confirm 仍在事务内执行最终分类、来源账户和目标账户身份校验，`0`、不存在、非当前用户、inactive、无法归一的类型组合、转账同账户和 `__none__`/`__invalid__` sentinel 都不会写入正式账单。

预览筛选只包含六个 canonical family，顺序固定为 `parser`、`platform_duplicate`、`transfer`、`history`、`learning`、`llm`。Rust `ImportPreviewSignalFamily` 是权威定义，TypeScript 保留受合同测试约束的显式镜像。family membership、`matching.<family>` 投影、筛选命中和 `metadata.counts.signals` 计数复用同一语义；六个计数键始终存在，包括零值。审核状态按 family 收紧：`needs_review` 只属于 learning 的合法状态，并在轻量索引和界面展示中归一为 `pending`；LLM 与 transfer 不接受 `needs_review`。所有 family 遇到未知的非空审核状态都会 fail-closed，不进入对应筛选、索引或计数；状态为空但携带该 family 的有效 evidence 时，仍按现有证据规则判断信号。`recurring`、`reconciliation` 与 `identity_validation` 只作为行级辅助信号展示，不进入筛选 family、索引或六类计数。分页预览、完整预览和轻量索引都保留同一 matching 语义，其中完整预览不会丢失 `matching.llm`。

multipart 上传按文件顺序进入有界 blocking worker 执行 dedicated parser 检测，每个文件必须且只能命中一个 dedicated parser 才会进入标准账单解析。parser 标准化会把 `不计收支` 等仅代表来源侧中性记账的原始类型按金额方向降为收入或支出，并把原始文本保留在 `original_type`/parser evidence 中。stage2 phase precedence 为：同批/跨批 dedup 与历史账单匹配先形成预览基底，只有结构化转账匹配和历史账单匹配能自动把预览行判定为转账；分类规则、recurring、learning、vector recall、LLM 或账户规则都不能自行把非转账行自动改成转账，但预览页人工编辑仍可手动选择“转账”。stage2 在分类和账户规则前会清理无结构化转账匹配授权的自动 `转账` 预览态，转账分类规则只在 `preview_matching_feedback.transfer` 明确给出未拒绝的 `transfer`/`transfer_cross_batch` 候选时运行；reclassify 复用同一规则清理既有未确认预览行的转账专属分类、目标账户、隐藏转账载荷和自动反馈。普通 preview update payload 不能覆盖 server-authoritative `matching_feedback`，只能通过可编辑字段保存时携带的手动编辑标记保护人工选择的 `转账` 类型，且该标记不授予转账分类规则权限。非转账分类规则按优先级在收入、支出、投资三类规则池内统一匹配，命中任一当前用户 active 分类规则后以该分类的类型和 `category_id` 为准重写预览类型；内置分类兜底仍只按当前类型做保守补齐，不作为独立投资关键词系统。recurring 与可自动应用的非转账 learning projection 可在收入、支出、投资之间继续改写类型、分类或显式账户，账户规则最后消费稳定后的预览类型、转账/投资上下文和仍为空的账户字段。进入 preview row、preview patch、reclassify 和 confirm 前会用当前用户 active 分类/账户 map 做身份校验；无效身份会被清空并写入 `preview_matching_feedback.identity_validation`，需要人工 review，confirm 在同一事务内锁定 selected preview rows、重读身份与 review 状态、创建账单并更新 session。原始 parser/payment/category 文本可作为 review evidence 展示，但不会回填 canonical 分类或账户 ID；支付方式、POS 渠道和 `/` 等占位符只保留在支付方式或描述证据列，不作为原始账户名或分类名兜底进入非法身份列表。确定性规则链未命中时，通过有并发上限的 Weaviate 召回派生 learning 建议；当前用户没有 `import_learning_features` 源行时会跳过 Weaviate recall，避免对不存在的向量源逐行发起无效网络请求；非转账行只查询 income、expense、investment 三个 scope，转账保护行只查询 transfer scope。Weaviate metadata 本身不会触发自动改写。LLM preview recommendation 会要求并解析 `suggested_type` 与 `suggested_category_id`，分类应用优先以当前用户 active 分类 ID 为准，旧式文本建议只能在 active 分类表中解析成功后写入 canonical 字段；没有结构化转账授权的 LLM 转账类型或转账分类只记录为 LLM 信号，不会自动把预览行升级为转账。

未命中 dedicated parser 的 CSV/TXT、HTML 表格、二进制 XLS 或 XLSX 文件通过 Rust `/api/bills/import/preview` 进入通用列映射预览。请求内直接上传只读取受 HTTP body limit 和 10 MiB 硬上限约束的字节；引用阶段一临时文件时必须同时携带当前 `session_id`，后端先按 PostgreSQL 校验 session/user scope，再只允许 canonical path 落在当前用户和 session 的临时目录内。阶段一多文件 dedicated 解析按文件顺序进入有界 blocking worker，不并发展开多个压缩表格，并以 64 MiB 请求级预算累计标准账单字符串与容器物化内存；超限返回 413，不持久化部分结果。Stage 1 parse 响应通过标准 `Server-Timing` 暴露 multipart、parser、staging 与 total，Stage 2 dedup 响应暴露 dedup、intelligence、preview insert、response 与 total；这些指标只用于运行态观测和 C0 性能证据，不参与业务分支。文本预览只接受 `auto`、UTF-8、GBK 或 GB18030，未知编码和二进制伪装失败关闭；spreadsheet parser 是 HTML/XLS/XLSX 的 magic、输入字节、行列、单元格与物化预算权威，并把一次完整受限物化结果交给 dedicated selection 的所有候选 parser 共享，来源探测不得旁路预算或再次解包。XLSX 在解析前额外限制 ZIP entry 数、总展开字节、单 worksheet 字节与 dimension；OLE/BIFF XLS 在进入 calamine 前先受限读取 Compound File 的 Workbook stream，预扫描 Dimensions、单元格坐标、稀疏跨度与单元格计数，随后只读取首个工作表并再次校验行列、单元格长度、总单元格数与总物化字节，损坏或超预算文件失败关闭。扩展名为 `.xls` 但内容实际为 HTML 表格的银行导出仍按 HTML 受限解析。`parse_generic` 的预览只保留样本行，正式列映射则完整物化预算内的所有行，避免把 512 行预览样本误作完整导入数据；CSV/TXT 使用原文本的借用切片定位表头，不再先收集并复制全部行。前端按当前未匹配文件而不是最初文件选择配置格式，并把实际检测/模板编码贯穿预览、保存与 generic parse。

确认接口只解析并兼容归一 `ConfirmCommand`；所有写入都在 repository 的同一 SQLx 事务中执行。事务先按 user scope 对 `import_sessions` 执行 `FOR UPDATE`，再校验 terminal receipt、expected session version 与 canonical command fingerprint，然后应用 preview patch 和选择集、重读身份与 review 状态、执行历史改写 CAS、创建正式账单、校验 confirm-time effect boundary、写入回执，最后清理可重建的 staging 子表。当前 confirm 不接受未支持的 effect variant。历史写侧 operation 只接受 `update_history` 与 `merge_transfer_history`；旧名称只在 HTTP/DB 读取边界归一化。任一步失败都会整体回滚并保留可重试 staging。

确认成功后保留 terminal `import_sessions` 行，并在 `metadata.confirm_receipt` 中持久化协议版本、canonical fingerprint、确认前 session version、HTTP status 与 success envelope。回执不保存原始命令、fingerprint 源材料、账单正文、凭据或 acknowledgement secret。同 fingerprint 请求直接返回持久化回执，不依赖已清理的子表；不同 fingerprint 返回冲突。普通 restage、cancel、cleanup 与 metadata upsert 不能覆盖 terminal status 或回执；只有显式用户数据擦除可以删除它。

learning 与 LLM 审核通过现有 matching candidate action 入口进入各自独立事务。事务锁定 preview row，校验 expected state，持久化合法 lifecycle transition，并把 committed、idempotent、stale/conflict 结果返回现有 REST envelope。语义 no-op 编辑保留 pending LLM；接受或拒绝会退出 pending；实质编辑和 reclassify 通过统一 invalidation 路径清理依赖旧字段的 learning、history、LLM 与辅助 recurring 状态。confirm 不重复执行已由独立审核事务持久化的 effect。

LLM memory 查询按 `created_at DESC, id DESC` 返回，前端仅采用每个 preview 的最新事件；`clear` 和实质编辑产生的清理状态作为墓碑压过旧 accept/reject，不会映射成 pending 或在 watch/rehydrate 后复活。`metadata.counts.signals` 同时包含六个 canonical key 是当前 v2 preview 完整投影的协议标记；在该协议下整页 row 都以服务端为权威，包括 `matching` 中缺少 `llm` 表示信号已被清理的情况。当 preview row 已有明确 LLM 权威状态或刚由服务端刷新时，memory 不得覆盖；仅旧式、metadata 缺失或不完整且缺少明确权威字段的行允许兼容补全。

导入确认与 learning/LLM lifecycle 使用现有 `tracing` 输出结构化事件，覆盖开始、session lock、fingerprint 分类、validation、operation CAS、receipt、child cleanup、commit、rollback、replay/conflict 与 lifecycle transition。事件只记录 session correlation、请求/锁定版本、canonical operation、schema version、阶段和 outcome；不记录命令体、fingerprint 值或源材料、ack token、账单文本、凭据和交易敏感字段。前端 Jest 通过仓库内 Vue transformer 直接加载生产 SFC，并输出 `lcov.info` 与 `coverage-final.json`；组件合同测试覆盖变更 SFC，端到端导入交互沿用 `src/web/e2e` 的 Playwright root，不另建 runner。

导入 staging 写入前统一规范化账单日期文本：常规日期/时间、银行 Excel 日期序列，以及“日期 + 小数日时间”会转换为标准 `YYYY-MM-DD HH:MM:SS` 文本再进入 PostgreSQL 时间字段。

账户识别以 `account_rules` 为权威；前端账户 DTO 不包含别名字段，账户规则以账户、表达式、优先级、启停状态为当前合同。`account_rules` 当前迁移后 schema 不再包含旧 `account_role_scope`、`transaction_type_scope` 与 `field_scope` 持久化列；旧 payload、query 或 settings bundle 中携带这些字段时只在 API/导入边界产生兼容 warning 并被忽略。导入运行时按稳定后的账单类型、账户角色和上下文字段包决定匹配目标；非正则账户规则只匹配规范化后的完整 token，避免“本行”误匹配“本行POS”等渠道文本。账户 API 在 DTO 边界把恢复数据或旧式数据中缺失的账户类型、账户分类归一成当前前端分类合同；缺失分类会优先保留显式值，再按账户名称、图标和旧式类型推断，不用统一现金兜底覆盖已恢复账户。

条件选择携带的草稿与集合更新在同一个 session 锁事务内执行。单次 mutation 最多接受 500 个唯一 preview 草稿 ID，写入前批量校验 session scope 和当前用户 active 分类；越界、跨会话或事务内部分应用都会整体拒绝。

跨页选择会在发出 mutation 前先使此前在途的 preview page generation 失效，成功后直接消费 mutation metadata，不追加分页 GET；服务端分页 reclassify 刷新会替换旧 generation，迟到响应不能覆盖新状态。

## 分类与规则中心

分类识别使用 `category_rules` 规则表达式；`category_rules` 后端核心当前以 facade + `types`、`parser`、`matching`、`terms`、`tests` 子文件组织，分别承载表达式 DTO、语法解析、文本匹配、term 转义/连接符工具和模块内合同测试。分类规则列表只暴露能投影出非空表达式的规则，避免旧恢复行或坏数据在规则中心显示为空匹配式。账户识别使用 `account_rules` 表和同一表达式匹配器模型；`account_rules` 后端核心当前以 facade + `engine`、`tests` 子文件组织，`engine` 承载候选编译、scope 归一、上下文字段准备和匹配解释，测试文件锁定拆分前行为。账户规则 API 和设置包导出不再传播旧 role/type/field scope 字段；旧 payload 或旧 bundle 中的 scope 字段会被忽略并返回兼容 warning。REST API 覆盖账户规则 list/create/update/delete/reorder/test，以及分类规则 list/create/update/delete/reorder/defaults/test。设置包按当前 PostgreSQL 主链导出并导入/upsert `accounts`、`transactionCategories`、`transactionTags`、`transactionTemplates`、`scheduledTransactions`、`categoryRecognitionRules` 与 `accountRecognitionRules`；`transactionTemplates` 和 `scheduledTransactions` 在账户、分类、标签引用重映射完成后写入 `transaction_templates`，有效导出不再以 unsupported section warning 跳过。

桌面规则中心的“规则配置”包含分类识别、账户识别和周期识别三个二级页；分类识别按一级分类聚类展示，同一分类目标的多条规则表达式在同一行内分行呈现，分类规则的展示归一、分类兜底、payload 构造和目标分组逻辑位于 `pairingcenter/components/categoryRulesModel.ts`，页面组件只装配响应式状态和动作流程。账户识别按账户主分类、父账户/子账户分组展示，主分类行只呈现图标和规则数量，同一账户下的多条规则表达式在同一行内分行呈现，不重复显示规则名、优先级、匹配次数等运行态元数据；账户规则编辑、测试、删除弹窗位于 `pairingcenter/components/AccountRuleDialogs.vue`。学习中心面板的本地 tab 归一、学习规则筛选、状态色和匹配类型展示映射位于 `pairingcenter/components/learningCenterPanelModel.ts`；`LearningCenterPanel.vue` 保留 facade，模板、样式和 LLM 配置/候选状态下沉到 `pairingcenter/components/learning-center/**`。移动端通过 `/account/rules` 提供同样分组后的账户规则列表与紧凑编辑/测试入口。

学习中心的规则列表、分页计数、编辑、启停和删除以 PostgreSQL `import_learning_lifecycle` 为权威，并在每次读写时按当前用户约束；更新事务会锁定目标 lifecycle 行，分类改写只接受当前用户存在的分类，跨用户或不存在的规则不产生可观察写入。前端学习规则与 OCR 配置弹窗使用 Vue deferred Teleport，只有对应 DOM target 就绪时才挂载；tab loading 只跟随当前可见数据源，避免后台 tab 请求遮蔽当前页面。

## Matching

正式账单的转账候选与导入配对保持同一核心约束：同日、5 分钟内、金额绝对值在 1 分容差内且方向相反、来源账户不同，并排除已配对或已 suppressed 的账单。接受候选会合并或删除对应账单并写入审计；拒绝候选会写入 suppression，避免同一对账单再次提示。

智能去重核心当前以 facade + 功能子文件组织：`src/backend/core/smart_dedup.rs` 聚合去重 DTO、public engine 和 serde 合同，exact/same-batch、platform-bank、transfer pair、similar duplicate、split group、merge helper、文本/金额匹配和 reconciliation helper 分别下沉到 `smart_dedup/**`。该拆分不改变 source priority、同批/跨批去重、转账候选、拆单候选、reconciliation candidate 或金额序列化语义。

规则中心的配对总览展示转账配对和重复配对；投资相关识别规则保留在分类/规则治理链路中。

## 预算与统计

预算按月/季/年层级同步，删除主预算或最后一个子预算时会清理派生父周期预算。预算 CRUD、导出、导入、执行、预测和快照按同一层级、筛选、账户/标签上下文与名称更新语义执行，并通过 `amount_cents` / `amountCents` 写入 minor units 列。统计链路由 Rust 读取账户、分类、汇率和资产趋势数据，所有金额响应字段均使用显式 cents/minor units 字段，前端图表只在展示时格式化为元。

预算与统计后端当前以 facade + 功能子文件组织：`src/backend/db/budgets.rs` 聚合预算 DTO、payload normalization、execution/history helper 和 record helper 子文件，`src/backend/db/budgets/postgres_reads.rs` 聚合预算 PostgreSQL listing、mutation、import/export、execution、forecast、history、period sync、row mapping 和 value helper 子文件；`src/backend/db/statistics.rs` 聚合统计 category、asset/analyzer、exchange、range、loader 和 helper 子文件；`src/backend/core/statistics.rs` 聚合统计 range、category、asset、anomaly、calendar、breakdown、Analyzer、exchange 和通用 helper 子文件。该拆分不改变 REST 路由、PostgreSQL user-scope、预算 period 层级同步、统计 cents/minor units、汇率 fallback 或图表 response 合同。

预算与统计前端当前以页面/store facade + 功能文件夹组织：桌面预算 `ListPage.vue` 保留页面装配入口，template/style、预算页类型、金额筛选、历史周期 helper 和展示 helper 下沉到 `views/desktop/budgets/list/**`；统计 `stores/statistics.ts` 保留 `useStatisticsStore` facade，筛选类型、页面/交易列表 query 构建，以及分类/趋势/资产趋势请求代次与统一错误映射分别下沉到 `stores/statistics/types.ts`、`pageParams.ts` 与 `loaders.ts`；桌面统计 `TransactionPage.vue` 保留页面装配入口，template/style 和统计/交易跳转链接构建下沉到 `views/desktop/statistics/transaction/**`；`consts/currency.ts` 保留 `ALL_CURRENCIES`、默认货币与父账户占位符 facade，ISO 4217 静态表按代码范围拆到 `consts/currency/**`。该拆分不改变预算筛选、历史图、预测面板、导入导出、统计日期/图表筛选、latest-request-wins、图表 drilldown、导出 dialog、currency code/symbol/fraction 或原 import 路径。

## 认证与备份

通用业务 Bearer 认证先校验 access JWT 的算法、HMAC 签名、token 类型、过期时间与 `user_id`，再以 token hash、用户、active 状态和过期时间查询 PostgreSQL `token_sessions`；session 不存在或已撤销时拒绝请求，session authority 超时或不可用时返回 503，不回退到 JWT-only。PostgreSQL `token_sessions` 同时承载 refresh token session 查询与原子轮换、logout、session 列表和指定 session 撤销，因此 logout/revoke/refresh 轮换会立即阻止旧 access token 继续访问受保护业务路由。登录、注册、邮箱验证、密码重置、refresh、logout、2FA、profile、cloud settings、external auth、user-data statistics/export/clear 直接读写 PostgreSQL 表。敏感动作通过当前密码、操作密码或签名 step-up token 校验，并写入认证或业务审计。

注册请求支持可选 `defaultPackage`。缺失、`null` 或 `none` 保持旧注册行为；`standard_daily_v1` 会在注册事务内一次性写入面向中国大陆个人/家庭账单的默认账户、交易分类、分类规则和账户识别规则，并在注册响应中返回 `defaultSeed` 写入摘要。桌面端和移动端注册页都提供独立 opt-in 开关；未勾选的新用户和所有现有用户不会自动写入这套默认包。该默认包写入的数据仍走当前 taxonomy/settings 主链，可随设置包导出并导入到其他账号。

认证后端当前以 facade + 功能子文件组织：`src/backend/db/auth_postgres.rs` 聚合身份存在性校验、登录/profile 读取、审计事件、session、2FA、profile 更新、cloud settings、注册、默认包 seed 和 shared helper，模块内测试下沉到 `auth_postgres/tests.rs`；`src/backend/db/auth_registration_defaults.rs` 聚合默认包类型、支出/收入/转账/投资分类、账户、分类规则、账户规则和合同测试子文件；`src/backend/http/auth_routes/public_auth_handlers.rs` 聚合 CORS、注册、登录、refresh 和 API/MCP token handler 子文件。该拆分不改变 REST path、JWT/refresh/session 语义、2FA/recovery code hash、注册默认包内容、user-scope SQL 或对外错误字段。

认证前端当前以 facade + 功能文件夹组织：`src/web/src/lib/userstate.ts` 保留 credential/session facade，storage key、凭据格式识别、CryptoJS 加解密、旧明文 refresh 清理/迁移与解锁恢复下沉到 `lib/userstate/credentials.ts`；`src/web/src/stores/user.ts` 保留 `useUserStore` facade，basic info/localStorage、profile/avatar、cloud settings、user-data statistics/export 和 settings bundle 动作下沉到 `stores/user/**`；桌面 `UserBasicSettingTab.vue` 与移动 `UserProfilePage.vue` 保留页面入口，template/style 和展示 label helper 下沉到相邻 `basic/**`、`profile/**` 功能文件夹。应用锁启用时，localStorage 中的 access/refresh credential 都以当前锁状态加密，明文只存在于已解锁 sessionStorage；旧明文 refresh 仅在 access 解锁成功后迁移，损坏密文与部分解锁 fail-closed。该拆分不改变 Pinia store 导出名、userstate import path、localStorage key、profile 保存 payload、头像 URL、settings bundle/user-data 调用、桌面/移动路由或现有视觉布局。

备份运行态负责本地 zip/Fernet 文件 I/O、公开名生成、文件 list/create/download/delete/verify/cleanup、job list/save 与 cloud sync 元数据；记录、任务和审计元数据写入 `backup_records`、`backup_jobs`、`backup_audit_logs`。备份 zip 成员名只允许包内相对路径，并拒绝绝对路径、Windows 盘符、ADS 冒号、控制字符和 `..` 穿越。

备份与云同步后端当前以 facade + 功能子文件组织：`src/backend/core/ops.rs` 保留备份/用户数据/报表/同步合同 facade，文件名、archive、cleanup、job、crypto、user-data、backup sync 和 report export 规则下沉到 `src/backend/core/ops/**`；`src/backend/http/backup_sync.rs` 保留云上传 facade，config、endpoint 防 SSRF、hash/signing 和 WebDAV/OSS/S3/COS/Azure provider 上传下沉到 `src/backend/http/backup_sync/**`。该拆分不改变 REST route、公开导出名、文件安全、Fernet key 派生、endpoint allowlist、provider 签名、secret redaction 或 backup audit 语义。

应用设置云同步前端当前以 facade + 功能文件夹组织：`AppCloudSyncPageBase.ts` 保留 shared base 入口，catalog 与选择状态下沉到 `views/base/settings/app-cloud-sync/**`；桌面 `AppCloudSyncSettingTab.vue` 与移动 `ApplicationCloudSyncSettingsPage.vue` 保留模板和路由入口，加载、启用/更新、禁用、toast/snackbar 处理分别下沉到相邻 `app-cloud-sync/**` action composable；`stores/setting.ts` 保留 Pinia 导出名，应用设置云同步序列化、反序列化、增量写回和 synced key map helper 下沉到 `stores/setting/cloudSync.ts`。该拆分不改变按钮文案、禁用状态、selection 行为、store action 名、路由或现有视觉布局。

## LLM/OCR

LLM 临时配置保存在 Rust 进程内 user-scoped map，saved config、候选项、memory event 与 annotation sample 由 PostgreSQL 迁移表承载并按 API key 规则脱敏。provider 生成保留 allowlist/SSRF 防护、响应体上限、候选截断和 rate limit。文本生成运行时按协议适配 OpenAI Chat 兼容接口、Anthropic Messages 与 Ollama，并为 OpenAI、Claude、DeepSeek、通义千问、SiliconFlow、智谱 GLM、xAI、Google Gemini OpenAI 兼容端点、OpenRouter、Azure OpenAI 和自定义 OpenAI-compatible 配置提供明确预设；非内置自定义端点仍需通过 `BILL_ANALYSER_LLM_BASE_URL_ALLOWLIST` 授权。每条 saved config 可通过 user-scoped 测试接口执行不激活、不改写配置的最小真实请求，验证端点、凭据与模型组合并返回 provider、model 和耗时；provider 错误只投影经过凭据脱敏和长度限制的结构化摘要。LLM 推荐分为两层：未命中确定性规则的 preview 行只以日期、分单位金额、类型、交易对方、描述、支付方式和 parser ID 构成最小 JSON，模型只能从当前用户 active 分类与账户候选中返回 canonical ID；人工修改后的 preview 行按 canonical 分类和来源/目标账户分别分组，模型只归纳项目规则语法支持的关键词表达式，分类候选审核通过后进入 `category_rules`，账户候选审核通过后进入 `account_rules`。模型返回的账户 ID 在应用前必须命中当前用户账户 allowlist，分类 ID 继续由 PostgreSQL user-scope 解析。OCR recognition 默认 disabled，配置后可通过 Tesseract、本地 JSON OCR 或 LLM vision provider 返回结构化交易草稿。

LLM/OCR、导入学习与 provider 配置后端当前以 facade + 功能子文件组织：`src/backend/http/import_routes/multipart_and_ocr.rs` 聚合 route facade、multipart 输入、OCR provider 分发、runtime config 和错误投影，multipart 解析、网络 LLM vision OCR、本地 JSON OCR、Tesseract OCR 与 provider auth refresh 分别下沉到 `multipart_and_ocr/**`；`src/backend/core/import_learning.rs` 聚合导入学习 public facade，feature/hash/token、green/blue policy、model registry、LLM preview memory 和 route response helper 下沉到 `core/import_learning/**`；`src/backend/db/llm.rs` 保留 LLM PostgreSQL facade 与共享 row/helper，saved config 与 candidate review 写入下沉到 `db/llm/**`。Weaviate 派生索引和 outbox 合同仍由 `src/backend/core/weaviate_derived.rs`、`src/backend/db/vector_outbox.rs`、`src/backend/http/weaviate.rs`、`weaviate_recall.rs` 与 `config_weaviate.rs` 分层承载。该拆分不改变 REST route、PostgreSQL user-scope、provider SSRF/secret redaction、OCR 错误合同、LLM memory lifecycle、Weaviate authority 或任何 live external-provider 调用边界。

LLM/OCR 与学习中心前端当前以 facade + 功能文件夹组织：`LearningCenterPanel.vue` 保留页面入口、tab 编排和学习规则动作，`learning-center/LearningCenterPanel.template.html`、`LearningCenterPanel.scss` 与 `useLearningCenterLlmConfig.ts` 分别承载模板、样式、LLM config/candidate 状态和 browser autofill 防护；LLM 新建配置弹窗按 provider 之下的接入方式使用标准二级页签区分 API 与 OAuth，API 页只接收 API Key，OAuth 页只需选择凭据格式并粘贴一份完整 JSON，token endpoint、refresh headers/body/params 等刷新请求数据由后端从该 JSON 归一化并自动补齐，提交时不会携带另一模式的隐藏凭据；`OcrConfigPanel.vue` 保留 OCR 配置入口，模板与样式下沉到 `ocr-config/**`；`llmConfigHelpers.ts` 保留兼容导出 facade，类型、provider 选项、payload 构造和响应转换下沉到 `llm-config/**`。该拆分不改变 import path、props、settings bundle section 或 provider secret 脱敏合同。

## 文档入口

- [后端导航图](backend-map.md) — Rust 后端分层、请求生命周期、导入管线、repository 数据流和验证矩阵
- [总体架构](overview-architecture.md) — Rust 后端、前端、PostgreSQL 与 Weaviate 主链
- [后端模块](overview-backend.md) — HTTP/Core/DB/Parser 责任边界
- [API 路由](overview-api-routes.md) — `/api/...` route modules 和合同约束
- [导入链路](overview-import.md) — 三阶段导入、preview、learning、LLM/OCR
- [导入全链路验收场景](import-full-chain-scenarios.md) — parser-first、stage2、preview、confirm、PostgreSQL/Weaviate 必需运行态矩阵
- [数据库与数据流](overview-database.md) — repository、事务、user-scope 和 staging 生命周期
- [Weaviate derived index](weaviate-derived-index.md) — 必需向量派生索引配置、bootstrap、outbox 和 rebuild
- [Matching 域](overview-matching.md) — transfer/learning/recurring 配对候选
- [统计与汇率](overview-statistics.md) — 统计主链、汇率 REST 和用户数据管理
- [认证与安全](overview-auth-security.md) — 认证、2FA、token、backup、step-up 和审计
- [测试结构](overview-testing.md) — Rust、前端和治理测试基线

## 验证基线

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 35
.\scripts\dev.ps1 check
```

```powershell
Set-Location src\web
npm run lint
npm run test:coverage
npm run build
```
