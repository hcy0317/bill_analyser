/// 把 dedup 后账单投影为 preview draft，负责金额分、转账目标金额和可见反馈的初始形状。
pub fn preview_draft_from_dedup_bill(bill: &DedupBill) -> ImportPreviewDraft {
    let amount_cents = bill.amount.to_cents();
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
        preview_amount_cents: amount_cents.abs(),
        preview_destination_amount_cents: preview_destination_amount_cents_for_bill(
            bill,
            amount_cents,
        ),
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

/// 构建 parser/dedup/transfer/annotation feedback，决定前端 signal family 的初始可见证据。
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
                "source_label": transfer_source_label(&bill.transfer_pair_sources),
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
        "amount_cents": bill.amount.to_cents(),
    }));
    for source in &bill.merged_from {
        chain.push(serde_json::json!({
            "role": "duplicate",
            "source": source.source,
            "template_id": source.template_id,
            "date": source.date,
            "source_amount_text": source.amount,
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
