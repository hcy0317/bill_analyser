# D9 parsers-source-normalization 结构债 ledger

本 ledger 固化 D9 parser/source normalization 域的当前结构边界。D9 继续复制 D1-D8 的“功能域 -> 功能文件夹”模板，先锁定 dedicated parser、来源识别、RawBill 到 StandardBill 标准化、金额/日期归一化和 fixtures/golden contracts，再拆后端 parser 结构、补中文说明、收紧治理基线并关闭域。

## 1. 覆盖范围

D9 覆盖：

- `src/backend/parsers/lib.rs`：parser registry、`RawBill`、`StandardBill`、parser tag 归一、金额/类型/描述标准化、`post_process_raw_bills` 和 serde 边界。
- `src/backend/parsers/dedicated/mod.rs`：dedicated parser dispatcher、auto detection、requested parser 分支、exactly-one/no_match/conflict decision、candidate evidence。
- `src/backend/parsers/dedicated/common.rs`：CSV/Excel/HTML 表格读取、编码解码、row map、金额/日期/时间 source-local helper。
- `src/backend/parsers/dedicated/{WeChat,Alipay,ICBC,CMBC,ABC,CCB}.rs`：各来源 parser 的 header 识别、行映射、来源字段保留和 source-local edge handling。
- `tests/backend/parsers/parser_contracts.rs` 与 `tests/backend/parsers/fixtures/parser_golden_contracts.json`：registry 顺序、结构拆分锚点、source label/tag、金额/日期/类型标准化、fixtures parse 和 conflict/no_match 合同。
- `tests/fixtures/import_samples/**`：微信、支付宝、工商银行、民生银行、农业银行、建设银行和 generic CSV fixture。

D9 不覆盖：

- 导入 preview staging、session、selection、confirm、stage2 分类/账户/learning/LLM/Weaviate 运行态，已分别归 D1/D8/D10。
- 交易 matching、正式账单 CRUD、分类规则、账户规则和周期规则，已归 D3/D4。
- OCR/LLM 视觉 provider、provider auth、secret redaction、LLM preview memory 和 Weaviate derived index，已归 D8。
- 全局 services/store/theme/router shell 和 frontend shared structure debt，归 D10。

## 2. 当前结构快照

- `src/backend/parsers/lib.rs` 当前 25 行，是 parser crate 的 public facade，只声明功能子文件并 re-export 既有 public API。
- `src/backend/parsers/{types,registry,tags,normalization,post_process,money_serde}.rs` 分别承载 `ParserInfo`/`RawBill`/`StandardBill` DTO、稳定 registry/source label、parser tag 归一、类型/金额归一、RawBill 到 StandardBill post-process 和 parser JSON 金额 serde 边界。
- `src/backend/parsers/dedicated/mod.rs` 当前 32 行，是 dedicated dispatcher facade，只声明来源 parser 与选择子文件并 re-export 既有 public API。
- `src/backend/parsers/dedicated/{registry,hints,selection,evidence,types}.rs` 分别承载 source parser registry、filename/content hint、requested/auto selection、candidate evidence 和 decision/result DTO。
- `src/backend/parsers/dedicated/common.rs` 当前 329 行，继续承载 CSV/Excel/HTML 表格读取和通用 source-local helper；本切片未进一步拆分，因为当前结构 gate 通过且 helper 仍服务单一 source-local 读取职责。
- 各 dedicated source parser 当前为小文件：Alipay 78 行、WeChat 113 行、ICBC 132 行、ABC 130 行、CCB 99 行、CMBC 149 行。
- `tests/backend/parsers/parser_contracts.rs` 当前 694 行，是 D9 behavior-lock/backend-shape 的主要锚点，已覆盖 registry、source split、golden contracts、fixtures、StandardBill JSON/serde 兼容、exactly-one detection、conflict/no_match 和 source-local edge cases。
- 当前 `node scripts/check-rust-backend-structure.mjs` 通过，D9 没有进入 Rust backend oversized baseline；后续 backend-shape 的目标是降低 facade 职责密度，而不是为了修复现有 structure gate failure。

## 3. 行为不变式

