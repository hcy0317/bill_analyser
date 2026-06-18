// 中文导读：账单解析层，负责 provider 检测、RawBill 采集和 StandardBill 标准化。
// 维护重点：只保留来源识别、字段清洗和 parser_tags，不写入导入 staging、分类、账户或数据库。
// 不变式：解析结果的金额、时间、类型和来源标签必须在进入导入管线前保持可复核的原始来源语义。

use serde::Serialize;

use crate::{parser_source_label, StandardBill};

mod common;

#[path = "ABC.rs"]
mod abc;
#[path = "Alipay.rs"]
mod alipay;
#[path = "CCB.rs"]
mod ccb;
#[path = "CMBC.rs"]
mod cmbc;
#[path = "ICBC.rs"]
mod icbc;
#[path = "WeChat.rs"]
mod wechat;

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

struct DedicatedParser {
    id: &'static str,
    parse: fn(&str, &[u8]) -> Vec<StandardBill>,
}

const AUTO_PARSERS: &[DedicatedParser] = &[
    DedicatedParser {
        id: "wechat",
        parse: wechat::parse,
    },
    DedicatedParser {
        id: "alipay",
        parse: alipay::parse,
    },
    DedicatedParser {
        id: "icbc",
        parse: icbc::parse,
    },
    DedicatedParser {
        id: "cmbc",
        parse: cmbc::parse,
    },
    DedicatedParser {
        id: "abc",
        parse: abc::parse,
    },
    DedicatedParser {
        id: "ccb",
        parse: ccb::parse,
    },
];

struct DedicatedParserMatch {
    candidate: DedicatedParserCandidate,
    bills: Vec<StandardBill>,
}

struct DedicatedParserSelection {
    decision: DedicatedParserDecision,
    selected: Option<DedicatedParserMatch>,
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn detect_dedicated_import_bytes(
    filename: &str,
    bytes: &[u8],
    requested_parser: &str,
) -> DedicatedParserDecision {
    select_dedicated_import_bytes(filename, bytes, requested_parser).decision
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn parse_dedicated_import_bytes(
    filename: &str,
    bytes: &[u8],
    requested_parser: &str,
) -> Option<DedicatedParseResult> {
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "parse_dedicated_import_bytes",
        "business operation entered"
    );
    parse_dedicated_import_bytes_with_decision(filename, bytes, requested_parser).parsed
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn parse_dedicated_import_bytes_with_decision(
    filename: &str,
    bytes: &[u8],
    requested_parser: &str,
) -> DedicatedParseSelectionResult {
    let selection = select_dedicated_import_bytes(filename, bytes, requested_parser);
    let decision = selection.decision;
    let parsed = selection.selected.map(|selected| DedicatedParseResult {
        parser_id: selected.candidate.parser_id,
        bills: selected.bills,
        delimiter: None,
        decision: decision.clone(),
    });
    DedicatedParseSelectionResult { decision, parsed }
}

#[tracing::instrument(level = "debug", skip_all)]
fn select_dedicated_import_bytes(
    filename: &str,
    bytes: &[u8],
    requested_parser: &str,
) -> DedicatedParserSelection {
    let requested = requested_parser.trim().to_ascii_lowercase();
    if matches!(
        requested.as_str(),
        "generic" | "csv" | "xlsx" | "xls" | "txt"
    ) {
        return DedicatedParserSelection {
            decision: no_match_decision(
                requested,
                "Requested parser is generic or column-mapped and is not a dedicated parser",
            ),
            selected: None,
        };
    }

    let matches = if requested.is_empty() || requested == "auto" {
        let auto_candidates = auto_parser_candidates(filename, bytes);
        let mut matches = auto_candidates
            .iter()
            .filter_map(|parser| parse_with_parser(parser, filename, bytes))
            .collect::<Vec<_>>();
        if auto_candidates.len() != AUTO_PARSERS.len() {
            matches.extend(
                AUTO_PARSERS
                    .iter()
                    .filter(|parser| {
                        !auto_candidates
                            .iter()
                            .any(|candidate| candidate.id == parser.id)
                    })
                    .filter_map(|parser| parse_with_parser(parser, filename, bytes)),
            );
        }
        matches
    } else if let Some(parser) = AUTO_PARSERS.iter().find(|parser| parser.id == requested) {
        parse_with_parser(parser, filename, bytes)
            .into_iter()
            .collect::<Vec<_>>()
    } else {
        return DedicatedParserSelection {
            decision: no_match_decision(requested, "Requested parser is not registered"),
            selected: None,
        };
    };

    dedicated_selection_from_matches(requested, matches)
}

fn auto_parser_candidates(filename: &str, bytes: &[u8]) -> &'static [DedicatedParser] {
    let normalized = filename.trim().to_ascii_lowercase();
    let parser_id = if normalized.contains("微信") || normalized.contains("wechat") {
        Some("wechat")
    } else if normalized.contains("支付宝") || normalized.contains("alipay") {
        Some("alipay")
    } else if normalized.contains("工商银行") || normalized.contains("icbc") {
        Some("icbc")
    } else if normalized.contains("民生银行") || normalized.contains("cmbc") {
        Some("cmbc")
    } else if normalized.contains("农业银行") || normalized.contains("abc") {
        Some("abc")
    } else if normalized.contains("建设银行") || normalized.contains("ccb") {
        Some("ccb")
    } else if normalized.starts_with("detail") && normalized.ends_with(".xlsx") {
        Some("abc")
    } else if normalized.starts_with("交易明细_") && normalized.ends_with(".xls") {
        Some("ccb")
    } else {
        auto_parser_id_from_content(filename, bytes)
    };
    match parser_id {
        Some(parser_id) => parser_slice(parser_id).unwrap_or(AUTO_PARSERS),
        None => AUTO_PARSERS,
    }
}

fn auto_parser_id_from_content(filename: &str, bytes: &[u8]) -> Option<&'static str> {
    let suffix = common::file_suffix(filename);
    if !matches!(suffix.as_str(), "csv" | "txt" | "xls")
        && !common::looks_like_html_table_payload(bytes)
    {
        return None;
    }
    let probe_len = bytes.len().min(128 * 1024);
    let probe = common::decode_text(&bytes[..probe_len]);
    if probe.contains("微信支付") || probe.contains("微信零钱") {
        Some("wechat")
    } else if probe.contains("支付宝") || probe.contains("Alipay") {
        Some("alipay")
    } else if probe.contains("中国民生银行") || probe.contains("民生银行") || probe.contains("CMBC")
    {
        Some("cmbc")
    } else if probe.contains("中国工商银行") || probe.contains("工商银行") || probe.contains("ICBC")
    {
        Some("icbc")
    } else if probe.contains("农业银行") || probe.contains("农业银⾏") {
        Some("abc")
    } else if probe.contains("建设银行") || probe.contains("China Construction Bank") {
        Some("ccb")
    } else {
        None
    }
}

