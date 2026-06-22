// 中文导读：账单解析层，负责 provider 检测、RawBill 采集和 StandardBill 标准化。
// 维护重点：只保留来源识别、字段清洗和 parser_tags，不写入导入 staging、分类、账户或数据库。
// 不变式：解析结果的金额、时间、类型和来源标签必须在进入导入管线前保持可复核的原始来源语义。

mod dedicated;
mod money_serde;
mod normalization;
mod post_process;
mod registry;
mod tags;
mod types;

pub use dedicated::{
    detect_dedicated_import_bytes, parse_dedicated_import_bytes,
    parse_dedicated_import_bytes_with_decision, DedicatedParseResult,
    DedicatedParseSelectionResult, DedicatedParserCandidate, DedicatedParserDecision,
};
pub use normalization::{normalize_amount_text, normalize_transaction_type};
pub use post_process::{aggregate_description, post_process_raw_bills};
pub use registry::{parser_registry, parser_source_label};
pub use tags::{
    build_parser_tags, normalize_parser_tags, normalize_parser_tags_text,
    normalize_parser_tags_value, resolve_parser_tags, serialize_parser_tags,
};
pub use types::{ParserInfo, RawBill, StandardBill};
