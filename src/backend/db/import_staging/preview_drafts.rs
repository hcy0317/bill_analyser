// 中文导读：导入预览 draft 构造与 feedback 投影辅助。
// 维护重点：把当前解析结果和 Postgres authoritative bill 候选转换成 import_preview_bills 可写 draft。
// 不变式：金额输出保持整数分，跨批候选仅生成待审核证据，不直接改写正式账单。

include!("preview_drafts/dedup.rs");
include!("preview_drafts/history_inputs.rs");
include!("preview_drafts/history_duplicate.rs");
include!("preview_drafts/history_transfer.rs");
include!("preview_drafts/default.rs");
include!("preview_drafts/helpers.rs");
include!("preview_drafts/tests.rs");