- parser-first multipart 上传必须保持每个文件 exactly-one dedicated parser 决策：`matched` 才进入标准账单解析，`no_match` 与 `conflict` 只返回 evidence，不写入标准行。
- auto detection 可以使用文件名和轻量内容 hint 缩小候选范围，但不能让 generic/column-mapped parser 走 dedicated dispatcher。
- `parser_registry()` 顺序保持 `wechat`、`alipay`、`icbc`、`cmbc`、`abc`、`ccb`，source label、class name、channel tag 和 supported extensions 受合同测试锁定。
- `RawBill` 只保留来源字段，不推断分类、账户、staging、dedup、learning 或 matching 决策。
- `StandardBill` 是 parser 层进入导入运行态前的最后草稿；`amount` 在 JSON 兼容边界仍按元数值序列化，Rust 内部使用 `Money` 保留分单位。
- `post_process_raw_bills` 负责日期文本标准化、金额符号归一、`不计收支`/投资/转账等 source-local 类型降级、description 聚合、parser tags 补齐和空日期/零金额过滤。
- parser 层不得写入数据库，不得调用账户/分类/learning/LLM/Weaviate，不得把 parser/payment/category 原始文本兜底成 canonical account/category ID。

## 4. 后续切片计划

- `behavior-lock`：补强 parser behavior contracts，优先覆盖 Excel serial date、日期 + 小数日时间、amount sign、`original_type`、generic reject、requested parser 和 conflict evidence。
- `backend-shape`：把 `lib.rs` 拆为 facade + `types`、`registry`、`tags`、`normalization`、`post_process`、`serde_money` 等功能子文件；把 `dedicated/mod.rs` 拆为 facade + `registry`、`selection`、`hints`、`evidence`、`types`；必要时把 `common.rs` 拆为 `csv`、`workbook`、`html`、`cells`、`values`。
- `frontend-shape`：D9 默认无直接前端实现；若发现 parser import UI 只读锚点需要调整，应记录为 D1/D10 shared lease，不在 D9 自行重构前端。
- `comment-pass`：按用户确认标准补齐导出函数、业务关键函数和复杂私有 helper 的中文说明；简单 getter、映射、事件转发不强制。
- `governance-docs`：同步 `docs/PROJECT_OVERVIEW.md` parser 当前结构事实，必要时收紧 parser 相关结构 baseline。
- `closeout`：汇总 D9 PR 链、CI、source branch 删除、parser behavior contracts、注释和 D10 移交边界。

## 5. 验收门槛

D9 完成前必须满足：

