# 导入链路

导入主链在 Rust runtime 中完成，保持三阶段用户体验：

1. **解析**：parser-first multipart 上传会并发检测多个上传文件并保持文件响应顺序；每个文件必须且只能由一个 dedicated parser 命中，命中结果会带 `parser_decision` 证据进入标准账单解析，未命中或多 parser 冲突会作为 unmatched 文件返回；JSON parse 使用已提供 parser id 生成标准账单 draft。
2. **去重预览**：写入 session/source/template/standard-row staging，执行同批重复折叠、同批转账配对、正式账单重复/转账 materialization、分类规则匹配、transfer/recurring/learning/LLM decision、账户规则匹配，再批量写入 preview staging；stage2 默认不在响应内联全量 preview，前端首屏请求 preview page。
3. **确认导入**：用户确认后在事务内写入正式账单表，并更新账户余额、学习事件和审计；涉及改写/合并正式账单的预览行必须带可见操作标记并提交服务端可验证 acknowledgement。

## 核心模块

- `src/backend/parsers`：provider parser 与标准账单结构。
- `src/backend/http/import_routes/`：导入 HTTP surface、multipart、preview、confirm、learning、LLM/OCR 调用。
- `src/backend/db/import_staging`：session、source、standard row、preview、template staging、decision、LLM memory 与 confirm repository。
- `src/backend/core/import_pipeline.rs`：导入管线合同。

条件选择携带的草稿与集合更新在同一个 session 锁事务内执行。单次 mutation 最多接受 500 个唯一 preview 草稿 ID，写入前批量校验 session scope 和当前用户 active 分类；越界、跨会话或事务内部分应用都会整体拒绝。

跨页选择会在发出 mutation 前先使此前在途的 preview page generation 失效，成功后直接消费 mutation metadata，不追加分页 GET；服务端分页 reclassify 刷新会替换旧 generation，迟到响应不能覆盖新状态。

## 行为约束

- parser-first 上传必须保持 provider 检测顺序稳定。
- 混合来源 multipart 上传必须按文件保持 parser id 和 parser tags，不允许使用首个文件 parser id 覆盖整批账单。
- dedicated parser 自动识别必须返回 exactly-one 决策；`no_match` 和 `conflict` 不写入标准账单，只保留 unmatched 文件和 parser decision evidence。
- `import_sources` 保存文件级 parser signal / confidence / decision metadata；`import_standard_rows` 保存标准化行、金额分单位、方向、parser payload 和原始标准 payload，source/row 写入必须与 parser template staging 处于同一事务。
- 同批重复按时间窗口、同向金额、方向和文本证据合并，保留基底预览行并把来源链、合并原因和成员写入 `import_decision_groups` / `import_decision_group_members`。
- 同批转账按时间窗口、同额反向金额和不同来源配对，以支出侧为基底合并交易对方、支付方式和描述；支出/收入两侧原始交易对方、支付方式、描述、账户和 parser 信息保留在 `matching.transfer.source_chain`，并写入 `same_batch_transfer` decision group。
- stage2 phase precedence 固定为：parser 标准行 → 同批重复/转账与正式账单 materialization → 分类规则/内置分类兜底 → recurring projection → 可自动应用的 learning projection → 账户规则匹配 → stage2 baseline 持久化。账户规则必须最后运行，因为 learning projection 可以改写 `preview_type`、分类和显式账户，recurring/transfer/investment 信号也会改变账户角色上下文。
- stage2 账户识别以 `account_rules` 为权威，但当前合同只保留“账户 + 表达式 + 优先级/启停”。旧 `account_role_scope`、`transaction_type_scope` 与 `field_scope` 不再落库，旧 payload/query/settings bundle 携带这些字段时只产生兼容 warning 并被忽略；匹配时由运行时上下文决定目标：转账按稳定后的来源/目标侧字段匹配来源/目标账户；投资先按 parser/支付方式匹配来源账户，再按交易对方优先、描述兜底匹配投资账户；收入/支出按最终 `preview_type` 匹配仍为空的来源账户。
- 账户规则不得覆盖 parser、learning 或用户编辑已经显式给出的账户；旧 scope payload 或旧设置包字段会被忽略并返回 warning，新设置包导出不再包含这些 scope 字段。
- 多文件 parser work 可以并发执行，但 session/template staging 仍保持一次性写入。
- preview page 承担 Check Data 的分页、排序、筛选与轻量聚合 metadata；core 计数、信号计数与完整选择快照由一次 conditional aggregate 计算，三类 facets 各自有界聚合。缺少分类、缺少账户和转账账户复核状态按当前预览字段计算；用户明确归属的当前用户 active 分类不会仅因修改前的类型投影再次标记为缺少分类，缺失、不存在或 inactive 分类仍失败关闭。
- preview signal filter 按信号列可见 family 计算，而不是任意 JSON 文本包含：parser 筛选只包含没有平台重复、转账匹配、历史改写、learning 和 LLM 可见信号的 parser 行；平台重复、转账匹配、历史改写、learning、LLM 筛选分别匹配对应可见 family。
- preview 的 transfer 和 LLM 建议只有 pending 状态展示接受/拒绝动作。learning 建议按 `recommendation_key` 进入生命周期表：默认 `yellow` 只在信号列展示推荐类型、分类和账户，不自动改写预览字段；用户接受/拒绝会写入 `import_learning_feedback_events` 并更新 `import_learning_lifecycle`，同一建议接受达到 3 次后变为 `green` 并允许后续导入自动应用。
- Weaviate 向量索引是运行态必需的 long-term learning 派生召回基础设施：由 PostgreSQL `vector_outbox_events` 和 `bill_weaviate_derived_index` CLI 负责 bootstrap、outbox batch 和 rebuild。Weaviate recall 只在当前用户存在 PostgreSQL `import_learning_features` 源行时发起；没有源行时直接跳过召回而不改变确定性规则、transfer、history rewrite、learning lifecycle 或 LLM 识别链路。Weaviate metadata 不能直接触发 auto-apply，生命周期阈值、反馈计数和 suppression 仍以 PostgreSQL 为准。
- 前端人工编辑类型、分类、账户等字段时只清除 transfer/learning/LLM 这些 actionable 建议提示，并通过 family-scoped update payload 持久化。
- 分类、来源账户与目标账户的 canonical id 必须来自当前用户 active 主数据；原始 parser/payment/category 文本只作为 review evidence 或非法字段提示展示，不会作为 accepted identity 写入 preview、confirm payload 或正式账单。
- confirm、cancel、导入失败后新建 session 都会立即清理当前用户的 import staging；系统不保留未完成导入续传状态。
- preview update/reclassify 要显式落库，不只改响应投影；非空 `preview_updates` 只读取、刷新、写回并返回目标 preview id，空更新才执行整 session 重新分类，非分页前端按目标 id 原位合并局部响应。前端服务端分页草稿在字段变化时立即缓存，迟到响应恢复草稿并重绑编辑器；跨页选择 mutation 直接消费响应 metadata，不再随后重复请求当前页；有效/无效/需标注选择在同一次 mutation 内先落库已浏览页面的有效性草稿，再按新状态做集合选择。
- confirm 必须使用事务保证 bills、tags、accounts、learning side effects 一致。
- 金额字段必须复核元/分转换。
