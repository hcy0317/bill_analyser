# 新增专用解析器标准流程

这份文档描述 Bill Analyser 中“新增一个专用 parser”时的最小标准流程。

它服务于两类读者：

- 人类开发者：需要知道从哪里开始、怎么证明不会误伤已有 parser
- AI/agent：需要一份稳定的仓库级 parser 工作流说明

共享 skill 入口见：`.agents/skills/add-parser-standard-flow/SKILL.md`。

## 当前运行时入口与关键文件

新增或调整 parser 前，先阅读这些真实落点：

- `crates/bill-analyser-parsers/src/lib.rs`
- `crates/bill-analyser-parsers/tests/parser_contracts.rs`
- `crates/bill-analyser-http/src/import_routes.rs`
- `crates/bill-analyser-http/tests/import_runtime_contract.rs`
- `src/bill_analyser/parsers/factory.py`
- `src/bill_analyser/parsers/base.py`
- `tests/test_parser_base_factory.py`
- `tests/new_ui/test_import_parser_alignment.py`
- 最相近的现有 parser 与其专项回归，例如 `tests/test_abc_parser.py`
- 如需稳定样本或构造器，优先查看 `tests/parser_test_support.py`

## 这份流程解决什么问题

新增 parser 最常见的问题不是“解析不出来”，而是：

- Rust 检测条件过宽，误伤相邻 parser
- Python `can_parse()` 过宽，导致迁移期 sidecar 与 Rust 行为不一致
- 输出字段看起来能跑，但和 `StandardBill` 契约不一致
- 只写了正例，没有写负例和歧义例
- 新 parser 能 parse，却和导入预览链不对齐

所以这份流程强调：**先定边界，再写 parser，再证明它不会误伤别人。**

当前浏览器上传的 parser-first 运行时在 Rust：`crates/bill-analyser-parsers` 提供专用 parser，`crates/bill-analyser-http` 调用 Rust parser-first 路径。Python `src/bill_analyser/parsers/**` 仍用于迁移期 sidecar parity、历史实现和对照测试；不要把 Python parser 当作新的 parser-first 主运行路径。

## 开始前先确认

### 1. 样本范围

确认以下问题：

- 目标机构/平台是什么
- 文件扩展名是什么（CSV/XLS/XLSX）
- 是否存在同一家机构的多个导出版本
- 是否已有稳定 fixture；如果没有，是否可以通过 `tests/parser_test_support.py` 构造样本

优先把稳定样本放到可复现的位置，例如：

- `tests/fixtures/import_samples/`
- `tests/parser_test_support.py`

不要只依赖 `bills/` 里的本地文件作为唯一证据。

### 2. 冲突 parser

新增 parser 前，必须先找出最可能冲突的已有 parser。

例如：

- 银行 Excel 账单之间可能互相误判
- 通用列映射路径与专用 parser 的核心字段可能出现对齐偏差

### 3. 契约边界

这条流程默认只处理：

- `crates/bill-analyser-parsers/**`
- 必要的 `crates/bill-analyser-http/**` parser-first 接入点
- 迁移期 Python parity 文件 `src/bill_analyser/parsers/**`
- parser 回归测试
- parser 流程文档

默认**不顺手带上**：

- REST 字段新增
- DB schema 扩展
- 导入预览 payload 改名

如果确实需要这些变化，请拆成单独切片。

## 标准实施步骤

1. 选择一个最相近的现有 Rust parser 作为对照
2. 先实现或收紧 Rust 检测逻辑，补相邻 parser 负例
3. 再实现或收紧 Rust 解析逻辑，并通过 `post_process_raw_bills()` 对齐标准输出
4. 只有 HTTP 上传入口行为需要变化时，才调整 `crates/bill-analyser-http/src/import_routes.rs`
5. 如本切片也影响 Python sidecar parity，再同步 `can_parse()` / `parse()` / `PARSER_CLASS_REGISTRY`
6. 重新检查 Rust 检测顺序；如触及 Python，再检查 `ParserFactory` 顺序
7. 补最小测试矩阵
8. 最后再更新文档和样本说明

## 检测冲突与优先级检查

Bill Analyser 的 parser-first 上传路径按 Rust 专用 parser 顺序逐个试探；迁移期 Python sidecar 仍按 `ParserFactory` 顺序逐个试探。所以“能 parse 成功”还不够。

最小要求：

