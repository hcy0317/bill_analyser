# 导入链路

导入主链在 Rust runtime 中完成，仍保持三阶段用户体验：

1. **解析**：parser-first multipart 上传会并发检测多个上传文件并保持文件响应顺序；每个文件必须且只能由一个 dedicated parser 命中，命中结果会带 `parser_decision` 证据进入标准账单解析，未命中或多 parser 冲突会作为 unmatched 文件返回；JSON parse 使用已提供 parser id 生成标准账单 draft。
2. **去重预览**：写入 session/source/template/standard-row staging，执行同批重复折叠、同批转账配对、历史重复/历史转账 materialization、分类规则匹配、账户规则匹配、transfer/recurring/learning/LLM decision，再批量写入 preview staging；stage2 默认不在响应内联全量 preview，前端首屏只请求 preview page，筛选/排序随 preview page query 由 Rust 端按条件分页并返回轻量 facets/counts，完整 preview 从 `preview_matching_feedback_json` 投影 parser/dedup/reconciliation/transfer/recurring/learning/LLM 信号。
3. **确认导入**：用户确认后在事务内写入正式账单表，并更新账户余额、学习事件和审计；涉及改写/合并历史账单的预览行必须带可见操作标记并提交服务端可验证 acknowledgement。

## 核心模块

- `src/backend/parsers`：provider parser 与标准账单结构。
- `src/backend/http/import_routes/`：导入 HTTP surface、multipart、preview、confirm、learning、LLM/OCR 调用。
- `src/backend/db/import_staging`：session、source、standard row、preview、template staging、decision、LLM memory 与 confirm repository。
- `src/backend/core/import_pipeline.rs`：导入管线合同。

更完整的导入管线图、数据流和修改路径见 [Rust 后端导航图](backend-map.md#import-pipeline)。可复跑验收矩阵和本地合同测试见 [导入全链路验收场景](import-full-chain-scenarios.md)。该图是导入相关路径索引的详细入口；本页只保留运行态约束。

## 行为约束

- parser-first 上传必须保持 provider 检测顺序稳定。
- 混合来源 multipart 上传必须按文件保持 parser id 和 parser tags，不允许使用首个文件 parser id 覆盖整批账单。
- dedicated parser 自动识别必须返回 exactly-one 决策；`no_match` 和 `conflict` 不写入标准账单，只保留 unmatched 文件和 parser decision evidence。
- `import_sources` 保存文件级 parser signal / confidence / decision metadata；`import_standard_rows` 保存标准化行、金额分单位、方向、parser payload 和原始标准 payload，source/row 写入必须与 parser template staging 处于同一事务。
- 同批重复按时间窗口、同向金额、方向和文本证据合并，保留基底预览行并把来源链、合并原因和成员写入 `import_decision_groups` / `import_decision_group_members`。
- 历史重复按当前用户、standard row 日期窗口、正式账单金额方向和文本证据查询；命中后以正式账单为基底生成 `database_duplicate` 预览行，`reconciliation` feedback 和 `import_history_materializations` 显式标记 `update_history`，并在 matching payload 暴露 `operation_id`、`acknowledgement_token` 与“将改写/合并历史账单”提示；confirm 会校验 selected preview ids、操作 id、历史账单 id/version、selection scope 与 ack token 后才更新历史账单并写入 `import_confirm_operations` 审计。
- 同批转账按时间窗口、同额反向金额和不同来源配对，以支出侧为基底合并交易对方、支付方式和描述；支出/收入两侧原始交易对方、支付方式、描述、账户和 parser 信息保留在 `matching.transfer.source_chain`，并写入 `same_batch_transfer` decision group。
- 历史转账按当前用户、standard row 日期窗口、同日时间容差、同额反向金额和不同来源查询正式账单；命中后以支出侧为基底生成 `transfer_cross_batch` 预览行，显式标记 `merge_transfer_history`，写入 `import_history_materializations` 和 `historical_transfer` decision group；confirm 在 ack 通过后以支出侧为基底更新历史账单或插入新转账基底并删除被合并的收入侧历史账单，保留标签/匹配反馈审计并同步相关账户余额。
- stage2 账户识别以 `account_rules` 为权威：转账先用隐藏支出/收入侧字段分别匹配来源/目标账户；投资先按 parser/支付方式匹配来源账户，再按交易对方优先、描述兜底匹配投资账户；收入/支出只匹配当前类型的账户规则。旧账户别名只作为迁移规则输入，不再独立驱动导入账户字段。
- 多文件 parser work 可以并发执行，但 session/template staging 仍保持一次性写入。
- stage2 processed 状态按 `session_id + user_id + parser_is_processed` 更新，避免大批量 `id IN (...)` 更新；preview 批量写入复用 prepared statement。
- preview page 承担 Check Data 的分页、排序、筛选与轻量聚合 metadata；缺少分类、缺少账户和转账账户复核状态按当前预览字段计算，人工补齐后不会被历史 annotation 或人工编辑标记继续计为待标注；旧 preview index 路由仅作为兼容读取面，不再是首屏预览依赖。
- preview 的 transfer 和 LLM 建议只有 pending 状态展示接受/拒绝动作。learning 建议按 `recommendation_key` 进入生命周期表：默认 `yellow` 只在信号列展示推荐类型、分类和账户，不自动改写预览字段；用户接受/拒绝会写入 `import_learning_feedback_events` 并更新 `import_learning_lifecycle`，同一建议接受达到 3 次后变为 `green` 并允许后续导入自动应用，green/auto-applied 信号在 Check Data 只显示拒绝按钮；green 状态再次拒绝达到 2 次会降回 yellow，yellow 拒绝达到 3 次写入 `import_learning_suppressions` 后不再推荐。接受会记录 applied/previous 快照；拒绝 pending 建议会在当前字段仍等于建议 applied 快照时回退到 stage2 已落库的分类规则和账户基线，基线不存在或字段已被人工改动时只记录拒绝状态并保留当前字段。带转账匹配证据的 learning 建议不得改变类型，只允许推荐分类和账户。
- 可选 Weaviate 向量索引只作为 long-term learning 的派生召回基础设施：默认禁用，启用后由 PostgreSQL `vector_outbox_events` 和 `bill_weaviate_derived_index` CLI 负责 bootstrap、outbox batch 和 rebuild。Weaviate metadata 不能直接触发 auto-apply，生命周期阈值、反馈计数和 suppression 仍以 PostgreSQL 为准。
- 前端人工编辑类型、分类、账户等字段时只清除 transfer/learning/LLM 这些 actionable 建议提示，并通过 family-scoped update payload 持久化；parser、dedup、recurring 和当前预览字段不随提示清除被删除。投资识别不再作为独立 preview 信号或 candidate family，结果只体现在分类规则链路命中的 `preview_type/category` 上。
- confirm、cancel、导入失败后新建 session 都会立即清理当前用户的 import staging；系统不保留未完成导入续传状态。
- preview update/reclassify 要显式落库，不只改响应投影；reclassify 会复用 stage2 智能链路刷新分类、账户、recurring 和 learning 信号。
- confirm 必须使用事务保证 bills、tags、accounts、learning side effects 一致。
- 金额字段必须复核元/分转换。
