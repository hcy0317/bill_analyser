use crate::{
    parser_source_label, spreadsheet::prepare_dedicated_spreadsheet_payload,
    SpreadsheetValidationErrorKind,
};

use super::{
    evidence::parser_evidence,
    hints::auto_parser_candidates,
    registry::AUTO_PARSERS,
    types::{
        DedicatedParseResult, DedicatedParseSelectionResult, DedicatedParser,
        DedicatedParserCandidate, DedicatedParserDecision, DedicatedParserInput,
        DedicatedParserMatch, DedicatedParserSelection,
    },
};

/// 只返回 dedicated parser 选择决策，用于上传前的来源识别和冲突解释。
#[tracing::instrument(level = "debug", skip_all)]
pub fn detect_dedicated_import_bytes(
    filename: &str,
    bytes: &[u8],
    requested_parser: &str,
) -> DedicatedParserDecision {
    select_dedicated_import_bytes(filename, bytes, requested_parser).decision
}

/// 解析指定上传文件并在 exactly-one dedicated parser 命中时返回标准账单。
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

/// 解析 dedicated parser 并同时返回选择决策，供 API 暴露 matched/no_match/conflict 证据。
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

/// 执行 requested/auto dedicated parser 选择流程，并保留未命中或冲突的结构化证据。
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

    let suffix = std::path::Path::new(filename)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let prepared_spreadsheet = if matches!(suffix.as_str(), "xls" | "xlsx") {
        match prepare_dedicated_spreadsheet_payload(bytes) {
            Ok(prepared) => Some(prepared),
            Err(error) => {
                return DedicatedParserSelection {
                    decision: no_match_decision_with_code(
                        requested,
                        error.message(),
                        Some(spreadsheet_error_code(error.kind())),
                    ),
                    selected: None,
                };
            }
        }
    } else {
        None
    };
    let input = DedicatedParserInput::new(filename, bytes, prepared_spreadsheet.as_ref());

    let matches = if requested.is_empty() || requested == "auto" {
        let auto_candidates = auto_parser_candidates(filename, bytes);
        let mut matches = auto_candidates
            .iter()
            .filter_map(|parser| parse_with_parser(parser, &input))
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
                    .filter_map(|parser| parse_with_parser(parser, &input)),
            );
        }
        matches
    } else if let Some(parser) = AUTO_PARSERS.iter().find(|parser| parser.id == requested) {
        parse_with_parser(parser, &input)
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

/// 尝试运行单个来源 parser；只有解析出账单时才生成候选证据。
#[tracing::instrument(level = "debug", skip_all)]
fn parse_with_parser(
    parser: &DedicatedParser,
    input: &DedicatedParserInput<'_>,
) -> Option<DedicatedParserMatch> {
    let bills = (parser.parse)(input);
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

/// 将候选解析结果收敛成 exactly-one/no_match/conflict 三态决策。
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
                    error_code: None,
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
                    error_code: None,
                    reason: "Multiple dedicated Rust parsers matched the uploaded file".to_string(),
                },
                selected: None,
            }
        }
    }
}

/// 构造未命中决策，保持 API 暴露的 no_match 字段结构一致。
fn no_match_decision(requested_parser: String, reason: &str) -> DedicatedParserDecision {
    no_match_decision_with_code(requested_parser, reason, None)
}

fn no_match_decision_with_code(
    requested_parser: String,
    reason: &str,
    error_code: Option<&str>,
) -> DedicatedParserDecision {
    DedicatedParserDecision {
        requested_parser,
        status: "no_match".to_string(),
        selected_parser_id: None,
        candidates: Vec::new(),
        conflict_group: Vec::new(),
        error_code: error_code.map(str::to_string),
        reason: reason.to_string(),
    }
}

fn spreadsheet_error_code(kind: SpreadsheetValidationErrorKind) -> &'static str {
    match kind {
        SpreadsheetValidationErrorKind::TooLarge => "spreadsheet_too_large",
        SpreadsheetValidationErrorKind::Invalid => "invalid_spreadsheet",
    }
}
