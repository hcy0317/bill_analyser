// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。

const INSERT_PREVIEW_BILL_SQL: &str = "
        INSERT INTO bills_preview (
            session_id, user_id, preview_date, preview_type,
            preview_amount, preview_destination_amount,
            preview_main_category, preview_sub_category,
            preview_source_account_id, preview_destination_account_id,
            preview_counterparty, preview_payment_method, preview_description,
            preview_parser_id, preview_parser_tags_json,
            preview_recurring_id, preview_recurring_name,
            preview_recurring_candidate_count, preview_recurring_match_score,
            preview_recurring_match_reasons, preview_recurring_matched_date,
            preview_selected, dedup_type, dedup_source_ids,
            preview_matching_feedback_json, created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26)
        ";

const INSERT_PARSER_TEMPLATE_SQL: &str = "
        INSERT INTO bills_parser_template (
            session_id, user_id, parser_date, parser_amount,
            parser_type, parser_description, parser_id,
            parser_tags_json, parser_counterparty, parser_payment_method,
            parser_original_type, parser_original_category,
            parser_account_id, parser_is_processed, created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, '0', ?14)
        ";

fn insert_preview_bill_on_connection(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
    draft: &ImportPreviewDraft,
) -> DbResult<()> {
    let mut statement = connection.prepare(INSERT_PREVIEW_BILL_SQL)?;
    insert_preview_bill_with_statement(
        &mut statement,
        session_id,
        user_id_i64(user_id)?,
        draft,
        &now_text(),
    )
}

fn insert_preview_bill_with_statement(
    statement: &mut rusqlite::Statement<'_>,
    session_id: &str,
    user_id: i64,
    draft: &ImportPreviewDraft,
    created_at: &str,
) -> DbResult<()> {
    let parser_tags_json = serialize_parser_tags(
        draft.preview_parser_tags.as_ref(),
        &draft.preview_parser_id,
        &draft.preview_payment_method,
        "",
    );
    let dedup_source_ids = draft
        .dedup_source_ids
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(",");

    statement.execute(
        params![
            session_id,
            user_id,
            normalize_bill_date_text(&draft.preview_date),
            draft.preview_type,
            draft.preview_amount,
            draft.preview_destination_amount,
            draft.preview_main_category,
            draft.preview_sub_category,
            draft.preview_source_account_id,
            draft.preview_destination_account_id,
            draft.preview_counterparty,
            draft.preview_payment_method,
            draft.preview_description,
            draft.preview_parser_id,
            parser_tags_json,
            draft.preview_recurring_id,
            draft.preview_recurring_name,
            draft.preview_recurring_candidate_count,
            draft.preview_recurring_match_score,
            draft.preview_recurring_match_reasons,
            draft.preview_recurring_matched_date,
            if draft.preview_selected { 1 } else { 0 },
            draft.dedup_type,
            dedup_source_ids,
            serialize_preview_matching_feedback(&draft.preview_matching_feedback),
            created_at,
        ],
    )?;
    Ok(())
}

fn insert_parser_template_on_connection(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
    draft: &ImportParserTemplateDraft,
) -> DbResult<()> {
    let mut statement = connection.prepare(INSERT_PARSER_TEMPLATE_SQL)?;
    insert_parser_template_with_statement(
        &mut statement,
        session_id,
        user_id_i64(user_id)?,
        draft,
        &now_text(),
    )
}

fn insert_parser_template_with_statement(
    statement: &mut rusqlite::Statement<'_>,
    session_id: &str,
    user_id: i64,
    draft: &ImportParserTemplateDraft,
    created_at: &str,
) -> DbResult<()> {
    let parser_tags_json = serialize_parser_tags(
        draft.parser_tags.as_ref(),
        &draft.parser_id,
        &draft.parser_payment_method,
        "",
    );

    statement.execute(
        params![
            session_id,
            user_id,
            normalize_bill_date_text(&draft.parser_date),
            draft.parser_amount,
            draft.parser_type,
            draft.parser_description,
            draft.parser_id,
            parser_tags_json,
            draft.parser_counterparty,
            draft.parser_payment_method,
            draft.parser_original_type,
            draft.parser_original_category,
            draft.parser_account_id,
            created_at,
        ],
    )?;
    Ok(())
}

