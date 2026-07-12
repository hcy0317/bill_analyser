/// 把历史转账候选投影成待审核 preview draft，只生成证据和预览态，不直接改写正式账单。
pub fn preview_draft_from_history_transfer(
    input: ImportHistoryTransferPreviewInput<'_>,
) -> ImportPreviewDraft {
    let history = &input.history_bill.bill;
    let import_is_outgoing = input.imported_bill.amount.is_negative();
    let (outgoing, incoming, outgoing_role, incoming_role) = if import_is_outgoing {
        (
            input.imported_bill,
            history,
            "import_outgoing",
            "current_bill_incoming",
        )
    } else {
        (
            history,
            input.imported_bill,
            "current_bill_outgoing",
            "import_incoming",
        )
    };
    let amount_cents = outgoing.amount.to_cents();
    let destination_amount_cents = incoming.amount.to_cents().abs();
    let source_ids = input
        .imported_bill
        .dedup_source_ids()
        .iter()
        .filter_map(|value| parse_positive_i64(value))
        .collect::<Vec<_>>();
    let counterparty = merge_preview_text(&outgoing.counterparty, &incoming.counterparty);
    let payment_method = merge_preview_text(&outgoing.payment_method, &incoming.payment_method);
    let description = merge_preview_text(&outgoing.description, &incoming.description);
    let transfer_sources = vec![
        build_transfer_source_snapshot(outgoing, "outgoing"),
        build_transfer_source_snapshot(incoming, "incoming"),
    ];
    let source_label = transfer_source_label(&transfer_sources);
    let planned_operation = normalize_history_operation("merge_current_bill_transfer")
        .expect("known history operation")
        .as_str();
    let source_chain = serde_json::json!([
        {
            "role": outgoing_role,
            "history_bill_id": if outgoing_role.starts_with("current_bill") { Some(input.history_bill.history_bill_id) } else { None },
            "history_bill_version": if outgoing_role.starts_with("current_bill") { Some(input.history_bill.history_bill_version) } else { None },
            "template_id": if outgoing_role.starts_with("import") { input.imported_bill.template_id.clone() } else { None },
            "parser_id": outgoing.parser_id,
            "source": outgoing.source_identifier(),
            "date": outgoing.date,
            "amount_cents": outgoing.amount.to_cents(),
            "counterparty": outgoing.counterparty,
            "payment_method": outgoing.payment_method,
            "description": outgoing.description,
        },
        {
            "role": incoming_role,
            "history_bill_id": if incoming_role.starts_with("current_bill") { Some(input.history_bill.history_bill_id) } else { None },
            "history_bill_version": if incoming_role.starts_with("current_bill") { Some(input.history_bill.history_bill_version) } else { None },
            "template_id": if incoming_role.starts_with("import") { input.imported_bill.template_id.clone() } else { None },
            "parser_id": incoming.parser_id,
            "source": incoming.source_identifier(),
            "date": incoming.date,
            "amount_cents": incoming.amount.to_cents(),
            "counterparty": incoming.counterparty,
            "payment_method": incoming.payment_method,
            "description": incoming.description,
        }
    ]);

    ImportPreviewDraft {
        preview_date: outgoing.date.clone(),
        preview_type: "转账".to_string(),
        preview_amount_cents: amount_cents.abs(),
        preview_destination_amount_cents: destination_amount_cents,
        preview_main_category: outgoing.main_category.clone(),
        preview_sub_category: outgoing.sub_category.clone(),
        preview_source_account_id: parse_positive_i64(&outgoing.source_account_id),
        preview_destination_account_id: parse_positive_i64(&incoming.source_account_id),
        preview_counterparty: counterparty,
        preview_payment_method: payment_method,
        preview_description: description,
        preview_parser_id: outgoing.parser_id.clone(),
        preview_parser_tags: Some(serde_json::json!([
            "postgres:bill",
            "signal:database_transfer",
            format!("parser:{}", outgoing.parser_id),
            format!("parser:{}", incoming.parser_id),
        ])),
        preview_selected: false,
        dedup_type: Some("transfer_cross_batch".to_string()),
        dedup_source_ids: source_ids.clone(),
        preview_matching_feedback: serde_json::json!({
            "parser": {
                "parser_id": outgoing.parser_id,
                "parser_tags": [
                    "postgres:bill",
                    "signal:database_transfer",
                    format!("parser:{}", outgoing.parser_id),
                    format!("parser:{}", incoming.parser_id),
                ],
                "payment_method": outgoing.payment_method,
                "counterparty": outgoing.counterparty,
            },
            "dedup": {
                "type": "transfer_cross_batch",
                "source_ids": source_ids,
                "source_count": input.imported_bill.dedup_source_ids().len(),
                "history_bill_id": input.history_bill.history_bill_id,
                "history_bill_version": input.history_bill.history_bill_version,
                "planned_operation": planned_operation,
                "source_chain": source_chain,
                "source_label": source_label,
            },
            "transfer": {
                "candidate_type": "transfer_cross_batch",
                "score": f64::from(input.score_percent) / 100.0,
                "level": input.level,
                "reason": "current PostgreSQL bill transfer pair",
                "review_status": "pending",
                "pair_order": if import_is_outgoing { "outgoing_import" } else { "outgoing_current_bill" },
                "source_chain": transfer_sources,
                "source_label": source_label,
            },
            "reconciliation": {
                "candidate_id": input.candidate_id,
                "candidate_type": "transfer",
                "group_key": input.group_key,
                "history_bill_id": input.history_bill.history_bill_id,
                "history_bill_version": input.history_bill.history_bill_version,
                "row_origin": "postgres_bill",
                "planned_operation": planned_operation,
                "history_role": if import_is_outgoing { "incoming" } else { "outgoing" },
                "history_summary": history_summary(input.history_bill),
                "import_role": if import_is_outgoing { "outgoing" } else { "incoming" },
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
                "reason": "transfer candidate is materialized for preview; confirm handles the current PostgreSQL bill update",
            }
        }),
        ..ImportPreviewDraft::default()
    }
}
