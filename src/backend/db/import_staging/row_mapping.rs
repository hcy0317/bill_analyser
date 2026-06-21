fn import_session_from_pg_row(row: &PgRow) -> DbResult<ImportSessionRow> {
    let created_at = format_pg_time(row.try_get("created_at")?);
    let updated_at = format_pg_time(row.try_get("updated_at")?);
    Ok(ImportSessionRow {
        id: row.try_get("id")?,
        session_id: row.try_get("session_key")?,
        user_id: row.try_get("user_id")?,
        status: row.try_get("status")?,
        file_count: row.try_get::<i64, _>("file_count")?,
        total_parsed: row.try_get::<i64, _>("total_parsed")?,
        total_preview: row.try_get::<i64, _>("total_preview")?,
        total_confirmed: row.try_get::<i64, _>("total_confirmed")?,
        created_at,
        updated_at,
    })
}

fn import_source_from_pg_row(row: &PgRow) -> DbResult<ImportSourceRow> {
    Ok(ImportSourceRow {
        id: row.try_get("id")?,
        session_id: row.try_get("session_key")?,
        user_id: row.try_get("user_id")?,
        source_index: row.try_get::<i32, _>("source_index")? as i64,
        original_file_name: row
            .try_get::<Option<String>, _>("original_file_name")?
            .unwrap_or_default(),
        parser_id: row.try_get("parser_id")?,
        parser_name: row.try_get("parser_name")?,
        parser_signal: row
            .try_get::<Option<String>, _>("parser_signal")?
            .unwrap_or_default(),
        parser_confidence: row
            .try_get::<Option<f64>, _>("parser_confidence")?
            .unwrap_or_default(),
        feature_signature: row.try_get("feature_signature")?,
        metadata: row.try_get("metadata")?,
        created_at: format_pg_time(row.try_get("created_at")?),
        updated_at: format_pg_time(row.try_get("updated_at")?),
    })
}

fn import_standard_row_from_pg_row(row: &PgRow) -> DbResult<ImportStandardRow> {
    Ok(ImportStandardRow {
        id: row.try_get("id")?,
        session_id: row.try_get("session_key")?,
        source_id: row.try_get("source_id")?,
        user_id: row.try_get("user_id")?,
        source_index: row.try_get::<i32, _>("source_index")? as i64,
        source_row_index: row.try_get::<i32, _>("source_row_index")? as i64,
        parser_id: row.try_get("parser_id")?,
        occurred_at: format_pg_time(row.try_get("occurred_at")?),
        amount_cents: row.try_get("amount_cents")?,
        direction: row.try_get("direction")?,
        transaction_type: row.try_get("transaction_type")?,
        merchant: row
            .try_get::<Option<String>, _>("merchant")?
            .unwrap_or_default(),
        payment_method: row
            .try_get::<Option<String>, _>("payment_method")?
            .unwrap_or_default(),
        description: row
            .try_get::<Option<String>, _>("description")?
            .unwrap_or_default(),
        parser_payload: row.try_get("parser_payload")?,
        standard_payload: row.try_get("standard_payload")?,
        created_at: format_pg_time(row.try_get("created_at")?),
        updated_at: format_pg_time(row.try_get("updated_at")?),
    })
}

fn parser_template_from_pg_row(row: &PgRow) -> DbResult<ImportParserTemplateRow> {
    let payload: Value = row.try_get("parser_payload")?;
    Ok(ImportParserTemplateRow {
        id: row.try_get("id")?,
        session_id: row.try_get("session_key")?,
        user_id: row.try_get("user_id")?,
        parser_date: format_pg_time(row.try_get("occurred_at")?),
        parser_amount: (row.try_get::<i64, _>("amount_cents")? as f64) / 100.0,
        parser_type: payload_text(&payload, "parser_type")
            .unwrap_or_else(|| row.try_get("transaction_type").unwrap_or_default()),
        parser_description: payload_text(&payload, "parser_description").unwrap_or_else(|| {
            row.try_get::<Option<String>, _>("description")
                .ok()
                .flatten()
                .unwrap_or_default()
        }),
        parser_id: payload_text(&payload, "parser_id")
            .or_else(|| {
                row.try_get::<Option<String>, _>("source_parser_id")
                    .ok()
                    .flatten()
            })
            .unwrap_or_else(|| "auto".to_string()),
        parser_tags: payload
            .get("parser_tags")
            .and_then(Value::as_array)
            .map(|items| items.iter().filter_map(value_string).collect())
            .unwrap_or_default(),
        parser_counterparty: payload_text(&payload, "parser_counterparty").unwrap_or_else(|| {
            row.try_get::<Option<String>, _>("merchant")
                .ok()
                .flatten()
                .unwrap_or_default()
        }),
        parser_payment_method: payload_text(&payload, "parser_payment_method").unwrap_or_else(
            || {
                row.try_get::<Option<String>, _>("payment_method")
                    .ok()
                    .flatten()
                    .unwrap_or_default()
            },
        ),
        parser_original_type: payload_text(&payload, "parser_original_type").unwrap_or_default(),
        parser_original_category: payload_text(&payload, "parser_original_category")
            .unwrap_or_default(),
        parser_account_id: payload_text(&payload, "parser_account_id").unwrap_or_default(),
        parser_is_processed: payload_bool(&payload, "parser_is_processed"),
        created_at: format_pg_time(row.try_get("created_at")?),
    })
}

