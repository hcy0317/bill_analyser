use serde::Serialize;

use crate::StandardBill;

#[derive(Debug, Clone, PartialEq)]
pub struct DedicatedParseResult {
    pub parser_id: String,
    pub bills: Vec<StandardBill>,
    pub delimiter: Option<char>,
    pub decision: DedicatedParserDecision,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DedicatedParseSelectionResult {
    pub decision: DedicatedParserDecision,
    pub parsed: Option<DedicatedParseResult>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DedicatedParserDecision {
    pub requested_parser: String,
    pub status: String,
    pub selected_parser_id: Option<String>,
    pub candidates: Vec<DedicatedParserCandidate>,
    pub conflict_group: Vec<String>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DedicatedParserCandidate {
    pub parser_id: String,
    pub parser_label: String,
    pub confidence: f64,
    pub parsed_count: usize,
    pub evidence: Vec<String>,
}

pub(super) struct DedicatedParser {
    pub(super) id: &'static str,
    pub(super) parse: fn(&str, &[u8]) -> Vec<StandardBill>,
}

pub(super) struct DedicatedParserMatch {
    pub(super) candidate: DedicatedParserCandidate,
    pub(super) bills: Vec<StandardBill>,
}

pub(super) struct DedicatedParserSelection {
    pub(super) decision: DedicatedParserDecision,
    pub(super) selected: Option<DedicatedParserMatch>,
}
