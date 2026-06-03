# 导入链路

导入主链在 Rust runtime 中完成，保持三阶段用户体验：

1. **解析**：parser-first multipart 上传会并发检测多个上传文件并保持文件响应顺序；每个文件必须且只能由一个 dedicated parser 命中，命中结果会带 `parser_decision` 证据进入标准账单解析，未命中或多 parser 冲突会作为 unmatched 文件返回；JSON parse 使用已提供 parser id 生成标准账单 draft。
2. **去重预览**：写入 session/source/template/standard-row staging，执行同批重复折叠、同批转账配对、正式账单重复/转账 materialization、分类规则匹配、账户规则匹配、transfer/recurring/learning/LLM decision，再批量写入 preview staging；stage2 默认不在响应内联全量 preview，前端首屏请求 preview page。
3. **确认导入**：用户确认后在事务内写入正式账单表，并更新账户余额、学习事件和审计；涉及改写/合并正式账单的预览行必须带可见操作标记并提交服务端可验证 acknowledgement。

## 核心模块

- `src/backend/parsers`：provider parser 与标准账单结构。
- `src/backend/http/import_routes/`：导入 HTTP surface、multipart、preview、confirm、learning、LLM/OCR 调用。
- `src/backend/db/import_staging`：session、source、standard row、preview、template staging、decision、LLM memory 与 confirm repository。
- `src/backend/core/import_pipeline.rs`：导入管线合同。

## 行为约束

- parser-first 上传必须保持 provider 检测顺序稳定。
- 混合来源 multipart 上传必须按文件保持 parser id 和 parser tags，不允许使用首个文件 parser id 覆盖整批账单。
- dedicated parser 自动识别必须返回 exactly-one 决策；`no_match` 和 `conflict` 不写入标准账单，只保留 unmatched 文件和 parser decision evidence。
- `import_sources` 保存文件级 parser signal / confidence / decision metadata；`import_standard_rows` 保存标准化行、金额分单位、方向、parser payload 和原始标准 payload，source/row 写入必须与 parser template staging 处于同一事务。
- 同批重复按时间窗口、同向金额、方向和文本证据合并，保留基底预览行并把来源链、合并原因和成员写入 `import_decision_groups` / `import_decision_group_members`。
- 同批转账按时间窗口、同额反向金额和不同来源配对，以支出侧为基底合并交易对方、支付方式和描述；支出/收入两侧原始交易对方、支付方式、描述、账户和 parser 信息保留在 `matching.transfer.source_chain`，并写入 `same_batch_transfer` decision group。
- stage2 账户识别以 `account_rules` 为权威：转账先用隐藏支出/收入侧字段分别匹配来源/目标账户；投资先按 parser/支付方式匹配来源账户，再按交易对方优先、描述兜底匹配投资账户；收入/支出只匹配当前类型的账户规则。
- 多文件 parser work 可以并发执行，但 session/template staging 仍保持一次性写入。
- preview page 承担 Check Data 的分页、排序、筛选与轻量聚合 metadata；缺少分类、缺少账户和转账账户复核状态按当前预览字段计算。
- preview 的 transfer 和 LLM 建议只有 pending 状态展示接受/拒绝动作。learning 建议按 `recommendation_key` 进入生命周期表：默认 `yellow` 只在信号列展示推荐类型、分类和账户，不自动改写预览字段；用户接受/拒绝会写入 `import_learning_feedback_events` 并更新 `import_learning_lifecycle`，同一建议接受达到 3 次后变为 `green` 并允许后续导入自动应用。
- Weaviate 向量索引是运行态必需的 long-term learning 派生召回基础设施：由 PostgreSQL `vector_outbox_events` 和 `bill_weaviate_derived_index` CLI 负责 bootstrap、outbox batch 和 rebuild。Weaviate metadata 不能直接触发 auto-apply，生命周期阈值、反馈计数和 suppression 仍以 PostgreSQL 为准。
- 前端人工编辑类型、分类、账户等字段时只清除 transfer/learning/LLM 这些 actionable 建议提示，并通过 family-scoped update payload 持久化。
- confirm、cancel、导入失败后新建 session 都会立即清理当前用户的 import staging；系统不保留未完成导入续传状态。
- preview update/reclassify 要显式落库，不只改响应投影；reclassify 会复用 stage2 智能链路刷新分类、账户、recurring 和 learning 信号。
- confirm 必须使用事务保证 bills、tags、accounts、learning side effects 一致。
- 金额字段必须复核元/分转换。