- 目标样本能命中目标 parser
- 最近邻 parser 必须拒绝该样本
- Rust `parse_dedicated_import_bytes()` 返回目标 `PARSER_ID`
- Python parity 在本切片范围内时，`ParserFactory.detect_parser()` 返回目标 `PARSER_ID`
- 如需调整 `PARSER_CLASS_REGISTRY` 顺序，必须能说明原因

如果一条改动会放宽检测条件，就必须新增对应负例，避免把别的 parser 吃进去。

## 标准输出字段与导入链对齐点

新增 parser 的输出必须兼容 `StandardBill`，即至少保持这些关键点：

- `date`：规范化为 `YYYY-MM-DD HH:MM:SS`
- `amount`：遵守仓库正负号语义
- `type`：映射到现有交易类型语义
- `description`：能支撑分类与预览复核
- `source_account_id`：与 parser 的 `PARSER_ID` 对齐

相关契约回归以这些文件为准：

- `crates/bill-analyser-parsers/tests/parser_contracts.rs`
- `crates/bill-analyser-http/tests/import_runtime_contract.rs`
- `tests/test_parser_base_factory.py`
- `tests/new_ui/test_import_parser_alignment.py`

## parser tags 设计合同（当前不落运行时）

路线图已经明确提到 parser tags，但当前仓库还没有把它做成正式 REST/DB 运行时字段。

因此，现阶段只维护**受控词表设计合同**，不在本流程里直接扩展运行时 payload。

推荐前缀：

- `parser:<parser_id>`
- `record_origin:<wallet_statement|bank_statement>`
- `channel:<wallet|bank_card|credit_card>`
- `txn_family:<expense|income|transfer|investment|refund>`
- `institution:<wechat|alipay|icbc|cmbc|abc|ccb>`

使用规则：

- 先写进文档、注释或测试说明
- 保持词表受控、可审阅
- 不要在这个切片里直接新增 API 字段或 DB 列

## 最小测试矩阵

新增 parser 至少应覆盖下面四类测试：

1. **正例识别**
   - 目标样本应被目标 parser 接受

2. **相邻 parser 负例**
   - 最可能冲突的 parser 应拒绝该样本

3. **Factory/契约回归**
   - `crates/bill-analyser-parsers/tests/parser_contracts.rs`
   - `tests/test_parser_base_factory.py`
   - 确认 Rust parser、迁移期 Python `ParserFactory` 和 `StandardBill` 契约不漂移

4. **导入预览/对齐回归**
   - `tests/new_ui/test_import_parser_alignment.py`
   - 确认专用 parser 与导入预览链仍然对齐

如果 parser 已进入稳定真实样本回归范围，再考虑补充：

- `tests/new_ui/test_original_local_bill_samples.py`

## 何时更新 fixture / manifest / PROJECT_OVERVIEW

### 更新 fixture / builder

当样本已经稳定、可复现，并且后续会被长期回归复用时，才更新：

- `tests/fixtures/import_samples/`
- `tests/parser_test_support.py`

### 更新 `docs/PROJECT_OVERVIEW.md`

只有在“系统当前如何工作”的稳定事实改变时才更新，例如：

- 正式新增了一个新的运行时 parser
- parser 检测顺序发生稳定变化
- 导入预览与专用 parser 的对齐行为出现新的稳定事实

### 不要更新的场景

- 一次性的实验样本
- 只是在调 `can_parse()` 的中间过程
- 还没通过回归验证的 parser 草稿

## 常见失败模式

- `can_parse()` 写得太宽，误伤其他银行/平台 parser
- 日期没有补齐到秒，导致标准输出不一致
- 金额正负号反了
- `description` 太空，后续分类质量下降
- parser 能 parse，但 `ParserFactory` 没注册或注册顺序错误
- 只做了正例，没有做负例与歧义例
- 顺手把 parser tags、REST、DB 改动混在同一刀里

## 提交前验收

最小验证建议：

- `./.venv/Scripts/python.exe -m pytest tests/test_parser_base_factory.py -v`
- `./.venv/Scripts/python.exe -m pytest tests/test_<parser>.py -v`
- `./.venv/Scripts/python.exe -m pytest tests/new_ui/test_import_parser_alignment.py -v`
- `cargo test -p bill-analyser-parsers`
- `cargo test -p bill-analyser-http --test import_runtime_contract`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 90`

如果这轮改动进入了 `src/bill_analyser/**` 运行时代码，最终验收仍应补：

- `./.venv/Scripts/python.exe -m pytest tests/ -v`
