// 中文导读：导入预览 draft 构造与 feedback 投影辅助。
// 维护重点：把 DedupBill/历史账单候选转换成 import_preview_bills 可写 draft。
// 不变式：金额输出保持预览层元单位，history duplicate 仅标记待改写而不直接确认。

pub fn preview_draft_from_dedup_bill(bill: &DedupBill) -> ImportPreviewDraft {
    let amount = money_to_yuan_f64(bill.amount);
    let preview_payment_method =
        first_non_empty([bill.payment_method.as_str(), bill.parser_id.as_str()]);
    let no_income_expenditure = dedup_bill_has_no_income_expenditure_source(bill);
    let transfer_requires_review = dedup_bill_transfer_requires_review(bill);
    let preview_matching_feedback = preview_matching_feedback_from_dedup_bill(
        bill,
        no_income_expenditure,
        transfer_requires_review,
    );
    ImportPreviewDraft {
        preview_date: bill.date.clone(),
        preview_type: bill.transaction_type.clone(),
        preview_amount: amount.abs(),
        preview_destination_amount: preview_destination_amount_for_bill(bill, amount),
        preview_main_category: bill.main_category.clone(),
        preview_sub_category: bill.sub_category.clone(),
        preview_source_account_id: parse_positive_i64(&bill.source_account_id),
        preview_destination_account_id: bill
            .destination_account_id
            .as_deref()
            .and_then(parse_positive_i64),
        preview_counterparty: bill.counterparty.clone(),
        preview_payment_method,
        preview_description: bill.description.clone(),
        preview_parser_id: bill.parser_id.clone(),
        preview_parser_tags: dedup_bill_parser_tags_value(bill),
        preview_selected: false,
        dedup_type: Some(
            bill.dedup_type
                .clone()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "remaining".to_string()),
        ),
        dedup_source_ids: bill
            .dedup_source_ids()
            .iter()
            .filter_map(|value| parse_positive_i64(value))
            .collect(),
        preview_matching_feedback,
        ..ImportPreviewDraft::default()
    }
}

fn preview_matching_feedback_from_dedup_bill(
    bill: &DedupBill,
    no_income_expenditure: bool,
    transfer_requires_review: bool,
) -> Value {
    let dedup_type = bill
        .dedup_type
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("remaining");
    let mut feedback = serde_json::Map::new();
    feedback.insert(
        "parser".to_string(),
        serde_json::json!({
            "parser_id": bill.parser_id,
            "parser_tags": &bill.parser_tags,
            "payment_method": bill.payment_method,
            "counterparty": bill.counterparty,
        }),
    );
    feedback.insert(
        "dedup".to_string(),
        serde_json::json!({
            "type": dedup_type,
            "source_ids": bill.dedup_source_ids(),
            "source_count": bill.dedup_source_ids().len(),
            "source_chain": dedup_source_chain(bill),
            "source_label": dedup_source_label(bill),
        }),
    );
    if dedup_type == "transfer" || dedup_type == "transfer_cross_batch" {
        feedback.insert(
            "transfer".to_string(),
            serde_json::json!({
                "candidate_type": dedup_type,
                "score": 1.0,
                "level": "high",
                "reason": "smart_dedup transfer pair",
                "review_status": "pending",
                "pair_order": bill.transfer_pair_order,
                "source_chain": &bill.transfer_pair_sources,
            }),
        );
    }
    if no_income_expenditure {
        feedback.insert(
            "annotation".to_string(),
            serde_json::json!({
                "status": "needs_review",
                "type": "no_income_expenditure",
                "review_status": "suppressed",
                "suppressed": true,
                "reason": "original parser type is no-income-expenditure",
            }),
        );
    }
    if transfer_requires_review {
        feedback.insert(
            "annotation".to_string(),
            serde_json::json!({
                "status": "needs_review",
                "type": "transfer_account_direction",
                "review_status": "requires_account_review",
                "suppressed": true,
                "reason": "transfer preview is missing source or destination account",
            }),
        );
    }
    Value::Object(feedback)
}