- parser behavior contracts 通过，且覆盖 exactly-one dedicated parser、no_match/conflict、requested parser、source label/tag、money/date/type normalization 和 repository fixtures。
- `src/backend/parsers/lib.rs` 与 `src/backend/parsers/dedicated/mod.rs` 退化为 facade 或明显降低职责密度；新增子文件保持功能域命名。
- 不修改 REST route、PostgreSQL schema、导入 staging、分类/账户/learning/LLM/Weaviate 行为或 UI 视觉。
- 金额单位复核：parser JSON 兼容边界保留元数值，进入业务 DTO 前仍通过 `Money`/cents 显式处理。
- 日期标准化复核：常规日期/时间、Excel 日期序列和“日期 + 小数日时间”仍进入标准 `YYYY-MM-DD HH:MM:SS` 文本。
- `node scripts/check-rust-backend-structure.mjs`、parser 相关 cargo tests、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 35` 在涉及业务代码的切片通过。
- PR CI 通过、自动 squash merge、来源分支删除、进度 JSON 与 ultragoal ledger 回写完成。

## 6. Ledger 切片记录

D9 `ledger` 切片只建立 parser/source normalization 域边界，不修改 Rust 生产代码、fixtures、测试或运行态行为。

本切片本地验证记录：

- `node scripts/check-rust-backend-structure.mjs` 通过，D9 没有新增或暴露 Rust backend structure failure。
- `cargo test -p bill-analyser-parsers --test parser_contracts` 覆盖 parser registry、source split、golden contracts、fixtures、exactly-one/no_match/conflict 和 dedicated source edge cases。
- `node scripts/check-backend-doc-map.mjs` 通过。
- `git diff --check` 通过。

## 7. Behavior-lock 记录

D9 `behavior-lock` 切片补强 `tests/backend/parsers/parser_contracts.rs`，继续只修改测试与本 ledger，不修改 parser 生产逻辑、fixtures、导入 staging、API、SQL 或 UI。

新增锁定范围：

- `post_process_normalizes_excel_serial_and_fractional_day_dates` 覆盖 Excel serial datetime 与“日期 + 小数日时间”进入 `YYYY-MM-DD HH:MM:SS` 的标准化，同时锁定收入/支出金额方向。
- `post_process_preserves_source_fields_while_normalizing_sign_and_tags` 覆盖投资/理财 source-local 类型降级、`original_type`/`original_category` 保留、channel 到 payment method 兜底、order id 到 transaction id 兜底、merchant/status 保留和 parser tag 归一。
- `dedicated_dispatcher_returns_decision_evidence_for_requested_parser` 覆盖 requested parser trim/lowercase、matched decision、selected parser、空 conflict group、稳定 reason 和 candidate evidence。

本切片本地验证记录：

- `cargo fmt --all -- --check` 通过。
- `cargo test -p bill-analyser-parsers --test parser_contracts` 通过，17 个 parser 合同测试全部通过。
- `cargo clippy --workspace --all-targets -- -D warnings` 通过。
- `node scripts/check-rust-backend-structure.mjs` 通过。
- `node scripts/check-backend-doc-map.mjs` 通过。
- `git diff --check` 通过。

## 8. Backend-shape 记录

D9 `backend-shape` 切片把 parser/source normalization 后端拆成 facade + 功能子文件，保持导出名、parser registry 顺序、dedicated exactly-one/no_match/conflict 决策、金额元/分兼容边界、日期标准化和导入 staging 前 `StandardBill` 合同不变。

本切片结构变化：

- `src/backend/parsers/lib.rs` 从混合 facade/DTO/helper 收敛为 public facade，`types.rs`、`registry.rs`、`tags.rs`、`normalization.rs`、`post_process.rs` 与 `money_serde.rs` 分别承接 DTO、注册表、tag、归一化、post-process 和金额 serde 职责。
- `src/backend/parsers/dedicated/mod.rs` 从 dispatcher 混合文件收敛为 facade，`dedicated/registry.rs`、`hints.rs`、`selection.rs`、`evidence.rs` 与 `types.rs` 分别承接来源 parser 注册、自动检测 hint、选择流程、候选证据和选择 DTO。
- 新增和迁移的导出函数、业务关键函数和复杂私有 helper 已补中文说明；简单 DTO 字段和直通 re-export 不强制逐项注释。

本切片本地验证记录：

- `cargo fmt --all` 已执行。
- `cargo fmt --all -- --check` 通过。
- `cargo test -p bill-analyser-parsers --test parser_contracts` 通过，19 个 parser 合同测试全部通过。
- `cargo clippy --workspace --all-targets -- -D warnings` 通过。
- `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 35` 通过，完整 Rust 工作区 coverage gate 生成 `workspace.lcov`。
- `node scripts/governance-normalizers.mjs changed-coverage --lcov workspace.lcov --diff .omx\ultragoal\evidence\d9-backend-shape-parser.diff --threshold 90` 通过，parser 源码 diff-line coverage 为 96.63%。
- `node scripts/check-rust-backend-structure.mjs` 通过，507 个 Rust backend 文件仍只保留 3 个既有 baseline entry。
- `node scripts/check-backend-doc-map.mjs` 通过。
- `git diff --check` 通过，仅输出 Windows 换行提示。

## 9. Frontend-shape 记录

D9 `frontend-shape` 切片不修改前端运行时代码。parser/source normalization 域的前端可见内容只通过导入预览读取 parser id、parser tag、source chain 和 matching summary 的只读投影；这些展示锚点已经归属 D1 导入预览域，剩余 shared services/store/router/model 结构债归 D10 终扫域。

本切片确认边界：

- D9 不拥有 `src/web/**` 前端实现，不重构 `ImportDialog.vue`、`ImportTransactionCheckDataTab.vue`、`importPreviewIndex.ts`、`checkDataMatching.ts` 或 `models/imported_transaction.ts`。
- parser 前端信号展示继续消费后端已有 `preview_parser_id`、`preview_parser_tags` 与 matching parser payload，不新增 parser API、store action 或 UI 视觉变更。
- 若后续发现 parser 展示锚点需要结构调整，应通过 D1 导入预览域或 D10 shared lease 执行，不在 D9 backend parser 域吸收前端结构债。

本切片本地验证记录：

- `cargo test -p bill-analyser-parsers --test parser_contracts` 通过，确认 D9 frontend-shape 没有削弱 parser 后端行为锁。
- `node scripts/check-rust-backend-structure.mjs` 通过，确认 parser 后端结构仍满足治理基线。
- `Set-Location src/web; npm run structure:check` 仍只暴露 D10 shared 前端债务；D9 未新增或接管前端 structure failure。
- `node scripts/check-backend-doc-map.mjs` 通过。
- `git diff --check` 通过。

## 10. Comment-pass 记录

D9 `comment-pass` 切片只补充 parser/source normalization 域的中文函数说明，不改变 parser registry 顺序、来源识别条件、字段映射、金额/日期标准化、dedicated exactly-one/no_match/conflict 决策或导入 staging 前的 `StandardBill` 合同。

本切片补充范围：

- dedicated parser DTO、注册表、选择器和候选证据 helper 的中文说明。
- `dedicated/common.rs` 中 CSV/Excel/HTML 表格读取、字段清洗、金额解析、日期时间压缩值归一和 row map helper 的中文说明。
- 微信、支付宝、工商、民生、农业、建设银行来源 parser 的 parse、格式探测和 RawBill 映射函数中文说明。
- parser JSON 兼容、parser tag 归一和 RawBill description 聚合的复杂私有 helper 中文说明。

本切片本地验证记录：

- `cargo fmt --all` 已执行。
- `cargo fmt --all -- --check` 通过。
- `cargo test -p bill-analyser-parsers --test parser_contracts` 通过，19 个 parser 合同测试全部通过。
- `cargo clippy --workspace --all-targets -- -D warnings` 通过。
- `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 35` 通过，完整 Rust 工作区 coverage gate 生成 `workspace.lcov`。
- `node scripts/governance-normalizers.mjs changed-coverage --lcov workspace.lcov --diff .omx\ultragoal\evidence\d9-comment-pass-parser.diff --threshold 90` 通过，新增 0 行可执行代码。
- `node scripts/check-rust-backend-structure.mjs` 通过，507 个 Rust backend 文件仍只保留 3 个既有 baseline entry。
- `node scripts/check-backend-doc-map.mjs` 通过。
- `git diff --check` 通过，仅输出 Windows 换行提示。

## 11. Governance-docs 记录

D9 `governance-docs` 切片只复核并记录 parser/source normalization 域的治理状态，不放宽结构基线、不修改运行时代码、不改变 `docs/PROJECT_OVERVIEW.md` 已记录的 parser facade + 功能子文件事实。

本切片治理结论：

- `scripts/rust-backend-structure-baseline.json` 当前 3 个 baseline entry 均不属于 `src/backend/parsers/**`；D9 parser 后端结构已经通过 gate，无需新增或放宽 parser baseline。
- `docs/PROJECT_OVERVIEW.md` 已记录 `src/backend/parsers/lib.rs`、`src/backend/parsers/dedicated/mod.rs` 及其功能子文件边界；本切片复核后不做措辞 churn。
- `src/web` 前端 structure check 仍只暴露 D10 shared 债务：`src/lib/services.ts`、`src/stores/index.ts`、`src/core/theme.ts`、`src/models/imported_transaction.ts`；D9 不接管这些 shared 文件。
- parser 金额、日期、registry、dedicated exactly-one/no_match/conflict 和 staging 前 `StandardBill` 合同仍以 `tests/backend/parsers/parser_contracts.rs` 为行为锁。

本切片本地验证记录：

- `node scripts/check-rust-backend-structure.mjs` 通过，507 个 Rust backend 文件仍只保留 3 个非 D9 baseline entry。
- `node scripts/check-backend-doc-map.mjs` 通过。
- `Set-Location src/web; npm run structure:check` 仍只失败 4 个 D10 shared 文件，D9 没有新增前端 structure failure。
- `cargo test -p bill-analyser-parsers --test parser_contracts` 通过，19 个 parser 合同测试全部通过。
- `rg` 复核 `docs/PROJECT_OVERVIEW.md` 中 parser 当前结构事实仍存在。
- `git diff --check` 通过，仅输出 Windows 换行提示。
