/// 把历史重复候选投影成待审核 preview draft，保留 destructive ack 所需的正式账单版本证据。
pub fn preview_draft_from_history_duplicate(
    input: ImportHistoryDuplicatePreviewInput<'_>,
) -> ImportPreviewDraft {
    let history = &input.history_bill.bill;
    let amount_cents = history.amount.to_cents();
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
            "role": "current_bill_base",
            "history_bill_id": input.history_bill.history_bill_id,
            "history_bill_version": input.history_bill.history_bill_version,
            "parser_id": history.parser_id,
            "source": history.source,
            "date": history.date,
            "amount_cents": history.amount.to_cents(),
        },
        {
            "role": "import_duplicate",
            "template_id": input.imported_bill.template_id,
            "parser_id": input.imported_bill.parser_id,
            "source": input.imported_bill.source_identifier(),
            "date": input.imported_bill.date,
            "amount_cents": input.imported_bill.amount.to_cents(),
        }
    ]);

    ImportPreviewDraft {
        preview_date: history.date.clone(),
        preview_type: history.transaction_type.clone(),
        preview_amount_cents: amount_cents.abs(),
        preview_destination_amount_cents: preview_destination_amount_cents_for_bill(
            history,
            amount_cents,
        ),
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
        preview_parser_id: "current_postgres_bill".to_string(),
        preview_parser_tags: Some(serde_json::json!([
            "postgres:bill",
            "signal:database_duplicate"
        ])),
        preview_selected: false,
        dedup_type: Some(DeduplicationType::DatabaseDuplicate.as_str().to_string()),
        dedup_source_ids: source_ids.clone(),
        preview_matching_feedback: serde_json::json!({
            "parser": {
                "parser_id": "current_postgres_bill",
                "parser_tags": ["postgres:bill", "signal:database_duplicate"],
                "payment_method": history.payment_method,
                "counterparty": history.counterparty,
            },
            "dedup": {
                "type": DeduplicationType::DatabaseDuplicate.as_str(),
                "source_ids": source_ids,
                "source_count": input.imported_bill.dedup_source_ids().len(),
                "history_bill_id": input.history_bill.history_bill_id,
                "history_bill_version": input.history_bill.history_bill_version,
                "planned_operation": "update_current_bill",
                "source_chain": source_chain,
            },
            "reconciliation": {
                "candidate_id": input.candidate_id,
                "candidate_type": "duplicate",
                "group_key": input.group_key,
                "history_bill_id": input.history_bill.history_bill_id,
                "history_bill_version": input.history_bill.history_bill_version,
                "row_origin": "postgres_bill",
                "planned_operation": "update_current_bill",
                "review_status": "pending",
                "time_diff_seconds": input.time_diff_seconds,
                "score": f64::from(input.score_percent) / 100.0,
                "level": input.level,
                "reason": input.reason,
                "notice": "将合并到当前 PostgreSQL 账单",
            },
            "annotation": {
                "status": "needs_review",
                "type": "current_bill_rewrite_pending",
                "review_status": "requires_confirm_runtime",
                "suppressed": true,
                "reason": "duplicate candidate is materialized for preview; confirm handles the current PostgreSQL bill update",
            }
        }),
        ..ImportPreviewDraft::default()
    }
}