fn dedup_source_chain(bill: &DedupBill) -> Vec<Value> {
    let mut chain = Vec::new();
    chain.push(serde_json::json!({
        "role": "base",
        "source": bill.source_identifier(),
        "parser_id": bill.parser_id,
        "template_id": bill.template_id,
        "date": bill.date,
        "amount": bill.amount.to_yuan_string(),
    }));
    for source in &bill.merged_from {
        chain.push(serde_json::json!({
            "role": "duplicate",
            "source": source.source,
            "template_id": source.template_id,
            "date": source.date,
            "amount": source.amount,
        }));
    }
    chain
}

fn dedup_source_label(bill: &DedupBill) -> String {
    dedup_source_chain(bill)
        .iter()
        .enumerate()
        .filter_map(|(index, value)| {
            let source = value
                .get("source")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .or_else(|| {
                    value
                        .get("parser_id")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                })?;
            Some(format!("来源{}: {}", index + 1, source))
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

fn dedup_bill_has_no_income_expenditure_source(bill: &DedupBill) -> bool {
    bill.original_type.trim().contains("不计收支")
        || bill
            .transfer_pair_sources
            .iter()
            .any(|source| source.original_type.trim().contains("不计收支"))
}

fn dedup_bill_transfer_requires_review(bill: &DedupBill) -> bool {
    let is_transfer = bill
        .dedup_type
        .as_deref()
        .is_some_and(|value| value.trim().to_ascii_lowercase().contains("transfer"));
    if !is_transfer {
        return false;
    }
    let source = parse_positive_i64(&bill.source_account_id);
    let destination = bill
        .destination_account_id
        .as_deref()
        .and_then(parse_positive_i64);
    source.is_none() || destination.is_none() || source == destination
}

pub fn preview_drafts_from_dedup_bills(bills: &[DedupBill]) -> Vec<ImportPreviewDraft> {
    bills.iter().map(preview_draft_from_dedup_bill).collect()
}

#[derive(Debug, Clone)]
pub struct ImportHistoryDuplicatePreviewInput<'a> {
    pub imported_bill: &'a DedupBill,
    pub history_bill: &'a ImportHistoryBillRow,
    pub candidate_id: &'a str,
    pub group_key: &'a str,
    pub time_diff_seconds: i64,
    pub score_percent: u8,
    pub level: &'a str,
    pub reason: &'a str,
}

pub fn preview_draft_from_history_duplicate(
    input: ImportHistoryDuplicatePreviewInput<'_>,
) -> ImportPreviewDraft {
    let history = &input.history_bill.bill;
    let amount = money_to_yuan_f64(history.amount);
    let source_ids = input
        .imported_bill
        .dedup_source_ids()
        .iter()
        .filter_map(|value| parse_positive_i64(value))
        .collect::<Vec<_>>();
    let counterparty = merge_preview_text(&history.counterparty, &input.imported_bill.counterparty);
    let payment_method =
        merge_preview_text(&history.payment_method, &input.imported_bill.payment_method);
    let description = merge_preview_text(&history.description, &input.imported_bill.description);
    let source_chain = serde_json::json!([
        {
            "role": "history_base",
            "history_bill_id": input.history_bill.history_bill_id,
            "history_bill_version": input.history_bill.history_bill_version,
            "parser_id": history.parser_id,
            "source": history.source,
            "date": history.date,
            "amount": history.amount.to_yuan_string(),
        },
        {
            "role": "import_duplicate",
            "template_id": input.imported_bill.template_id,
            "parser_id": input.imported_bill.parser_id,
            "source": input.imported_bill.source_identifier(),
            "date": input.imported_bill.date,
            "amount": input.imported_bill.amount.to_yuan_string(),
        }
    ]);

    ImportPreviewDraft {
        preview_date: history.date.clone(),
        preview_type: history.transaction_type.clone(),
        preview_amount: amount.abs(),
        preview_destination_amount: preview_destination_amount_for_bill(history, amount),
        preview_main_category: history.main_category.clone(),
        preview_sub_category: history.sub_category.clone(),
        preview_source_account_id: parse_positive_i64(&history.source_account_id),
        preview_destination_account_id: history
            .destination_account_id
            .as_deref()
            .and_then(parse_positive_i64),
        preview_counterparty: counterparty,
        preview_payment_method: payment_method,
        preview_description: description,
        preview_parser_id: "history_db".to_string(),
        preview_parser_tags: Some(serde_json::json!(["history:db", "signal:database_duplicate"])),
        preview_selected: false,
        dedup_type: Some(DeduplicationType::DatabaseDuplicate.as_str().to_string()),
        dedup_source_ids: source_ids.clone(),
        preview_matching_feedback: serde_json::json!({
            "parser": {
                "parser_id": "history_db",
                "parser_tags": ["history:db", "signal:database_duplicate"],
                "payment_method": history.payment_method,
                "counterparty": history.counterparty,
            },
            "dedup": {
                "type": DeduplicationType::DatabaseDuplicate.as_str(),
                "source_ids": source_ids,
                "source_count": input.imported_bill.dedup_source_ids().len(),
                "history_bill_id": input.history_bill.history_bill_id,
                "history_bill_version": input.history_bill.history_bill_version,
                "planned_operation": "update_history",
                "source_chain": source_chain,
            },
            "reconciliation": {
                "candidate_id": input.candidate_id,
                "candidate_type": "duplicate",
                "group_key": input.group_key,
                "history_bill_id": input.history_bill.history_bill_id,
                "history_bill_version": input.history_bill.history_bill_version,
                "row_origin": "history",
                "planned_operation": "update_history",
                "review_status": "pending",
                "time_diff_seconds": input.time_diff_seconds,
                "score": f64::from(input.score_percent) / 100.0,
                "level": input.level,
                "reason": input.reason,
                "notice": "将改写/合并历史账单",
            },
            "annotation": {
                "status": "needs_review",
                "type": "history_rewrite_pending",
                "review_status": "requires_history_confirm_runtime",
                "suppressed": true,
                "reason": "historical duplicate is materialized for preview; confirm rewrite is handled by history-confirm operation",
            }
        }),
        ..ImportPreviewDraft::default()
    }
}

impl Default for ImportPreviewDraft {
    fn default() -> Self {
        Self {
            preview_date: String::new(),
            preview_type: String::new(),
            preview_amount: 0.0,
            preview_destination_amount: 0.0,
            preview_main_category: String::new(),
            preview_sub_category: String::new(),
            preview_source_account_id: None,
            preview_destination_account_id: None,
            preview_counterparty: String::new(),
            preview_payment_method: String::new(),
            preview_description: String::new(),
            preview_parser_id: String::new(),
            preview_parser_tags: None,
            preview_recurring_id: None,
            preview_recurring_name: String::new(),
            preview_recurring_candidate_count: 0,
            preview_recurring_match_score: 0.0,
            preview_recurring_match_reasons: String::new(),
            preview_recurring_matched_date: String::new(),
            preview_selected: false,
            dedup_type: None,
            dedup_source_ids: Vec::new(),
            preview_matching_feedback: Value::Object(Default::default()),
        }
    }
}

fn merge_preview_text(left: &str, right: &str) -> String {
    let left = left.trim();
    let right = right.trim();
    if left.is_empty() {
        return right.to_string();
    }
    if right.is_empty() || left == right || left.contains(right) {
        return left.to_string();
    }
    if right.contains(left) {
        return right.to_string();
    }
    let mut values = Vec::new();
    for value in [left, right] {
        for part in value
            .split('|')
            .map(str::trim)
            .filter(|part| !part.is_empty())
        {
            if !values
                .iter()
                .any(|existing: &String| existing == part || existing.contains(part) || part.contains(existing))
            {
                values.push(part.to_string());
            }
        }
    }
    values.join(" | ")
}
