use serde::Serialize;

use crate::StandardBill;

/// dedicated parser 成功解析后的结果，保留 parser id、标准账单和选择决策证据。
#[derive(Debug, Clone, PartialEq)]
pub struct DedicatedParseResult {
    pub parser_id: String,
    pub bills: Vec<StandardBill>,
    pub delimiter: Option<char>,
    pub decision: DedicatedParserDecision,
}

/// dedicated parser 的完整选择输出，API 可同时读取三态决策和命中的解析结果。
#[derive(Debug, Clone, PartialEq)]
pub struct DedicatedParseSelectionResult {
    pub decision: DedicatedParserDecision,
    pub parsed: Option<DedicatedParseResult>,
}

/// dedicated parser 选择决策，描述 requested/auto 场景下的 matched、no_match 或 conflict 状态。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DedicatedParserDecision {
    pub requested_parser: String,
    pub status: String,
    pub selected_parser_id: Option<String>,
    pub candidates: Vec<DedicatedParserCandidate>,
    pub conflict_group: Vec<String>,
    pub reason: String,
}

/// dedicated parser 候选证据，供冲突解释和导入预览定位来源识别依据。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DedicatedParserCandidate {
    pub parser_id: String,
    pub parser_label: String,
    pub confidence: f64,
    pub parsed_count: usize,
    pub evidence: Vec<String>,
}

/// 单个来源 parser 的内部注册项，保持 parser id 与解析函数绑定。
pub(super) struct DedicatedParser {
    pub(super) id: &'static str,
    pub(super) parse: fn(&str, &[u8]) -> Vec<StandardBill>,
}

/// 已命中的 dedicated parser 结果，选择阶段用它同时携带候选证据和账单。
pub(super) struct DedicatedParserMatch {
    pub(super) candidate: DedicatedParserCandidate,
    pub(super) bills: Vec<StandardBill>,
}

/// dedicated parser 选择流程的内部聚合结果，保留最终决策和 exactly-one 命中的账单。
pub(super) struct DedicatedParserSelection {
    pub(super) decision: DedicatedParserDecision,
    pub(super) selected: Option<DedicatedParserMatch>,
}
