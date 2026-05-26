// 中文导读：HTTP 运行态层，负责导入解析响应中的 source/standard row 台账投影。
// 维护重点：只生成 staging draft 和 parser decision JSON，不执行数据库写入。
// 不变式：金额进入 standard row 台账时使用分单位，parser decision 保留文件级来源证据。

fn parsed_standard_bills_from_standard_bills(
    standard_bills: Vec<StandardBill>,
    parser_id: &str,
    source_index: i64,
    parser_decision: Value,
) -> Vec<ImportParsedStandardBill> {
    standard_bills
        .into_iter()
        .enumerate()
        .map(|(source_row_index, bill)| ImportParsedStandardBill {
            source_index,
            source_row_index: i64::try_from(source_row_index).unwrap_or(i64::MAX),
            parser_id: parser_id.to_string(),
            bill,
            parser_decision: parser_decision.clone(),
        })
        .collect()
}

struct ImportSourceDraftInput<'a> {
    session_id: &'a str,
    source_index: i64,
    original_file_name: &'a str,
    parser_id: &'a str,
    parser_signal: &'a str,
    parser_confidence: f64,
    parser_decision: Value,
    body: &'a [u8],
}

fn import_source_draft(input: ImportSourceDraftInput<'_>) -> ImportSourceDraft {
    ImportSourceDraft {
        source_index: input.source_index,
        original_file_name: input.original_file_name.to_string(),
        parser_id: input.parser_id.to_string(),
        parser_name: parser_source_label(input.parser_id).into_owned(),
        parser_signal: input.parser_signal.to_string(),
        parser_confidence: input.parser_confidence,
        feature_signature: import_source_feature_signature(
            input.session_id,
            input.source_index,
            input.original_file_name,
            input.parser_id,
            input.parser_signal,
            input.body,
        ),
        metadata: json!({
            "parser_decision": input.parser_decision,
        }),
    }
}

fn import_standard_row_drafts_from_parsed_bills(
    parsed_bills: &[ImportParsedStandardBill],
) -> Vec<ImportStandardRowDraft> {
    parsed_bills
        .iter()
        .map(|parsed| {
            let standard_payload =
                serde_json::to_value(&parsed.bill).unwrap_or_else(|_| json!({}));
            ImportStandardRowDraft {
                source_index: parsed.source_index,
                source_row_index: parsed.source_row_index,
                occurred_at: parsed.bill.date.clone(),
                amount_cents: parsed.bill.amount.to_cents(),
                direction: standard_row_direction(&parsed.bill),
                transaction_type: standard_row_transaction_type(&parsed.bill.transaction_type),
                merchant: parsed.bill.counterparty.clone(),
                payment_method: parsed.bill.payment_method.clone(),
                description: parsed.bill.description.clone(),
                parser_payload: json!({
                    "parser_id": parsed.parser_id,
                    "parser_tags": parsed.bill.parser_tags,
                    "parser_decision": parsed.parser_decision,
                    "original_type": parsed.bill.original_type,
                    "original_category": parsed.bill.original_category,
                    "transaction_id": parsed.bill.transaction_id,
                    "merchant_id": parsed.bill.merchant_id,
                    "status": parsed.bill.status,
                }),
                standard_payload,
            }
        })
        .collect()
}

fn standard_row_direction(bill: &StandardBill) -> String {
    if bill.amount.is_negative() {
        "expense".to_string()
    } else {
        "income".to_string()
    }
}

fn standard_row_transaction_type(raw_type: &str) -> String {
    match raw_type.trim() {
        "收入" => "income".to_string(),
        "支出" => "expense".to_string(),
        "转账" => "transfer".to_string(),
        "投资" => "investment".to_string(),
        "退款" => "refund".to_string(),
        other if !other.is_empty() => other.to_string(),
        _ => "expense".to_string(),
    }
}

fn provided_parser_decision(parser_id: &str) -> Value {
    json!({
        "requested_parser": parser_id,
        "status": "provided",
        "selected_parser_id": parser_id,
        "candidates": [{
            "parser_id": parser_id,
            "parser_label": parser_source_label(parser_id),
            "confidence": 1.0,
            "parsed_count": 0,
            "evidence": ["provided_standard_rows"],
        }],
        "conflict_group": [],
        "reason": "Parser id was provided with already-normalized standard rows",
    })
}

fn parser_decision_json(decision: &DedicatedParserDecision) -> Value {
    serde_json::to_value(decision).unwrap_or_else(|_| json!({}))
}

fn selected_parser_confidence(decision: &DedicatedParserDecision) -> f64 {
    let Some(selected_parser_id) = decision.selected_parser_id.as_deref() else {
        return 0.0;
    };
    decision
        .candidates
        .iter()
        .find(|candidate| candidate.parser_id == selected_parser_id)
        .map(|candidate| candidate.confidence)
        .unwrap_or_default()
}

fn import_source_feature_signature(
    session_id: &str,
    source_index: i64,
    original_file_name: &str,
    parser_id: &str,
    parser_signal: &str,
    body: &[u8],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(session_id.as_bytes());
    hasher.update(b"\0");
    hasher.update(source_index.to_string().as_bytes());
    hasher.update(b"\0");
    hasher.update(original_file_name.as_bytes());
    hasher.update(b"\0");
    hasher.update(parser_id.as_bytes());
    hasher.update(b"\0");
    hasher.update(parser_signal.as_bytes());
    hasher.update(b"\0");
    hasher.update(body);
    format!("{:x}", hasher.finalize())
}