fn preview_from_pg_row(row: &PgRow) -> DbResult<ImportPreviewRow> {
    let payload: Value = row.try_get("preview_payload")?;
    let session_id: String = row.try_get("session_key")?;
    let preview_amount_cents = payload_i64(&payload, "preview_amount_cents")
        .unwrap_or_else(|| row.try_get::<i64, _>("amount_cents").unwrap_or_default());
    Ok(ImportPreviewRow {
        id: row.try_get("id")?,
        session_id,
        user_id: row.try_get("user_id")?,
        preview_date: payload_text(&payload, "preview_date").unwrap_or_else(|| {
            format_pg_time(row.try_get("occurred_at").unwrap_or_else(|_| Utc::now()))
        }),
        preview_type: payload_text(&payload, "preview_type")
            .unwrap_or_else(|| row.try_get("transaction_type").unwrap_or_default()),
        preview_amount_cents,
        preview_destination_amount_cents: payload_i64(&payload, "preview_destination_amount_cents")
            .unwrap_or_default(),
        category_id: payload_i64(&payload, "category_id")
            .or_else(|| payload_i64(&payload, "categoryId"))
            .or_else(|| row.try_get::<Option<i64>, _>("category_id").ok().flatten()),
        preview_main_category: payload_text(&payload, "preview_main_category").unwrap_or_default(),
        preview_sub_category: payload_text(&payload, "preview_sub_category").unwrap_or_default(),
        preview_source_account_id: payload_i64(&payload, "preview_source_account_id")
            .or_else(|| row.try_get::<Option<i64>, _>("account_id").ok().flatten()),
        preview_destination_account_id: payload_i64(&payload, "preview_destination_account_id")
            .or_else(|| {
                row.try_get::<Option<i64>, _>("transfer_target_account_id")
                    .ok()
                    .flatten()
            }),
        preview_counterparty: payload_text(&payload, "preview_counterparty").unwrap_or_else(|| {
            row.try_get::<Option<String>, _>("merchant")
                .ok()
                .flatten()
                .unwrap_or_default()
        }),
        preview_payment_method: payload_text(&payload, "preview_payment_method").unwrap_or_else(
            || {
                row.try_get::<Option<String>, _>("payment_method")
                    .ok()
                    .flatten()
                    .unwrap_or_default()
            },
        ),
        preview_description: payload_text(&payload, "preview_description").unwrap_or_else(|| {
            row.try_get::<Option<String>, _>("description")
                .ok()
                .flatten()
                .unwrap_or_default()
        }),
        preview_parser_id: payload_text(&payload, "preview_parser_id").unwrap_or_default(),
        preview_parser_tags: payload_array_strings(&payload, "preview_parser_tags"),
        preview_recurring_id: payload_i64(&payload, "preview_recurring_id"),
        preview_recurring_name: payload_text(&payload, "preview_recurring_name")
            .unwrap_or_default(),
        preview_recurring_candidate_count: payload_i64(
            &payload,
            "preview_recurring_candidate_count",
        )
        .unwrap_or_default(),
        preview_recurring_match_score: payload_f64(&payload, "preview_recurring_match_score")
            .unwrap_or_default(),
        preview_recurring_match_reasons: payload_text(&payload, "preview_recurring_match_reasons")
            .unwrap_or_default(),
        preview_recurring_matched_date: payload_text(&payload, "preview_recurring_matched_date")
            .unwrap_or_default(),
        preview_selected: row
            .try_get("selected")
            .unwrap_or_else(|_| payload_bool(&payload, "preview_selected")),
        dedup_type: payload_text(&payload, "dedup_type").unwrap_or_default(),
        dedup_source_ids: row
            .try_get::<Vec<i64>, _>("merged_source_ids")
            .unwrap_or_default(),
        preview_matching_feedback: payload
            .get("preview_matching_feedback")
            .cloned()
            .unwrap_or_else(|| json!({})),
        created_at: format_pg_time(row.try_get("created_at")?),
    })
}

fn import_decision_member_from_pg_row(row: &PgRow) -> DbResult<ImportDecisionGroupMemberRow> {
    Ok(ImportDecisionGroupMemberRow {
        id: row.try_get("id")?,
        group_id: row.try_get("group_id")?,
        preview_row_id: row.try_get("preview_row_id")?,
        standard_row_id: row.try_get("standard_row_id")?,
        history_bill_id: row.try_get("history_bill_id")?,
        member_role: row.try_get("member_role")?,
        parser_name: row
            .try_get::<Option<String>, _>("parser_name")?
            .unwrap_or_default(),
        metadata: row.try_get("metadata")?,
        created_at: format_pg_time(row.try_get("created_at")?),
    })
}

fn llm_memory_event_from_pg_row(row: &PgRow) -> DbResult<LlmMemoryEventRow> {
    Ok(LlmMemoryEventRow {
        id: row.try_get("id")?,
        user_id: row.try_get("user_id")?,
        session_id: row.try_get("session_id")?,
        preview_id: row.try_get("preview_id")?,
        event_type: row.try_get("event_type")?,
        decision: row.try_get("decision")?,
        prompt_text: row.try_get("prompt_text")?,
        llm_response_raw: row.try_get("llm_response_raw")?,
        llm_provider: row.try_get("llm_provider")?,
        llm_model: row.try_get("llm_model")?,
        suggested_main_category: row.try_get("suggested_main_category")?,
        suggested_sub_category: row.try_get("suggested_sub_category")?,
        suggested_source_account: row.try_get("suggested_source_account")?,
        suggested_destination_account: row.try_get("suggested_destination_account")?,
        confidence: row.try_get("confidence")?,
        user_correction_category: row.try_get("user_correction_category")?,
        user_correction_account: row.try_get("user_correction_account")?,
        snapshot_before: row.try_get("snapshot_before")?,
        snapshot_after: row.try_get("snapshot_after")?,
        metadata: row.try_get("metadata")?,
        created_at: format_pg_time(row.try_get("created_at")?),
    })
}
