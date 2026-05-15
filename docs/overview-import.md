# 导入链路

导入主链在 Rust runtime 中完成，仍保持三阶段用户体验：

1. **解析**：parser-first 上传或 JSON parse 识别账单来源，生成标准账单 draft。
2. **去重预览**：写入 session/preview staging，执行 dedup、账户匹配、分类匹配、transfer/recurring/learning decision。
3. **确认导入**：用户确认后在事务内写入正式账单表，并更新账户余额、学习事件和审计。

## 核心模块

- `src/backend/parsers`：provider parser 与标准账单结构。
- `src/backend/http/import_routes/`：导入 HTTP surface、multipart、preview、confirm、learning、LLM/OCR 调用。
- `src/backend/db/imports`：session、preview、staging、learning、template mapping 与 confirm repository。
- `src/backend/core/import_pipeline.rs`：导入管线合同。

## 行为约束

- parser-first 上传必须保持 provider 检测顺序稳定。
- preview update/reclassify 要显式落库，不只改响应投影。
- confirm 必须使用事务保证 bills、tags、accounts、learning side effects 一致。
- 金额字段必须复核元/分转换。