fn parser_slice(parser_id: &str) -> Option<&'static [DedicatedParser]> {
    AUTO_PARSERS
        .iter()
        .find(|parser| parser.id == parser_id)
        .map(std::slice::from_ref)
}

#[tracing::instrument(level = "debug", skip_all)]
fn parse_with_parser(
    parser: &DedicatedParser,
    filename: &str,
    bytes: &[u8],
) -> Option<DedicatedParserMatch> {
    let bills = (parser.parse)(filename, bytes);
    if bills.is_empty() {
        None
    } else {
        Some(DedicatedParserMatch {
            candidate: DedicatedParserCandidate {
                parser_id: parser.id.to_string(),
                parser_label: parser_source_label(parser.id).into_owned(),
                confidence: 1.0,
                parsed_count: bills.len(),
                evidence: parser_evidence(parser.id, &bills),
            },
            bills,
        })
    }
}

fn dedicated_selection_from_matches(
    requested: String,
    mut matches: Vec<DedicatedParserMatch>,
) -> DedicatedParserSelection {
    match matches.len() {
        0 => DedicatedParserSelection {
            decision: no_match_decision(
                requested,
                "No dedicated Rust parser matched the uploaded file",
            ),
            selected: None,
        },
        1 => {
            let selected = matches.remove(0);
            DedicatedParserSelection {
                decision: DedicatedParserDecision {
                    requested_parser: requested,
                    status: "matched".to_string(),
                    selected_parser_id: Some(selected.candidate.parser_id.clone()),
                    candidates: vec![selected.candidate.clone()],
                    conflict_group: Vec::new(),
                    reason: "Exactly one dedicated Rust parser matched the uploaded file"
                        .to_string(),
                },
                selected: Some(selected),
            }
        }
        _ => {
            let candidates = matches
                .into_iter()
                .map(|candidate_match| candidate_match.candidate)
                .collect::<Vec<_>>();
            DedicatedParserSelection {
                decision: DedicatedParserDecision {
                    requested_parser: requested,
                    status: "conflict".to_string(),
                    selected_parser_id: None,
                    conflict_group: candidates
                        .iter()
                        .map(|candidate| candidate.parser_id.clone())
                        .collect(),
                    candidates,
                    reason: "Multiple dedicated Rust parsers matched the uploaded file".to_string(),
                },
                selected: None,
            }
        }
    }
}

fn no_match_decision(requested_parser: String, reason: &str) -> DedicatedParserDecision {
    DedicatedParserDecision {
        requested_parser,
        status: "no_match".to_string(),
        selected_parser_id: None,
        candidates: Vec::new(),
        conflict_group: Vec::new(),
        reason: reason.to_string(),
    }
}

fn parser_evidence(parser_id: &str, bills: &[StandardBill]) -> Vec<String> {
    let mut evidence = vec![
        format!("parsed_count={}", bills.len()),
        format!("parser_label={}", parser_source_label(parser_id)),
    ];
    if bills.iter().any(|bill| {
        bill.parser_tags
            .iter()
            .any(|tag| tag == &format!("parser:{parser_id}"))
    }) {
        evidence.push(format!("parser_tag=parser:{parser_id}"));
    }
    if bills.iter().any(|bill| !bill.date.trim().is_empty()) {
        evidence.push("has_transaction_time=true".to_string());
    }
    if bills.iter().any(|bill| bill.amount.to_cents() != 0) {
        evidence.push("has_amount=true".to_string());
    }
    evidence
}

#[cfg(test)]
mod tests {
    use super::auto_parser_candidates;

    fn candidate_ids(filename: &str, bytes: &[u8]) -> Vec<&'static str> {
        auto_parser_candidates(filename, bytes)
            .iter()
            .map(|parser| parser.id)
            .collect()
    }

    #[test]
    fn auto_parser_candidates_use_lightweight_content_hints() {
        assert_eq!(
            candidate_ids(
                "账号6226192003866332交易明细.xls",
                b"<html>\xd6\xd0\xb9\xfa\xc3\xf1\xc9\xfa\xd2\xf8\xd0\xd0</html>"
            ),
            ["cmbc"]
        );
        assert_eq!(
            candidate_ids("unknown.csv", "交易时间,交易金额\n微信支付,1.00".as_bytes()),
            ["wechat"]
        );
    }

    #[test]
    fn auto_parser_candidates_use_known_bank_export_filename_hints() {
        assert_eq!(candidate_ids("detail20250825.xlsx", b""), ["abc"]);
        assert_eq!(
            candidate_ids("交易明细_9316_20230827_20250827.xls", b""),
            ["ccb"]
        );
    }
}
