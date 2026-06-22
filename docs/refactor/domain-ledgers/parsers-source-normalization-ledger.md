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

- `src/backend/parsers/lib.rs` 当前 578 行，是 parser crate 的 public facade，同时仍承载 `ParserInfo` registry、`RawBill`/`StandardBill` DTO、tag/type/amount/description helper 与 `post_process_raw_bills`。
- `src/backend/parsers/dedicated/mod.rs` 当前 394 行，承载 source parser registry、auto filename/content hint、requested parser 分支、match selection、decision/candidate/evidence DTO 和单元测试。
- `src/backend/parsers/dedicated/common.rs` 当前 329 行，承载 CSV/Excel/HTML 表格读取和通用 source-local helper。
- 各 dedicated source parser 当前为小文件：Alipay 78 行、WeChat 113 行、ICBC 132 行、ABC 130 行、CCB 99 行、CMBC 149 行。
- `tests/backend/parsers/parser_contracts.rs` 当前 509 行，是 D9 behavior-lock 的主要锚点，已覆盖 registry、source split、golden contracts、fixtures、exactly-one detection、conflict/no_match 和 source-local edge cases。
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
