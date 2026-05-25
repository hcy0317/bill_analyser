# 导入链路

导入主链在 Rust runtime 中完成，仍保持三阶段用户体验：

1. **解析**：parser-first multipart 上传会并发解析多个上传文件并保持文件响应顺序；每个文件的 parser identity 会随对应标准账单写入 template staging，避免混合来源上传时把不同账单合并成同一来源；JSON parse 识别账单来源，生成标准账单 draft。
2. **去重预览**：写入 session/template staging，执行 dedup、账户别名匹配、分类规则匹配、transfer/recurring/learning/LLM decision，再批量写入 preview staging；stage2 默认不在响应内联全量 preview，前端首屏只请求 preview page，筛选/排序随 preview page query 由 Rust 端按条件分页并返回轻量 facets/counts，完整 preview 从 `preview_matching_feedback_json` 投影 parser/dedup/transfer/recurring/learning/LLM 信号。
3. **确认导入**：用户确认后在事务内写入正式账单表，并更新账户余额、学习事件和审计。

## 核心模块

- `src/backend/parsers`：provider parser 与标准账单结构。
- `src/backend/http/import_routes/`：导入 HTTP surface、multipart、preview、confirm、learning、LLM/OCR 调用。
- `src/backend/db/import_staging`：session、preview、template staging、decision、LLM memory 与 confirm repository。
- `src/backend/core/import_pipeline.rs`：导入管线合同。

更完整的导入管线图、数据流和修改路径见 [Rust 后端导航图](backend-map.md#import-pipeline)。该图是导入相关路径索引的详细入口；本页只保留运行态约束。

## 行为约束

- parser-first 上传必须保持 provider 检测顺序稳定。
- 混合来源 multipart 上传必须按文件保持 parser id 和 parser tags，不允许使用首个文件 parser id 覆盖整批账单。
- 多文件 parser work 可以并发执行，但 session/template staging 仍保持一次性写入。
- stage2 processed 状态按 `session_id + user_id + parser_is_processed` 更新，避免大批量 `id IN (...)` 更新；preview 批量写入复用 prepared statement。
- preview page 承担 Check Data 的分页、排序、筛选与轻量聚合 metadata；缺少分类、缺少账户和转账账户复核状态按当前预览字段计算，人工补齐后不会被历史 annotation 或人工编辑标记继续计为待标注；旧 preview index 路由仅作为兼容读取面，不再是首屏预览依赖。
- preview 的 transfer、learning 和 LLM 建议只有 pending 状态展示接受/拒绝动作；learning `auto_applied` / `auto-applied` 按已应用状态投影，只允许撤销自动应用，不再显示接受/拒绝动作；接受会记录 applied/previous 快照，清除已接受建议时仅在当前字段仍等于 applied 快照时恢复 previous，人工改动后的清除不会覆盖人工值；拒绝 pending 建议会在当前字段仍等于建议 applied 快照时回退到 stage2 已落库的分类规则和账户基线，基线不存在或字段已被人工改动时只记录拒绝状态并保留当前字段。
- 前端人工编辑类型、分类、账户等字段时只清除 transfer/learning/LLM 这些 actionable 建议提示，并通过 family-scoped update payload 持久化；parser、dedup、recurring 和当前预览字段不随提示清除被删除。投资识别不再作为独立 preview 信号或 candidate family，结果只体现在分类规则链路命中的 `preview_type/category` 上。
- confirm、cancel、导入失败后新建 session 都会立即清理当前用户的 import staging；系统不保留未完成导入续传状态。
- preview update/reclassify 要显式落库，不只改响应投影；reclassify 会复用 stage2 智能链路刷新分类、账户、recurring 和 learning 信号。
- confirm 必须使用事务保证 bills、tags、accounts、learning side effects 一致。
- 金额字段必须复核元/分转换。
