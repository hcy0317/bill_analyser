// 中文导读：Postgres import staging 类型层，定义当前导入 session、preview 与学习/LLM DTO。
// 维护重点：类型保持当前 v2 API 所需形状，存储实现由 Postgres adapter 负责。
// 不变式：导入预览与 standard row 金额字段统一使用整数分；外部 parser 模板保留原始元单位边界。

include!("types/session.rs");
include!("types/preview_draft.rs");
include!("types/parser_template.rs");
include!("preview_drafts.rs");
include!("types/preview_query.rs");
include!("types/preview_patch.rs");
include!("types/preview_decision_learning.rs");
include!("types/recurring_annotation_memory.rs");
include!("types/confirm_history.rs");
include!("types/confirm_command.rs");
