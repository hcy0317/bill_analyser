# 导入链路

导入主链在 Rust runtime 中完成，仍保持三阶段用户体验：

1. **解析**：parser-first multipart 上传会并发解析多个上传文件并保持文件响应顺序；JSON parse 识别账单来源，生成标准账单 draft。
2. **去重预览**：写入 session/template staging，执行 dedup、账户匹配、分类匹配、transfer/recurring/learning decision，再批量写入 preview staging；stage2 默认不在响应内联全量 preview，前端通过 preview index/page 读取。
3. **确认导入**：用户确认后在事务内写入正式账单表，并更新账户余额、学习事件和审计。

## 核心模块

- `src/backend/parsers`：provider parser 与标准账单结构。
- `src/backend/http/import_routes/`：导入 HTTP surface、multipart、preview、confirm、learning、LLM/OCR 调用。
- `src/backend/db/imports`：session、preview、staging、learning、template mapping 与 confirm repository。
- `src/backend/core/import_pipeline.rs`：导入管线合同。

## 行为约束

- parser-first 上传必须保持 provider 检测顺序稳定。
- 多文件 parser work 可以并发执行，但 session/template staging 仍保持一次性写入。
- stage2 processed 状态按 `session_id + user_id + parser_is_processed` 更新，避免大批量 `id IN (...)` 更新；preview 批量写入复用 prepared statement。
- preview index 只读取筛选与列表索引所需字段，完整预览明细继续走 preview page/update/confirm 相关查询。
- preview update/reclassify 要显式落库，不只改响应投影。
- confirm 必须使用事务保证 bills、tags、accounts、learning side effects 一致。
- 金额字段必须复核元/分转换。