fn execute_preview_patch(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
    patch: &ImportPreviewPatch,
) -> DbResult<usize> {
    if patch.preview_id <= 0 {
        return Ok(0);
    }

    let mut assignments = Vec::new();
    let mut params = Vec::new();
    for (field, value) in &patch.changes {
        assignments.push(format!("{} = ?", field.column_name()));
        params.push(value.clone().into_sql_value());
    }

    if patch.clear_transfer_decision {
        if let Some(raw_feedback) = connection
            .query_row(
                "
                SELECT preview_matching_feedback_json
                FROM bills_preview
                WHERE id = ?1 AND session_id = ?2 AND user_id = ?3
                ",
                params![patch.preview_id, session_id, user_id_i64(user_id)?],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?
        {
            assignments.push("preview_matching_feedback_json = ?".to_string());
            params.push(SqlValue::Text(clear_transfer_matching_feedback(Some(
                raw_feedback.as_deref().unwrap_or(""),
            ))));
        }
    }

    if assignments.is_empty() {
        return Ok(0);
    }

    params.push(SqlValue::Integer(patch.preview_id));
    params.push(SqlValue::Text(session_id.to_string()));
    params.push(SqlValue::Integer(user_id_i64(user_id)?));
    let query = format!(
        "UPDATE bills_preview SET {} WHERE id = ? AND session_id = ? AND user_id = ?",
        assignments.join(", ")
    );
    connection
        .execute(&query, rusqlite::params_from_iter(params))
        .map_err(DbError::from)
}

fn import_session_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ImportSessionRow> {
    Ok(ImportSessionRow {
        id: row.get("id")?,
        session_id: row.get("session_id")?,
        user_id: row.get("user_id")?,
        status: row.get("status")?,
        file_count: row.get("file_count")?,
        total_parsed: row.get("total_parsed")?,
        total_preview: row.get("total_preview")?,
        total_confirmed: row.get("total_confirmed")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn annotation_sample_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<ImportAnnotationSampleRow> {
    Ok(ImportAnnotationSampleRow {
        id: row.get("id")?,
        session_id: row.get("session_id")?,
        user_id: row.get("user_id")?,
        preview_id: row.get("preview_id")?,
        annotated_type: row.get("annotated_type")?,
        annotated_category_id: row.get("annotated_category_id")?,
        annotated_source_account_id: row.get("annotated_source_account_id")?,
        annotated_destination_account_id: row.get("annotated_destination_account_id")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn llm_memory_event_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<LlmMemoryEventRow> {
    let snapshot_before: Option<String> = row.get("snapshot_before")?;
    let snapshot_after: Option<String> = row.get("snapshot_after")?;
    let metadata: Option<String> = row.get("metadata")?;
    Ok(LlmMemoryEventRow {
        id: row.get("id")?,
        user_id: row.get("user_id")?,
        session_id: row.get("session_id")?,
        preview_id: row.get("preview_id")?,
        event_type: row.get("event_type")?,
        decision: row.get("decision")?,
        prompt_text: row.get("prompt_text")?,
        llm_response_raw: row.get("llm_response_raw")?,
        llm_provider: row.get("llm_provider")?,
        llm_model: row.get("llm_model")?,
        suggested_main_category: row.get("suggested_main_category")?,
        suggested_sub_category: row.get("suggested_sub_category")?,
        suggested_source_account: row.get("suggested_source_account")?,
        suggested_destination_account: row.get("suggested_destination_account")?,
        confidence: row.get("confidence")?,
        user_correction_category: row.get("user_correction_category")?,
        user_correction_account: row.get("user_correction_account")?,
        snapshot_before: parse_optional_json_object(snapshot_before.as_deref()),
        snapshot_after: parse_optional_json_object(snapshot_after.as_deref()),
        metadata: parse_optional_json_object(metadata.as_deref()),
        created_at: row.get("created_at")?,
    })
}

fn parser_template_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ImportParserTemplateRow> {
    let parser_tags_json: Option<String> = row.get("parser_tags_json")?;
    Ok(ImportParserTemplateRow {
        id: row.get("id")?,
        session_id: row.get("session_id")?,
        user_id: row.get("user_id")?,
        parser_date: row.get("parser_date")?,
        parser_amount: row.get("parser_amount")?,
        parser_type: row.get("parser_type")?,
        parser_description: row
            .get::<_, Option<String>>("parser_description")?
            .unwrap_or_default(),
        parser_id: row.get("parser_id")?,
        parser_tags: parse_string_vec(parser_tags_json.as_deref()),
        parser_counterparty: row
            .get::<_, Option<String>>("parser_counterparty")?
            .unwrap_or_default(),
        parser_payment_method: row
            .get::<_, Option<String>>("parser_payment_method")?
            .unwrap_or_default(),
        parser_original_type: row
            .get::<_, Option<String>>("parser_original_type")?
            .unwrap_or_default(),
        parser_original_category: row
            .get::<_, Option<String>>("parser_original_category")?
            .unwrap_or_default(),
        parser_account_id: row
            .get::<_, Option<String>>("parser_account_id")?
            .unwrap_or_default(),
        parser_is_processed: row.get::<_, String>("parser_is_processed")? == "1",
        created_at: row.get("created_at")?,
    })
}

fn preview_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ImportPreviewRow> {
    let parser_tags_json: Option<String> = row.get("preview_parser_tags_json")?;
    let dedup_source_ids: Option<String> = row.get("dedup_source_ids")?;
    let feedback_json: Option<String> = row.get("preview_matching_feedback_json")?;

    Ok(ImportPreviewRow {
        id: row.get("id")?,
        session_id: row.get("session_id")?,
        user_id: row.get("user_id")?,
        preview_date: row.get("preview_date")?,
        preview_type: row.get("preview_type")?,
        preview_amount: row.get("preview_amount")?,
        preview_destination_amount: row.get("preview_destination_amount")?,
        preview_main_category: row
            .get::<_, Option<String>>("preview_main_category")?
            .unwrap_or_default(),
        preview_sub_category: row
            .get::<_, Option<String>>("preview_sub_category")?
            .unwrap_or_default(),
        preview_source_account_id: row.get("preview_source_account_id")?,
        preview_destination_account_id: row.get("preview_destination_account_id")?,
        preview_counterparty: row
            .get::<_, Option<String>>("preview_counterparty")?
            .unwrap_or_default(),
        preview_payment_method: row
            .get::<_, Option<String>>("preview_payment_method")?
            .unwrap_or_default(),
        preview_description: row
            .get::<_, Option<String>>("preview_description")?
            .unwrap_or_default(),
        preview_parser_id: row
            .get::<_, Option<String>>("preview_parser_id")?
            .unwrap_or_default(),
        preview_parser_tags: parse_string_vec(parser_tags_json.as_deref()),
        preview_recurring_id: row.get("preview_recurring_id")?,
        preview_recurring_name: row
            .get::<_, Option<String>>("preview_recurring_name")?
            .unwrap_or_default(),
        preview_recurring_candidate_count: row
            .get::<_, Option<i64>>("preview_recurring_candidate_count")?
            .unwrap_or_default(),
        preview_recurring_match_score: row
            .get::<_, Option<f64>>("preview_recurring_match_score")?
            .unwrap_or_default(),
        preview_recurring_match_reasons: row
            .get::<_, Option<String>>("preview_recurring_match_reasons")?
            .unwrap_or_default(),
        preview_recurring_matched_date: row
            .get::<_, Option<String>>("preview_recurring_matched_date")?
            .unwrap_or_default(),
        preview_selected: row.get::<_, i64>("preview_selected")? != 0,
        dedup_type: row
            .get::<_, Option<String>>("dedup_type")?
            .unwrap_or_default(),
        dedup_source_ids: parse_i64_csv(dedup_source_ids.as_deref()),
        preview_matching_feedback: parse_json_object(feedback_json.as_deref()),
        created_at: row.get("created_at")?,
    })
}

fn preview_filter_index_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<ImportPreviewFilterIndexRow> {
    let parser_tags_json: Option<String> = row.get("preview_parser_tags_json")?;
    let dedup_source_ids: Option<String> = row.get("dedup_source_ids")?;
    let feedback_json: Option<String> = row.get("preview_matching_feedback_json")?;
    Ok(ImportPreviewFilterIndexRow {
        id: row.get("id")?,
        preview_date: row.get("preview_date")?,
        preview_type: row.get("preview_type")?,
        preview_amount: row.get("preview_amount")?,
        preview_main_category: row
            .get::<_, Option<String>>("preview_main_category")?
            .unwrap_or_default(),
        preview_sub_category: row
            .get::<_, Option<String>>("preview_sub_category")?
            .unwrap_or_default(),
        preview_source_account_id: row.get("preview_source_account_id")?,
        preview_destination_account_id: row.get("preview_destination_account_id")?,
        preview_counterparty: row
            .get::<_, Option<String>>("preview_counterparty")?
            .unwrap_or_default(),
        preview_payment_method: row
            .get::<_, Option<String>>("preview_payment_method")?
            .unwrap_or_default(),
        preview_description: row
            .get::<_, Option<String>>("preview_description")?
            .unwrap_or_default(),
        preview_parser_id: row
            .get::<_, Option<String>>("preview_parser_id")?
            .unwrap_or_default(),
        preview_parser_tags: parse_string_vec(parser_tags_json.as_deref()),
        preview_recurring_id: row.get("preview_recurring_id")?,
        preview_recurring_candidate_count: row
            .get::<_, Option<i64>>("preview_recurring_candidate_count")?
            .unwrap_or_default(),
        preview_recurring_match_reasons: row
            .get::<_, Option<String>>("preview_recurring_match_reasons")?
            .unwrap_or_default(),
        preview_recurring_matched_date: row
            .get::<_, Option<String>>("preview_recurring_matched_date")?
            .unwrap_or_default(),
        preview_selected: row
            .get::<_, Option<i64>>("preview_selected")?
            .unwrap_or(1)
            != 0,
        dedup_type: row
            .get::<_, Option<String>>("dedup_type")?
            .unwrap_or_default(),
        dedup_source_ids: parse_i64_csv(dedup_source_ids.as_deref()),
        preview_matching_feedback: parse_json_object(feedback_json.as_deref()),
    })
}
